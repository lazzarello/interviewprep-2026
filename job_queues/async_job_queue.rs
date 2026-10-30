//! In-memory async worker pool with delayed retries and a dead-letter queue.
//! shortcut: jobs are lost on process exit, add durable storage for crash recovery.

use std::cmp::Ordering;
use std::collections::{hash_map::RandomState, BinaryHeap, VecDeque};
use std::future::{pending, Future};
use std::hash::BuildHasher;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{sleep_until, Instant};

#[derive(Debug, PartialEq, Eq)]
enum JobStatus {
    Pending,
    Running,
    Completed,
    Retrying,
    Failed,
}

type JobAction<T> =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T, String>> + Send>> + Send + Sync>;
type JobHandle<T> = Arc<Mutex<Job<T>>>;

struct Job<T> {
    id: u64,
    scheduled_at: Instant,
    action: JobAction<T>,
    max_retries: u32,
    attempts: u64,
    base_delay: Duration,
    max_delay: Duration,
    status: JobStatus,
    error: Option<String>,
    result: Option<T>,
}

struct DelayedJob<T> {
    scheduled_at: Instant,
    job: JobHandle<T>,
}

impl<T> PartialEq for DelayedJob<T> {
    fn eq(&self, other: &Self) -> bool {
        self.scheduled_at == other.scheduled_at
    }
}
impl<T> Eq for DelayedJob<T> {}
impl<T> PartialOrd for DelayedJob<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for DelayedJob<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.scheduled_at.cmp(&self.scheduled_at)
    }
}

fn retry_delay(base: Duration, max: Duration, attempt: u64) -> Duration {
    let backoff =
        (base.as_secs_f64() * 2_f64.powi((attempt - 1).min(1023) as i32)).min(max.as_secs_f64());
    // A randomly seeded standard-library hash supplies non-security jitter without an RNG dependency.
    let jitter = 0.8 + 0.4 * (RandomState::new().hash_one(()) as f64 / u64::MAX as f64);
    Duration::try_from_secs_f64(backoff * jitter).expect("Retry delay was validated at enqueue")
}

struct AsyncJobQueue<T> {
    incoming: Option<mpsc::UnboundedSender<JobHandle<T>>>,
    pending: watch::Sender<usize>,
    dlq: Arc<Mutex<Vec<JobHandle<T>>>>,
    driver: Option<JoinHandle<Result<(), String>>>,
    next_id: u64,
}

