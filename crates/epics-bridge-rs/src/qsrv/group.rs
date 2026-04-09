//! GroupChannel and GroupMonitor: multi-record composite PVA channel.
//!
//! Corresponds to C++ QSRV's `PDBGroupPV` / `PDBGroupChannel` / `PDBGroupMonitor`.
//! A group PV combines fields from multiple EPICS database records
//! into a single PvStructure.

use std::sync::Arc;

use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::database::db_access::DbSubscription;
use epics_base_rs::types::DbFieldType;
use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType};

use super::convert::{dbf_to_scalar_type, epics_to_pv_field};
use super::group_config::{GroupMember, GroupPvDef, TriggerDef};
use super::monitor::BridgeMonitor;
use super::pvif::{self, FieldMapping, NtType};
use crate::error::{BridgeError, BridgeResult};

// ---------------------------------------------------------------------------
// Nested field path support
// ---------------------------------------------------------------------------

/// Navigate a dot-separated field path (e.g., "a.b.c") within a PvStructure,
/// returning the leaf PvField. Corresponds to C++ QSRV `FieldName`.
pub fn get_nested_field<'a>(pv: &'a PvStructure, path: &str) -> Option<&'a PvField> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }

    let mut current_struct = pv;
    for (i, part) in parts.iter().enumerate() {
        let field = current_struct.get_field(part)?;
        if i == parts.len() - 1 {
            return Some(field);
        }
        match field {
            PvField::Structure(s) => current_struct = s,
            _ => return None, // intermediate path element is not a structure
        }
    }
    None
}

/// Set a value at a dot-separated field path within a PvStructure.
/// Creates intermediate structures as needed.
pub fn set_nested_field(pv: &mut PvStructure, path: &str, value: PvField) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // Simple case: direct field
        if let Some(pos) = pv.fields.iter().position(|(n, _)| n == parts[0]) {
            pv.fields[pos].1 = value;
        } else {
            pv.fields.push((parts[0].to_string(), value));
        }
        return;
    }

    // Navigate/create intermediate structures
    let first = parts[0];
    let rest = parts[1..].join(".");

    // Find or create the intermediate structure
    let sub = if let Some(pos) = pv.fields.iter().position(|(n, _)| n == first) {
        if let PvField::Structure(ref mut s) = pv.fields[pos].1 {
            s
        } else {
            // Replace non-structure with empty structure
            pv.fields[pos].1 = PvField::Structure(PvStructure::new(""));
            if let PvField::Structure(ref mut s) = pv.fields[pos].1 {
                s
            } else {
                unreachable!()
            }
        }
    } else {
        pv.fields
            .push((first.to_string(), PvField::Structure(PvStructure::new(""))));
        if let PvField::Structure(ref mut s) = pv.fields.last_mut().unwrap().1 {
            s
        } else {
            unreachable!()
        }
    };

    set_nested_field(sub, &rest, value);
}

/// Insert a nested FieldDesc at a dot-separated path.
///
/// Counterpart of [`set_nested_field`] for type introspection. Builds
/// intermediate `Structure` descriptors as needed so the advertised
/// schema matches the runtime payload shape.
pub fn set_nested_field_desc(fields: &mut Vec<(String, FieldDesc)>, path: &str, leaf: FieldDesc) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        if let Some(pos) = fields.iter().position(|(n, _)| n == parts[0]) {
            fields[pos].1 = leaf;
        } else {
            fields.push((parts[0].to_string(), leaf));
        }
        return;
    }

    let first = parts[0];
    let rest = parts[1..].join(".");

    // Find or create the intermediate structure descriptor
    let sub_fields: &mut Vec<(String, FieldDesc)> =
        if let Some(pos) = fields.iter().position(|(n, _)| n == first) {
            match &mut fields[pos].1 {
                FieldDesc::Structure { fields: f, .. } => f,
                other => {
                    *other = FieldDesc::Structure {
                        struct_id: String::new(),
                        fields: Vec::new(),
                    };
                    if let FieldDesc::Structure { fields: f, .. } = &mut fields[pos].1 {
                        f
                    } else {
                        unreachable!()
                    }
                }
            }
        } else {
            fields.push((
                first.to_string(),
                FieldDesc::Structure {
                    struct_id: String::new(),
                    fields: Vec::new(),
                },
            ));
            if let FieldDesc::Structure { fields: f, .. } = &mut fields.last_mut().unwrap().1 {
                f
            } else {
                unreachable!()
            }
        };

    set_nested_field_desc(sub_fields, &rest, leaf);
}

