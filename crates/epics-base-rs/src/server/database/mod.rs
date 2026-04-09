pub mod db_access;
mod field_io;
mod links;
mod processing;
mod scan_index;

use crate::runtime::sync::RwLock;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::server::pv::ProcessVariable;
use crate::server::record::{Record, RecordInstance, ScanType};
use crate::types::EpicsValue;

/// Parse a PV name into (base_name, field_name).
/// "TEMP.EGU" → ("TEMP", "EGU")
/// "TEMP"     → ("TEMP", "VAL")
pub fn parse_pv_name(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((base, field)) => (base, field),
        None => (name, "VAL"),
    }
}

/// Apply timestamp to a record based on its TSE field.
/// `is_soft` indicates a Soft Channel device type.
fn apply_timestamp(common: &mut super::record::CommonFields, _is_soft: bool) {
    match common.tse {
        0 => {
            // generalTime current time (default behavior).
            // Always update — C EPICS recGblGetTimeStamp sets TIME on every process.
            common.time = crate::runtime::general_time::get_current();
        }
        -1 => {
            // Device-provided time; fallback to generalTime BestTime if not set
            if common.time == std::time::SystemTime::UNIX_EPOCH {
                common.time = crate::runtime::general_time::get_event(-1);
            }
        }
        -2 => {
            // Keep TIME field as-is
        }
        _ => {
            // generalTime event time
            common.time = crate::runtime::general_time::get_event(common.tse as i32);
        }
    }
}

/// Unified entry in the PV database.
pub enum PvEntry {
    Simple(Arc<ProcessVariable>),
    Record(Arc<RwLock<RecordInstance>>),
}

/// Callback for resolving external PV names (CA/PVA links).
/// Returns the current value of the external PV, or None if unavailable.
pub type ExternalPvResolver = Arc<dyn Fn(&str) -> Option<EpicsValue> + Send + Sync>;

/// Async hook invoked by [`PvDatabase::has_name`] when a name is not yet
/// in the database. Used by the CA gateway and similar proxy components
/// to lazily populate PVs on first search.
///
/// The resolver should:
/// 1. Determine whether the name should be served (e.g., check ACL)
/// 2. Take whatever action is needed to make `has_name` return true on
///    a subsequent call (e.g., subscribe to an upstream IOC and call
///    `add_pv` with a placeholder value)
/// 3. Return `true` if the name is now resolvable, `false` otherwise
///
/// Returning `true` causes `has_name` to re-check the database. The
/// resolver may take some time (TCP search, upstream connect handshake);
/// the caller (UDP search responder, TCP CREATE_CHANNEL handler) will
/// `.await` it.
pub type SearchResolver = Arc<
    dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

struct PvDatabaseInner {
    simple_pvs: RwLock<HashMap<String, Arc<ProcessVariable>>>,
    records: RwLock<HashMap<String, Arc<RwLock<RecordInstance>>>>,
    /// Scan index: maps scan type → sorted set of (PHAS, record_name).
    scan_index: RwLock<HashMap<ScanType, BTreeSet<(i16, String)>>>,
    /// CP link index: maps source_record → list of target records to process when source changes.
    cp_links: RwLock<HashMap<String, Vec<String>>>,
    /// Optional resolver for external PVs (ca://, pva:// links).
    external_resolver: RwLock<Option<ExternalPvResolver>>,
    /// Optional async resolver invoked on `has_name` misses (e.g. CA gateway).
    search_resolver: RwLock<Option<SearchResolver>>,
}

/// Database of all process variables hosted by this server.
#[derive(Clone)]
pub struct PvDatabase {
    inner: Arc<PvDatabaseInner>,
}

/// Select which link indices are active based on SELM and SELN.
/// SELM: 0=All, 1=Specified, 2=Mask
fn select_link_indices(selm: i16, seln: i16, count: usize) -> Vec<usize> {
    match selm {
        0 => (0..count).collect(),
        1 => {
            let i = seln as usize;
            if i < count { vec![i] } else { vec![] }
        }
        2 => (0..count)
            .filter(|i| (seln as u16) & (1 << i) != 0)
            .collect(),
        _ => (0..count).collect(),
    }
}

