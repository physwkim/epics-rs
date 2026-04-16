use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use crate::ndarray::NDArray;

/// Tracks the number of queued (in-flight) arrays across plugins.
/// Used by drivers to perform a bounded wait at end of acquisition.
pub struct QueuedArrayCounter {
    count: AtomicUsize,
    mutex: parking_lot::Mutex<()>,
    condvar: parking_lot::Condvar,
}

impl QueuedArrayCounter {
    /// Create a new counter starting at zero.
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            mutex: parking_lot::Mutex::new(()),
            condvar: parking_lot::Condvar::new(),
        }
    }

    /// Increment the queued count (called before send).
    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement the queued count. Notifies waiters when reaching zero.
    pub fn decrement(&self) {
        let prev = self.count.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let _guard = self.mutex.lock();
            self.condvar.notify_all();
        }
    }

    /// Current queued count.
    pub fn get(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    /// Wait until count reaches zero, or timeout expires.
    /// Returns `true` if count is zero, `false` on timeout.
    pub fn wait_until_zero(&self, timeout: Duration) -> bool {
        let mut guard = self.mutex.lock();
        if self.count.load(Ordering::Acquire) == 0 {
            return true;
        }
        !self
            .condvar
            .wait_while_for(
                &mut guard,
                |_| self.count.load(Ordering::Acquire) != 0,
                timeout,
            )
            .timed_out()
    }
}

