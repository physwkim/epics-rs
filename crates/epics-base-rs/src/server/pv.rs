use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use crate::runtime::sync::{Mutex, RwLock, mpsc};

use crate::server::snapshot::Snapshot;
use crate::types::{DbFieldType, EpicsValue};

/// A monitor event sent to subscribers when a PV value changes.
/// Carries a full Snapshot so GR/CTRL metadata (PREC, EGU, limits) is available.
#[derive(Debug, Clone)]
pub struct MonitorEvent {
    pub snapshot: Snapshot,
    /// Origin writer ID. When non-zero, subscribers with the same
    /// `ignore_origin` can filter out self-triggered events.
    /// Used to prevent sequencer write-back loops.
    ///
    /// **Scope**: Currently tagged on `put_pv_and_post_with_origin` events only.
    /// Events from `process_record_with_links` (process path) always have
    /// origin=0. If a future sequencer needs to filter process-path events
    /// too, origin tagging can be extended to the process path by passing
    /// origin through `ProcessOutcome` or `process_record_with_links`.
    pub origin: u64,
}

/// A subscriber waiting for PV value updates.
pub struct Subscriber {
    pub sid: u32,
    pub data_type: DbFieldType,
    pub mask: u16,
    pub tx: mpsc::Sender<MonitorEvent>,
    /// Last-value coalescing slot. When the bounded mpsc above is full,
    /// the producer stores the newest event here, overwriting any prior
    /// pending overflow value. The consumer drains this after each normal
    /// recv() to deliver the most recent state — matching libca rsrv
    /// "drop oldest, keep newest" semantics.
    pub coalesced: Arc<StdMutex<Option<MonitorEvent>>>,
}

/// A process variable hosted by the server.
pub struct ProcessVariable {
    pub name: String,
    pub value: RwLock<EpicsValue>,
    pub subscribers: Mutex<Vec<Subscriber>>,
}

impl ProcessVariable {
    pub fn new(name: String, initial: EpicsValue) -> Self {
        Self {
            name,
            value: RwLock::new(initial),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Get the current value.
    pub async fn get(&self) -> EpicsValue {
        self.value.read().await.clone()
    }

    /// Build a Snapshot (minimal: value + zero alarm + now, no metadata).
    pub async fn snapshot(&self) -> Snapshot {
        let value = self.value.read().await.clone();
        Snapshot::new(value, 0, 0, crate::runtime::time::now_wall())
    }

    /// Set a new value and notify all subscribers.
    pub async fn set(&self, new_value: EpicsValue) {
        {
            let mut val = self.value.write().await;
            *val = new_value.clone();
        }
        self.notify_subscribers(new_value).await;
    }

    /// Notify all subscribers of a new value.
    async fn notify_subscribers(&self, value: EpicsValue) {
        let mut subs = self.subscribers.lock().await;
        // Remove subscribers whose channel has been dropped
        subs.retain(|sub| !sub.tx.is_closed());
        for sub in subs.iter() {
            let snapshot = Snapshot::new(value.clone(), 0, 0, crate::runtime::time::now_wall());
            let event = MonitorEvent {
                snapshot,
                origin: 0,
            };
            if sub.tx.try_send(event.clone()).is_err() {
                // Queue full — overwrite any prior pending overflow with
                // the newest event. The consumer will pick it up via
                // `pop_coalesced` after the next normal recv.
                if let Ok(mut slot) = sub.coalesced.lock() {
                    *slot = Some(event);
                }
            }
        }
    }

    /// Add a subscriber. Returns the receiver for monitor events.
    pub async fn add_subscriber(
        &self,
        sid: u32,
        data_type: DbFieldType,
        mask: u16,
    ) -> mpsc::Receiver<MonitorEvent> {
        let (tx, rx) = mpsc::channel(64);
        let sub = Subscriber {
            sid,
            data_type,
            mask,
            tx,
            coalesced: Arc::new(StdMutex::new(None)),
        };
        self.subscribers.lock().await.push(sub);
        rx
    }

    /// Remove a subscriber by subscription ID.
    pub async fn remove_subscriber(&self, sid: u32) {
        let mut subs = self.subscribers.lock().await;
        subs.retain(|s| s.sid != sid);
    }

    /// Take any pending coalesced overflow value for the given subscriber.
    /// Called by the per-subscription forwarder task after each delivery
    /// so a slow consumer always converges on the latest known value.
    pub async fn pop_coalesced(&self, sid: u32) -> Option<MonitorEvent> {
        let subs = self.subscribers.lock().await;
        let sub = subs.iter().find(|s| s.sid == sid)?;
        sub.coalesced.lock().ok()?.take()
    }
}