impl PvDatabase {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(PvDatabaseInner {
                simple_pvs: RwLock::new(HashMap::new()),
                external_resolver: RwLock::new(None),
                search_resolver: RwLock::new(None),
                records: RwLock::new(HashMap::new()),
                scan_index: RwLock::new(HashMap::new()),
                cp_links: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Install an async resolver invoked when [`PvDatabase::has_name`]
    /// fails to find a name. Used by proxy/gateway implementations to
    /// lazily populate PVs on first search.
    pub async fn set_search_resolver(&self, resolver: SearchResolver) {
        *self.inner.search_resolver.write().await = Some(resolver);
    }

    /// Remove the previously installed search resolver, if any.
    pub async fn clear_search_resolver(&self) {
        *self.inner.search_resolver.write().await = None;
    }

    /// Set an external PV resolver for CA/PVA link resolution.
    /// The resolver is called synchronously from link reads.
    pub async fn set_external_resolver(&self, resolver: ExternalPvResolver) {
        *self.inner.external_resolver.write().await = Some(resolver);
    }

    /// Resolve an external PV name via the registered resolver.
    pub(crate) async fn resolve_external_pv(&self, name: &str) -> Option<EpicsValue> {
        let resolver = self.inner.external_resolver.read().await;
        resolver.as_ref().and_then(|r| r(name))
    }

    /// Add a simple PV with an initial value.
    pub async fn add_pv(&self, name: &str, initial: EpicsValue) {
        let pv = Arc::new(ProcessVariable::new(name.to_string(), initial));
        self.inner
            .simple_pvs
            .write()
            .await
            .insert(name.to_string(), pv);
    }

    /// Add a record (accepts a boxed Record to avoid double-boxing).
    pub async fn add_record(&self, name: &str, record: Box<dyn Record>) {
        let instance = RecordInstance::new_boxed(name.to_string(), record);
        let scan = instance.common.scan;
        let phas = instance.common.phas;
        self.inner
            .records
            .write()
            .await
            .insert(name.to_string(), Arc::new(RwLock::new(instance)));

        // Register in scan index
        if scan != ScanType::Passive {
            self.inner
                .scan_index
                .write()
                .await
                .entry(scan)
                .or_default()
                .insert((phas, name.to_string()));
        }
    }

    /// Internal: synchronous lookup without invoking the search resolver.
    async fn find_entry_no_resolve(&self, name: &str) -> Option<PvEntry> {
        let (base, _field) = parse_pv_name(name);

        if let Some(pv) = self.inner.simple_pvs.read().await.get(name) {
            return Some(PvEntry::Simple(pv.clone()));
        }
        if let Some(rec) = self.inner.records.read().await.get(base) {
            return Some(PvEntry::Record(rec.clone()));
        }
        None
    }

    /// Internal: synchronous existence check without resolver.
    async fn has_name_no_resolve(&self, name: &str) -> bool {
        let (base, _) = parse_pv_name(name);
        if self.inner.simple_pvs.read().await.contains_key(name) {
            return true;
        }
        self.inner.records.read().await.contains_key(base)
    }

    /// Look up an entry by name. Supports "record.FIELD" syntax.
    ///
    /// If the name is not found and a search resolver is installed,
    /// the resolver is invoked once. If the resolver returns true, the
    /// database is re-checked.
    pub async fn find_entry(&self, name: &str) -> Option<PvEntry> {
        if let Some(entry) = self.find_entry_no_resolve(name).await {
            return Some(entry);
        }
        // Try the search resolver
        let resolver = self.inner.search_resolver.read().await.clone();
        if let Some(r) = resolver {
            if r(name.to_string()).await {
                return self.find_entry_no_resolve(name).await;
            }
        }
        None
    }

    /// Check if a base name exists (for UDP search).
    ///
    /// If the name is not in the database and a search resolver is installed,
    /// the resolver is invoked. The resolver may populate the database
    /// (e.g., subscribe to an upstream IOC and add a placeholder PV) and
    /// return true; this method then re-checks.
    pub async fn has_name(&self, name: &str) -> bool {
        if self.has_name_no_resolve(name).await {
            return true;
        }
        let resolver = self.inner.search_resolver.read().await.clone();
        if let Some(r) = resolver {
            if r(name.to_string()).await {
                return self.has_name_no_resolve(name).await;
            }
        }
        false
    }

    /// Look up a simple PV by name (backward-compatible).
    pub async fn find_pv(&self, name: &str) -> Option<Arc<ProcessVariable>> {
        if let Some(pv) = self.inner.simple_pvs.read().await.get(name) {
            return Some(pv.clone());
        }
        None
    }

    /// Get a record Arc by name.
    pub async fn get_record(&self, name: &str) -> Option<Arc<RwLock<RecordInstance>>> {
        self.inner.records.read().await.get(name).cloned()
    }

    /// Get all record names.
    pub async fn all_record_names(&self) -> Vec<String> {
        self.inner.records.read().await.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_select_link_indices() {
        // All
        assert_eq!(select_link_indices(0, 0, 6), vec![0, 1, 2, 3, 4, 5]);
        // Specified
        assert_eq!(select_link_indices(1, 2, 6), vec![2]);
        assert_eq!(select_link_indices(1, 10, 6), Vec::<usize>::new());
        // Mask: seln=5 = 0b101 -> indices 0 and 2
        assert_eq!(select_link_indices(2, 5, 6), vec![0, 2]);
    }
}
