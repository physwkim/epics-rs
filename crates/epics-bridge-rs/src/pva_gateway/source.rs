//! `ChannelSource` impl that bridges the gateway's [`ChannelCache`] to
//! the downstream [`epics_pva_rs::server`].
//!
//! Mirrors the role of `pva2pva GWServerChannelProvider` (server.cpp):
//! every downstream PVA op (search, get, put, monitor, get_field) is
//! resolved by looking up the PV name in the cache and forwarding to
//! the cached upstream channel. Monitor subscriptions are fanned out
//! through a per-entry tokio broadcast channel so multiple downstream
//! clients share one upstream subscription.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use epics_pva_rs::pvdata::{FieldDesc, PvField};
use epics_pva_rs::server_native::source::ChannelSource;

use super::channel_cache::ChannelCache;

/// `ChannelSource` impl handed to the downstream `PvaServer`. Cheap
/// to clone (Arc-backed cache + a couple of `Duration`s).
#[derive(Clone)]
pub struct GatewayChannelSource {
    cache: Arc<ChannelCache>,
    /// How long to wait for the upstream to deliver a first monitor
    /// event when a downstream client searches for a previously
    /// unseen PV. Pass through to `ChannelCache::lookup`.
    pub connect_timeout: Duration,
    /// Bridge subscriber-side mpsc capacity. Each downstream subscriber
    /// has its own bridge task that copies broadcast events into this
    /// mpsc; an overrun causes the next event to be dropped. Pick a
    /// generous default (matches pvxs queue size 64).
    pub subscriber_queue: usize,
    /// Per-call timeout for forwarded RPC requests. Configurable so
    /// long-running channelArchiver-style RPCs don't get cut off at
    /// an arbitrary 30 s ceiling. Default 30 s (matches pvxs).
    pub rpc_timeout: Duration,
    /// Hard cap on simultaneous live subscribe-bridge tasks across all
    /// downstream peers. The PvaServer enforces a per-connection
    /// channel cap; this is the gateway-wide ceiling that defends
    /// against a coordinated burst from many peers exhausting the
    /// gateway's monitor-fanout machinery. Default 100 000.
    pub max_subscribers: usize,
    /// Live subscribe-bridge counter (decremented when the bridge
    /// task exits). Shared via Arc so cloning the source preserves
    /// the count across the multiple `Arc<dyn ChannelSourceObj>`
    /// handles the runtime holds.
    subscriber_count: Arc<AtomicUsize>,
}