impl Default for QueuedArrayCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Array message with optional queued-array counter and completion signal.
/// When dropped, decrements the counter (if present) — this signals that
/// the downstream plugin has finished processing the array.
pub struct ArrayMessage {
    pub array: Arc<NDArray>,
    pub(crate) counter: Option<Arc<QueuedArrayCounter>>,
    /// When Some, the sender awaits this to confirm downstream processing completed.
    /// Fired when ArrayMessage is dropped (i.e., after plugin process_array finishes).
    pub(crate) done_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for ArrayMessage {
    fn drop(&mut self) {
        if let Some(tx) = self.done_tx.take() {
            let _ = tx.send(());
        }
        if let Some(c) = self.counter.take() {
            c.decrement();
        }
    }
}

/// Sender held by upstream. Fully async, reliable (no drops).
///
/// # `blocking_callbacks` semantics
///
/// Both modes use reliable async enqueue (`send().await`). The difference is
/// how long the caller waits:
///
/// - `blocking_callbacks=0`: waits until the message is in the downstream queue
///   (enqueue guaranteed, processing NOT awaited).
/// - `blocking_callbacks=1`: waits until the downstream plugin has finished
///   processing the array (enqueue + completion awaited).
///
/// Neither mode drops arrays due to back-pressure — the caller yields instead.
#[derive(Clone)]
pub struct NDArraySender {
    tx: tokio::sync::mpsc::Sender<ArrayMessage>,
    port_name: String,
    enabled: Arc<AtomicBool>,
    blocking_mode: Arc<AtomicBool>,
    queued_counter: Option<Arc<QueuedArrayCounter>>,
}

impl NDArraySender {
    /// Publish an array downstream (async, reliable).
    ///
    /// - `enable_callbacks=0`: returns immediately, array not sent.
    /// - `blocking_callbacks=0`: awaits queue admission only.
    /// - `blocking_callbacks=1`: awaits queue admission + downstream processing completion.
    pub async fn publish(&self, array: Arc<NDArray>) {
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        if let Some(ref c) = self.queued_counter {
            c.increment();
        }

        let blocking = self.blocking_mode.load(Ordering::Acquire);
        let (done_tx, done_rx) = if blocking {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let msg = ArrayMessage {
            array,
            counter: self.queued_counter.clone(),
            done_tx,
        };

        if self.tx.send(msg).await.is_err() {
            // Channel closed — counter was decremented by ArrayMessage::drop
            return;
        }

        // blocking_callbacks=1: wait for downstream to finish processing
        if let Some(rx) = done_rx {
            let _ = rx.await;
        }
    }

    /// Whether this sender's plugin has callbacks enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// Whether this sender's plugin is in blocking mode.
    pub fn is_blocking(&self) -> bool {
        self.blocking_mode.load(Ordering::Acquire)
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// Set the queued-array counter for tracking in-flight arrays.
    pub fn set_queued_counter(&mut self, counter: Arc<QueuedArrayCounter>) {
        self.queued_counter = Some(counter);
    }

    /// Set the enabled/blocking mode flags (used by plugin runtime wiring).
    pub(crate) fn set_mode_flags(
        &mut self,
        enabled: Arc<AtomicBool>,
        blocking_mode: Arc<AtomicBool>,
    ) {
        self.enabled = enabled;
        self.blocking_mode = blocking_mode;
    }
}

/// Receiver held by downstream plugin.
pub struct NDArrayReceiver {
    rx: tokio::sync::mpsc::Receiver<ArrayMessage>,
}

impl NDArrayReceiver {
    /// Blocking receive (for use in std::thread data processing loops).
    pub fn blocking_recv(&mut self) -> Option<Arc<NDArray>> {
        self.rx.blocking_recv().map(|msg| msg.array.clone())
    }

    /// Async receive.
    pub async fn recv(&mut self) -> Option<Arc<NDArray>> {
        self.rx.recv().await.map(|msg| msg.array.clone())
    }

    /// Receive the full ArrayMessage (crate-internal). The message's Drop
    /// will signal completion when the caller is done with it.
    pub(crate) async fn recv_msg(&mut self) -> Option<ArrayMessage> {
        self.rx.recv().await
    }
}

/// Create a matched sender/receiver pair.
pub fn ndarray_channel(port_name: &str, queue_size: usize) -> (NDArraySender, NDArrayReceiver) {
    let (tx, rx) = tokio::sync::mpsc::channel(queue_size.max(1));
    (
        NDArraySender {
            tx,
            port_name: port_name.to_string(),
            enabled: Arc::new(AtomicBool::new(true)),
            blocking_mode: Arc::new(AtomicBool::new(false)),
            queued_counter: None,
        },
        NDArrayReceiver { rx },
    )
}

/// Fan-out: publishes arrays to multiple downstream receivers.
pub struct NDArrayOutput {
    senders: Vec<NDArraySender>,
}

impl NDArrayOutput {
    pub fn new() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    pub fn add(&mut self, sender: NDArraySender) {
        self.senders.push(sender);
    }

    pub fn remove(&mut self, port_name: &str) {
        self.senders.retain(|s| s.port_name != port_name);
    }

    /// Remove a sender by port name and return it (if found).
    pub fn take(&mut self, port_name: &str) -> Option<NDArraySender> {
        let idx = self.senders.iter().position(|s| s.port_name == port_name)?;
        Some(self.senders.swap_remove(idx))
    }

    /// Publish an array to all downstream receivers (async, reliable, concurrent).
    ///
    /// Each sender is awaited independently — a slow downstream does not
    /// block enqueue to sibling downstreams. The function returns after
    /// all senders have completed their publish (enqueue or completion,
    /// depending on `blocking_callbacks`).
    pub async fn publish(&self, array: Arc<NDArray>) {
        let futs = self.senders.iter().map(|s| s.publish(array.clone()));
        futures_util::future::join_all(futs).await;
    }

    /// Publish an array to a single downstream receiver by index (for scatter/round-robin).
    pub async fn publish_to(&self, index: usize, array: Arc<NDArray>) {
        if let Some(sender) = self.senders.get(index % self.senders.len().max(1)) {
            sender.publish(array).await;
        }
    }

    pub fn num_senders(&self) -> usize {
        self.senders.len()
    }

    /// Clone the senders list (for publishing outside a lock in async context).
    pub(crate) fn senders_clone(&self) -> Vec<NDArraySender> {
        self.senders.clone()
    }
}

/// Cloneable async handle for publishing arrays to downstream plugins.
///
/// This is the public API for driver acquisition tasks.
/// Internally it snapshots the sender list, releases the lock, then
/// publishes to all senders concurrently.
///
/// # Example
/// ```ignore
/// if config.array_callbacks {
///     publisher.publish(Arc::new(frame)).await;
/// }
/// ```
#[derive(Clone)]
pub struct ArrayPublisher {
    output: Arc<parking_lot::Mutex<NDArrayOutput>>,
}

impl ArrayPublisher {
    /// Create a publisher backed by the given output.
    pub fn new(output: Arc<parking_lot::Mutex<NDArrayOutput>>) -> Self {
        Self { output }
    }

    /// Publish an array to all downstream plugins (async, concurrent fan-out).
    pub async fn publish(&self, array: Arc<NDArray>) {
        let senders = self.output.lock().senders_clone();
        let futs = senders.iter().map(|s| s.publish(array.clone()));
        futures_util::future::join_all(futs).await;
    }
}

impl Default for NDArrayOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ndarray::{NDArray, NDDataType, NDDimension};

    fn make_test_array(id: i32) -> Arc<NDArray> {
        let mut arr = NDArray::new(vec![NDDimension::new(4)], NDDataType::UInt8);
        arr.unique_id = id;
        Arc::new(arr)
    }

    #[tokio::test]
    async fn test_publish_receive_basic() {
        let (sender, mut receiver) = ndarray_channel("TEST", 10);
        sender.publish(make_test_array(1)).await;
        sender.publish(make_test_array(2)).await;

        let a1 = receiver.recv().await.unwrap();
        assert_eq!(a1.unique_id, 1);
        let a2 = receiver.recv().await.unwrap();
        assert_eq!(a2.unique_id, 2);
    }

    #[tokio::test]
    async fn test_publish_no_drop() {
        // With reliable send().await, even a queue of 1 should not drop
        let (sender, mut receiver) = ndarray_channel("TEST", 1);

        // Spawn publisher that sends 3 arrays
        let s = sender.clone();
        let pub_handle = tokio::spawn(async move {
            s.publish(make_test_array(1)).await;
            s.publish(make_test_array(2)).await;
            s.publish(make_test_array(3)).await;
        });

        // Receive all 3 — no drops
        let a1 = receiver.recv().await.unwrap();
        assert_eq!(a1.unique_id, 1);
        let a2 = receiver.recv().await.unwrap();
        assert_eq!(a2.unique_id, 2);
        let a3 = receiver.recv().await.unwrap();
        assert_eq!(a3.unique_id, 3);

        pub_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_blocking_callbacks_completion_wait() {
        let (sender, mut receiver) = ndarray_channel("TEST", 10);
        sender.blocking_mode.store(true, Ordering::Release);

        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        // Spawn receiver that takes some time to process
        let recv_handle = tokio::spawn(async move {
            let msg = receiver.recv_msg().await.unwrap();
            assert_eq!(msg.array.unique_id, 42);
            // Simulate processing time
            tokio::time::sleep(Duration::from_millis(50)).await;
            completed_clone.store(true, Ordering::Release);
            // msg dropped here → done_tx fires
        });

        // publish() should wait for completion
        sender.publish(make_test_array(42)).await;

        // By the time publish returns, downstream should have completed
        assert!(completed.load(Ordering::Acquire));

        recv_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_fanout_three_receivers() {
        let (s1, mut r1) = ndarray_channel("P1", 10);
        let (s2, mut r2) = ndarray_channel("P2", 10);
        let (s3, mut r3) = ndarray_channel("P3", 10);

        let mut output = NDArrayOutput::new();
        output.add(s1);
        output.add(s2);
        output.add(s3);

        output.publish(make_test_array(42)).await;

        assert_eq!(r1.recv().await.unwrap().unique_id, 42);
        assert_eq!(r2.recv().await.unwrap().unique_id, 42);
        assert_eq!(r3.recv().await.unwrap().unique_id, 42);
    }

    #[test]
    fn test_blocking_recv() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (sender, mut receiver) = ndarray_channel("TEST", 10);

        let handle = std::thread::spawn(move || {
            let arr = receiver.blocking_recv().unwrap();
            arr.unique_id
        });

        rt.block_on(sender.publish(make_test_array(99)));
        let id = handle.join().unwrap();
        assert_eq!(id, 99);
    }

    #[tokio::test]
    async fn test_channel_closed_on_receiver_drop() {
        let (sender, receiver) = ndarray_channel("TEST", 10);
        drop(receiver);
        // Sending to closed channel should not panic
        sender.publish(make_test_array(1)).await;
    }

    #[test]
    fn test_queued_counter_basic() {
        let counter = QueuedArrayCounter::new();
        assert_eq!(counter.get(), 0);
        counter.increment();
        assert_eq!(counter.get(), 1);
        counter.increment();
        assert_eq!(counter.get(), 2);
        counter.decrement();
        assert_eq!(counter.get(), 1);
        counter.decrement();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_queued_counter_wait_until_zero() {
        let counter = Arc::new(QueuedArrayCounter::new());
        counter.increment();
        counter.increment();

        let c = counter.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            c.decrement();
            std::thread::sleep(Duration::from_millis(10));
            c.decrement();
        });

        assert!(counter.wait_until_zero(Duration::from_secs(5)));
        h.join().unwrap();
    }

    #[test]
    fn test_queued_counter_wait_timeout() {
        let counter = Arc::new(QueuedArrayCounter::new());
        counter.increment();
        assert!(!counter.wait_until_zero(Duration::from_millis(10)));
    }

    #[tokio::test]
    async fn test_publish_increments_counter() {
        let counter = Arc::new(QueuedArrayCounter::new());
        let (mut sender, mut _receiver) = ndarray_channel("TEST", 10);
        sender.set_queued_counter(counter.clone());

        sender.publish(make_test_array(1)).await;
        assert_eq!(counter.get(), 1);
        sender.publish(make_test_array(2)).await;
        assert_eq!(counter.get(), 2);
    }

    #[tokio::test]
    async fn test_message_drop_decrements() {
        let counter = Arc::new(QueuedArrayCounter::new());
        counter.increment();
        let msg = ArrayMessage {
            array: make_test_array(1),
            counter: Some(counter.clone()),
            done_tx: None,
        };
        assert_eq!(counter.get(), 1);
        drop(msg);
        assert_eq!(counter.get(), 0);
    }
}