// ---------------------------------------------------------------------------
// GroupChannel
// ---------------------------------------------------------------------------

/// A PVA channel backed by a group of EPICS database records.
pub struct GroupChannel {
    db: Arc<PvDatabase>,
    def: GroupPvDef,
    access: super::provider::AccessContext,
}

impl GroupChannel {
    pub fn new(db: Arc<PvDatabase>, def: GroupPvDef) -> Self {
        Self {
            db,
            def,
            access: super::provider::AccessContext::allow_all(),
        }
    }

    /// Inject an access control context (for [`super::provider::BridgeProvider`]).
    pub fn with_access(mut self, access: super::provider::AccessContext) -> Self {
        self.access = access;
        self
    }

    /// Read all member values and compose into a single PvStructure.
    ///
    /// Internal method. Both `Channel::get()` and `GroupMonitor::poll()`
    /// (via the cached `group_channel`) call this. Performs an access
    /// read check on entry — defensive: callers also check, but if a
    /// new caller is added later this guarantees the policy still holds.
    pub(crate) async fn read_group(&self) -> BridgeResult<PvStructure> {
        if !self.access.can_read(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "read denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }

        let struct_id = self.def.struct_id.as_deref().unwrap_or("structure");
        let mut pv = PvStructure::new(struct_id);

        for member in &self.def.members {
            if member.mapping == FieldMapping::Proc {
                continue;
            }

            let field = self.read_member(member).await?;
            // Support nested field paths (e.g., "a.b.c")
            set_nested_field(&mut pv, &member.field_name, field);
        }

        Ok(pv)
    }

    /// Read only specific members by field name and compose a partial PvStructure.
    /// Same access enforcement as [`read_group`].
    async fn read_partial(&self, field_names: &[String]) -> BridgeResult<PvStructure> {
        if !self.access.can_read(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "read denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }

        let struct_id = self.def.struct_id.as_deref().unwrap_or("structure");
        let mut pv = PvStructure::new(struct_id);

        for member in &self.def.members {
            if member.mapping == FieldMapping::Proc {
                continue;
            }
            if !field_names.contains(&member.field_name) {
                continue;
            }

            let field = self.read_member(member).await?;
            set_nested_field(&mut pv, &member.field_name, field);
        }

        Ok(pv)
    }

    /// Read a single member's value from the database.
    async fn read_member(&self, member: &GroupMember) -> BridgeResult<PvField> {
        let (record_name, field_name) =
            epics_base_rs::server::database::parse_pv_name(&member.channel);

        let rec = self
            .db
            .get_record(record_name)
            .await
            .ok_or_else(|| BridgeError::RecordNotFound(record_name.to_string()))?;

        let instance = rec.read().await;

        match member.mapping {
            FieldMapping::Scalar => {
                let snapshot = instance.snapshot_for_field(field_name).ok_or_else(|| {
                    BridgeError::FieldNotFound {
                        record: record_name.to_string(),
                        field: field_name.to_string(),
                    }
                })?;
                let rtyp = instance.record.record_type();
                let nt_type = NtType::from_record_type(rtyp);
                Ok(PvField::Structure(pvif::snapshot_to_pv_structure(
                    &snapshot, nt_type,
                )))
            }
            FieldMapping::Plain => {
                let value = instance.resolve_field(field_name).ok_or_else(|| {
                    BridgeError::FieldNotFound {
                        record: record_name.to_string(),
                        field: field_name.to_string(),
                    }
                })?;
                Ok(epics_to_pv_field(&value))
            }
            FieldMapping::Meta => {
                let snapshot = instance.snapshot_for_field(field_name).ok_or_else(|| {
                    BridgeError::FieldNotFound {
                        record: record_name.to_string(),
                        field: field_name.to_string(),
                    }
                })?;
                let mut meta = PvStructure::new("meta_t");
                meta.fields.push((
                    "alarm".into(),
                    PvField::Structure(build_alarm_from_snapshot(&snapshot)),
                ));
                meta.fields.push((
                    "timeStamp".into(),
                    PvField::Structure(build_timestamp_from_snapshot(&snapshot)),
                ));
                Ok(PvField::Structure(meta))
            }
            FieldMapping::Any => {
                let value = instance.resolve_field(field_name).ok_or_else(|| {
                    BridgeError::FieldNotFound {
                        record: record_name.to_string(),
                        field: field_name.to_string(),
                    }
                })?;
                Ok(epics_to_pv_field(&value))
            }
            FieldMapping::Proc => Ok(PvField::Scalar(epics_pva_rs::pvdata::ScalarValue::Int(0))),
        }
    }

    /// Introspect a member's actual DBF type and record type from the database.
    async fn introspect_member(&self, member: &GroupMember) -> BridgeResult<(NtType, ScalarType)> {
        let (record_name, field_name) =
            epics_base_rs::server::database::parse_pv_name(&member.channel);

        let rec = self
            .db
            .get_record(record_name)
            .await
            .ok_or_else(|| BridgeError::RecordNotFound(record_name.to_string()))?;

        let instance = rec.read().await;
        let rtyp = instance.record.record_type();
        let nt_type = NtType::from_record_type(rtyp);

        let field_upper = field_name.to_ascii_uppercase();
        let value_dbf = instance
            .record
            .field_list()
            .iter()
            .find(|f| f.name == field_upper)
            .map(|f| f.dbf_type)
            .unwrap_or(DbFieldType::Double);

        Ok((nt_type, dbf_to_scalar_type(value_dbf)))
    }

    /// Look up a member's actual DBF field type from the database.
    /// Returns `Double` as a fallback if the record/field can't be found.
    async fn member_dbf_type(&self, member: &GroupMember) -> DbFieldType {
        let (record_name, field_name) =
            epics_base_rs::server::database::parse_pv_name(&member.channel);

        let rec = match self.db.get_record(record_name).await {
            Some(r) => r,
            None => return DbFieldType::Double,
        };
        let instance = rec.read().await;
        let field_upper = field_name.to_ascii_uppercase();
        instance
            .record
            .field_list()
            .iter()
            .find(|f| f.name == field_upper)
            .map(|f| f.dbf_type)
            .unwrap_or(DbFieldType::Double)
    }

    /// Convert an incoming PvField to an EpicsValue typed against the
    /// member's actual DBF field. This avoids context-free fallback
    /// conversions (e.g. ScalarValue::Long → EpicsValue::Double).
    ///
    /// For arrays and structures, falls back to `pv_field_to_epics`.
    async fn convert_member_value(
        &self,
        member: &GroupMember,
        pv_field: &epics_pva_rs::pvdata::PvField,
    ) -> Option<epics_base_rs::types::EpicsValue> {
        use epics_pva_rs::pvdata::PvField;
        match pv_field {
            PvField::Scalar(sv) => {
                let target = self.member_dbf_type(member).await;
                Some(super::convert::scalar_to_epics_typed(sv, target))
            }
            // Arrays and structures: defer to the fallback array converter.
            // C++ QSRV uses dbChannelFinalNoElements + DBR types for arrays;
            // for now we delegate to pv_field_to_epics which preserves
            // element types.
            _ => super::convert::pv_field_to_epics(pv_field),
        }
    }
}

impl super::provider::Channel for GroupChannel {
    fn channel_name(&self) -> &str {
        &self.def.name
    }

    async fn get(&self, request: &PvStructure) -> BridgeResult<PvStructure> {
        if !self.access.can_read(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "read denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }
        let full = self.read_group().await?;
        Ok(pvif::filter_by_request(&full, request))
    }

    async fn put(&self, value: &PvStructure) -> BridgeResult<()> {
        if !self.access.can_write(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "write denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }

        let opts = super::channel::PutOptions::from_pv_request(value);
        let use_process = opts.process != super::channel::ProcessMode::Inhibit;

        let mut ordered: Vec<&GroupMember> = self.def.members.iter().collect();
        ordered.sort_by_key(|m| m.put_order);

        if self.def.atomic {
            // Atomic put: convert all values up-front (DBF-typed), then
            // perform the actual writes in order. In C++ QSRV this uses
            // DBManyLocker to hold all record locks simultaneously.
            // Since epics-base-rs doesn't expose multi-lock, we write
            // sequentially without yielding between writes.
            let mut writes: Vec<(&GroupMember, Option<epics_base_rs::types::EpicsValue>)> =
                Vec::new();

            for member in &ordered {
                if member.mapping == FieldMapping::Proc {
                    // Proc has no value — write entry stays None,
                    // process_record() runs in the apply phase
                    writes.push((member, None));
                    continue;
                }

                // Use nested lookup so members with dotted field paths
                // (e.g., "axis.position") resolve correctly. The read
                // path uses set_nested_field — put must use the same
                // path semantics.
                let epics_val = match get_nested_field(value, &member.field_name) {
                    Some(pv_field) => self.convert_member_value(member, pv_field).await,
                    None => None,
                };
                writes.push((member, epics_val));
            }

            for (member, val) in writes {
                let (record_name, field_name) =
                    epics_base_rs::server::database::parse_pv_name(&member.channel);

                if member.mapping == FieldMapping::Proc {
                    self.db
                        .process_record(record_name)
                        .await
                        .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                } else if let Some(epics_val) = val {
                    if use_process {
                        self.db
                            .put_record_field_from_ca(record_name, field_name, epics_val)
                            .await
                            .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                    } else {
                        self.db
                            .put_pv(&format!("{record_name}.{field_name}"), epics_val)
                            .await
                            .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                    }
                }
            }
        } else {
            // Non-atomic put: write each member individually.
            // IMPORTANT: Proc members are checked BEFORE the request-field
            // lookup because they have no value to read — process_record()
            // must run regardless of whether the request contains that field
            // (matches C++ pdbgroup.cpp:300+ allowProc semantics).
            for member in ordered {
                let (record_name, field_name) =
                    epics_base_rs::server::database::parse_pv_name(&member.channel);

                if member.mapping == FieldMapping::Proc {
                    self.db
                        .process_record(record_name)
                        .await
                        .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                    continue;
                }

                // Nested-aware lookup (matches read-side set_nested_field)
                let pv_field = match get_nested_field(value, &member.field_name) {
                    Some(f) => f,
                    None => continue,
                };

                let epics_val = match self.convert_member_value(member, pv_field).await {
                    Some(v) => v,
                    None => continue,
                };

                if use_process {
                    self.db
                        .put_record_field_from_ca(record_name, field_name, epics_val)
                        .await
                        .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                } else {
                    self.db
                        .put_pv(&format!("{record_name}.{field_name}"), epics_val)
                        .await
                        .map_err(|e| BridgeError::PutRejected(e.to_string()))?;
                }
            }
        }

        Ok(())
    }

    async fn get_field(&self) -> BridgeResult<FieldDesc> {
        let struct_id = self.def.struct_id.as_deref().unwrap_or("structure");
        let mut fields: Vec<(String, FieldDesc)> = Vec::new();

        for member in &self.def.members {
            if member.mapping == FieldMapping::Proc {
                continue;
            }

            let (nt_type, scalar_type) = self.introspect_member(member).await?;

            // Build the leaf descriptor and apply member-level +id if set.
            let mut desc = match member.mapping {
                FieldMapping::Scalar => pvif::build_field_desc_for_nt(nt_type, scalar_type),
                FieldMapping::Plain => FieldDesc::Scalar(scalar_type),
                FieldMapping::Meta => meta_desc(),
                FieldMapping::Any => FieldDesc::Scalar(scalar_type),
                FieldMapping::Proc => continue,
            };
            if let Some(member_id) = &member.struct_id
                && let FieldDesc::Structure { struct_id, .. } = &mut desc
            {
                *struct_id = member_id.clone();
            }

            // Place the descriptor at its (possibly nested) path.
            // The read side uses set_nested_field — introspection must
            // emit the same shape so clients see consistent type info.
            set_nested_field_desc(&mut fields, &member.field_name, desc);
        }

        Ok(FieldDesc::Structure {
            struct_id: struct_id.into(),
            fields,
        })
    }

    async fn create_monitor(&self) -> BridgeResult<AnyMonitor> {
        // Read enforcement: deny monitor creation when the client lacks
        // read access. start() also re-checks defensively.
        if !self.access.can_read(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "monitor create denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }
        Ok(AnyMonitor::Group(
            GroupMonitor::new(self.db.clone(), self.def.clone()).with_access(self.access.clone()),
        ))
    }
}

// ---------------------------------------------------------------------------
// GroupMonitor
// ---------------------------------------------------------------------------

/// Event from a group member subscription, sent through the fan-in channel.
struct MemberEvent {
    member_index: usize,
}

/// A PVA monitor for a group PV that subscribes to all member records.
///
/// Corresponds to C++ QSRV's `PDBGroupMonitor` + `pdb_group_event()`.
/// Uses a fan-in channel pattern: each member subscription spawns a task
/// that forwards events to a single receiver, enabling concurrent wait
/// across all members.
pub struct GroupMonitor {
    db: Arc<PvDatabase>,
    def: GroupPvDef,
    running: bool,
    /// Reusable GroupChannel for read_group/read_partial calls.
    /// Created once in start() instead of per-event in poll().
    /// The internal GroupChannel inherits the same `access` context so
    /// any read enforcement applied at create_monitor time stays in effect.
    group_channel: Option<GroupChannel>,
    /// Initial complete group snapshot (sent on first poll)
    initial_snapshot: Option<PvStructure>,
    /// Fan-in receiver for member events
    event_rx: Option<tokio::sync::mpsc::Receiver<MemberEvent>>,
    /// Handles for spawned per-member tasks
    _tasks: Vec<tokio::task::JoinHandle<()>>,
    /// Access control context propagated from the parent GroupChannel.
    access: super::provider::AccessContext,
}

impl GroupMonitor {
    pub fn new(db: Arc<PvDatabase>, def: GroupPvDef) -> Self {
        Self {
            db,
            def,
            running: false,
            group_channel: None,
            initial_snapshot: None,
            event_rx: None,
            _tasks: Vec::new(),
            access: super::provider::AccessContext::allow_all(),
        }
    }

    /// Inject an access control context. Called by `GroupChannel::create_monitor`.
    pub fn with_access(mut self, access: super::provider::AccessContext) -> Self {
        self.access = access;
        self
    }
}

impl super::provider::PvaMonitor for GroupMonitor {
    async fn start(&mut self) -> BridgeResult<()> {
        if self.running {
            return Ok(());
        }

        // Read enforcement: refuse to spin up upstream subscriptions
        // for a client that lacks read permission on this group.
        if !self.access.can_read(&self.def.name) {
            return Err(BridgeError::PutRejected(format!(
                "monitor read denied for group {} (user='{}' host='{}')",
                self.def.name, self.access.user, self.access.host
            )));
        }

        // Create fan-in channel for member events
        let (tx, rx) = tokio::sync::mpsc::channel::<MemberEvent>(64);

        // Subscribe to all members that have triggers and spawn forwarding tasks
        for (idx, member) in self.def.members.iter().enumerate() {
            if matches!(member.triggers, TriggerDef::None) {
                continue;
            }

            let (record_name, _) = epics_base_rs::server::database::parse_pv_name(&member.channel);

            if let Some(mut sub) = DbSubscription::subscribe(&self.db, record_name).await {
                let tx = tx.clone();
                let handle = tokio::spawn(async move {
                    // Forward subscription events to the fan-in channel
                    while sub.recv_snapshot().await.is_some() {
                        if tx.send(MemberEvent { member_index: idx }).await.is_err() {
                            break; // receiver dropped
                        }
                    }
                });
                self._tasks.push(handle);
            }
        }

        // Create a reusable GroupChannel once (instead of per-event in poll).
        // Propagate the same access context so any subsequent reads triggered
        // by trigger evaluation also honor read enforcement.
        let group_channel =
            GroupChannel::new(self.db.clone(), self.def.clone()).with_access(self.access.clone());

        // Read initial complete group snapshot (like C++ BaseMonitor::connect)
        if let Ok(snapshot) = group_channel.read_group().await {
            self.initial_snapshot = Some(snapshot);
        }

        self.group_channel = Some(group_channel);
        self.event_rx = Some(rx);
        self.running = true;
        Ok(())
    }

    async fn poll(&mut self) -> Option<PvStructure> {
        // Return initial snapshot first (C++ BaseMonitor::connect behavior)
        if let Some(initial) = self.initial_snapshot.take() {
            return Some(initial);
        }

        let rx = self.event_rx.as_mut()?;

        loop {
            let event = rx.recv().await?;

            let member = match self.def.members.get(event.member_index) {
                Some(m) => m,
                None => continue,
            };

            let group_channel = self.group_channel.as_ref()?;

            match &member.triggers {
                TriggerDef::None => continue,
                TriggerDef::All => {
                    // Re-read entire group
                    return group_channel.read_group().await.ok();
                }
                TriggerDef::Fields(field_names) => {
                    // Partial update: only re-read triggered fields
                    return group_channel.read_partial(field_names).await.ok();
                }
            }
        }
    }

    async fn stop(&mut self) {
        // Drop the receiver first to signal tasks to stop
        self.event_rx = None;

        // Abort spawned tasks
        for handle in self._tasks.drain(..) {
            handle.abort();
        }

        self.running = false;
        self.group_channel = None;
        self.initial_snapshot = None;
    }
}

// ---------------------------------------------------------------------------
// AnyMonitor
// ---------------------------------------------------------------------------

/// Enum dispatch for monitor types (single record vs group).
pub enum AnyMonitor {
    Single(BridgeMonitor),
    Group(GroupMonitor),
}

impl super::provider::PvaMonitor for AnyMonitor {
    async fn poll(&mut self) -> Option<PvStructure> {
        match self {
            Self::Single(m) => m.poll().await,
            Self::Group(m) => m.poll().await,
        }
    }

    async fn start(&mut self) -> BridgeResult<()> {
        match self {
            Self::Single(m) => m.start().await,
            Self::Group(m) => m.start().await,
        }
    }

    async fn stop(&mut self) {
        match self {
            Self::Single(m) => m.stop().await,
            Self::Group(m) => m.stop().await,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn meta_desc() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "meta_t".into(),
        fields: vec![
            (
                "alarm".into(),
                FieldDesc::Structure {
                    struct_id: "alarm_t".into(),
                    fields: vec![
                        ("severity".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("status".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("message".into(), FieldDesc::Scalar(ScalarType::String)),
                    ],
                },
            ),
            (
                "timeStamp".into(),
                FieldDesc::Structure {
                    struct_id: "time_t".into(),
                    fields: vec![
                        (
                            "secondsPastEpoch".into(),
                            FieldDesc::Scalar(ScalarType::Long),
                        ),
                        ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
                    ],
                },
            ),
        ],
    }
}

fn build_alarm_from_snapshot(snapshot: &epics_base_rs::server::snapshot::Snapshot) -> PvStructure {
    use epics_pva_rs::pvdata::ScalarValue;
    let mut alarm = PvStructure::new("alarm_t");
    alarm.fields.push((
        "severity".into(),
        PvField::Scalar(ScalarValue::Int(snapshot.alarm.severity as i32)),
    ));
    alarm.fields.push((
        "status".into(),
        PvField::Scalar(ScalarValue::Int(snapshot.alarm.status as i32)),
    ));
    alarm.fields.push((
        "message".into(),
        PvField::Scalar(ScalarValue::String(String::new())),
    ));
    alarm
}

fn build_timestamp_from_snapshot(
    snapshot: &epics_base_rs::server::snapshot::Snapshot,
) -> PvStructure {
    use epics_pva_rs::pvdata::ScalarValue;
    use std::time::UNIX_EPOCH;

    let mut ts = PvStructure::new("time_t");
    let (secs, nanos) = match snapshot.timestamp.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos() as i32),
        Err(_) => (0, 0),
    };
    ts.fields.push((
        "secondsPastEpoch".into(),
        PvField::Scalar(ScalarValue::Long(secs)),
    ));
    ts.fields.push((
        "nanoseconds".into(),
        PvField::Scalar(ScalarValue::Int(nanos)),
    ));
    ts.fields
        .push(("userTag".into(), PvField::Scalar(ScalarValue::Int(0))));
    ts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_field_set_simple() {
        let mut pv = PvStructure::new("test");
        set_nested_field(
            &mut pv,
            "x",
            PvField::Scalar(epics_pva_rs::pvdata::ScalarValue::Int(42)),
        );
        assert!(pv.get_field("x").is_some());
    }

    #[test]
    fn nested_field_set_deep() {
        let mut pv = PvStructure::new("test");
        set_nested_field(
            &mut pv,
            "a.b.c",
            PvField::Scalar(epics_pva_rs::pvdata::ScalarValue::Double(2.5)),
        );
        let a = pv.get_field("a");
        assert!(a.is_some());
        if let Some(PvField::Structure(a_struct)) = a {
            if let Some(PvField::Structure(b_struct)) = a_struct.get_field("b") {
                assert!(b_struct.get_field("c").is_some());
            } else {
                panic!("expected b structure");
            }
        } else {
            panic!("expected a structure");
        }
    }

    #[test]
    fn nested_field_roundtrip() {
        use epics_pva_rs::pvdata::ScalarValue;

        let mut pv = PvStructure::new("test");
        set_nested_field(&mut pv, "a.b", PvField::Scalar(ScalarValue::Int(99)));

        // Verify get_nested_field returns the same value
        let field = get_nested_field(&pv, "a.b");
        assert!(field.is_some());
        if let Some(PvField::Scalar(ScalarValue::Int(v))) = field {
            assert_eq!(*v, 99);
        } else {
            panic!("expected Int(99)");
        }
    }

    #[test]
    fn nested_field_overwrite() {
        use epics_pva_rs::pvdata::ScalarValue;

        let mut pv = PvStructure::new("test");
        set_nested_field(&mut pv, "x.y", PvField::Scalar(ScalarValue::Int(1)));
        set_nested_field(&mut pv, "x.y", PvField::Scalar(ScalarValue::Int(2)));

        if let Some(PvField::Scalar(ScalarValue::Int(v))) = get_nested_field(&pv, "x.y") {
            assert_eq!(*v, 2);
        } else {
            panic!("expected Int(2)");
        }
    }

    #[test]
    fn nested_field_siblings() {
        use epics_pva_rs::pvdata::ScalarValue;

        let mut pv = PvStructure::new("test");
        set_nested_field(&mut pv, "a.x", PvField::Scalar(ScalarValue::Int(1)));
        set_nested_field(&mut pv, "a.y", PvField::Scalar(ScalarValue::Int(2)));

        assert!(get_nested_field(&pv, "a.x").is_some());
        assert!(get_nested_field(&pv, "a.y").is_some());
    }

    // -----------------------------------------------------------------
    // FieldDesc nested schema tests (for get_field introspection)
    // -----------------------------------------------------------------

    #[test]
    fn nested_desc_simple() {
        use epics_pva_rs::pvdata::ScalarType;

        let mut fields: Vec<(String, FieldDesc)> = Vec::new();
        set_nested_field_desc(&mut fields, "x", FieldDesc::Scalar(ScalarType::Double));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "x");
        assert!(matches!(fields[0].1, FieldDesc::Scalar(ScalarType::Double)));
    }

    #[test]
    fn nested_desc_deep() {
        use epics_pva_rs::pvdata::ScalarType;

        let mut fields: Vec<(String, FieldDesc)> = Vec::new();
        set_nested_field_desc(
            &mut fields,
            "axis.position",
            FieldDesc::Scalar(ScalarType::Double),
        );
        set_nested_field_desc(
            &mut fields,
            "axis.velocity",
            FieldDesc::Scalar(ScalarType::Double),
        );

        // Should produce: [axis: structure { position: Double, velocity: Double }]
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "axis");
        if let FieldDesc::Structure { fields: sub, .. } = &fields[0].1 {
            assert_eq!(sub.len(), 2);
            assert_eq!(sub[0].0, "position");
            assert_eq!(sub[1].0, "velocity");
        } else {
            panic!("expected nested structure");
        }
    }

    #[test]
    fn nested_desc_overwrite() {
        use epics_pva_rs::pvdata::ScalarType;

        let mut fields: Vec<(String, FieldDesc)> = Vec::new();
        set_nested_field_desc(&mut fields, "x", FieldDesc::Scalar(ScalarType::Int));
        set_nested_field_desc(&mut fields, "x", FieldDesc::Scalar(ScalarType::Double));
        assert_eq!(fields.len(), 1);
        assert!(matches!(fields[0].1, FieldDesc::Scalar(ScalarType::Double)));
    }

    #[test]
    fn nested_desc_mixed_depth() {
        use epics_pva_rs::pvdata::ScalarType;

        let mut fields: Vec<(String, FieldDesc)> = Vec::new();
        set_nested_field_desc(&mut fields, "name", FieldDesc::Scalar(ScalarType::String));
        set_nested_field_desc(
            &mut fields,
            "axis.position",
            FieldDesc::Scalar(ScalarType::Double),
        );

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "name");
        assert_eq!(fields[1].0, "axis");
    }
}
