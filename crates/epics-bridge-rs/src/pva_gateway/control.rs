//! G-G2: runtime-control PVs exposed under a configurable prefix.
//!
//! Mirrors `pva2pva` `ServerConfig::control_prefix` semantics — when an
//! operator sets a non-empty prefix on the gateway, a small set of
//! dynamic diagnostic PVs is added alongside the proxied namespace so
//! `pvget <prefix>:cacheSize` etc. return live state without going
//! through any upstream IOC.
//!
//! The PVs are intentionally read-only: writable control (drop one PV
//! from the cache, flush all entries, reload config) would need a
//! credentialed RPC surface which is out of scope for the first
//! implementation. Read-only diagnostics already cover the most
//! requested ops dashboards.
//!
//! ## Exposed PVs
//!
//! All names use the configurable prefix (no default — the feature is
//! opt-in via [`super::gateway::PvaGatewayConfig::control_prefix`]):
//!
//! | PV | Type | Description |
//! |----|------|-------------|
//! | `<prefix>:cacheSize` | Long | Live count of cached upstream entries |
//! | `<prefix>:upstreamCount` | Long | Alias of cacheSize (pva2pva-compat) |
//! | `<prefix>:liveSubscribers` | Long | Current bridge-task count (downstream sub bridges) |
//! | `<prefix>:report` | String | Multi-line diagnostic snapshot |

use std::sync::Arc;

use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};
use epics_pva_rs::server_native::ChannelSource;
use tokio::sync::mpsc;

use super::channel_cache::ChannelCache;
use super::source::GatewayChannelSource;

/// Diagnostic-PV source that lives behind the gateway's
/// `control_prefix`. Owned by the gateway alongside the proxy
/// `GatewayChannelSource`; both are registered into a
/// `CompositeSource` and dispatched in priority order.
#[derive(Clone)]
pub struct ControlSource {
    prefix: String,
    cache: Arc<ChannelCache>,
    gateway_source: GatewayChannelSource,
}

impl ControlSource {
    pub fn new(
        prefix: impl Into<String>,
        cache: Arc<ChannelCache>,
        gateway_source: GatewayChannelSource,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            cache,
            gateway_source,
        }
    }

    fn pv_names(&self) -> [String; 4] {
        [
            format!("{}:cacheSize", self.prefix),
            format!("{}:upstreamCount", self.prefix),
            format!("{}:liveSubscribers", self.prefix),
            format!("{}:report", self.prefix),
        ]
    }

    /// Build the NTScalar-shaped value for a Long counter so PVA
    /// clients see the same structure regardless of which counter PV
    /// they ask for. We deliberately keep the field set minimal
    /// (`value` only — no alarm/timeStamp shells) so the descriptor
    /// is small and the encode path stays cheap when these PVs are
    /// polled at high cadence.
    fn nt_scalar_long(v: i64) -> PvField {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Long(v))));
        PvField::Structure(s)
    }

    fn nt_scalar_long_desc() -> FieldDesc {
        FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Long))],
        }
    }

    fn nt_scalar_string(v: String) -> PvField {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::String(v))));
        PvField::Structure(s)
    }

    fn nt_scalar_string_desc() -> FieldDesc {
        FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::String))],
        }
    }

    fn matches(&self, name: &str) -> bool {
        self.pv_names().iter().any(|n| n == name)
    }
}

impl ChannelSource for ControlSource {
    async fn list_pvs(&self) -> Vec<String> {
        self.pv_names().to_vec()
    }

    async fn has_pv(&self, name: &str) -> bool {
        self.matches(name)
    }

    async fn get_introspection(&self, name: &str) -> Option<FieldDesc> {
        if !self.matches(name) {
            return None;
        }
        if name.ends_with(":report") {
            Some(Self::nt_scalar_string_desc())
        } else {
            Some(Self::nt_scalar_long_desc())
        }
    }

    async fn get_value(&self, name: &str) -> Option<PvField> {
        if !self.matches(name) {
            return None;
        }
        // Live snapshot: pulled at every GET. Cheap — no upstream
        // round-trip, just a HashMap len() under a tokio::Mutex plus
        // an atomic load for the bridge-task count.
        let cache_size = self.cache.entry_count().await as i64;
        let live_subs = self.gateway_source.live_subscribers() as i64;

        if name.ends_with(":cacheSize") || name.ends_with(":upstreamCount") {
            Some(Self::nt_scalar_long(cache_size))
        } else if name.ends_with(":liveSubscribers") {
            Some(Self::nt_scalar_long(live_subs))
        } else if name.ends_with(":report") {
            let report = format!(
                "cacheSize={cache_size} upstreamCount={cache_size} liveSubscribers={live_subs}"
            );
            Some(Self::nt_scalar_string(report))
        } else {
            None
        }
    }

    async fn is_writable(&self, _name: &str) -> bool {
        // Read-only diagnostics. PUT routes via the proxy
        // `GatewayChannelSource` for the namespace it owns; an attempt
        // to PUT one of the control PVs will surface
        // `is_writable=false` and the server will reject it with the
        // standard "channel not writable" status.
        false
    }

    async fn put_value(&self, _name: &str, _value: PvField) -> Result<(), String> {
        Err("control PVs are read-only".to_string())
    }

    async fn subscribe(&self, name: &str) -> Option<mpsc::Receiver<PvField>> {
        // Control PVs are snapshots: no live event source. The PVA
        // server's MONITOR INIT path bails when both subscribe_raw
        // and subscribe return None — the initial-snapshot emit
        // doesn't run, so a returning-None subscribe makes
        // `pvmonitor <prefix>:cacheSize` silently produce zero
        // frames. Return an empty channel: the server then falls
        // through to the get_value initial-snapshot path and emits
        // the current reading once before the channel idles.
        if !self.matches(name) {
            return None;
        }
        let (_tx, rx) = mpsc::channel::<PvField>(1);
        Some(rx)
    }
}
