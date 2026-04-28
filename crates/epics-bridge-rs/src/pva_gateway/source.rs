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
use std::time::Duration;

use tokio::sync::mpsc;

use epics_pva_rs::pvdata::{FieldDesc, PvField};
use epics_pva_rs::server_native::source::ChannelSource;

use super::channel_cache::ChannelCache;

/// Default operation timeout for forwarded RPC calls. Matches the
/// upstream `PvaClient::pvrpc` timeout so we don't double-wait.
const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

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
}

impl GatewayChannelSource {
    pub fn new(cache: Arc<ChannelCache>) -> Self {
        Self {
            cache,
            connect_timeout: Duration::from_secs(5),
            subscriber_queue: 64,
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
        // Only report writable when the upstream actually has the PV
        // resolvable (cache hit OR a successful lookup). For an
        // unknown name, returning `true` would invite the downstream
        // client to issue PUT requests that we'd then have to reject —
        // pvxs convention is to advertise PUT capability honestly.
        self.cache.lookup(name, self.connect_timeout).await.is_ok()
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
            DEFAULT_RPC_TIMEOUT,
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
        let entry = self.cache.lookup(name, self.connect_timeout).await.ok()?;
        let mut bcast_rx = entry.subscribe();
        // pvxs sends one event per subscribe so the downstream sees
        // the current value immediately; emit our cached snapshot the
        // same way.
        let initial = entry.snapshot();

        let (mpsc_tx, mpsc_rx) = mpsc::channel(self.subscriber_queue);
        tokio::spawn(async move {
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
