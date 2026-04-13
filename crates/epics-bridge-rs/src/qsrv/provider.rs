//! ChannelProvider trait and BridgeProvider implementation.
//!
//! Corresponds to C++ QSRV's `PDBProvider` (pdb.h/pdb.cpp).
//!
//! The trait definitions here are temporary — they will move to `epics-pva-rs`
//! once the PVA server is implemented by the spvirit maintainer.

use std::collections::HashMap;
use std::sync::Arc;

use epics_base_rs::server::database::PvDatabase;
use epics_pva_rs::pvdata::{FieldDesc, PvStructure};

use epics_base_rs::types::DbFieldType;

use super::channel::BridgeChannel;
use super::group::GroupChannel;
use super::group_config::GroupPvDef;
use super::pvif::NtType;
use crate::error::{BridgeError, BridgeResult};

// ---------------------------------------------------------------------------
// Access control
// ---------------------------------------------------------------------------

/// Access control interface for PVA channels.
///
/// Corresponds to C++ QSRV's per-channel ASCLIENT checks.
/// Default implementation allows all access.
pub trait AccessControl: Send + Sync {
    /// Check if the client can read this channel.
    fn can_read(&self, _channel: &str, _user: &str, _host: &str) -> bool {
        true
    }

    /// Check if the client can write to this channel.
    fn can_write(&self, _channel: &str, _user: &str, _host: &str) -> bool {
        true
    }
}

/// Default access control that allows all operations.
pub struct AllowAllAccess;
impl AccessControl for AllowAllAccess {}

/// Per-channel client identity used for access enforcement.
///
/// Carries the access control policy plus the user/host of whichever
/// downstream client opened this channel. The PVA server is expected to
/// fill in `user`/`host` from the connection's authentication context;
/// when no PVA server is wired up yet, both fields are empty strings,
/// in which case [`AccessControl`] implementations should fall back to
/// their default (typically permit-all).
#[derive(Clone)]
pub struct AccessContext {
    pub access: Arc<dyn AccessControl>,
    pub user: String,
    pub host: String,
}

impl AccessContext {
    /// Construct a context for an unauthenticated request (empty user/host).
    pub fn anonymous(access: Arc<dyn AccessControl>) -> Self {
        Self {
            access,
            user: String::new(),
            host: String::new(),
        }
    }

    /// Construct a context with explicit credentials.
    pub fn with_identity(access: Arc<dyn AccessControl>, user: String, host: String) -> Self {
        Self { access, user, host }
    }

    /// Allow-all context (used by tests and the default `BridgeProvider`).
    pub fn allow_all() -> Self {
        Self::anonymous(Arc::new(AllowAllAccess))
    }

    pub fn can_read(&self, channel: &str) -> bool {
        self.access.can_read(channel, &self.user, &self.host)
    }

    pub fn can_write(&self, channel: &str) -> bool {
        self.access.can_write(channel, &self.user, &self.host)
    }
}

impl Default for AccessContext {
    fn default() -> Self {
        Self::allow_all()
    }
}

// ---------------------------------------------------------------------------
// Trait definitions (to be moved to epics-pva-rs)
// ---------------------------------------------------------------------------

/// PVA ChannelProvider interface.
///
/// Corresponds to C++ `pva::ChannelProvider`. A PVA server calls into this
/// trait to resolve channel names and create channel instances.
pub trait ChannelProvider: Send + Sync {
    /// Provider name (e.g., "BRIDGE").
    fn provider_name(&self) -> &str;

    /// Check if a channel name exists (for UDP search responses).
    fn channel_find(&self, name: &str) -> impl std::future::Future<Output = bool> + Send;

    /// List all available channel names.
    fn channel_list(&self) -> impl std::future::Future<Output = Vec<String>> + Send;

    /// Create a channel for the given name.
    fn create_channel(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = BridgeResult<AnyChannel>> + Send;
}

/// PVA Channel interface.
///
/// Corresponds to C++ `pva::Channel`. Each instance is bound to a single
/// PV (record or group).
pub trait Channel: Send + Sync {
    /// The channel (PV) name.
    fn channel_name(&self) -> &str;

    /// Get: read current value + metadata as a PvStructure.
    fn get(
        &self,
        request: &PvStructure,
    ) -> impl std::future::Future<Output = BridgeResult<PvStructure>> + Send;

    /// Put: write a PvStructure value into the record.
    fn put(
        &self,
        value: &PvStructure,
    ) -> impl std::future::Future<Output = BridgeResult<()>> + Send;

    /// GetField: return the type description (FieldDesc) for introspection.
    fn get_field(&self) -> impl std::future::Future<Output = BridgeResult<FieldDesc>> + Send;

