//! Auto-restart master process supervisor.
//!
//! Corresponds to C++ ca-gateway's master process pattern (NRESTARTS=10,
//! RESTART_INTERVAL=10*60s, RESTART_DELAY=10s in `gateway.cc:22-24`).
//!
//! The master forks a child gateway process. If the child exits, the
//! master restarts it after a delay. If too many restarts happen within
//! the configured window, the master gives up.
//!
//! In Rust we offer this as a thin wrapper around any async fallible
//! task, callable from a separate `master` mode of the binary or as a
//! library function.

use std::time::{Duration, Instant};

/// Restart policy.
#[derive(Debug, Clone, Copy)]
pub struct RestartPolicy {
    /// Maximum restart attempts within `window`.
    pub max_restarts: u32,
    /// Window over which `max_restarts` is counted.
    pub window: Duration,
    /// Delay between restart attempts.
    pub delay: Duration,
}

impl Default for RestartPolicy {
    /// C++ ca-gateway defaults: 10 restarts in 10 minutes, 10s delay.
    fn default() -> Self {
        Self {
            max_restarts: 10,
            window: Duration::from_secs(600),
            delay: Duration::from_secs(10),
        }
    }
}

/// Supervises an async task with auto-restart.
///
/// Returns Ok if the task ever returns Ok, Err if it gives up.
///
/// ```ignore
/// use epics_bridge_rs::ca_gateway::master::{supervise, RestartPolicy};
///
/// supervise(RestartPolicy::default(), || async {
///     run_gateway().await
/// }).await
/// ```
pub async fn supervise<F, Fut, E>(
    policy: RestartPolicy,
    mut task_factory: F,
) -> Result<(), SuperviseError<E>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
    E: std::fmt::Debug,
{
    let mut restart_times: Vec<Instant> = Vec::with_capacity(policy.max_restarts as usize + 1);
    let mut attempt = 0u32;

    loop {
        attempt += 1;
        eprintln!("[ca-gateway-rs/master] starting attempt #{attempt}");
        let result = task_factory().await;

        match result {
            Ok(()) => {
                eprintln!("[ca-gateway-rs/master] task exited normally");
                return Ok(());
            }
            Err(e) => {
                eprintln!("[ca-gateway-rs/master] task failed: {e:?}");
            }
        }

        // Trim old restart timestamps outside the window
        let now = Instant::now();
        restart_times.retain(|t| now.duration_since(*t) <= policy.window);
        restart_times.push(now);

        if restart_times.len() as u32 > policy.max_restarts {
            eprintln!(
                "[ca-gateway-rs/master] giving up after {} restarts in {:?}",
                restart_times.len(),
                policy.window
            );
            return Err(SuperviseError::TooManyRestarts);
        }

        eprintln!(
            "[ca-gateway-rs/master] restart {} of {} in {:?}",
            restart_times.len(),
            policy.max_restarts,
            policy.delay
        );
        tokio::time::sleep(policy.delay).await;
    }
}

#[derive(Debug)]
pub enum SuperviseError<E> {
    TooManyRestarts,
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for SuperviseError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyRestarts => write!(f, "supervisor: too many restarts"),
            Self::Inner(e) => write!(f, "supervisor inner error: {e}"),
        }
    }
}

impl<E: std::fmt::Display + std::fmt::Debug> std::error::Error for SuperviseError<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn supervise_immediate_success() {
        let policy = RestartPolicy {
            max_restarts: 3,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(1),
        };
        let result: Result<(), SuperviseError<&str>> =
            supervise(policy, || async { Ok::<(), &str>(()) }).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn supervise_eventual_success() {
        let count = Arc::new(AtomicU32::new(0));
        let policy = RestartPolicy {
            max_restarts: 5,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(1),
        };
        let count_clone = count.clone();
        let result: Result<(), SuperviseError<&str>> = supervise(policy, || {
            let c = count_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::Relaxed);
                if n < 2 {
                    Err::<(), &str>("not yet")
                } else {
                    Ok::<(), &str>(())
                }
            }
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn supervise_too_many_restarts() {
        let policy = RestartPolicy {
            max_restarts: 2,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(1),
        };
        let result: Result<(), SuperviseError<&str>> =
            supervise(policy, || async { Err::<(), &str>("always fails") }).await;
        assert!(matches!(result, Err(SuperviseError::TooManyRestarts)));
    }
}
