use std::fmt::Display;
use std::time::Duration;
use tracing::debug;

/// Retry an async operation with exponential backoff.
///
/// - `max_attempts`: total number of tries (must be >= 1)
/// - `base_backoff`: the initial backoff duration; doubles on each subsequent retry
/// - `label`: human-readable description used in debug logs
/// - `op`: an async closure that returns `Result<T, E>`
///
/// Returns `Ok(value)` on the first successful attempt, or `Err(last_error)` if
/// all attempts are exhausted.
pub async fn retry_with_backoff<T, E, F, Fut>(
    max_attempts: u32,
    base_backoff: Duration,
    label: &str,
    mut op: F,
) -> Result<T, E>
where
    E: Display,
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut last_err = None;
    for attempt in 1..=max_attempts {
        match op().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                debug!(
                    "{} attempt {}/{} failed: {}",
                    label, attempt, max_attempts, e
                );
                last_err = Some(e);
                if attempt < max_attempts {
                    let backoff = base_backoff * (1 << (attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    Err(last_err.expect("max_attempts must be >= 1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let result = retry_with_backoff(3, Duration::from_millis(1), "test", || async {
            Ok::<_, String>(42)
        })
        .await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let counter = AtomicU32::new(0);
        let result = retry_with_backoff(3, Duration::from_millis(1), "test", || {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt == 0 {
                    Err("transient failure".to_string())
                } else {
                    Ok(42)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausts_all_attempts() {
        let counter = AtomicU32::new(0);
        let result = retry_with_backoff(3, Duration::from_millis(1), "test", || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Err::<i32, _>("always fails".to_string()) }
        })
        .await;
        assert_eq!(result.unwrap_err(), "always fails");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
