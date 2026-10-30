//! Interview-sized leader-lease scheduler with automatic failover.
//! shortcut: coordination is in-memory, use an atomic shared store for separate processes.
//! shortcut: no fencing or deduplication, add both before scheduling critical side effects.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinSet;
use tokio::time::{sleep, Instant};

const LEASE_TTL: Duration = Duration::from_millis(400);

type JobAction = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

struct ScheduledJob {
    next_run: Instant,
    job_id: String,
    interval: Duration,
    action: JobAction,
}

impl PartialEq for ScheduledJob {
    fn eq(&self, other: &Self) -> bool {
        self.next_run == other.next_run
    }
}

impl Eq for ScheduledJob {}

impl PartialOrd for ScheduledJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; earlier deadlines must sort first.
        other.next_run.cmp(&self.next_run)
    }
}

#[derive(Default)]
struct DistributedLeaseStore {
    leader_id: Option<String>,
    lease_expires_at: Option<Instant>,
}

impl DistributedLeaseStore {
    fn try_acquire_or_renew(&mut self, node_id: &str, ttl: Duration) -> Result<bool, &'static str> {
        if node_id.is_empty() || ttl.is_zero() {
            return Err("Node ID and lease TTL must be nonempty/positive");
        }
        let now = Instant::now();
        let expires_at = now.checked_add(ttl).ok_or("Lease TTL is too large")?;
        if self.leader_id.as_deref() != Some(node_id)
            && self.lease_expires_at.is_some_and(|expiry| now < expiry)
        {
            return Ok(false);
        }
        self.leader_id = Some(node_id.to_owned());
        self.lease_expires_at = Some(expires_at);
        Ok(true)
    }
}

struct DistributedCronNode {
    node_id: String,
    lease_store: Arc<Mutex<DistributedLeaseStore>>,
    schedule: BinaryHeap<ScheduledJob>,
}

impl DistributedCronNode {
    fn new(node_id: &str, lease_store: Arc<Mutex<DistributedLeaseStore>>) -> Self {
        Self {
            node_id: node_id.to_owned(),
            lease_store,
            schedule: BinaryHeap::new(),
        }
    }

