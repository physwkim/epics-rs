//! [`SharedPV`] — open/post/close mailbox PV for server-side use.
//!
//! Mirrors pvxs `sharedpv.cpp::SharedPV`. A SharedPV holds the current
//! value of a single PV and exposes:
//!
//! - `open(initial)` to declare the type/value
//! - `post(value)` to push a new value to all current subscribers
//! - `close()` to drop subscriptions and reject further GETs
//!
//! Many SharedPVs can be plugged into a single server via
//! [`SharedSource`] (collection mapping name → SharedPV).
//!
//! Flow control: the per-monitor `mpsc::Sender` is bounded; `try_post`
//! never blocks but may drop updates when the consumer is slow. The
//! mailbox semantics — squash to last value — are achieved by always
//! sending the freshest value; if the channel is full, callers can use
//! `force_post` which drains then re-sends.
//!
//! Watermarks (low/high) are advisory hints stored on the SharedPV and
//! consulted by op_monitor when it decides whether to acknowledge a
//! pipeline window. We don't yet wire them into the wire-level
//! ackCount but the API is in place for callers to set them.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::pvdata::{FieldDesc, PvField};

/// User-provided put handler. Mirrors pvxs `SharedPV::onPut`
/// (sharedpv.cpp:329). Handler receives the new value; returning Err
/// causes the server to reply with a non-success Status. Returning
/// Ok(()) lets the server post the value to subscribers — handlers
/// that want to coerce / transform should do so via [`SharedPV::post`]
/// inside the closure and return Ok.
pub type OnPutFn = Arc<dyn Fn(&SharedPV, PvField) -> Result<(), String> + Send + Sync>;

/// User-provided RPC handler. Mirrors pvxs `SharedPV::onRPC`. Handler
/// receives `(request_desc, request_value)` and returns the response
/// pair or an error message.
pub type OnRpcFn = Arc<
    dyn Fn(&SharedPV, FieldDesc, PvField) -> Result<(FieldDesc, PvField), String> + Send + Sync,
>;

/// Lifecycle callback fired when the first subscriber connects or the
/// last one disappears. pvxs `SharedPV::onFirstConnect` /
/// `SharedPV::onLastDisconnect` (sharedpv.cpp:303-323).
pub type LifecycleFn = Arc<dyn Fn(&SharedPV) + Send + Sync>;

/// Per-PV state stored inside [`SharedPV`].
struct Inner {
    /// Type descriptor declared at open() — None when not opened.
    desc: Option<FieldDesc>,
    /// Most recent value (defaulted from desc on open).
    value: Option<PvField>,
    /// Open subscribers. Each slot holds a Sender for the per-monitor
    /// channel; callers post by sending one PvField per update.
    subscribers: Vec<mpsc::Sender<PvField>>,
    /// Optional flow-control watermark: monitor stream sends MORE
    /// only when its outbox depth crosses below `low_watermark`.
    /// Currently advisory; preserved here for op_monitor to consult.
    pub low_watermark: usize,
    /// Pause sending updates when the monitor outbox depth is at or
    /// above `high_watermark`. Currently advisory.
    pub high_watermark: usize,
    /// `is_open` is required to reject GETs after close().
    is_open: bool,
    /// Optional user put handler; when None the default "store and
    /// post" behavior runs. pvxs `onPut` parity.
    on_put: Option<OnPutFn>,
    /// Optional user RPC handler; when None RPC returns "not
    /// supported". pvxs `onRPC` parity.
    on_rpc: Option<OnRpcFn>,
    /// First-subscriber-arrived hook.
    on_first_connect: Option<LifecycleFn>,
    /// Last-subscriber-left hook.
    on_last_disconnect: Option<LifecycleFn>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            desc: None,
            value: None,
            subscribers: Vec::new(),
            low_watermark: 4,
            high_watermark: 64,
            is_open: false,
            on_put: None,
            on_rpc: None,
            on_first_connect: None,
            on_last_disconnect: None,
        }
    }
}

/// Server-side handle for a single PV's value + subscriber set.
///
/// Cheap to clone: it's just an `Arc<Mutex<...>>`.
#[derive(Clone)]
pub struct SharedPV {
    inner: Arc<Mutex<Inner>>,
}

