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
use crate::error::{BridgeError, BridgeResult};
use super::group::GroupChannel;
use super::group_config::GroupPvDef;
use super::pvif::NtType;

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
    fn channel_find(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = bool> + Send;

    /// List all available channel names.
    fn channel_list(
        &self,
    ) -> impl std::future::Future<Output = Vec<String>> + Send;

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
    fn get_field(
        &self,
    ) -> impl std::future::Future<Output = BridgeResult<FieldDesc>> + Send;

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
    fn poll(
        &mut self,
    ) -> impl std::future::Future<Output = Option<PvStructure>> + Send;

    /// Start the monitor (begin receiving events).
    fn start(
        &mut self,
    ) -> impl std::future::Future<Output = BridgeResult<()>> + Send;

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
    /// Access control policy.
    access: Box<dyn AccessControl>,
}

impl BridgeProvider {
    pub fn new(db: Arc<PvDatabase>) -> Self {
        Self {
            db,
            groups: HashMap::new(),
            record_cache: tokio::sync::RwLock::new(HashMap::new()),
            access: Box::new(AllowAllAccess),
        }
    }

    /// Set a custom access control policy.
    pub fn set_access_control(&mut self, access: Box<dyn AccessControl>) {
        self.access = access;
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
        // Check group PVs first
        if let Some(def) = self.groups.get(name) {
            return Ok(AnyChannel::Group(GroupChannel::new(
                self.db.clone(),
                def.clone(),
            )));
        }

        // Single record channel — use metadata cache to avoid repeated introspection
        let (record_name, _) = epics_base_rs::server::database::parse_pv_name(name);

        // Check cache first
        {
            let cache = self.record_cache.read().await;
            if let Some(&(nt_type, value_dbf)) = cache.get(record_name) {
                return Ok(AnyChannel::Single(BridgeChannel::from_cached(
                    self.db.clone(),
                    record_name.to_string(),
                    nt_type,
                    value_dbf,
                )));
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

            return Ok(AnyChannel::Single(channel));
        }

        Err(BridgeError::ChannelNotFound(name.to_string()))
    }
}
