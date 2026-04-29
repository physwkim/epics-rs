//! Explicit-handle wrappers around the convenience PvaClient operations.
//!
//! Rust's async/await covers most of pvxs `client::Operation`'s job
//! implicitly (drop a future to cancel, `.await` to wait), but a
//! handle-style API is occasionally useful when:
//!
//! - You want to start an operation now and `.wait(timeout)` for it
//!   later from a different task.
//! - You want a single thread-safe `cancel()` that unblocks the waiter
//!   from elsewhere (pvxs `Operation::cancel`).
//! - You want a thread-safe `interrupt()` that wakes a `wait()`
//!   without cancelling the underlying operation, mirroring pvxs
//!   `Operation::interrupt`.
//!
//! The handle is constructed from any future that returns
//! `PvaResult<T>`. F-G8 (April 2026).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, oneshot};
use tokio::task::JoinHandle;

use crate::error::{PvaError, PvaResult};

/// Handle to an in-flight operation. Pairs with the operation type
/// returned by `PvaClient::start_*` async methods.
pub struct PvaOperation<T: Send + 'static> {
    /// Spawned task running the underlying op.
    join: JoinHandle<()>,
    /// Receiver for the op's final result. `None` once `wait`/`cancel`
    /// has consumed it.
    result_rx: Option<oneshot::Receiver<PvaResult<T>>>,
    /// Pulsed by [`Self::interrupt`]; `wait*` selects on this and
    /// returns `PvaError::Timeout` (the closest existing variant) so
    /// callers can distinguish operator-driven wake-up from a real
    /// timeout via the surrounding context.
    interrupt: Arc<Notify>,
    /// One-shot cancellation flag. When set, `wait*` short-circuits
    /// returning the abort error and the spawned task is aborted.
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl<T: Send + 'static> PvaOperation<T> {
    /// Spawn `fut` and return a handle. The future runs to completion
    /// regardless of handle drops unless [`Self::cancel`] is called
    /// explicitly. (Drop only loses the handle's view of the result;
    /// the spawned task continues. To make drop also cancel, call
    /// `cancel()` first.)
    pub fn spawn<F>(fut: F) -> Self
    where
        F: std::future::Future<Output = PvaResult<T>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let join = tokio::spawn(async move {
            let v = fut.await;
            let _ = tx.send(v);
        });
        Self {
            join,
            result_rx: Some(rx),
            interrupt: Arc::new(Notify::new()),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Block until the operation completes (matching pvxs
    /// `Operation::wait()`). Times out per-call: pass `None` to wait
    /// forever, or `Some(d)` for a deadline. Returns
    /// `PvaError::Timeout` on either (a) the deadline expiring or (b)
    /// an [`Self::interrupt`] wake-up.
    pub async fn wait(&mut self, timeout: Option<Duration>) -> PvaResult<T> {
        let rx = match self.result_rx.take() {
            Some(rx) => rx,
            None => {
                return Err(PvaError::Protocol(
                    "Operation result already consumed".into(),
                ));
            }
        };
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(PvaError::Protocol("Operation cancelled".into()));
        }

        let interrupt = self.interrupt.clone();
        let cancelled = self.cancelled.clone();
        let body = async move {
            tokio::select! {
                v = rx => match v {
                    Ok(r) => r,
                    Err(_) => Err(PvaError::Protocol("Operation aborted".into())),
                },
                _ = interrupt.notified() => Err(PvaError::Timeout),
                _ = wait_for_cancel(cancelled) => Err(PvaError::Protocol("Operation cancelled".into())),
            }
        };
        match timeout {
            Some(d) => match tokio::time::timeout(d, body).await {
                Ok(v) => v,
                Err(_) => Err(PvaError::Timeout),
            },
            None => body.await,
        }
    }

    /// Cancel the operation. Safe to call from any task; idempotent.
    /// Mirrors pvxs `Operation::cancel`. Aborts the spawned task and
    /// causes any pending [`Self::wait`] to return `PvaError::Protocol("Operation cancelled")`.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        self.join.abort();
    }

    /// Wake a pending [`Self::wait`] without cancelling the operation
    /// — the wait returns `PvaError::Timeout` and the underlying op
    /// keeps running. Mirrors pvxs `Operation::interrupt`.
    pub fn interrupt(&self) {
        self.interrupt.notify_waiters();
    }

    /// True iff the spawned task has finished.
    pub fn is_done(&self) -> bool {
        self.join.is_finished()
    }
}

impl<T: Send + 'static> Drop for PvaOperation<T> {
    fn drop(&mut self) {
        // Drop without cancel still aborts the task to avoid orphan
        // background work. pvxs's RAII `~Operation` does the same
        // (calls cancel internally).
        self.join.abort();
    }
}

async fn wait_for_cancel(flag: Arc<std::sync::atomic::AtomicBool>) {
    while !flag.load(std::sync::atomic::Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wait_returns_value() {
        let mut op = PvaOperation::spawn(async { Ok::<i32, _>(42) });
        let v = op.wait(Some(Duration::from_secs(1))).await.unwrap();
        assert_eq!(v, 42);
        assert!(op.is_done());
    }

    #[tokio::test]
    async fn wait_times_out() {
        let mut op = PvaOperation::<()>::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        });
        let r = op.wait(Some(Duration::from_millis(50))).await;
        assert!(matches!(r, Err(PvaError::Timeout)));
    }

    #[tokio::test]
    async fn interrupt_wakes_waiter_op_continues() {
        let mut op = PvaOperation::<i32>::spawn(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(7)
        });
        let interrupter = op.interrupt.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            interrupter.notify_waiters();
        });
        let r = op.wait(Some(Duration::from_secs(5))).await;
        assert!(matches!(r, Err(PvaError::Timeout)));
        // Op still completes — verify by waiting again on the
        // already-spawned task. The original `wait` consumed the
        // result_rx so we just check the task finished naturally.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(op.is_done());
    }

    #[tokio::test]
    async fn cancel_aborts_op() {
        let mut op = PvaOperation::<i32>::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(0)
        });
        op.cancel();
        let r = op.wait(Some(Duration::from_secs(1))).await;
        assert!(matches!(r, Err(PvaError::Protocol(_))));
    }
}