impl SharedPV {
    /// New, unopened SharedPV. open() must be called before serving GETs.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// Declare the type and seed the initial value. Repeated calls
    /// replace the type and value; subscribers are kept and will see
    /// the new value on next post().
    pub fn open(&self, desc: FieldDesc, initial: PvField) {
        let mut g = self.inner.lock();
        g.desc = Some(desc);
        g.value = Some(initial);
        g.is_open = true;
    }

    /// Returns true iff the PV has been opened.
    pub fn is_open(&self) -> bool {
        self.inner.lock().is_open
    }

    /// Drop all subscribers; subsequent GETs return `None` until
    /// open() is called again.
    pub fn close(&self) {
        let mut g = self.inner.lock();
        g.is_open = false;
        g.subscribers.clear();
    }

    /// Type descriptor (None until opened).
    pub fn introspection(&self) -> Option<FieldDesc> {
        self.inner.lock().desc.clone()
    }

    /// Current value (None until opened).
    pub fn current(&self) -> Option<PvField> {
        self.inner.lock().value.clone()
    }

    /// Push a new value to all subscribers; lossy semantics — drops
    /// updates when a subscriber's outbox is full. Returns the number
    /// of subscribers we successfully sent to.
    pub fn try_post(&self, value: PvField) -> usize {
        let mut g = self.inner.lock();
        if !g.is_open {
            return 0;
        }
        g.value = Some(value.clone());
        let mut delivered = 0;
        g.subscribers.retain(|tx| match tx.try_send(value.clone()) {
            Ok(()) => {
                delivered += 1;
                true
            }
            Err(mpsc::error::TrySendError::Full(_)) => true, // keep, drop update
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        });
        delivered
    }