impl GatewayChannelSource {
    pub fn new(cache: Arc<ChannelCache>) -> Self {
        Self {
            cache,
            connect_timeout: Duration::from_secs(5),
            subscriber_queue: 64,
            rpc_timeout: Duration::from_secs(30),
            max_subscribers: 100_000,
            subscriber_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Cache handle — useful for the gateway's own diagnostics.
    pub fn cache(&self) -> &Arc<ChannelCache> {
        &self.cache
    }

    /// Diagnostic accessor: how many entries are currently cached.
    pub async fn cached_entry_count(&self) -> usize {
        self.cache.entry_count().await
    }

    /// Diagnostic: live subscribe-bridge tasks.
    pub fn live_subscribers(&self) -> usize {
        self.subscriber_count.load(Ordering::Relaxed)
    }
}

impl ChannelSource for GatewayChannelSource {
    async fn list_pvs(&self) -> Vec<String> {
        self.cache.names().await
    }

    async fn has_pv(&self, name: &str) -> bool {
        // Trigger an upstream lookup so the very first downstream
        // SEARCH for a previously-unseen PV resolves correctly.
        // Subsequent calls hit the fast path.
        self.cache.lookup(name, self.connect_timeout).await.is_ok()
    }

    async fn get_introspection(&self, name: &str) -> Option<FieldDesc> {
        let entry = self.cache.lookup(name, self.connect_timeout).await.ok()?;
        entry.introspection()
    }

    async fn get_value(&self, name: &str) -> Option<PvField> {
        let entry = self.cache.lookup(name, self.connect_timeout).await.ok()?;
        // Prefer the cached monitor snapshot — same value the upstream
        // server would return to a fresh GET, no extra round-trip.
        entry.snapshot()
    }

    async fn put_value(&self, name: &str, value: PvField) -> Result<(), String> {
        // Look up the entry to keep the upstream channel alive (and
        // confirm the PV exists) before issuing the PUT through the
        // shared client. The client's connection pool reuses the
        // already-open server connection.
        let _entry = self
            .cache
            .lookup(name, self.connect_timeout)
            .await
            .map_err(|e| e.to_string())?;
        let value_str = pvfield_to_pvput_string(&value)
            .ok_or_else(|| "unsupported PvField shape for upstream PUT".to_string())?;
        self.cache
            .client()
            .pvput(name, &value_str)
            .await
            .map_err(|e| e.to_string())
    }

    async fn is_writable(&self, name: &str) -> bool {
        // Peek-only: report writable iff the entry is already in the
        // cache. We deliberately do NOT trigger a fresh upstream
        // lookup here. If we did, a malicious or buggy client could
        // probe N random names against `is_writable` and force N
        // upstream search-and-subscribe cycles, each holding an
        // upstream-monitor task open until `connect_timeout` fires
        // (search-storm vector). The honest answer for an unseen PV
        // is "I don't know yet" — pvxs convention treats that as
        // not-writable, which is what we return.
        self.cache.peek(name).await.is_some()
    }

    /// Forward an RPC request through the upstream client. The default
    /// trait impl returns "RPC not supported", which is a major p2pApp
    /// parity gap (review §1). With this override, RPC requests pass
    /// through transparently — `pvrpc` reuses the cached channel
    /// connection-pool entry so we don't pay a fresh search per call.
    async fn rpc(
        &self,
        name: &str,
        request_desc: FieldDesc,
        request_value: PvField,
    ) -> Result<(FieldDesc, PvField), String> {
        let _entry = self
            .cache
            .lookup(name, self.connect_timeout)
            .await
            .map_err(|e| e.to_string())?;
        let result = tokio::time::timeout(
            self.rpc_timeout,
            self.cache
                .client()
                .pvrpc(name, &request_desc, &request_value),
        )
        .await;
        match result {
            Ok(Ok(pair)) => Ok(pair),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(format!("upstream rpc timeout for {name}")),
        }
    }

    async fn subscribe(&self, name: &str) -> Option<mpsc::Receiver<PvField>> {
        // Gateway-wide subscriber cap (PG-G3). The underlying
        // PvaServer enforces a per-connection channel cap; this is
        // the global ceiling that defends against a coordinated
        // burst of N peers each requesting M monitors.
        let prev = self.subscriber_count.fetch_add(1, Ordering::Relaxed);
        if prev >= self.max_subscribers {
            self.subscriber_count.fetch_sub(1, Ordering::Relaxed);
            tracing::warn!(
                pv = %name,
                live = prev,
                cap = self.max_subscribers,
                "pva-gateway: subscriber cap reached, refusing"
            );
            return None;
        }

        let entry = match self.cache.lookup(name, self.connect_timeout).await {
            Ok(e) => e,
            Err(_) => {
                self.subscriber_count.fetch_sub(1, Ordering::Relaxed);
                return None;
            }
        };
        let mut bcast_rx = entry.subscribe();
        // pvxs sends one event per subscribe so the downstream sees
        // the current value immediately; emit our cached snapshot the
        // same way.
        let initial = entry.snapshot();

        let (mpsc_tx, mpsc_rx) = mpsc::channel(self.subscriber_queue);
        let counter = self.subscriber_count.clone();
        tokio::spawn(async move {
            // RAII: ensure the counter is always decremented even on
            // panic / early-return paths.
            struct CounterGuard(Arc<AtomicUsize>);
            impl Drop for CounterGuard {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::Relaxed);
                }
            }
            let _guard = CounterGuard(counter);

            if let Some(v) = initial {
                if mpsc_tx.send(v).await.is_err() {
                    return;
                }
            }
            loop {
                match bcast_rx.recv().await {
                    Ok(v) => {
                        if mpsc_tx.send(v).await.is_err() {
                            return;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Slow consumer; broadcast dropped some
                        // events. Swallow and keep going — next event
                        // resyncs the cache.
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
        });

        Some(mpsc_rx)
    }

    /// Forward downstream-to-gateway backpressure into telemetry. The
    /// PvaServer fires this when a per-connection monitor outbox crosses
    /// the high watermark (downstream peer not draining fast enough).
    /// We don't have a primitive to pause the upstream pipeline today —
    /// PvaClient::pvmonitor_typed has no `pause()` — but the bridge
    /// task's `mpsc.send().await` already propagates backpressure by
    /// causing broadcast `Lagged` events upstream of the slow peer. So
    /// the only thing left is to surface the crossing for operator
    /// observability. PG-G4 noted true upstream pipeline cancel as
    /// future work; logging is the action this round.
    fn notify_watermark_high(&self, name: &str) {
        tracing::warn!(
            pv = %name,
            "pva-gateway: downstream monitor outbox crossed high watermark"
        );
    }

    fn notify_watermark_low(&self, name: &str) {
        tracing::debug!(
            pv = %name,
            "pva-gateway: downstream monitor outbox drained below low watermark"
        );
    }
}

/// Convert a `PvField` into the string form pvput accepts. Covers:
/// * `Scalar` / `ScalarArray` directly
/// * `Structure` containing a `.value` field (NTScalar / NTScalarArray /
///   NTEnum index — anything where the put target is the canonical
///   `value` subfield)
/// * `Variant` and `Union` by recursively unwrapping the inner field
///
/// Returns `None` for shapes pvput cannot represent in string form
/// (e.g. nested structures with no `value` field). Callers surface
/// the `None` to the downstream client as a typed error so the user
/// gets a clear "unsupported PvField shape" message instead of a
/// silent drop. Without `pvput_field` (typed PUT through the client
/// API) on `PvaClient` this is the best the gateway can do today;
/// see review §3d for the longer-term plan.
fn pvfield_to_pvput_string(v: &PvField) -> Option<String> {
    match v {
        PvField::Scalar(sv) => Some(scalar_to_string(sv)),
        PvField::ScalarArray(items) => {
            // pvput accepts space-separated values for arrays.
            let parts: Vec<String> = items.iter().map(scalar_to_string).collect();
            Some(parts.join(" "))
        }
        PvField::Structure(s) => {
            for (name, field) in &s.fields {
                if name == "value" {
                    return pvfield_to_pvput_string(field);
                }
            }
            None
        }
        PvField::Variant(boxed) => pvfield_to_pvput_string(&boxed.value),
        PvField::Union {
            selector, value, ..
        } => {
            if *selector < 0 {
                None
            } else {
                pvfield_to_pvput_string(value)
            }
        }
        _ => None,
    }
}

fn scalar_to_string(sv: &epics_pva_rs::pvdata::ScalarValue) -> String {
    use epics_pva_rs::pvdata::ScalarValue::*;
    match sv {
        Boolean(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        Byte(x) => x.to_string(),
        UByte(x) => x.to_string(),
        Short(x) => x.to_string(),
        UShort(x) => x.to_string(),
        Int(x) => x.to_string(),
        UInt(x) => x.to_string(),
        Long(x) => x.to_string(),
        ULong(x) => x.to_string(),
        Float(x) => x.to_string(),
        Double(x) => x.to_string(),
        String(s) => s.clone(),
    }
}
