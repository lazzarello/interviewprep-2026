//! Async port of termie's first rate limiter example.
//! shortcut: state is process-local, use shared atomic storage for distributed quotas.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::{sleep, Instant};

#[derive(Debug, PartialEq)]
enum RateLimitError {
    InvalidInput(&'static str),
    RateLimitExceeded { retry_after: f64 },
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::RateLimitExceeded { retry_after } => {
                write!(f, "Rate limit exceeded. Retry after {retry_after:.2}s.")
            }
        }
    }
}

impl std::error::Error for RateLimitError {}

struct BucketState {
    tokens: f64,
    last_updated: Instant,
}

struct AsyncTokenBucket {
    rate: f64,
    capacity: f64,
    blocking: bool,
    state: Mutex<BucketState>,
}

impl AsyncTokenBucket {
    fn new(rate: f64, capacity: Option<f64>, blocking: bool) -> Result<Self, RateLimitError> {
        if !rate.is_finite() || rate <= 0.0 {
            return Err(RateLimitError::InvalidInput(
                "Rate must be finite and positive",
            ));
        }
        let capacity = capacity.unwrap_or(rate);
        if !capacity.is_finite() || capacity <= 0.0 {
            return Err(RateLimitError::InvalidInput(
                "Capacity must be finite and positive",
            ));
        }
        Ok(Self {
            rate,
            capacity,
            blocking,
            state: Mutex::new(BucketState {
                tokens: capacity,
                last_updated: Instant::now(),
            }),
        })
    }

    async fn acquire(&self, tokens: f64, blocking: Option<bool>) -> Result<(), RateLimitError> {
        if !tokens.is_finite() || tokens <= 0.0 || tokens > self.capacity {
            return Err(RateLimitError::InvalidInput(
                "Requested tokens must be finite, positive, and no greater than capacity",
            ));
        }
        loop {
            let mut state = self.state.lock().await;
            let now = Instant::now();
            let elapsed = now.duration_since(state.last_updated).as_secs_f64();
            state.tokens = self.capacity.min(state.tokens + elapsed * self.rate);
            state.last_updated = now;
            if state.tokens >= tokens {
                state.tokens -= tokens;
                return Ok(());
            }

            let retry_after = (tokens - state.tokens) / self.rate;
            let wait = Duration::try_from_secs_f64(retry_after)
                .map_err(|_| RateLimitError::InvalidInput("Required wait is too large"))?;
            if !blocking.unwrap_or(self.blocking) {
                return Err(RateLimitError::RateLimitExceeded { retry_after });
            }
            // Sleeping under the lock would make fail-fast callers wait behind us.
            drop(state);
            sleep(wait.max(Duration::from_nanos(1))).await;
        }
    }

