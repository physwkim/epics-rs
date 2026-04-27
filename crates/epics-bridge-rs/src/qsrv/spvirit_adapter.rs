//! Adapter that exposes a [`BridgeProvider`] (qsrv) through the
//! [`spvirit_server::PvStore`] trait, so that the spvirit PVA server can
//! serve EPICS records (single-record and group composite PVs) over PVA.
//!
//! Bridging is done in two directions:
//!   - **Read path**: qsrv produces `epics_pva_rs::pvdata::PvStructure`; the
//!     adapter converts it to `spvirit_types::NtPayload::Generic` carrying a
//!     recursive `PvValue` tree, preserving `struct_id` and field names so
//!     that NTScalar / NTEnum / NTScalarArray responses stay wire-compatible.
//!   - **Write path**: the protocol handler decodes incoming PUT bytes into
//!     `spvirit_codec::spvd_decode::DecodedValue`; the adapter rewraps it as
//!     a `PvStructure` and dispatches to the qsrv channel's `put`.

use std::collections::HashMap;
use std::sync::Arc;

// spvirit-server is no longer a dependency — the native ChannelSource impl
// below replaces the legacy PvStore impl. spvirit-codec / spvirit-types
// imports are still used by NTNDArray plugin PV handling (registered via
// NDPvaConfigure → `register_pva_pv`); those are scheduled for Phase 5.
#[allow(unused_imports)]
use spvirit_codec::spvd_decode::{
    DecodedValue, FieldDesc as SpvdFieldDesc, FieldType, StructureDesc, TypeCode,
};
#[allow(unused_imports)]
use spvirit_types::{NtPayload, PvValue, ScalarArrayValue, ScalarValue};
use tokio::sync::{RwLock, mpsc};

use epics_pva_rs::pvdata::{PvField, PvStructure, ScalarType, ScalarValue as PvaScalarValue};

use super::group::AnyMonitor;
use super::provider::{AnyChannel, BridgeProvider, Channel, ChannelProvider, PvaMonitor};

/// Handle for a PVA plugin PV: latest snapshot + subscriber list.
///
/// Registered via [`QsrvPvStore::register_pva_pv`] so that the spvirit
/// PVA server can serve NTNDArray (or any NtPayload) produced by
/// areaDetector PVA plugins.
#[derive(Clone)]
pub struct PvaPvHandle {
    pub latest: Arc<parking_lot::Mutex<Option<NtPayload>>>,
    pub subscribers: Arc<parking_lot::Mutex<Vec<mpsc::Sender<NtPayload>>>>,
}

// ---------------------------------------------------------------------------
// Global PVA PV registry — NDPvaConfigure stores handles here during st.cmd,
// the CA+PVA runner reads them at server startup.
// ---------------------------------------------------------------------------

static PVA_PV_REGISTRY: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, PvaPvHandle>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Register a PVA plugin PV. Called from `NDPvaConfigure` during st.cmd.
pub fn register_pva_pv_global(pv_name: &str, handle: PvaPvHandle) {
    PVA_PV_REGISTRY
        .lock()
        .unwrap()
        .insert(pv_name.to_string(), handle);
}

/// Take all registered PVA plugin PVs. Called by [`run_ca_pva_qsrv_ioc`]
/// to wire them into `QsrvPvStore`.
pub fn take_registered_pva_pvs() -> std::collections::HashMap<String, PvaPvHandle> {
    std::mem::take(&mut *PVA_PV_REGISTRY.lock().unwrap())
}

/// PvStore implementation backed by a qsrv [`BridgeProvider`].
///
/// Handles single-record PVs, group composite PVs, and PVA plugin PVs
/// (NTNDArray from areaDetector). Group PVs ride on the
/// `NtPayload::Generic` variant with a recursive `PvValue` tree.
pub struct QsrvPvStore {
    provider: Arc<BridgeProvider>,
    /// Per-PV cache of opened channels.
    channels: RwLock<HashMap<String, Arc<AnyChannel>>>,
    /// PVA plugin PVs (e.g., NTNDArray from NDPluginPva).
    pva_pvs: Arc<RwLock<HashMap<String, PvaPvHandle>>>,
}