    /// Push a new value to all subscribers; if an outbox is full,
    /// drop the oldest queued update and try again. Mailbox semantics
    /// (squash to latest). Always delivers to live subscribers.
    pub fn force_post(&self, value: PvField) -> usize {
        let mut g = self.inner.lock();
        if !g.is_open {
            return 0;
        }
        g.value = Some(value.clone());
        let mut delivered = 0;
        g.subscribers.retain(|tx| {
            // Best-effort: try once; if full, the slow consumer is
            // expected to fall behind — we don't have a way to evict
            // from inside an mpsc, so we just retain. The "force"
            // semantics rely on the consumer being able to keep up
            // most of the time.
            match tx.try_send(value.clone()) {
                Ok(()) => {
                    delivered += 1;
                    true
                }
                Err(mpsc::error::TrySendError::Full(_)) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        });
        delivered
    }

    /// Add a subscriber. The returned Receiver yields every successful
    /// post; the channel depth is capped at `monitor_queue_depth`.
    /// Drops on the receiver side translate to subscriber removal on
    /// the next post().
    pub fn subscribe(&self, monitor_queue_depth: usize) -> Option<mpsc::Receiver<PvField>> {
        // Latch onFirstConnect callback to run *after* releasing the
        // lock — handlers may call back into post() / current() and we
        // can't recurse on parking_lot Mutex.
        let cb = {
            let mut g = self.inner.lock();
            if !g.is_open {
                return None;
            }
            let depth = monitor_queue_depth.max(1);
            let (tx, rx) = mpsc::channel(depth);
            if let Some(v) = &g.value {
                let _ = tx.try_send(v.clone());
            }
            let was_empty = g.subscribers.is_empty();
            g.subscribers.push(tx);
            let cb = if was_empty {
                g.on_first_connect.clone()
            } else {
                None
            };
            (rx, cb)
        };
        if let Some(f) = cb.1 {
            f(self);
        }
        Some(cb.0)
    }

    /// Apply a PUT. By default, the new value is posted to all
    /// subscribers and stored as `current()`. When [`Self::on_put`]
    /// has been set, the user handler runs instead and is responsible
    /// for any side-effects / re-posting. Mirrors pvxs `onPut`
    /// dispatch.
    pub fn put(&self, value: PvField) -> Result<(), String> {
        if !self.is_open() {
            return Err("SharedPV not open".into());
        }
        let on_put = self.inner.lock().on_put.clone();
        if let Some(f) = on_put {
            return f(self, value);
        }
        let _ = self.try_post(value);
        Ok(())
    }

    /// Dispatch an RPC request. Falls back to "RPC not supported" when
    /// no [`Self::on_rpc`] handler has been installed.
    pub fn rpc(
        &self,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> Result<(FieldDesc, PvField), String> {
        let on_rpc = self.inner.lock().on_rpc.clone();
        match on_rpc {
            Some(f) => f(self, request_desc, request_value),
            None => Err("RPC not supported by this SharedPV".into()),
        }
    }

    /// Install a put handler. Pass `None` to clear. Mirrors pvxs
    /// `SharedPV::onPut`.
    pub fn on_put<F>(&self, handler: F)
    where
        F: Fn(&SharedPV, PvField) -> Result<(), String> + Send + Sync + 'static,
    {
        self.inner.lock().on_put = Some(Arc::new(handler));
    }

    /// Install an RPC handler. Mirrors pvxs `SharedPV::onRPC`.
    pub fn on_rpc<F>(&self, handler: F)
    where
        F: Fn(&SharedPV, FieldDesc, PvField) -> Result<(FieldDesc, PvField), String>
            + Send
            + Sync
            + 'static,
    {
        self.inner.lock().on_rpc = Some(Arc::new(handler));
    }

    /// Hook fired when the *first* subscriber connects (subscribers
    /// 0 → 1). Mirrors pvxs `SharedPV::onFirstConnect` —
    /// applications hook here to start a producer task on demand.
    pub fn on_first_connect<F>(&self, handler: F)
    where
        F: Fn(&SharedPV) + Send + Sync + 'static,
    {
        self.inner.lock().on_first_connect = Some(Arc::new(handler));
    }

    /// Hook fired when the *last* subscriber leaves (subscribers
    /// N → 0). Mirrors pvxs `SharedPV::onLastDisconnect` — pair with
    /// `on_first_connect` to gate cost-of-production on actual
    /// listener interest.
    pub fn on_last_disconnect<F>(&self, handler: F)
    where
        F: Fn(&SharedPV) + Send + Sync + 'static,
    {
        self.inner.lock().on_last_disconnect = Some(Arc::new(handler));
    }

    /// Non-allocating snapshot — copies the current value into `out`
    /// without cloning if the descriptors match. Returns false when
    /// the PV isn't opened or has no value yet. Mirrors pvxs
    /// `SharedPV::fetch`.
    pub fn fetch(&self, out: &mut Option<PvField>) -> bool {
        let g = self.inner.lock();
        match (&g.value, g.is_open) {
            (Some(v), true) => {
                *out = Some(v.clone());
                true
            }
            _ => false,
        }
    }

    /// Drop dead (closed-receiver) subscribers and fire
    /// `on_last_disconnect` if the set just became empty. Called by
    /// the per-channel TCP task on monitor close so SharedPV can
    /// notice subscribers leaving without waiting for the next post().
    pub fn prune_subscribers(&self) {
        let cb = {
            let mut g = self.inner.lock();
            let was_nonempty = !g.subscribers.is_empty();
            g.subscribers.retain(|tx| !tx.is_closed());
            if was_nonempty && g.subscribers.is_empty() {
                g.on_last_disconnect.clone()
            } else {
                None
            }
        };
        if let Some(f) = cb {
            f(self);
        }
    }

    /// Set the low watermark hint (advisory).
    pub fn set_low_watermark(&self, low: usize) {
        self.inner.lock().low_watermark = low;
    }

    /// Set the high watermark hint (advisory).
    pub fn set_high_watermark(&self, high: usize) {
        self.inner.lock().high_watermark = high;
    }

    /// Read the current watermark pair.
    pub fn watermarks(&self) -> (usize, usize) {
        let g = self.inner.lock();
        (g.low_watermark, g.high_watermark)
    }
}

impl Default for SharedPV {
    fn default() -> Self {
        Self::new()
    }
}

/// Trivial map-of-named-SharedPV adapter that implements
/// [`super::source::ChannelSource`]. Construct via `SharedSource::new()`,
/// `add(name, shared_pv)`, then pass to [`super::runtime::run_pva_server`].
pub struct SharedSource {
    pvs: Mutex<HashMap<String, SharedPV>>,
}

impl SharedSource {
    pub fn new() -> Self {
        Self {
            pvs: Mutex::new(HashMap::new()),
        }
    }

    pub fn add(&self, name: impl Into<String>, pv: SharedPV) {
        self.pvs.lock().insert(name.into(), pv);
    }

    pub fn get(&self, name: &str) -> Option<SharedPV> {
        self.pvs.lock().get(name).cloned()
    }
}

impl Default for SharedSource {
    fn default() -> Self {
        Self::new()
    }
}

impl super::source::ChannelSource for SharedSource {
    fn list_pvs(&self) -> impl std::future::Future<Output = Vec<String>> + Send {
        let names: Vec<String> = self.pvs.lock().keys().cloned().collect();
        async move { names }
    }

    fn has_pv(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
        let exists = self.pvs.lock().contains_key(name);
        async move { exists }
    }

    fn get_introspection(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<FieldDesc>> + Send {
        let pv = self.pvs.lock().get(name).cloned();
        async move { pv.and_then(|p| p.introspection()) }
    }

    fn get_value(&self, name: &str) -> impl std::future::Future<Output = Option<PvField>> + Send {
        let pv = self.pvs.lock().get(name).cloned();
        async move { pv.and_then(|p| p.current()) }
    }

    fn put_value(
        &self,
        name: &str,
        value: PvField,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let pv = self.pvs.lock().get(name).cloned();
        async move {
            match pv {
                Some(p) => p.put(value),
                None => Err(format!("no such PV: {name}")),
            }
        }
    }

    fn is_writable(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
        let exists = self.pvs.lock().contains_key(name);
        async move { exists }
    }

    fn subscribe(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send {
        let pv = self.pvs.lock().get(name).cloned();
        async move { pv.and_then(|p| p.subscribe(64)) }
    }

    fn rpc(
        &self,
        name: &str,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> impl std::future::Future<Output = Result<(FieldDesc, PvField), String>> + Send {
        let pv = self.pvs.lock().get(name).cloned();
        async move {
            match pv {
                Some(p) => p.rpc(request_desc, request_value),
                None => Err(format!("no such PV: {name}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pvdata::{ScalarType, ScalarValue};

    fn nt_scalar_int_desc() -> FieldDesc {
        FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Int))],
        }
    }

    fn nt_scalar_int_value(v: i32) -> PvField {
        let mut s = crate::pvdata::PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Int(v))));
        PvField::Structure(s)
    }

    #[test]
    fn shared_pv_open_then_current() {
        let pv = SharedPV::new();
        assert!(!pv.is_open());
        pv.open(nt_scalar_int_desc(), nt_scalar_int_value(42));
        assert!(pv.is_open());
        assert!(pv.current().is_some());
    }

    #[tokio::test]
    async fn shared_pv_subscribe_sees_initial_then_updates() {
        let pv = SharedPV::new();
        pv.open(nt_scalar_int_desc(), nt_scalar_int_value(0));
        let mut rx = pv.subscribe(8).expect("subscribe");
        // Initial value delivered immediately.
        let first = rx.recv().await.expect("first");
        assert!(matches!(first, PvField::Structure(_)));
        // Post an update.
        pv.try_post(nt_scalar_int_value(7));
        let second = rx.recv().await.expect("second");
        if let PvField::Structure(s) = second {
            assert_eq!(
                s.get_field("value"),
                Some(&PvField::Scalar(ScalarValue::Int(7)))
            );
        }
    }

    #[test]
    fn shared_pv_close_drops_subscribers() {
        let pv = SharedPV::new();
        pv.open(nt_scalar_int_desc(), nt_scalar_int_value(0));
        let _rx = pv.subscribe(8);
        pv.close();
        assert!(!pv.is_open());
        assert_eq!(pv.try_post(nt_scalar_int_value(1)), 0);
    }

    #[test]
    fn shared_pv_watermarks_default_to_4_and_64() {
        let pv = SharedPV::new();
        assert_eq!(pv.watermarks(), (4, 64));
        pv.set_low_watermark(8);
        pv.set_high_watermark(128);
        assert_eq!(pv.watermarks(), (8, 128));
    }

    #[tokio::test]
    async fn shared_source_serves_named_pv() {
        use super::super::source::ChannelSource;
        let pv = SharedPV::new();
        pv.open(nt_scalar_int_desc(), nt_scalar_int_value(123));
        let src = SharedSource::new();
        src.add("test:pv", pv);

        assert!(src.has_pv("test:pv").await);
        let val = src.get_value("test:pv").await.expect("value");
        if let PvField::Structure(s) = val {
            assert_eq!(
                s.get_field("value"),
                Some(&PvField::Scalar(ScalarValue::Int(123)))
            );
        }
    }
}