impl<T: Send + 'static> AsyncJobQueue<T> {
    fn new(num_workers: usize) -> Result<Self, String> {
        if num_workers == 0 {
            return Err("Worker count must be positive".into());
        }
        tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let (incoming, receiver) = mpsc::unbounded_channel();
        let (pending, _) = watch::channel(0);
        let dlq = Arc::new(Mutex::new(Vec::new()));
        let driver = tokio::spawn(Self::run(
            receiver,
            pending.clone(),
            Arc::clone(&dlq),
            num_workers,
        ));
        Ok(Self {
            incoming: Some(incoming),
            pending,
            dlq,
            driver: Some(driver),
            next_id: 1,
        })
    }

    fn enqueue<F, Fut>(
        &mut self,
        action: F,
        max_retries: u32,
        base_delay: Duration,
        max_delay: Duration,
    ) -> Result<JobHandle<T>, String>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, String>> + Send + 'static,
    {
        let longest_delay = Duration::try_from_secs_f64(max_delay.as_secs_f64() * 1.2)
            .map_err(|_| "Maximum retry delay is too large")?;
        if Instant::now().checked_add(longest_delay).is_none() {
            return Err("Maximum retry delay is too large".into());
        }
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or("Job ID space exhausted")?;
        let job = Arc::new(Mutex::new(Job {
            id,
            scheduled_at: Instant::now(),
            action: Arc::new(move || Box::pin(action())),
            max_retries,
            attempts: 0,
            base_delay,
            max_delay,
            status: JobStatus::Pending,
            error: None,
            result: None,
        }));
        let incoming = self.incoming.as_ref().ok_or("Queue is stopped")?;
        self.pending.send_modify(|count| *count += 1);
        if incoming.send(Arc::clone(&job)).is_err() {
            self.pending.send_modify(|count| *count -= 1);
            return Err("Queue worker pool has stopped".into());
        }
        Ok(job)
    }

    async fn join(&mut self) -> Result<(), String> {
        let mut pending = self.pending.subscribe();
        let driver = self
            .driver
            .as_mut()
            .ok_or("Queue worker pool has stopped")?;
        tokio::select! {
            finished = driver => {
                self.driver = None;
                finished.map_err(|e| e.to_string())??;
                Err("Queue worker pool stopped unexpectedly".into())
            }
            done = pending.wait_for(|count| *count == 0) => {
                done.map(|_| ()).map_err(|e| e.to_string())
            }
        }
    }

    async fn dead_letter_queue(&self) -> Vec<JobHandle<T>> {
        self.dlq.lock().await.clone()
    }

    /// Close submissions and finish ready, running, and retrying jobs before exiting.
    async fn stop(mut self) -> Result<(), String> {
        self.incoming.take();
        let result = self
            .driver
            .as_mut()
            .ok_or("Queue worker pool has stopped")?
            .await
            .map_err(|e| e.to_string())?;
        self.driver = None;
        result
    }

    async fn run(
        mut incoming: mpsc::UnboundedReceiver<JobHandle<T>>,
        pending_count: watch::Sender<usize>,
        dlq: Arc<Mutex<Vec<JobHandle<T>>>>,
        num_workers: usize,
    ) -> Result<(), String> {
        let mut ready = VecDeque::<JobHandle<T>>::new();
        let mut delayed = BinaryHeap::<DelayedJob<T>>::new();
        let mut workers = JoinSet::new();
        let mut closed = false;
        loop {
            let now = Instant::now();
            while delayed.peek().is_some_and(|job| job.scheduled_at <= now) {
                let job = delayed.pop().unwrap().job;
                job.lock().await.status = JobStatus::Pending;
                ready.push_back(job);
            }
            while workers.len() < num_workers {
                let Some(job) = ready.pop_front() else { break };
                let action = {
                    let mut state = job.lock().await;
                    state.status = JobStatus::Running;
                    state.attempts += 1;
                    Arc::clone(&state.action)
                };
                workers.spawn(async move { (job, action().await) });
            }
            if closed && ready.is_empty() && delayed.is_empty() && workers.is_empty() {
                return Ok(());
            }
            let next_retry = delayed.peek().map(|job| job.scheduled_at);
            tokio::select! {
                submission = incoming.recv(), if !closed => {
                    match submission {
                        Some(job) => ready.push_back(job),
                        None => closed = true,
                    }
                }
                completed = workers.join_next(), if !workers.is_empty() => {
                    let (job, result) = completed.unwrap().map_err(|e| e.to_string())?;
                    let mut state = job.lock().await;
                    match result {
                        Ok(result) => {
                            state.result = Some(result);
                            state.error = None;
                            state.status = JobStatus::Completed;
                        }
                        Err(error) => {
                            state.error = Some(error);
                            if state.attempts <= u64::from(state.max_retries) {
                                let delay = retry_delay(state.base_delay, state.max_delay, state.attempts);
                                state.scheduled_at = Instant::now().checked_add(delay)
                                    .ok_or("Retry deadline is too large")?;
                                state.status = JobStatus::Retrying;
                                println!("  Job {} failed (attempt {}), retrying in {:.2}s: {}",
                                    state.id, state.attempts, delay.as_secs_f64(), state.error.as_deref().unwrap());
                                delayed.push(DelayedJob { scheduled_at: state.scheduled_at, job: Arc::clone(&job) });
                                continue;
                            }
                            state.status = JobStatus::Failed;
                            dlq.lock().await.push(Arc::clone(&job));
                            println!("  Job {} exhausted {} retries; moved to DLQ: {}",
                                state.id, state.max_retries, state.error.as_deref().unwrap());
                        }
                    }
                    pending_count.send_modify(|count| *count -= 1);
                }
                _ = async {
                    match next_retry {
                        Some(deadline) => sleep_until(deadline).await,
                        None => pending::<()>().await,
                    }
                } => {}
            }
        }
    }
}

