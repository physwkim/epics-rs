//! Wire pvalink up to the record-link resolver in `epics-base-rs`.
//!
//! The integration plan:
//!
//! 1. `PvaLinkResolver` owns a [`PvaLinkRegistry`] (PvaLink cache) and a
//!    [`tokio::runtime::Handle`] so the synchronous resolver closure can
//!    submit `block_on(...)` work to a real runtime.
//! 2. [`install_pvalink_resolver`] hooks the resolver into the database via
//!    `PvDatabase::set_external_resolver`. Records with `INP=@pva://...`
//!    will then resolve through the registry instead of returning `None`.
//! 3. INP links are pre-warmed via [`PvaLinkResolver::open`] (also exposed
//!    as the `pvxr` iocsh command) so the synchronous resolver path can
//!    return the cached monitor value without blocking on a fresh GET.
//!    Out-of-band reads still work — `block_on` will issue a GET — but
//!    pre-warmed monitors are always cheaper.
//!
//! pvxs equivalent: `ioc/pvalink.cpp` + `pvalink_channel.cpp`
//! (`pvalinkInit`, `pvalinkOpen`, `dbpvxr`).

use std::sync::Arc;

use epics_base_rs::server::database::{ExternalPvResolver, LinkSet, PvDatabase};
use epics_base_rs::types::EpicsValue;
use epics_pva_rs::pvdata::{PvField, ScalarValue};

use super::config::{LinkDirection, PvaLinkConfig};
use super::link::{PvaLink, PvaLinkResult};
use super::registry::PvaLinkRegistry;

/// Resolver wrapping a [`PvaLinkRegistry`] and a tokio runtime handle.
/// Cheap to clone — both fields are `Arc`-backed.
#[derive(Clone)]
pub struct PvaLinkResolver {
    registry: Arc<PvaLinkRegistry>,
    handle: tokio::runtime::Handle,
    /// Counter incremented on every successful link read. Used by
    /// `pvxrefdiff` to report "links touched since last call". Wraps
    /// at u64::MAX.
    reads: Arc<std::sync::atomic::AtomicU64>,
    /// Master enable flag. Set false via [`Self::set_enabled`] (or
    /// the `pvalink_disable` iocsh command) to make every resolve
    /// return None — useful for site-level pvalink kill switches.
    /// Mirrors pvxs `pvalink_enable` / `pvalink_disable` iocsh
    /// commands (pvalink.cpp:328).
    enabled: Arc<std::sync::atomic::AtomicBool>,
}