impl QsrvPvStore {
    pub fn new(provider: Arc<BridgeProvider>) -> Self {
        Self {
            provider,
            channels: RwLock::new(HashMap::new()),
            pva_pvs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn provider(&self) -> &Arc<BridgeProvider> {
        &self.provider
    }

    /// Register a PVA plugin PV (e.g., NTNDArray from NDPluginPva).
    ///
    /// After registration, the PV is discoverable via `has_pv`, readable
    /// via `get_snapshot`, and subscribable via `subscribe`.
    pub async fn register_pva_pv(
        &self,
        pv_name: &str,
        latest: Arc<parking_lot::Mutex<Option<NtPayload>>>,
        subscribers: Arc<parking_lot::Mutex<Vec<mpsc::Sender<NtPayload>>>>,
    ) {
        self.pva_pvs.write().await.insert(
            pv_name.to_string(),
            PvaPvHandle {
                latest,
                subscribers,
            },
        );
    }

    async fn channel(&self, name: &str) -> Option<Arc<AnyChannel>> {
        if let Some(c) = self.channels.read().await.get(name) {
            return Some(c.clone());
        }
        let fresh = self.provider.create_channel(name).await.ok()?;
        let arc = Arc::new(fresh);
        self.channels
            .write()
            .await
            .insert(name.to_string(), arc.clone());
        Some(arc)
    }
}


// ── ChannelSource impl (native PvAccess server) ──────────────────────────
//
// In addition to the legacy spvirit `PvStore` impl above, expose the same
// data via the native [`epics_pva_rs::server_native::ChannelSource`] trait.
// This is the path used by `epics_pva_rs::server::PvaServer::run_with_source`
// (no spvirit_server runtime involvement).

impl epics_pva_rs::server_native::ChannelSource for QsrvPvStore {
    fn list_pvs(&self) -> impl std::future::Future<Output = Vec<String>> + Send {
        let provider = self.provider.clone();
        let pva_pvs = self.pva_pvs.clone();
        async move {
            let mut names = provider.channel_list().await;
            for key in pva_pvs.read().await.keys() {
                if !names.contains(key) {
                    names.push(key.clone());
                }
            }
            names.sort();
            names
        }
    }

    fn has_pv(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
        let provider = self.provider.clone();
        let pva_pvs = self.pva_pvs.clone();
        let name = name.to_string();
        async move {
            if pva_pvs.read().await.contains_key(&name) {
                return true;
            }
            provider.channel_find(&name).await
        }
    }

    fn get_introspection(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<epics_pva_rs::pvdata::FieldDesc>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            channel.get_field().await.ok()
        }
    }

    fn get_value(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<PvField>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            let empty_request = PvStructure::new("");
            match channel.get(&empty_request).await {
                Ok(pv) => Some(PvField::Structure(pv)),
                Err(e) => {
                    tracing::debug!("qsrv get_value({name_owned}) failed: {e}");
                    None
                }
            }
        }
    }

    fn put_value(
        &self,
        name: &str,
        value: PvField,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self
                .channel(&name_owned)
                .await
                .ok_or_else(|| format!("PV not found: {name_owned}"))?;
            let pv = match value {
                PvField::Structure(s) => s,
                other => {
                    return Err(format!(
                        "qsrv PUT expects a structure value, got {other}"
                    ))
                }
            };
            channel.put(&pv).await.map_err(|e| e.to_string())
        }
    }

    fn is_writable(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
        let provider = self.provider.clone();
        let name = name.to_string();
        async move { provider.channel_find(&name).await }
    }

    fn subscribe(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            let mut monitor = channel.create_monitor().await.ok()?;
            monitor.start().await.ok()?;
            let (tx, rx) = mpsc::channel::<PvField>(64);
            tokio::spawn(async move {
                while let Some(snapshot) = monitor.poll().await {
                    if tx.send(PvField::Structure(snapshot)).await.is_err() {
                        break;
                    }
                }
                monitor.stop().await;
            });
            Some(rx)
        }
    }
}