    fn add_job<F, Fut>(
        &mut self,
        job_id: &str,
        interval: Duration,
        action: F,
    ) -> Result<(), &'static str>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if interval.is_zero() {
            return Err("Job interval must be positive");
        }
        let next_run = Instant::now()
            .checked_add(interval)
            .ok_or("Job interval is too large")?;
        self.schedule.push(ScheduledJob {
            next_run,
            job_id: job_id.to_owned(),
            interval,
            action: Box::new(move || Box::pin(action())),
        });
        Ok(())
    }

    async fn run(
        mut self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut executions = JoinSet::new();
        while !*shutdown.borrow() {
            while let Some(result) = executions.try_join_next() {
                result?;
            }
            let is_leader = self
                .lease_store
                .lock()
                .await
                .try_acquire_or_renew(&self.node_id, LEASE_TTL)?;
            let poll_delay = if is_leader {
                let now = Instant::now();
                while self.schedule.peek().is_some_and(|job| job.next_run <= now) {
                    let mut job = self.schedule.pop().expect("Due job was just checked");
                    job.next_run = now
                        .checked_add(job.interval)
                        .ok_or("Job interval is too large")?;
                    println!("  [DISPATCHED] '{}' by {}", job.job_id, self.node_id);
                    executions.spawn((job.action)());
                    self.schedule.push(job);
                }
                Duration::from_millis(50)
            } else {
                Duration::from_millis(100)
            };
            tokio::select! {
                _ = sleep(poll_delay) => {}
                changed = shutdown.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
        // A graceful stop finishes dispatched jobs but leaves the lease to expire.
        while let Some(result) = executions.join_next().await {
            result?;
        }
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Interview-Ready Distributed Cron Service Demo ===");
    let store = Arc::new(Mutex::new(DistributedLeaseStore::default()));
    let executed = Arc::new(AtomicUsize::new(0));
    let mut nodes = Vec::new();
    for node_id in ["node-1", "node-2"] {
        let mut node = DistributedCronNode::new(node_id, Arc::clone(&store));
        let executed = Arc::clone(&executed);
        node.add_job("heartbeat-task", Duration::from_millis(200), move || {
            let executed = Arc::clone(&executed);
            async move {
                executed.fetch_add(1, AtomicOrdering::Relaxed);
            }
        })?;
        nodes.push(node);
    }
    let node2 = nodes.pop().unwrap();
    let node1 = nodes.pop().unwrap();
    let (stop1, shutdown1) = watch::channel(false);
    let (stop2, shutdown2) = watch::channel(false);
    let task1 = tokio::spawn(node1.run(shutdown1));
    // Let node-1 claim leadership before starting the standby.
    tokio::task::yield_now().await;
    let task2 = tokio::spawn(node2.run(shutdown2));

    sleep(Duration::from_millis(500)).await;
    println!(
        "\n[Status] Current leader: {:?}",
        store.lock().await.leader_id
    );
    println!("\n>>> Simulating node-1 failure: stopping its heartbeats...");
    stop1.send(true)?;
    task1.await??;

    sleep(Duration::from_millis(600)).await;
    println!(
        "\n[Status] After failover, new leader: {:?}",
        store.lock().await.leader_id
    );
    stop2.send(true)?;
    task2.await??;
    println!(
        "\nTotal tasks executed: {}",
        executed.load(AtomicOrdering::Relaxed)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::advance;

    #[tokio::test(start_paused = true)]
    async fn lease_renewal_excludes_followers_until_expiry() {
        let mut store = DistributedLeaseStore::default();
        assert_eq!(store.try_acquire_or_renew("node-1", LEASE_TTL), Ok(true));
        assert_eq!(store.try_acquire_or_renew("node-2", LEASE_TTL), Ok(false));
        advance(Duration::from_millis(300)).await;
        assert_eq!(store.try_acquire_or_renew("node-1", LEASE_TTL), Ok(true));
        advance(Duration::from_millis(200)).await;
        assert_eq!(store.try_acquire_or_renew("node-2", LEASE_TTL), Ok(false));
        advance(Duration::from_millis(200)).await;
        assert_eq!(store.try_acquire_or_renew("node-2", LEASE_TTL), Ok(true));
        assert_eq!(store.try_acquire_or_renew("node-1", LEASE_TTL), Ok(false));
    }

    #[tokio::test(start_paused = true)]
    async fn invalid_leases_and_intervals_do_not_mutate_state() {
        let mut store = DistributedLeaseStore::default();
        assert!(store.try_acquire_or_renew("", LEASE_TTL).is_err());
        for ttl in [Duration::ZERO, Duration::MAX] {
            assert!(store.try_acquire_or_renew("node-1", ttl).is_err());
        }
        assert!(store.leader_id.is_none());
        let mut node = DistributedCronNode::new("node-1", Arc::new(Mutex::new(store)));
        for interval in [Duration::ZERO, Duration::MAX] {
            assert!(node.add_job("invalid", interval, || async {}).is_err());
        }
        assert!(node.schedule.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn heap_orders_deadlines_and_keeps_equal_deadline_jobs() {
        let store = Arc::new(Mutex::new(DistributedLeaseStore::default()));
        let mut node = DistributedCronNode::new("node-1", store);
        for (id, millis) in [("late", 300), ("early-a", 100), ("early-b", 100)] {
            node.add_job(id, Duration::from_millis(millis), || async {})
                .unwrap();
        }
        let first = node.schedule.pop().unwrap();
        let second = node.schedule.pop().unwrap();
        let last = node.schedule.pop().unwrap();
        assert_eq!(first.next_run, second.next_run);
        assert!(first.next_run < last.next_run);
        assert_ne!(first.job_id, second.job_id);
        assert_eq!(last.job_id, "late");
    }

    #[tokio::test(start_paused = true)]
    async fn jobs_repeat_and_missed_ticks_collapse_to_one_dispatch() {
        let store = Arc::new(Mutex::new(DistributedLeaseStore::default()));
        let mut node = DistributedCronNode::new("node-1", store);
        let (sent, mut received) = mpsc::unbounded_channel();
        node.add_job("periodic", Duration::from_millis(200), move || {
            let sent = sent.clone();
            async move {
                sent.send(Instant::now()).unwrap();
            }
        })
        .unwrap();
        let start = Instant::now();
        let (stop, shutdown) = watch::channel(false);
        let task = tokio::spawn(node.run(shutdown));
        tokio::task::yield_now().await;
        advance(Duration::from_millis(200)).await;
        assert_eq!(
            received.recv().await.unwrap() - start,
            Duration::from_millis(200)
        );
        advance(Duration::from_secs(1)).await;
        assert_eq!(
            received.recv().await.unwrap() - start,
            Duration::from_millis(1200)
        );
        assert!(received.try_recv().is_err());
        advance(Duration::from_millis(200)).await;
        assert_eq!(
            received.recv().await.unwrap() - start,
            Duration::from_millis(1400)
        );
        stop.send(true).unwrap();
        task.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn follower_takes_over_only_after_crashed_leaders_lease_expires() {
        let store = Arc::new(Mutex::new(DistributedLeaseStore::default()));
        let (sent, mut received) = mpsc::unbounded_channel();
        let mut nodes = Vec::new();
        for id in ["node-1", "node-2"] {
            let mut node = DistributedCronNode::new(id, Arc::clone(&store));
            let sent = sent.clone();
            node.add_job("heartbeat", Duration::from_millis(200), move || {
                let sent = sent.clone();
                async move {
                    sent.send(id).unwrap();
                }
            })
            .unwrap();
            nodes.push(node);
        }
        let node2 = nodes.pop().unwrap();
        let node1 = nodes.pop().unwrap();
        let (_stop1, shutdown1) = watch::channel(false);
        let (stop2, shutdown2) = watch::channel(false);
        let task1 = tokio::spawn(node1.run(shutdown1));
        tokio::task::yield_now().await;
        let task2 = tokio::spawn(node2.run(shutdown2));
        tokio::task::yield_now().await;
        advance(Duration::from_millis(200)).await;
        assert_eq!(received.recv().await, Some("node-1"));
        task1.abort();
        assert!(task1.await.unwrap_err().is_cancelled());
        advance(Duration::from_millis(399)).await;
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        assert_eq!(store.lock().await.leader_id.as_deref(), Some("node-1"));
        advance(Duration::from_millis(101)).await;
        assert_eq!(received.recv().await, Some("node-2"));
        assert_eq!(store.lock().await.leader_id.as_deref(), Some("node-2"));
        stop2.send(true).unwrap();
        task2.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn long_jobs_do_not_block_scheduling_and_graceful_stop_drains_them() {
        let store = Arc::new(Mutex::new(DistributedLeaseStore::default()));
        let mut node = DistributedCronNode::new("node-1", store);
        let (sent, mut received) = mpsc::unbounded_channel();
        node.add_job("slow", Duration::from_millis(100), move || {
            let sent = sent.clone();
            async move {
                sent.send("started").unwrap();
                sleep(Duration::from_secs(1)).await;
                sent.send("finished").unwrap();
            }
        })
        .unwrap();
        let (stop, shutdown) = watch::channel(false);
        let task = tokio::spawn(node.run(shutdown));
        assert_eq!(received.recv().await, Some("started"));
        assert_eq!(received.recv().await, Some("started"));
        stop.send(true).unwrap();
        task.await.unwrap().unwrap();
        let mut finished = 0;
        while let Ok(event) = received.try_recv() {
            assert_eq!(event, "finished");
            finished += 1;
        }
        assert_eq!(finished, 2);
    }
}