    async fn try_acquire(&self, tokens: f64) -> Result<bool, RateLimitError> {
        match self.acquire(tokens, Some(false)).await {
            Ok(()) => Ok(true),
            Err(RateLimitError::RateLimitExceeded { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Rust equivalent of the Python decorator: acquire one token, then await the operation.
    async fn limit<F: Future>(
        &self,
        operation: F,
        blocking: Option<bool>,
    ) -> Result<F::Output, RateLimitError> {
        self.acquire(1.0, blocking).await?;
        Ok(operation.await)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    for blocking in [false, true] {
        println!(
            "=== Demo: {} Mode ===",
            if blocking {
                "Waiting/Blocking"
            } else {
                "Denying"
            }
        );
        println!("Settings: 2 tokens/sec, Burst Capacity = 3 tokens (blocking={blocking})");
        let limiter = Arc::new(AsyncTokenBucket::new(2.0, Some(3.0), blocking)?);
        let start = Instant::now();
        let mut requests = JoinSet::new();
        for request_id in 1..=6 {
            let limiter = Arc::clone(&limiter);
            requests.spawn(async move {
                let operation = async {
                    println!(
                        "[{:5.2}s] Executed API request #{request_id}",
                        start.elapsed().as_secs_f64()
                    );
                };
                if let Err(error) = limiter.limit(operation, None).await {
                    println!(
                        "[{:5.2}s] [DENIED] Request #{request_id} rejected! ({error})",
                        start.elapsed().as_secs_f64()
                    );
                }
            });
        }
        while let Some(result) = requests.join_next().await {
            result?;
        }
        println!(
            "Immediate extra token available: {}\n",
            limiter.try_acquire(1.0).await?
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{advance, timeout};

    #[test]
    fn validates_configuration_and_defaults_capacity_to_rate() {
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(AsyncTokenBucket::new(invalid, Some(3.0), true).is_err());
            assert!(AsyncTokenBucket::new(2.0, Some(invalid), true).is_err());
        }
        let limiter = AsyncTokenBucket::new(2.0, None, true).unwrap();
        assert_eq!(limiter.capacity, 2.0);
    }

    #[tokio::test(start_paused = true)]
    async fn burst_then_denial_reports_retry_after_without_waiting() {
        let limiter = AsyncTokenBucket::new(2.0, Some(3.0), false).unwrap();
        let start = Instant::now();
        for _ in 0..3 {
            assert_eq!(limiter.try_acquire(1.0).await, Ok(true));
        }
        assert_eq!(limiter.try_acquire(1.0).await, Ok(false));
        assert_eq!(
            limiter.acquire(1.0, None).await,
            Err(RateLimitError::RateLimitExceeded { retry_after: 0.5 })
        );
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn fractional_tokens_refill_and_capacity_is_capped() {
        let limiter = AsyncTokenBucket::new(2.0, Some(3.0), false).unwrap();
        limiter.acquire(3.0, None).await.unwrap();
        advance(Duration::from_millis(125)).await;
        assert_eq!(limiter.try_acquire(0.25).await, Ok(true));
        assert_eq!(limiter.try_acquire(0.25).await, Ok(false));
        advance(Duration::from_secs(10)).await;
        assert_eq!(limiter.try_acquire(3.0).await, Ok(true));
        assert_eq!(limiter.try_acquire(1.0).await, Ok(false));
    }

    #[tokio::test(start_paused = true)]
    async fn blocking_override_waits_and_denying_override_does_not() {
        let limiter = AsyncTokenBucket::new(2.0, Some(1.0), false).unwrap();
        limiter.acquire(1.0, None).await.unwrap();
        let start = Instant::now();
        limiter.acquire(1.0, Some(true)).await.unwrap();
        assert_eq!(start.elapsed(), Duration::from_millis(500));

        let limiter = AsyncTokenBucket::new(2.0, Some(1.0), true).unwrap();
        limiter.acquire(1.0, None).await.unwrap();
        let start = Instant::now();
        assert!(matches!(
            limiter.acquire(1.0, Some(false)).await,
            Err(RateLimitError::RateLimitExceeded { .. })
        ));
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_request_does_not_block_fail_fast_or_spend_tokens_when_cancelled() {
        let limiter = Arc::new(AsyncTokenBucket::new(2.0, Some(1.0), true).unwrap());
        limiter.acquire(1.0, None).await.unwrap();
        let waiting_limiter = Arc::clone(&limiter);
        let waiting = tokio::spawn(async move { waiting_limiter.acquire(1.0, None).await });
        tokio::task::yield_now().await;
        assert_eq!(
            timeout(Duration::from_millis(1), limiter.try_acquire(1.0))
                .await
                .unwrap(),
            Ok(false)
        );
        waiting.abort();
        assert!(waiting.await.unwrap_err().is_cancelled());
        advance(Duration::from_millis(500)).await;
        assert_eq!(limiter.try_acquire(1.0).await, Ok(true));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_requests_share_the_burst_and_refill_budget() {
        let limiter = Arc::new(AsyncTokenBucket::new(2.0, Some(3.0), true).unwrap());
        let start = Instant::now();
        let mut requests = JoinSet::new();
        for _ in 0..6 {
            let limiter = Arc::clone(&limiter);
            requests.spawn(async move {
                limiter.acquire(1.0, None).await.unwrap();
                Instant::now()
            });
        }
        let mut elapsed = Vec::new();
        while let Some(result) = requests.join_next().await {
            elapsed.push(result.unwrap().duration_since(start));
        }
        elapsed.sort();
        assert_eq!(
            elapsed,
            vec![
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::from_millis(500),
                Duration::from_secs(1),
                Duration::from_millis(1500)
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn invalid_requests_do_not_change_available_tokens() {
        let limiter = AsyncTokenBucket::new(2.0, Some(3.0), true).unwrap();
        for invalid in [0.0, -1.0, 4.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                limiter.acquire(invalid, None).await,
                Err(RateLimitError::InvalidInput(_))
            ));
            assert!(matches!(
                limiter.try_acquire(invalid).await,
                Err(RateLimitError::InvalidInput(_))
            ));
        }
        assert_eq!(limiter.try_acquire(3.0).await, Ok(true));
    }

    #[tokio::test(start_paused = true)]
    async fn limit_runs_operation_only_after_acquiring_a_token() {
        let limiter = AsyncTokenBucket::new(2.0, Some(1.0), false).unwrap();
        assert_eq!(limiter.limit(async { 42 }, None).await, Ok(42));
        let mut ran = false;
        let result = limiter
            .limit(
                async {
                    ran = true;
                },
                None,
            )
            .await;
        assert!(matches!(
            result,
            Err(RateLimitError::RateLimitExceeded { .. })
        ));
        assert!(!ran);
    }
}