async fn monitor_bridge_loop(mut monitor: AnyMonitor, tx: mpsc::Sender<NtPayload>, pv: String) {
    while let Some(snapshot) = monitor.poll().await {
        let payload = pv_structure_to_nt_payload(&snapshot);
        if tx.send(payload).await.is_err() {
            break;
        }
    }
    let _ = pv; // suppressed-warning sink if tracing is disabled
    monitor.stop().await;
}

// ── PvStructure → NtPayload / PvValue / StructureDesc ────────────────────

fn pv_structure_to_nt_payload(pv: &PvStructure) -> NtPayload {
    let (struct_id, fields) = pv_structure_to_generic_parts(pv);
    NtPayload::Generic { struct_id, fields }
}

fn pv_structure_to_generic_parts(pv: &PvStructure) -> (String, Vec<(String, PvValue)>) {
    let fields = pv
        .fields
        .iter()
        .map(|(name, field)| (name.clone(), pv_field_to_pv_value(field)))
        .collect();
    (pv.struct_id.clone(), fields)
}

fn pv_field_to_pv_value(field: &PvField) -> PvValue {
    match field {
        PvField::Scalar(sv) => PvValue::Scalar(pva_scalar_to_spvirit(sv)),
        PvField::ScalarArray(items) => PvValue::ScalarArray(pva_array_to_spvirit(items)),
        PvField::Structure(nested) => {
            let (struct_id, fields) = pv_structure_to_generic_parts(nested);
            PvValue::Structure { struct_id, fields }
        }
        // The native qsrv PV shapes don't currently emit composite/union/
        // variant variants. Fall back to an empty structure so the spvirit
        // adapter doesn't crash on a hypothetical case.
        PvField::StructureArray(_)
        | PvField::Union { .. }
        | PvField::UnionArray(_)
        | PvField::Variant(_)
        | PvField::VariantArray(_)
        | PvField::Null => PvValue::Structure {
            struct_id: String::new(),
            fields: Vec::new(),
        },
    }
}

fn pva_scalar_to_spvirit(v: &PvaScalarValue) -> ScalarValue {
    match v {
        PvaScalarValue::Boolean(b) => ScalarValue::Bool(*b),
        PvaScalarValue::Byte(i) => ScalarValue::I8(*i),
        PvaScalarValue::Short(i) => ScalarValue::I16(*i),
        PvaScalarValue::Int(i) => ScalarValue::I32(*i),
        PvaScalarValue::Long(i) => ScalarValue::I64(*i),
        PvaScalarValue::UByte(i) => ScalarValue::U8(*i),
        PvaScalarValue::UShort(i) => ScalarValue::U16(*i),
        PvaScalarValue::UInt(i) => ScalarValue::U32(*i),
        PvaScalarValue::ULong(i) => ScalarValue::U64(*i),
        PvaScalarValue::Float(f) => ScalarValue::F32(*f),
        PvaScalarValue::Double(f) => ScalarValue::F64(*f),
        PvaScalarValue::String(s) => ScalarValue::Str(s.clone()),
    }
}