impl<T> Drop for AsyncJobQueue<T> {
    fn drop(&mut self) {
        // Do not leave detached workers running if the owner forgets graceful shutdown.
        if let Some(driver) = &self.driver {
            driver.abort();
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    println!("=== Async Job Queue with Retries & DLQ Demo ===");
    let mut queue = AsyncJobQueue::<String>::new(2)?;
    let clean = queue.enqueue(
        || async { Ok("Successfully processed Job-Clean on attempt 1".to_owned()) },
        3,
        Duration::from_millis(500),
        Duration::from_secs(30),
    )?;
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let flaky = queue.enqueue(
        move || {
            let attempts = Arc::clone(&attempts);
            async move {
                let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if attempt <= 2 {
                    Err(format!("Transient network drop on attempt {attempt}"))
                } else {
                    Ok(format!(
                        "Successfully processed Job-Flaky on attempt {attempt}"
                    ))
                }
            }
        },
        3,
        Duration::from_millis(300),
        Duration::from_secs(30),
    )?;
    let poison = queue.enqueue(
        || async { Err("Invalid payload: schema mismatch".to_owned()) },
        2,
        Duration::from_millis(200),
        Duration::from_secs(30),
    )?;
    println!("Enqueued clean, flaky, and poison-pill jobs; awaiting completion...\n");
    queue.join().await?;
    println!("\n--- Summary ---");
    for job in [clean, flaky, poison] {
        let job = job.lock().await;
        println!(
            "Job {} ({:?}): attempts = {}, result = {:?}, error = {:?}",
            job.id, job.status, job.attempts, job.result, job.error
        );
    }
    println!(
        "Dead-Letter Queue entries: {}",
        queue.dead_letter_queue().await.len()
    );
    queue.stop().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tokio::time::{sleep, timeout};

    #[tokio::test(start_paused = true)]
    async fn clean_flaky_and_poison_jobs_reach_terminal_states() {
        let mut queue = AsyncJobQueue::new(2).unwrap();
        let clean = queue
            .enqueue(
                || async { Ok(42) },
                3,
                Duration::from_millis(50),
                Duration::from_secs(1),
            )
            .unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let flaky = queue
            .enqueue(
                move || {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        if attempts.fetch_add(1, AtomicOrdering::Relaxed) < 2 {
                            Err("temporary".into())
                        } else {
                            Ok(99)
                        }
                    }
                },
                3,
                Duration::from_millis(50),
                Duration::from_secs(1),
            )
            .unwrap();
        let poison = queue
            .enqueue(
                || async { Err("invalid payload".into()) },
                2,
                Duration::from_millis(50),
                Duration::from_secs(1),
            )
            .unwrap();
        queue.join().await.unwrap();
        let clean = clean.lock().await;
        assert_eq!(clean.status, JobStatus::Completed);
        assert_eq!(clean.attempts, 1);
        assert_eq!(clean.result, Some(42));
        let flaky = flaky.lock().await;
        assert_eq!(flaky.status, JobStatus::Completed);
        assert_eq!(flaky.attempts, 3);
        assert_eq!(flaky.result, Some(99));
        assert!(flaky.error.is_none());
        let poison_state = poison.lock().await;
        assert_eq!(poison_state.status, JobStatus::Failed);
        assert_eq!(poison_state.attempts, 3);
        assert_eq!(poison_state.error.as_deref(), Some("invalid payload"));
        let dlq = queue.dead_letter_queue().await;
        assert_eq!(dlq.len(), 1);
        assert!(Arc::ptr_eq(&dlq[0], &poison));
        drop(poison_state);
        queue.stop().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn retry_backoff_does_not_occupy_the_only_worker() {
        let mut queue = AsyncJobQueue::<()>::new(1).unwrap();
        let poison = queue
            .enqueue(
                || async { Err("retry me".into()) },
                1,
                Duration::from_secs(10),
                Duration::from_secs(10),
            )
            .unwrap();
        let (started, mut received) = mpsc::unbounded_channel();
        let start = Instant::now();
        queue
            .enqueue(
                move || {
                    let started = started.clone();
                    async move {
                        started.send(Instant::now()).unwrap();
                        Ok(())
                    }
                },
                0,
                Duration::ZERO,
                Duration::ZERO,
            )
            .unwrap();
        assert_eq!(received.recv().await.unwrap(), start);
        queue.join().await.unwrap();
        assert!(start.elapsed() >= Duration::from_secs(8));
        assert!(start.elapsed() <= Duration::from_millis(12_001));
        assert_eq!(poison.lock().await.attempts, 2);
        queue.stop().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn worker_pool_enforces_concurrency_and_runs_in_background() {
        let mut queue = AsyncJobQueue::<()>::new(2).unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (started, mut received) = mpsc::unbounded_channel();
        for _ in 0..4 {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let started = started.clone();
            queue
                .enqueue(
                    move || {
                        let active = Arc::clone(&active);
                        let peak = Arc::clone(&peak);
                        let started = started.clone();
                        async move {
                            let current = active.fetch_add(1, AtomicOrdering::Relaxed) + 1;
                            peak.fetch_max(current, AtomicOrdering::Relaxed);
                            started.send(()).unwrap();
                            sleep(Duration::from_secs(1)).await;
                            active.fetch_sub(1, AtomicOrdering::Relaxed);
                            Ok(())
                        }
                    },
                    0,
                    Duration::ZERO,
                    Duration::ZERO,
                )
                .unwrap();
        }
        let start = Instant::now();
        received.recv().await.unwrap();
        received.recv().await.unwrap();
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert_eq!(active.load(AtomicOrdering::Relaxed), 2);
        queue.join().await.unwrap();
        assert_eq!(peak.load(AtomicOrdering::Relaxed), 2);
        assert_eq!(active.load(AtomicOrdering::Relaxed), 0);
        assert_eq!(start.elapsed(), Duration::from_secs(2));
        queue.stop().await.unwrap();
    }

    #[test]
    fn exponential_delay_is_capped_before_twenty_percent_jitter() {
        for (attempt, base_millis) in [(1, 100), (2, 200), (3, 400), (4, 500), (10000, 500)] {
            for _ in 0..20 {
                let delay = retry_delay(
                    Duration::from_millis(100),
                    Duration::from_millis(500),
                    attempt,
                );
                assert!(delay >= Duration::from_micros(base_millis * 800));
                assert!(delay <= Duration::from_micros(base_millis * 1200));
            }
        }
        assert_eq!(
            retry_delay(Duration::ZERO, Duration::ZERO, 10000),
            Duration::ZERO
        );
    }

    #[tokio::test(start_paused = true)]
    async fn zero_retries_fail_once_and_equal_retry_deadlines_keep_all_jobs() {
        let mut queue = AsyncJobQueue::<()>::new(2).unwrap();
        let once = queue
            .enqueue(
                || async { Err("no retry".into()) },
                0,
                Duration::ZERO,
                Duration::ZERO,
            )
            .unwrap();
        let mut retries = Vec::new();
        for _ in 0..5 {
            retries.push(
                queue
                    .enqueue(
                        || async { Err("retry".into()) },
                        2,
                        Duration::ZERO,
                        Duration::ZERO,
                    )
                    .unwrap(),
            );
        }
        queue.join().await.unwrap();
        assert_eq!(once.lock().await.attempts, 1);
        for job in retries {
            assert_eq!(job.lock().await.attempts, 3);
        }
        assert_eq!(queue.dead_letter_queue().await.len(), 6);
        queue.stop().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn validates_configuration_and_supports_repeated_join_and_graceful_stop() {
        assert!(AsyncJobQueue::<()>::new(0).is_err());
        let mut queue = AsyncJobQueue::new(1).unwrap();
        assert!(queue
            .enqueue(
                || async { Ok(()) },
                1,
                Duration::from_secs(1),
                Duration::MAX
            )
            .is_err());
        queue.join().await.unwrap();
        let first = queue
            .enqueue(|| async { Ok(()) }, 0, Duration::ZERO, Duration::ZERO)
            .unwrap();
        queue.join().await.unwrap();
        let second = queue
            .enqueue(|| async { Ok(()) }, 1, Duration::ZERO, Duration::ZERO)
            .unwrap();
        let retrying = queue
            .enqueue(
                || async { Err("still failing".into()) },
                1,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .unwrap();
        queue.stop().await.unwrap();
        assert_eq!(first.lock().await.status, JobStatus::Completed);
        assert_eq!(second.lock().await.status, JobStatus::Completed);
        let retried = retrying.lock().await;
        assert_eq!(retried.status, JobStatus::Failed);
        assert_eq!(retried.attempts, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_graceful_stop_does_not_leave_detached_workers() {
        let mut queue = AsyncJobQueue::new(1).unwrap();
        let completed = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&completed);
        let (started, mut received) = mpsc::unbounded_channel();
        queue
            .enqueue(
                move || {
                    let counter = Arc::clone(&counter);
                    let started = started.clone();
                    async move {
                        started.send(()).unwrap();
                        sleep(Duration::from_secs(1)).await;
                        counter.fetch_add(1, AtomicOrdering::Relaxed);
                        Ok(())
                    }
                },
                0,
                Duration::ZERO,
                Duration::ZERO,
            )
            .unwrap();
        received.recv().await.unwrap();
        let stopping = tokio::spawn(queue.stop());
        tokio::task::yield_now().await;
        stopping.abort();
        assert!(stopping.await.unwrap_err().is_cancelled());
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(2)).await;
        assert_eq!(completed.load(AtomicOrdering::Relaxed), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn job_panic_surfaces_instead_of_hanging_join() {
        let mut queue = AsyncJobQueue::<()>::new(1).unwrap();
        queue
            .enqueue(
                || async { panic!("broken job") },
                0,
                Duration::ZERO,
                Duration::ZERO,
            )
            .unwrap();
        let result = timeout(Duration::from_secs(1), queue.join()).await.unwrap();
        assert!(result.unwrap_err().contains("broken job"));
    }
}