    /// Create a monitor for this channel.
    fn create_monitor(
        &self,
    ) -> impl std::future::Future<Output = BridgeResult<super::group::AnyMonitor>> + Send;
}

/// PVA Monitor interface.
///
/// Corresponds to C++ `pva::Monitor` / `BaseMonitor`.
pub trait PvaMonitor: Send + Sync {
    /// Wait for the next update. Returns `None` when the monitor is closed.
    fn poll(&mut self) -> impl std::future::Future<Output = Option<PvStructure>> + Send;

    /// Start the monitor (begin receiving events).
    fn start(&mut self) -> impl std::future::Future<Output = BridgeResult<()>> + Send;

    /// Stop the monitor.
    fn stop(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

// ---------------------------------------------------------------------------
// AnyChannel — enum dispatch for Channel trait
// ---------------------------------------------------------------------------

/// Concrete channel type returned by BridgeProvider.
///
/// Uses enum dispatch instead of `dyn Channel` because async trait methods
/// with `impl Future` return types are not dyn-compatible.
pub enum AnyChannel {
    Single(BridgeChannel),
    Group(GroupChannel),
}

impl Channel for AnyChannel {
    fn channel_name(&self) -> &str {
        match self {
            Self::Single(ch) => ch.channel_name(),
            Self::Group(ch) => ch.channel_name(),
        }
    }

    async fn get(&self, request: &PvStructure) -> BridgeResult<PvStructure> {
        match self {
            Self::Single(ch) => ch.get(request).await,
            Self::Group(ch) => ch.get(request).await,
        }
    }

    async fn put(&self, value: &PvStructure) -> BridgeResult<()> {
        match self {
            Self::Single(ch) => ch.put(value).await,
            Self::Group(ch) => ch.put(value).await,
        }
    }

    async fn get_field(&self) -> BridgeResult<FieldDesc> {
        match self {
            Self::Single(ch) => ch.get_field().await,
            Self::Group(ch) => ch.get_field().await,
        }
    }

    async fn create_monitor(&self) -> BridgeResult<super::group::AnyMonitor> {
        match self {
            Self::Single(ch) => ch.create_monitor().await,
            Self::Group(ch) => ch.create_monitor().await,
        }
    }
}

// ---------------------------------------------------------------------------
// BridgeProvider
// ---------------------------------------------------------------------------

/// Bridge ChannelProvider that exposes EPICS database records as PVA channels.
///
/// Corresponds to C++ `PDBProvider`. Includes channel caching for reuse
/// and pluggable access control.
pub struct BridgeProvider {
    db: Arc<PvDatabase>,
    groups: HashMap<String, GroupPvDef>,
    /// Metadata cache for single-record channels: (NtType, DbFieldType).
    /// Avoids repeated record introspection on every create_channel() call.
    /// Corresponds to C++ PDBProvider's transient_pv_map.
    record_cache: tokio::sync::RwLock<HashMap<String, (NtType, DbFieldType)>>,
    /// Access control policy. Shared with channels via `Arc` so a single
    /// policy serves all channels and can be swapped atomically.
    access: Arc<dyn AccessControl>,
}

impl BridgeProvider {
    pub fn new(db: Arc<PvDatabase>) -> Self {
        Self {
            db,
            groups: HashMap::new(),
            record_cache: tokio::sync::RwLock::new(HashMap::new()),
            access: Arc::new(AllowAllAccess),
        }
    }

    /// Set a custom access control policy.
    pub fn set_access_control(&mut self, access: Arc<dyn AccessControl>) {
        self.access = access;
    }

    /// Get a clone of the current access control policy.
    pub fn access_control(&self) -> Arc<dyn AccessControl> {
        self.access.clone()
    }

    /// Check if a client can write to a channel.
    pub fn can_write(&self, channel: &str, user: &str, host: &str) -> bool {
        self.access.can_write(channel, user, host)
    }

    /// Check if a client can read from a channel.
    pub fn can_read(&self, channel: &str, user: &str, host: &str) -> bool {
        self.access.can_read(channel, user, host)
    }

    /// Load group PV definitions from a JSON config string.
    pub fn load_group_config(&mut self, json: &str) -> BridgeResult<()> {
        let defs = super::group_config::parse_group_config(json)?;
        for def in defs {
            self.groups.insert(def.name.clone(), def);
        }
        Ok(())
    }

    /// Load group PV definitions from a JSON file.
    pub fn load_group_file(&mut self, path: &str) -> BridgeResult<()> {
        let content = std::fs::read_to_string(path)?;
        self.load_group_config(&content)
    }

    /// Load group definitions from a record's info(Q:group, ...) tag.
    pub fn load_info_group(&mut self, record_name: &str, json: &str) -> BridgeResult<()> {
        let defs = super::group_config::parse_info_group(record_name, json)?;
        super::group_config::merge_group_defs(&mut self.groups, defs);
        Ok(())
    }

    /// Access the underlying database.
    pub fn database(&self) -> &Arc<PvDatabase> {
        &self.db
    }

    /// Access group definitions.
    pub fn groups(&self) -> &HashMap<String, GroupPvDef> {
        &self.groups
    }

    /// Clear the record metadata cache.
    pub async fn clear_cache(&self) {
        self.record_cache.write().await.clear();
    }
}

impl ChannelProvider for BridgeProvider {
    fn provider_name(&self) -> &str {
        "BRIDGE"
    }

    async fn channel_find(&self, name: &str) -> bool {
        if self.groups.contains_key(name) {
            return true;
        }
        self.db.has_name(name).await
    }

    async fn channel_list(&self) -> Vec<String> {
        let mut names = self.db.all_record_names().await;
        names.extend(self.groups.keys().cloned());
        names.sort();
        names
    }

    async fn create_channel(&self, name: &str) -> BridgeResult<AnyChannel> {
        // Default: create with allow-all (anonymous) access. PVA server
        // implementations should call create_channel_for to inject the
        // real client identity.
        self.create_channel_for(name, "", "").await
    }
}

impl BridgeProvider {
    /// Create a channel with explicit client identity for access control.
    ///
    /// Used by the PVA server when it knows the connecting client's
    /// authenticated user/host. The trait method [`ChannelProvider::create_channel`]
    /// delegates to this with empty identity (anonymous mode).
    pub async fn create_channel_for(
        &self,
        name: &str,
        user: &str,
        host: &str,
    ) -> BridgeResult<AnyChannel> {
        let access_ctx =
            AccessContext::with_identity(self.access.clone(), user.to_string(), host.to_string());

        // Check group PVs first
        if let Some(def) = self.groups.get(name) {
            return Ok(AnyChannel::Group(
                GroupChannel::new(self.db.clone(), def.clone()).with_access(access_ctx),
            ));
        }

        // Single record channel — use metadata cache to avoid repeated introspection
        let (record_name, _) = epics_base_rs::server::database::parse_pv_name(name);

        // Check cache first
        {
            let cache = self.record_cache.read().await;
            if let Some(&(nt_type, value_dbf)) = cache.get(record_name) {
                return Ok(AnyChannel::Single(
                    BridgeChannel::from_cached(
                        self.db.clone(),
                        record_name.to_string(),
                        nt_type,
                        value_dbf,
                    )
                    .with_access(access_ctx),
                ));
            }
        }

        // Cache miss — introspect and create
        if self.db.has_name(name).await {
            let channel = BridgeChannel::new(self.db.clone(), name).await?;

            // Populate cache
            let mut cache = self.record_cache.write().await;
            cache.insert(
                record_name.to_string(),
                (channel.nt_type(), channel.value_dbf()),
            );

            return Ok(AnyChannel::Single(channel.with_access(access_ctx)));
        }

        Err(BridgeError::ChannelNotFound(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Access control that denies all writes, allows all reads.
    struct ReadOnly;
    impl AccessControl for ReadOnly {
        fn can_write(&self, _: &str, _: &str, _: &str) -> bool {
            false
        }
    }

    /// Access control that denies a specific channel name.
    struct DenySpecific(String);
    impl AccessControl for DenySpecific {
        fn can_read(&self, channel: &str, _: &str, _: &str) -> bool {
            channel != self.0
        }
        fn can_write(&self, channel: &str, _: &str, _: &str) -> bool {
            channel != self.0
        }
    }

    #[test]
    fn access_context_allow_all() {
        let ctx = AccessContext::allow_all();
        assert!(ctx.can_read("ANY"));
        assert!(ctx.can_write("ANY"));
    }

    #[test]
    fn access_context_read_only() {
        let ctx = AccessContext::anonymous(Arc::new(ReadOnly));
        assert!(ctx.can_read("X"));
        assert!(!ctx.can_write("X"));
    }

    #[test]
    fn access_context_with_identity() {
        let ctx =
            AccessContext::with_identity(Arc::new(AllowAllAccess), "alice".into(), "host1".into());
        assert_eq!(ctx.user, "alice");
        assert_eq!(ctx.host, "host1");
    }

    #[test]
    fn access_context_deny_specific() {
        let ctx = AccessContext::anonymous(Arc::new(DenySpecific("SECRET".to_string())));
        assert!(ctx.can_read("PUBLIC"));
        assert!(!ctx.can_read("SECRET"));
        assert!(ctx.can_write("PUBLIC"));
        assert!(!ctx.can_write("SECRET"));
    }

    #[test]
    fn provider_set_access_control() {
        let db = Arc::new(PvDatabase::new());
        let mut provider = BridgeProvider::new(db);
        // Default policy
        assert!(provider.can_read("X", "u", "h"));
        assert!(provider.can_write("X", "u", "h"));

        // Swap to read-only
        provider.set_access_control(Arc::new(ReadOnly));
        assert!(provider.can_read("X", "u", "h"));
        assert!(!provider.can_write("X", "u", "h"));
    }

    #[tokio::test]
    async fn read_only_channel_blocks_writes() {
        // Construct a channel directly with from_cached + with_access(ReadOnly).
        // We bypass create_channel here because BridgeChannel::new() requires
        // a real record in the database (which is non-trivial test setup);
        // the access enforcement path is identical regardless.
        let db = Arc::new(PvDatabase::new());
        let access = AccessContext::anonymous(Arc::new(ReadOnly));
        let ch = BridgeChannel::from_cached(
            db,
            "PROT".to_string(),
            super::super::pvif::NtType::Scalar,
            epics_base_rs::types::DbFieldType::Double,
        )
        .with_access(access);

        let mut put_struct = PvStructure::new("epics:nt/NTScalar:1.0");
        put_struct.fields.push((
            "value".into(),
            epics_pva_rs::pvdata::PvField::Scalar(epics_pva_rs::pvdata::ScalarValue::Double(2.0)),
        ));
        let result = ch.put(&put_struct).await;
        assert!(result.is_err(), "expected access denied");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("denied"),
            "expected denial message, got: {err}"
        );
    }

    #[tokio::test]
    async fn deny_specific_channel_blocks_named() {
        let db = Arc::new(PvDatabase::new());
        let access = AccessContext::anonymous(Arc::new(DenySpecific("BLOCKED".to_string())));
        let ch = BridgeChannel::from_cached(
            db.clone(),
            "BLOCKED".to_string(),
            super::super::pvif::NtType::Scalar,
            epics_base_rs::types::DbFieldType::Double,
        )
        .with_access(access);

        let req = PvStructure::new("");
        let result = ch.get(&req).await;
        assert!(result.is_err(), "expected read denied for BLOCKED");

        // A different channel name with the same policy should NOT be blocked
        let ok_access = AccessContext::anonymous(Arc::new(DenySpecific("BLOCKED".to_string())));
        let ch2 = BridgeChannel::from_cached(
            db,
            "ALLOWED".to_string(),
            super::super::pvif::NtType::Scalar,
            epics_base_rs::types::DbFieldType::Double,
        )
        .with_access(ok_access);
        // Get will fail because no record exists, but it should fail with
        // RecordNotFound, not access denied.
        let result = ch2.get(&req).await;
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            !err.contains("denied"),
            "ALLOWED channel should pass access check, got: {err}"
        );
    }

    /// Read-deny access control: blocks all reads, allows all writes.
    /// Used to verify monitor enforcement (which is read).
    struct WriteOnly;
    impl AccessControl for WriteOnly {
        fn can_read(&self, _: &str, _: &str, _: &str) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn create_monitor_blocks_when_read_denied() {
        let db = Arc::new(PvDatabase::new());
        let access = AccessContext::anonymous(Arc::new(WriteOnly));
        let ch = BridgeChannel::from_cached(
            db,
            "PROT".to_string(),
            super::super::pvif::NtType::Scalar,
            epics_base_rs::types::DbFieldType::Double,
        )
        .with_access(access);

        // create_monitor must reject before even constructing the BridgeMonitor.
        // AnyMonitor doesn't implement Debug so we destructure manually.
        let result = ch.create_monitor().await;
        match result {
            Ok(_) => panic!("expected monitor create denied, got Ok"),
            Err(e) => {
                let err = format!("{e}");
                assert!(
                    err.contains("monitor create denied"),
                    "expected monitor denial message, got: {err}"
                );
            }
        }
    }

    #[tokio::test]
    async fn bridge_monitor_start_blocks_when_read_denied() {
        // Defense-in-depth: even if a monitor is constructed via with_access
        // bypassing create_monitor, start() must still enforce.
        let db = Arc::new(PvDatabase::new());
        let access = AccessContext::anonymous(Arc::new(WriteOnly));
        let mut monitor = super::super::monitor::BridgeMonitor::new(
            db,
            "PROT".to_string(),
            super::super::pvif::NtType::Scalar,
        )
        .with_access(access);

        let result = monitor.start().await;
        assert!(result.is_err(), "expected monitor start denied");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("monitor read denied"),
            "expected start denial, got: {err}"
        );
    }
}