/// Turn a `Vec<ScalarValue>` (qsrv's scalar-array representation) into the
/// typed `ScalarArrayValue` expected by spvirit-types.
///
/// qsrv never mixes types within a single array (the value is populated from
/// a single `EpicsValue::*Array` variant), so we pick the type from the first
/// element. An empty array defaults to `F64` — arbitrary, but the descriptor
/// we emit will match the value bytes regardless.
fn pva_array_to_spvirit(items: &[PvaScalarValue]) -> ScalarArrayValue {
    match items.first() {
        Some(PvaScalarValue::Boolean(_)) => ScalarArrayValue::Bool(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Boolean(b) => *b,
                    _ => false,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Byte(_)) => ScalarArrayValue::I8(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Byte(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Short(_)) => ScalarArrayValue::I16(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Short(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Int(_)) => ScalarArrayValue::I32(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Int(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Long(_)) => ScalarArrayValue::I64(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Long(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::UByte(_)) => ScalarArrayValue::U8(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::UByte(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::UShort(_)) => ScalarArrayValue::U16(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::UShort(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::UInt(_)) => ScalarArrayValue::U32(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::UInt(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::ULong(_)) => ScalarArrayValue::U64(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::ULong(i) => *i,
                    _ => 0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Float(_)) => ScalarArrayValue::F32(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Float(f) => *f,
                    _ => 0.0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::Double(_)) => ScalarArrayValue::F64(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::Double(f) => *f,
                    _ => 0.0,
                })
                .collect(),
        ),
        Some(PvaScalarValue::String(_)) => ScalarArrayValue::Str(
            items
                .iter()
                .map(|v| match v {
                    PvaScalarValue::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
        ),
        None => ScalarArrayValue::F64(Vec::new()),
    }
}

/// Convert an epics_pva_rs `FieldDesc` (from `Channel::get_field()`) to
/// spvirit's `StructureDesc`. This produces accurate type descriptors for
/// group PVs where the structure is composite.
fn epics_field_desc_to_structure_desc(desc: &epics_pva_rs::pvdata::FieldDesc) -> StructureDesc {
    match desc {
        epics_pva_rs::pvdata::FieldDesc::Structure { struct_id, fields } => {
            let sid = if struct_id.is_empty() {
                None
            } else {
                Some(struct_id.clone())
            };
            StructureDesc {
                struct_id: sid,
                fields: fields
                    .iter()
                    .map(|(name, fd)| SpvdFieldDesc {
                        name: name.clone(),
                        field_type: epics_field_desc_to_field_type(fd),
                    })
                    .collect(),
            }
        }
        // Top-level descriptor should always be a Structure; fall back to
        // an empty structure for scalar/array (shouldn't happen in practice).
        _ => StructureDesc {
            struct_id: None,
            fields: Vec::new(),
        },
    }
}

fn epics_field_desc_to_field_type(desc: &epics_pva_rs::pvdata::FieldDesc) -> FieldType {
    match desc {
        epics_pva_rs::pvdata::FieldDesc::Scalar(st) => {
            FieldType::Scalar(scalar_type_to_typecode(*st))
        }
        epics_pva_rs::pvdata::FieldDesc::ScalarArray(st) => {
            FieldType::ScalarArray(scalar_type_to_typecode(*st))
        }
        epics_pva_rs::pvdata::FieldDesc::Structure { struct_id, fields } => {
            let sid = if struct_id.is_empty() {
                None
            } else {
                Some(struct_id.clone())
            };
            FieldType::Structure(StructureDesc {
                struct_id: sid,
                fields: fields
                    .iter()
                    .map(|(name, fd)| SpvdFieldDesc {
                        name: name.clone(),
                        field_type: epics_field_desc_to_field_type(fd),
                    })
                    .collect(),
            })
        }
        // Phase 1 expansion left these new variants unmapped — they require
        // adding union/variant support to the spvirit FieldType, which is
        // exactly what Phase 5 replaces. Fall back to an empty structure.
        epics_pva_rs::pvdata::FieldDesc::StructureArray { .. }
        | epics_pva_rs::pvdata::FieldDesc::Union { .. }
        | epics_pva_rs::pvdata::FieldDesc::UnionArray { .. }
        | epics_pva_rs::pvdata::FieldDesc::Variant
        | epics_pva_rs::pvdata::FieldDesc::VariantArray
        | epics_pva_rs::pvdata::FieldDesc::BoundedString(_) => FieldType::Structure(StructureDesc {
            struct_id: None,
            fields: Vec::new(),
        }),
    }
}

fn scalar_type_to_typecode(st: ScalarType) -> TypeCode {
    match st {
        ScalarType::Boolean => TypeCode::Boolean,
        ScalarType::Byte => TypeCode::Int8,
        ScalarType::Short => TypeCode::Int16,
        ScalarType::Int => TypeCode::Int32,
        ScalarType::Long => TypeCode::Int64,
        ScalarType::UByte => TypeCode::UInt8,
        ScalarType::UShort => TypeCode::UInt16,
        ScalarType::UInt => TypeCode::UInt32,
        ScalarType::ULong => TypeCode::UInt64,
        ScalarType::Float => TypeCode::Float32,
        ScalarType::Double => TypeCode::Float64,
        ScalarType::String => TypeCode::String,
    }
}

fn pv_structure_to_descriptor(pv: &PvStructure) -> StructureDesc {
    let struct_id = if pv.struct_id.is_empty() {
        None
    } else {
        Some(pv.struct_id.clone())
    };
    StructureDesc {
        struct_id,
        fields: pv
            .fields
            .iter()
            .map(|(name, field)| SpvdFieldDesc {
                name: name.clone(),
                field_type: pv_field_to_field_type(field),
            })
            .collect(),
    }
}

fn pv_field_to_field_type(field: &PvField) -> FieldType {
    match field {
        PvField::Scalar(sv) => match sv {
            PvaScalarValue::Boolean(_) => FieldType::Scalar(TypeCode::Boolean),
            PvaScalarValue::Byte(_) => FieldType::Scalar(TypeCode::Int8),
            PvaScalarValue::Short(_) => FieldType::Scalar(TypeCode::Int16),
            PvaScalarValue::Int(_) => FieldType::Scalar(TypeCode::Int32),
            PvaScalarValue::Long(_) => FieldType::Scalar(TypeCode::Int64),
            PvaScalarValue::UByte(_) => FieldType::Scalar(TypeCode::UInt8),
            PvaScalarValue::UShort(_) => FieldType::Scalar(TypeCode::UInt16),
            PvaScalarValue::UInt(_) => FieldType::Scalar(TypeCode::UInt32),
            PvaScalarValue::ULong(_) => FieldType::Scalar(TypeCode::UInt64),
            PvaScalarValue::Float(_) => FieldType::Scalar(TypeCode::Float32),
            PvaScalarValue::Double(_) => FieldType::Scalar(TypeCode::Float64),
            PvaScalarValue::String(_) => FieldType::String,
        },
        PvField::ScalarArray(items) => match items.first() {
            Some(PvaScalarValue::Boolean(_)) => FieldType::ScalarArray(TypeCode::Boolean),
            Some(PvaScalarValue::Byte(_)) => FieldType::ScalarArray(TypeCode::Int8),
            Some(PvaScalarValue::Short(_)) => FieldType::ScalarArray(TypeCode::Int16),
            Some(PvaScalarValue::Int(_)) => FieldType::ScalarArray(TypeCode::Int32),
            Some(PvaScalarValue::Long(_)) => FieldType::ScalarArray(TypeCode::Int64),
            Some(PvaScalarValue::UByte(_)) => FieldType::ScalarArray(TypeCode::UInt8),
            Some(PvaScalarValue::UShort(_)) => FieldType::ScalarArray(TypeCode::UInt16),
            Some(PvaScalarValue::UInt(_)) => FieldType::ScalarArray(TypeCode::UInt32),
            Some(PvaScalarValue::ULong(_)) => FieldType::ScalarArray(TypeCode::UInt64),
            Some(PvaScalarValue::Float(_)) => FieldType::ScalarArray(TypeCode::Float32),
            Some(PvaScalarValue::Double(_)) => FieldType::ScalarArray(TypeCode::Float64),
            Some(PvaScalarValue::String(_)) | None => FieldType::StringArray,
        },
        PvField::Structure(nested) => FieldType::Structure(pv_structure_to_descriptor(nested)),
        // Composite shapes that this legacy adapter doesn't model — fall back
        // to an empty structure. The native qsrv source (added in Phase 5)
        // handles these cases properly.
        PvField::StructureArray(_)
        | PvField::Union { .. }
        | PvField::UnionArray(_)
        | PvField::Variant(_)
        | PvField::VariantArray(_)
        | PvField::Null => FieldType::Structure(StructureDesc {
            struct_id: None,
            fields: Vec::new(),
        }),
    }
}

// ── DecodedValue → PvStructure (put path) ────────────────────────────────

/// Wrap a `DecodedValue` as a `PvStructure` so the qsrv channel can consume
/// it with its existing `put` implementation.
///
/// If the decoded value is already a structure, it is reused (its top-level
/// `struct_id` is empty since the codec does not carry it through). For a
/// bare scalar / array, we synthesize an NTScalar-shaped wrapper with just
/// a `value` field, which is what the qsrv `put` path parses via
/// `pv_structure_to_epics`.
fn decoded_to_pv_structure(value: &DecodedValue, channel_name: &str) -> PvStructure {
    match value {
        DecodedValue::Structure(fields) => {
            let mut pv = PvStructure::new("");
            for (name, inner) in fields {
                if let Some(field) = decoded_to_pv_field(inner) {
                    pv.fields.push((name.clone(), field));
                }
            }
            pv
        }
        _ => {
            let mut pv = PvStructure::new("epics:nt/NTScalar:1.0");
            if let Some(field) = decoded_to_pv_field(value) {
                pv.fields.push(("value".to_string(), field));
            } else {
                tracing::debug!(
                    "qsrv put({channel_name}): unsupported decoded value shape {value:?}",
                );
            }
            pv
        }
    }
}

fn decoded_to_pv_field(value: &DecodedValue) -> Option<PvField> {
    match value {
        DecodedValue::Boolean(b) => Some(PvField::Scalar(PvaScalarValue::Boolean(*b))),
        DecodedValue::Int8(v) => Some(PvField::Scalar(PvaScalarValue::Byte(*v))),
        DecodedValue::Int16(v) => Some(PvField::Scalar(PvaScalarValue::Short(*v))),
        DecodedValue::Int32(v) => Some(PvField::Scalar(PvaScalarValue::Int(*v))),
        DecodedValue::Int64(v) => Some(PvField::Scalar(PvaScalarValue::Long(*v))),
        DecodedValue::UInt8(v) => Some(PvField::Scalar(PvaScalarValue::UByte(*v))),
        DecodedValue::UInt16(v) => Some(PvField::Scalar(PvaScalarValue::UShort(*v))),
        DecodedValue::UInt32(v) => Some(PvField::Scalar(PvaScalarValue::UInt(*v))),
        DecodedValue::UInt64(v) => Some(PvField::Scalar(PvaScalarValue::ULong(*v))),
        DecodedValue::Float32(v) => Some(PvField::Scalar(PvaScalarValue::Float(*v))),
        DecodedValue::Float64(v) => Some(PvField::Scalar(PvaScalarValue::Double(*v))),
        DecodedValue::String(s) => Some(PvField::Scalar(PvaScalarValue::String(s.clone()))),
        DecodedValue::Array(items) => {
            let converted: Vec<PvaScalarValue> = items
                .iter()
                .filter_map(|item| match decoded_to_pv_field(item) {
                    Some(PvField::Scalar(sv)) => Some(sv),
                    _ => None,
                })
                .collect();
            Some(PvField::ScalarArray(converted))
        }
        DecodedValue::Structure(fields) => {
            let mut pv = PvStructure::new("");
            for (name, inner) in fields {
                if let Some(field) = decoded_to_pv_field(inner) {
                    pv.fields.push((name.clone(), field));
                }
            }
            Some(PvField::Structure(pv))
        }
        DecodedValue::Null | DecodedValue::Raw(_) => None,
    }
}

/// Unused by the trait plumbing but kept for symmetry / potential future
/// descriptor-derived conversions.
#[allow(dead_code)]
fn scalar_type_to_type_code(t: ScalarType) -> TypeCode {
    match t {
        ScalarType::Boolean => TypeCode::Boolean,
        ScalarType::Byte => TypeCode::Int8,
        ScalarType::Short => TypeCode::Int16,
        ScalarType::Int => TypeCode::Int32,
        ScalarType::Long => TypeCode::Int64,
        ScalarType::UByte => TypeCode::UInt8,
        ScalarType::UShort => TypeCode::UInt16,
        ScalarType::UInt => TypeCode::UInt32,
        ScalarType::ULong => TypeCode::UInt64,
        ScalarType::Float => TypeCode::Float32,
        ScalarType::Double => TypeCode::Float64,
        ScalarType::String => TypeCode::String,
    }
}

// ---------------------------------------------------------------------------
// CA + PVA dual-protocol runner for IocApplication
// ---------------------------------------------------------------------------

/// Runs a combined CA + PVA IOC with QSRV bridge.
///
/// Designed as a protocol runner for [`IocApplication::run`]. Starts a CA
/// server in the background, creates a `QsrvPvStore` wrapping the database,
/// registers any PVA plugin PVs (NTNDArray from NDPluginPva), then runs the
/// PVA server with an interactive iocsh shell.
///
/// # Example
///
/// ```rust,ignore
/// AdIoc::new()
///     .run_with_script_and_runner("st.cmd", run_ca_pva_qsrv_ioc)
///     .await
/// ```
pub async fn run_ca_pva_qsrv_ioc(
    config: epics_base_rs::server::ioc_app::IocRunConfig,
) -> epics_base_rs::error::CaResult<()> {
    use epics_base_rs::error::CaError;

    let db = config.db.clone();
    let ca_port = config.port;
    let pva_port: u16 = std::env::var("EPICS_PVA_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5075);

    // ── QSRV bridge ──
    let provider = Arc::new(BridgeProvider::new(db.clone()));
    let store = Arc::new(QsrvPvStore::new(provider));

    // Register PVA plugin PVs (NTNDArray from NDPvaConfigure).
    // Handles were stored in the global registry during st.cmd execution.
    let pva_pvs = take_registered_pva_pvs();
    for (pv_name, handle) in pva_pvs {
        eprintln!("QSRV: registering PVA PV: {pv_name}");
        store
            .register_pva_pv(&pv_name, handle.latest, handle.subscribers)
            .await;
    }

    // ── CA server (background) ──
    let ca_server = epics_ca_rs::server::CaServer::from_parts(
        db.clone(),
        ca_port,
        config.acf.clone(),
        config.autosave_config.clone(),
        config.autosave_manager.clone(),
    );
    epics_base_rs::runtime::task::spawn(async move {
        if let Err(e) = ca_server.run().await {
            eprintln!("CA server error: {e}");
        }
    });

    // ── PVA server (foreground with iocsh) ──
    let pva_server = epics_pva_rs::server::PvaServer::from_parts(
        db,
        pva_port,
        config.acf,
        config.autosave_config,
        config.autosave_manager,
    );

    let shell_commands = config.shell_commands;
    pva_server
        .run_with_source_and_shell(store, move |shell| {
            for cmd in shell_commands {
                shell.register(cmd);
            }
        })
        .await
        .map_err(|e| CaError::InvalidValue(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pv_scalar_struct_to_generic_preserves_struct_id() {
        let mut pv = PvStructure::new("epics:nt/NTScalar:1.0");
        pv.fields.push((
            "value".into(),
            PvField::Scalar(PvaScalarValue::Double(3.125)),
        ));

        let (struct_id, fields) = pv_structure_to_generic_parts(&pv);
        assert_eq!(struct_id, "epics:nt/NTScalar:1.0");
        assert_eq!(fields.len(), 1);
        match &fields[0].1 {
            PvValue::Scalar(ScalarValue::F64(v)) => assert_eq!(*v, 3.125),
            other => panic!("expected scalar F64, got {other:?}"),
        }
    }

    #[test]
    fn pv_structure_array_to_spvirit_uniform_type() {
        let arr = vec![
            PvaScalarValue::Double(1.0),
            PvaScalarValue::Double(2.0),
            PvaScalarValue::Double(3.0),
        ];
        let sav = pva_array_to_spvirit(&arr);
        match sav {
            ScalarArrayValue::F64(v) => assert_eq!(v, vec![1.0, 2.0, 3.0]),
            other => panic!("expected F64 array, got {other:?}"),
        }
    }

    #[test]
    fn nested_pv_structure_nests_generic() {
        let mut alarm = PvStructure::new("alarm_t");
        alarm
            .fields
            .push(("severity".into(), PvField::Scalar(PvaScalarValue::Int(2))));

        let mut outer = PvStructure::new("epics:nt/NTScalar:1.0");
        outer
            .fields
            .push(("value".into(), PvField::Scalar(PvaScalarValue::Double(1.0))));
        outer
            .fields
            .push(("alarm".into(), PvField::Structure(alarm)));

        let (_, fields) = pv_structure_to_generic_parts(&outer);
        match &fields[1].1 {
            PvValue::Structure { struct_id, fields } => {
                assert_eq!(struct_id, "alarm_t");
                assert_eq!(fields.len(), 1);
            }
            other => panic!("expected nested structure, got {other:?}"),
        }
    }

    #[test]
    fn decoded_structure_to_pv_structure_roundtrip() {
        let decoded =
            DecodedValue::Structure(vec![("value".to_string(), DecodedValue::Float64(42.0))]);
        let pv = decoded_to_pv_structure(&decoded, "TEST");
        assert_eq!(pv.fields.len(), 1);
        match &pv.fields[0].1 {
            PvField::Scalar(PvaScalarValue::Double(v)) => assert_eq!(*v, 42.0),
            other => panic!("expected scalar double, got {other:?}"),
        }
    }

    #[test]
    fn bare_scalar_decoded_wraps_in_nt_scalar() {
        let decoded = DecodedValue::Int32(7);
        let pv = decoded_to_pv_structure(&decoded, "TEST");
        assert_eq!(pv.struct_id, "epics:nt/NTScalar:1.0");
        match pv.get_field("value") {
            Some(PvField::Scalar(PvaScalarValue::Int(v))) => assert_eq!(*v, 7),
            other => panic!("expected int scalar, got {other:?}"),
        }
    }

    #[test]
    fn descriptor_from_nt_scalar_has_matching_field_types() {
        let mut pv = PvStructure::new("epics:nt/NTScalar:1.0");
        pv.fields
            .push(("value".into(), PvField::Scalar(PvaScalarValue::Double(0.0))));
        let desc = pv_structure_to_descriptor(&pv);
        assert_eq!(desc.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));
        assert_eq!(desc.fields.len(), 1);
        match &desc.fields[0].field_type {
            FieldType::Scalar(TypeCode::Float64) => {}
            other => panic!("expected Float64 scalar, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn has_pv_falls_through_to_provider() {
        use epics_base_rs::server::database::PvDatabase;
        let db = Arc::new(PvDatabase::new());
        db.add_pv("TEST:X", epics_base_rs::types::EpicsValue::Double(1.0))
            .await;
        let provider = Arc::new(BridgeProvider::new(db));
        let store = QsrvPvStore::new(provider);
        assert!(store.has_pv("TEST:X").await);
        assert!(!store.has_pv("NOT:THERE").await);
    }
}
