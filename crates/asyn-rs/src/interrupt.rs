use std::time::SystemTime;

use tokio::sync::broadcast;

use crate::error::AsynResult;
use crate::param::ParamValue;

/// Filter for selecting which interrupts to receive.
#[derive(Debug, Clone, Default)]
pub struct InterruptFilter {
    /// If set, only receive interrupts with this reason (parameter index).
    pub reason: Option<usize>,
    /// If set, only receive interrupts with this addr.
    pub addr: Option<i32>,
    /// For UInt32Digital: bitmask of bits this subscriber is interested in.
    /// If set, only interrupts where changed bits overlap this mask are forwarded.
    /// C parity: pInterrupt->mask in asynUInt32DigitalInterrupt.
    pub uint32_mask: Option<u32>,
}

/// RAII subscription handle. Dropping this cancels the subscription.
pub struct InterruptSubscription {
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl InterruptSubscription {
    fn new(cancel_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            cancel_tx: Some(cancel_tx),
        }
    }
}

impl Drop for InterruptSubscription {
    fn drop(&mut self) {
        // Signal the forwarding task to stop
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Value delivered through the interrupt system.
#[derive(Debug, Clone)]
pub struct InterruptValue {
    pub reason: usize,
    pub addr: i32,
    pub value: ParamValue,
    pub timestamp: SystemTime,
    /// For UInt32Digital: bitmask of which bits changed (for per-callback filtering).
    pub uint32_changed_mask: u32,
}

/// Manages interrupt/callback delivery via async broadcast channel.
///
/// Async subscribers use `tokio::sync::broadcast` (multiple consumers OK).
/// Filtered subscriptions are created via `register_interrupt_user`.
pub struct InterruptManager {
    async_tx: broadcast::Sender<InterruptValue>,
}

impl InterruptManager {
    pub fn new(async_capacity: usize) -> Self {
        let (async_tx, _) = broadcast::channel(async_capacity);
        Self { async_tx }
    }

    /// Create an InterruptManager that shares an existing broadcast sender.
    /// This allows subscribing to interrupts from a driver that has been moved
    /// into an actor.
    pub fn from_broadcast_sender(sender: broadcast::Sender<InterruptValue>) -> Self {
        Self { async_tx: sender }
    }

    /// Subscribe for async interrupt delivery. Multiple subscribers OK.
    pub fn subscribe_async(&self) -> broadcast::Receiver<InterruptValue> {
        self.async_tx.subscribe()
    }

    /// Clone the broadcast sender. This allows external code to subscribe
    /// to interrupts even after the InterruptManager is moved.
    pub fn broadcast_sender(&self) -> broadcast::Sender<InterruptValue> {
        self.async_tx.clone()
    }

    /// Send an interrupt to all subscribers.
    /// Silently ignores errors if no receivers are registered.
    pub fn notify(&self, value: InterruptValue) {
        let _ = self.async_tx.send(value);
    }

    /// Register a filtered interrupt subscription.
    ///
    /// Returns an RAII `InterruptSubscription` (dropping it unsubscribes) and an
    /// `mpsc::Receiver<InterruptValue>` for receiving matching interrupts.
    ///
    /// The filter is applied by a background tokio task that subscribes to the
    /// broadcast channel and forwards matching values.
    pub fn register_interrupt_user(
        &self,
        filter: InterruptFilter,
    ) -> (
        InterruptSubscription,
        tokio::sync::mpsc::Receiver<InterruptValue>,
    ) {
        let mut intr_rx = self.async_tx.subscribe();
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => break,
                    recv = intr_rx.recv() => {
                        match recv {
                            Ok(iv) => {
                                if let Some(r) = filter.reason {
                                    if iv.reason != r { continue; }
                                }
                                if let Some(a) = filter.addr {
                                    if iv.addr != a { continue; }
                                }
                                // C parity: UInt32Digital per-callback mask filtering
                                if let Some(m) = filter.uint32_mask {
                                    if iv.uint32_changed_mask & m == 0 { continue; }
                                }
                                if tx.send(iv).await.is_err() { break; }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {}
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });

        (InterruptSubscription::new(cancel_tx), rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_subscribe_receive() {
        let im = InterruptManager::new(16);
        let mut rx = im.subscribe_async();
        im.notify(InterruptValue {
            reason: 1,
            addr: 0,
            value: ParamValue::Float64(3.14),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });
        let v = rx.recv().await.unwrap();
        assert_eq!(v.reason, 1);
    }

    #[tokio::test]
    async fn test_async_multiple_subscribers() {
        let im = InterruptManager::new(16);
        let mut rx1 = im.subscribe_async();
        let mut rx2 = im.subscribe_async();
        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(99),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });
        let v1 = rx1.recv().await.unwrap();
        let v2 = rx2.recv().await.unwrap();
        assert_eq!(v1.reason, 0);
        assert_eq!(v2.reason, 0);
    }

    #[tokio::test]
    async fn test_register_interrupt_user_filter_by_reason() {
        let im = InterruptManager::new(16);
        let (_sub, mut rx) = im.register_interrupt_user(InterruptFilter {
            reason: Some(1),
            addr: None,
            ..Default::default()
        });

        // Send reason 0 — should NOT be received
        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(10),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        // Send reason 1 — should be received
        im.notify(InterruptValue {
            reason: 1,
            addr: 0,
            value: ParamValue::Int32(20),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        let v = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.reason, 1);
        if let ParamValue::Int32(n) = v.value {
            assert_eq!(n, 20);
        } else {
            panic!("expected Int32");
        }
    }

    #[tokio::test]
    async fn test_register_interrupt_user_filter_by_addr() {
        let im = InterruptManager::new(16);
        let (_sub, mut rx) = im.register_interrupt_user(InterruptFilter {
            reason: None,
            addr: Some(3),
            ..Default::default()
        });

        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(1),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });
        im.notify(InterruptValue {
            reason: 0,
            addr: 3,
            value: ParamValue::Int32(2),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        let v = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.addr, 3);
    }

    #[tokio::test]
    async fn test_register_interrupt_user_no_filter() {
        let im = InterruptManager::new(16);
        let (_sub, mut rx) = im.register_interrupt_user(InterruptFilter::default());

        im.notify(InterruptValue {
            reason: 5,
            addr: 2,
            value: ParamValue::Float64(1.5),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        let v = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.reason, 5);
        assert_eq!(v.addr, 2);
    }

    #[tokio::test]
    async fn test_register_interrupt_user_drop_unsubscribes() {
        let im = InterruptManager::new(16);
        let (sub, mut rx) = im.register_interrupt_user(InterruptFilter::default());

        // Drop subscription
        drop(sub);

        // Allow the spawned task to see the cancel signal
        tokio::task::yield_now().await;

        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(999),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        // Should not receive anything (or channel is closed)
        let result = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        match result {
            Ok(None) => {} // channel closed — correct
            Err(_) => {}   // timed out — also acceptable (task hasn't exited yet)
            Ok(Some(_)) => panic!("should not receive after unsubscribe"),
        }
    }

    #[tokio::test]
    async fn test_register_interrupt_user_multiple_subscribers() {
        let im = InterruptManager::new(16);
        let (_sub1, mut rx1) = im.register_interrupt_user(InterruptFilter {
            reason: Some(0),
            addr: None,
            ..Default::default()
        });
        let (_sub2, mut rx2) = im.register_interrupt_user(InterruptFilter {
            reason: Some(1),
            addr: None,
            ..Default::default()
        });

        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(10),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });
        im.notify(InterruptValue {
            reason: 1,
            addr: 0,
            value: ParamValue::Int32(20),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });

        let v1 = tokio::time::timeout(std::time::Duration::from_millis(100), rx1.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v1.reason, 0);

        let v2 = tokio::time::timeout(std::time::Duration::from_millis(100), rx2.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v2.reason, 1);
    }

    #[test]
    fn test_notify_no_subscribers_no_panic() {
        let im = InterruptManager::new(16);
        im.notify(InterruptValue {
            reason: 0,
            addr: 0,
            value: ParamValue::Int32(1),
            timestamp: SystemTime::now(),
            uint32_changed_mask: 0,
        });
    }
}