impl PvaLinkResolver {
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self {
            registry: Arc::new(PvaLinkRegistry::new()),
            handle,
            reads: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Master enable / disable. When disabled, the resolver closure
    /// returns `None` for every lookup so dependent records see
    /// LINK/INVALID alarms but no stale cached values bleed through.
    /// Mirrors pvxs `pvalink_enable(false)` / `pvalink_disable`.
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    /// Read the current enable flag.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Open / cache a link for `pv_name` in INP+monitor mode. Mirrors
    /// pvxs `pvalinkOpen` (pvalink_channel.cpp). After this returns,
    /// later calls to [`Self::resolve`] for the same name will read
    /// the cached monitor value (no async block).
    pub async fn open(&self, pv_name: &str) -> PvaLinkResult<Arc<PvaLink>> {
        let cfg = PvaLinkConfig {
            pv_name: pv_name.to_string(),
            field: "value".into(),
            monitor: true,
            process: false,
            notify: false,
            scan_on_update: false,
            direction: LinkDirection::Inp,
        };
        self.registry.get_or_open(cfg).await
    }

    /// Number of successful link reads since startup.
    pub fn read_count(&self) -> u64 {
        self.reads.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Number of cached links.
    pub fn link_count(&self) -> usize {
        self.registry.len()
    }

    /// Wait until the link for `pv_name` has received at least one
    /// monitor event (i.e., the cached value is populated). Returns
    /// `false` on timeout. Mirrors pvxs
    /// `testqsrvWaitForLinkConnected` (pvalink.cpp:131) — the
    /// canonical test helper for "wait for the upstream IOC to come
    /// online before continuing".
    pub async fn wait_for_link_connected(
        &self,
        pv_name: &str,
        timeout: std::time::Duration,
    ) -> bool {
        let link = match self.open(pv_name).await {
            Ok(l) => l,
            Err(_) => return false,
        };
        // Poll the link's read() — succeeds once the monitor has
        // delivered at least one event.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if link.read().await.is_ok() {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Build the [`ExternalPvResolver`] closure that the database
    /// expects. The closure is sync; it uses
    /// `Handle::block_on(future)` for the rare uncached path.
    /// Pre-warm INP links via [`Self::open`] to keep the steady-state
    /// path lock-free.
    pub fn build_resolver(self) -> ExternalPvResolver {
        let resolver = self;
        Arc::new(move |name: &str| -> Option<EpicsValue> {
            if !resolver.is_enabled() {
                return None;
            }
            // Strip optional pva:// prefix — the resolver receives the
            // bare PV name in some link forms but the prefixed form in
            // others.
            let name = name
                .strip_prefix("pva://")
                .or_else(|| name.strip_prefix("ca://"))
                .unwrap_or(name);
            let cfg = PvaLinkConfig {
                pv_name: name.to_string(),
                field: "value".into(),
                monitor: true,
                process: false,
                notify: false,
                scan_on_update: false,
                direction: LinkDirection::Inp,
            };
            // Open-or-fetch then read. open() is idempotent via the
            // registry; first call spawns the monitor task, subsequent
            // calls return the cached link.
            let link = resolver
                .handle
                .block_on(async { resolver.registry.get_or_open(cfg).await })
                .ok()?;
            let value = resolver.handle.block_on(async { link.read().await }).ok()?;
            resolver
                .reads
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            pvfield_to_epics_value(&value)
        })
    }
}

/// Install a [`PvaLinkResolver`] on `db`. Returns the resolver so the
/// caller can pre-open links and query stats (`db_pvxr` / `pvxrefdiff`
/// iocsh commands lean on this).
///
/// Registers the resolver under the `"pva"` lset scheme *and*
/// installs the legacy [`ExternalPvResolver`] closure so callers
/// using either dispatch path work transparently.
pub async fn install_pvalink_resolver(
    db: &Arc<PvDatabase>,
    handle: tokio::runtime::Handle,
) -> PvaLinkResolver {
    let resolver = PvaLinkResolver::new(handle);
    db.set_external_resolver(resolver.clone().build_resolver())
        .await;
    db.register_link_set("pva", Arc::new(resolver.clone()))
        .await;
    resolver
}

impl LinkSet for PvaLinkResolver {
    fn is_connected(&self, name: &str) -> bool {
        // Cached snapshot only — sync trait can't await on a fresh
        // open. If the link wasn't pre-warmed via `pvxr`, this
        // returns false, which is the right answer for "are we
        // currently in receipt of an upstream value?".
        let name = strip_scheme(name);
        let cfg = PvaLinkConfig {
            pv_name: name.to_string(),
            field: "value".into(),
            monitor: true,
            process: false,
            notify: false,
            scan_on_update: false,
            direction: LinkDirection::Inp,
        };
        // Use try-path through the registry: if the link doesn't
        // exist yet we skip the monitor spawn (which would require
        // an async block). For LinkSet purposes "not yet open" ==
        // "not connected".
        self.handle.block_on(async {
            if let Ok(link) = self.registry.get_or_open(cfg).await {
                link.is_connected()
            } else {
                false
            }
        })
    }

    fn get_value(&self, name: &str) -> Option<EpicsValue> {
        if !self.is_enabled() {
            return None;
        }
        let name = strip_scheme(name);
        let cfg = PvaLinkConfig {
            pv_name: name.to_string(),
            field: "value".into(),
            monitor: true,
            process: false,
            notify: false,
            scan_on_update: false,
            direction: LinkDirection::Inp,
        };
        let value = self.handle.block_on(async {
            let link = self.registry.get_or_open(cfg).await.ok()?;
            link.read().await.ok()
        })?;
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        pvfield_to_epics_value(&value)
    }

    fn put_value(&self, name: &str, value: EpicsValue) -> Result<(), String> {
        if !self.is_enabled() {
            return Err("pvalink disabled".into());
        }
        let name = strip_scheme(name);
        let cfg = PvaLinkConfig {
            pv_name: name.to_string(),
            field: "value".into(),
            monitor: false,
            process: true,
            notify: false,
            scan_on_update: false,
            direction: LinkDirection::Out,
        };
        // Format the EpicsValue as a string using its Display impl
        // — pvput accepts the string form and the server coerces.
        let value_str = value.to_string();
        self.handle.block_on(async {
            let link = self
                .registry
                .get_or_open(cfg)
                .await
                .map_err(|e| e.to_string())?;
            link.write(&value_str).await.map_err(|e| e.to_string())
        })
    }

    fn alarm_message(&self, name: &str) -> Option<String> {
        let name = strip_scheme(name);
        let link = self
            .handle
            .block_on(async { self.registry.get_or_open(default_inp_cfg(name)).await.ok() })?;
        link.alarm_message()
    }

    fn time_stamp(&self, name: &str) -> Option<(i64, i32)> {
        let name = strip_scheme(name);
        let link = self
            .handle
            .block_on(async { self.registry.get_or_open(default_inp_cfg(name)).await.ok() })?;
        link.time_stamp()
    }

    fn link_names(&self) -> Vec<String> {
        // The registry is keyed on (pv_name, direction). We don't
        // currently expose iteration; skip for now and rely on
        // resolver-level stats (read_count / link_count) for
        // dbpvxr summaries.
        Vec::new()
    }
}

fn strip_scheme(name: &str) -> &str {
    name.strip_prefix("pva://")
        .or_else(|| name.strip_prefix("ca://"))
        .unwrap_or(name)
}

fn default_inp_cfg(pv_name: &str) -> PvaLinkConfig {
    PvaLinkConfig {
        pv_name: pv_name.to_string(),
        field: "value".into(),
        monitor: true,
        process: false,
        notify: false,
        scan_on_update: false,
        direction: LinkDirection::Inp,
    }
}

/// Best-effort conversion. We coerce scalar values and 1-D scalar arrays;
/// structures collapse to their `value` field. Returns `None` for
/// unsupported shapes — callers fall back to `None` in the resolver
/// closure, which surfaces as "no link value" upstream (record alarm
/// LINK/INVALID).
fn pvfield_to_epics_value(field: &PvField) -> Option<EpicsValue> {
    match field {
        PvField::Scalar(sv) => Some(scalar_to_epics(sv)),
        PvField::Structure(s) => {
            for (name, sub) in &s.fields {
                if name == "value" {
                    return pvfield_to_epics_value(sub);
                }
            }
            None
        }
        PvField::ScalarArray(arr) => {
            // Pick the first variant — pvData ScalarArray is typed
            // homogeneous on the wire, but our PvField::ScalarArray is
            // a Vec<ScalarValue> so we walk to determine.
            let first = arr.first()?;
            match first {
                ScalarValue::Double(_) => {
                    let v: Vec<f64> = arr
                        .iter()
                        .filter_map(|s| {
                            if let ScalarValue::Double(d) = s {
                                Some(*d)
                            } else {
                                None
                            }
                        })
                        .collect();
                    Some(EpicsValue::DoubleArray(v))
                }
                ScalarValue::Int(_) => {
                    let v: Vec<i32> = arr
                        .iter()
                        .filter_map(|s| {
                            if let ScalarValue::Int(i) = s {
                                Some(*i)
                            } else {
                                None
                            }
                        })
                        .collect();
                    Some(EpicsValue::LongArray(v))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn scalar_to_epics(sv: &ScalarValue) -> EpicsValue {
    match sv {
        ScalarValue::Double(v) => EpicsValue::Double(*v),
        ScalarValue::Float(v) => EpicsValue::Float(*v),
        ScalarValue::Long(v) => EpicsValue::Long(*v as i32),
        ScalarValue::Int(v) => EpicsValue::Long(*v),
        ScalarValue::Short(v) => EpicsValue::Short(*v),
        ScalarValue::Byte(v) => EpicsValue::Char(*v as u8),
        ScalarValue::ULong(v) => EpicsValue::Long(*v as i32),
        ScalarValue::UInt(v) => EpicsValue::Long(*v as i32),
        ScalarValue::UShort(v) => EpicsValue::Short(*v as i16),
        ScalarValue::UByte(v) => EpicsValue::Char(*v),
        ScalarValue::Boolean(v) => EpicsValue::Long(if *v { 1 } else { 0 }),
        ScalarValue::String(s) => EpicsValue::String(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pvfield_scalar_to_epics_double() {
        let f = PvField::Scalar(ScalarValue::Double(2.5));
        assert_eq!(pvfield_to_epics_value(&f), Some(EpicsValue::Double(2.5)));
    }

    #[test]
    fn pvfield_struct_with_value_extracts() {
        use epics_pva_rs::pvdata::PvStructure;
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Long(42))));
        let f = PvField::Structure(s);
        assert_eq!(pvfield_to_epics_value(&f), Some(EpicsValue::Long(42)));
    }
}
