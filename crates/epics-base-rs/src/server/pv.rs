use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use crate::runtime::sync::{Mutex, RwLock, mpsc};

use crate::error::CaError;
use crate::server::snapshot::Snapshot;
use crate::types::{DbFieldType, EpicsValue};

/// Identity of the client driving a `WriteHook` invocation. Carries
/// the user/host/peer fields the CA TCP handler already tracks for
/// audit + access security, so a proxy hook (gateway, ACL filter,
/// putlog) can make decisions without re-deriving them.
#[derive(Debug, Clone, Default)]
pub struct WriteContext {
    /// CA `CLIENT_NAME` username, or empty if unknown.
    pub user: String,
    /// CA `HOST_NAME` hostname (or peer IP fallback), used for ACF
    /// matching against `HAG(...)` groups.
    pub host: String,
    /// Raw `peer.ip():peer.port()` string, retained for audit/log use.
    pub peer: String,
}

/// Async hook invoked by client-originated writes (CA `caput`, CA
/// `WRITE_NOTIFY`) before the PV's local value is set. Used by the CA
/// gateway and similar proxies to forward writes upstream instead of
/// landing them in the local `ProcessVariable`.
///
/// The hook receives the proposed new value plus a [`WriteContext`]
/// identifying the client, and must return either:
/// * `Ok(())` — the write was accepted (e.g. forwarded to upstream).
///   The caller does NOT update the local `value` field — the
///   subsequent upstream-monitor event is expected to do that. This
///   matches CA-gateway semantics where the cached value reflects
///   reality after the round-trip.
/// * `Err(CaError)` — the write was rejected. The caller surfaces
///   the error to the CA client (`WRITE_NOTIFY` carries the ECA
///   status). The hook itself decides whether to update local state
///   on rejection.
///
/// The hook is consulted only on the client → server path. Internal
/// callers (`ProcessVariable::set`, `put_pv_and_post`) bypass it so
/// the upstream-monitor forwarder can update local state without
/// recursing into itself.
///
/// ## Stale-local hazard
///
/// "Hook returns `Ok` → caller does NOT update local value" assumes
/// the upstream will emit a monitor event reflecting the new value.
/// EPICS records can violate that assumption: PP=NO fields,
/// PUT-only fields (e.g. `.PROC`), and records configured to suppress
/// monitor events on identical values. In those cases the shadow
/// PV remains at its pre-put value indefinitely — caput appears to
/// succeed but `caget` afterwards returns the old value.
///
/// Hook implementors who target such records SHOULD update the local
/// `ProcessVariable` themselves on `Ok` — typically by invoking
/// `pv.set(new_value).await` AFTER the upstream put-ack, accepting
/// the cost of one local mutation per put. The base hook contract
/// stays "do nothing on Ok" because most monitor-driven shadows
/// (the CA gateway's primary use case) WILL receive a monitor event
/// and updating locally would race with it.
///
/// ## Reentrancy
///
/// The TCP write path clones the hook `Arc` and releases the read
/// guard BEFORE invoking it, so a hook that calls
/// `pv.set_write_hook(...)` to swap itself does not deadlock. A hook
/// that calls `pv.set(...)` reentrantly is allowed but defeats the
/// "let the upstream-monitor update local state" contract — the
/// reentrant `set` will be silently overwritten by the next
/// upstream event.
pub type WriteHook = Arc<
    dyn Fn(EpicsValue, WriteContext) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), CaError>> + Send>,
        > + Send
        + Sync,
>;

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
    /// Optional hook consulted on client-originated writes. When set,
    /// the CA TCP write path delegates to the hook instead of doing a
    /// local `pv.set()`. See [`WriteHook`].
    write_hook: RwLock<Option<WriteHook>>,
}

impl ProcessVariable {
    pub fn new(name: String, initial: EpicsValue) -> Self {
        Self {
            name,
            value: RwLock::new(initial),
            subscribers: Mutex::new(Vec::new()),
            write_hook: RwLock::new(None),
        }
    }

    /// Install a write hook. Replaces any previously-installed hook.
    pub async fn set_write_hook(&self, hook: WriteHook) {
        *self.write_hook.write().await = Some(hook);
    }

    /// Remove any installed write hook.
    pub async fn clear_write_hook(&self) {
        *self.write_hook.write().await = None;
    }

    /// Snapshot of the installed write hook (clone of the `Arc`), or
    /// `None` if none. Used by the CA TCP write path; cheap.
    pub async fn write_hook(&self) -> Option<WriteHook> {
        self.write_hook.read().await.clone()
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
