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

    async fn is_writable(&self, _name: &str) -> bool {
        // pvxs gateway is permit-by-default; ACL belongs in the
        // downstream server's auth-complete hook (future work).
        true
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

/// Convert a `PvField` into the string form pvput accepts. Limited
/// to the scalar / scalar-array shapes the gateway covers today;
/// returns `None` otherwise so the caller can surface a typed error.
fn pvfield_to_pvput_string(v: &PvField) -> Option<String> {
    match v {
        PvField::Scalar(sv) => Some(scalar_to_string(sv)),
        PvField::ScalarArray(items) => {
            // pvput accepts space-separated values for arrays.
            let parts: Vec<String> = items.iter().map(scalar_to_string).collect();
            Some(parts.join(" "))
        }
        PvField::Structure(s) => {
            // Common case: NTScalar `.value` field. Drill in.
            for (name, field) in &s.fields {
                if name == "value" {
                    return pvfield_to_pvput_string(field);
                }
            }
            None
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
