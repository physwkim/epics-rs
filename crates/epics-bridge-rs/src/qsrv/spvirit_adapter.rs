//! Adapter that exposes a [`BridgeProvider`] (qsrv) through the
//! [`spvirit_server::PvStore`] trait, so that the spvirit PVA server can
//! serve EPICS records (single-record and group composite PVs) over PVA.
//!
//! Bridging is done in two directions:
//!   - **Read path**: qsrv produces `epics_pva_rs::pvdata::PvStructure`; the
//!     adapter converts it to `spvirit_types::NtPayload::Structure` using
//!     `NtStructure` / `NtField`, preserving `struct_id` and field names so
//!     that NTScalar / NTEnum / NTScalarArray responses stay wire-compatible.
//!   - **Write path**: the protocol handler decodes incoming PUT bytes into
//!     `spvirit_codec::spvd_decode::DecodedValue`; the adapter rewraps it as
//!     a `PvStructure` and dispatches to the qsrv channel's `put`.

use std::collections::HashMap;
use std::sync::Arc;

use spvirit_codec::spvd_decode::{
    DecodedValue, FieldDesc as SpvdFieldDesc, FieldType, StructureDesc, TypeCode,
};
use spvirit_server::PvStore;
use spvirit_types::{NtField, NtPayload, NtStructure, ScalarArrayValue, ScalarValue};
use tokio::sync::{RwLock, mpsc};

use epics_pva_rs::pvdata::{PvField, PvStructure, ScalarType, ScalarValue as PvaScalarValue};

use super::group::AnyMonitor;
use super::provider::{AnyChannel, BridgeProvider, Channel, ChannelProvider, PvaMonitor};

/// PvStore implementation backed by a qsrv [`BridgeProvider`].
///
/// Handles both single-record PVs and group composite PVs. Group PVs ride
/// on the `NtPayload::Structure` variant introduced in spvirit-types 0.1.7.
pub struct QsrvPvStore {
    provider: Arc<BridgeProvider>,
    /// Per-PV cache of opened channels. qsrv channels are stateless enough
    /// that caching is just an optimization — re-creating on every call
    /// would work, but would throw away `BridgeProvider`'s metadata cache
    /// win by re-opening on every get/put.
    channels: RwLock<HashMap<String, Arc<AnyChannel>>>,
}

impl QsrvPvStore {
    pub fn new(provider: Arc<BridgeProvider>) -> Self {
        Self {
            provider,
            channels: RwLock::new(HashMap::new()),
        }
    }

    pub fn provider(&self) -> &Arc<BridgeProvider> {
        &self.provider
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

impl PvStore for QsrvPvStore {
    fn has_pv(&self, name: &str) -> impl Future<Output = bool> + Send {
        let provider = self.provider.clone();
        let name = name.to_string();
        async move { provider.channel_find(&name).await }
    }

    fn get_snapshot(&self, name: &str) -> impl Future<Output = Option<NtPayload>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            let empty_request = PvStructure::new("");
            match channel.get(&empty_request).await {
                Ok(pv) => Some(pv_structure_to_nt_payload(&pv)),
                Err(e) => {
                    tracing::debug!("qsrv get_snapshot({name_owned}) failed: {e}");
                    None
                }
            }
        }
    }

    fn get_descriptor(&self, name: &str) -> impl Future<Output = Option<StructureDesc>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            let empty_request = PvStructure::new("");
            match channel.get(&empty_request).await {
                Ok(pv) => Some(pv_structure_to_descriptor(&pv)),
                Err(_) => None,
            }
        }
    }

    fn put_value(
        &self,
        name: &str,
        value: &DecodedValue,
    ) -> impl Future<Output = Result<Vec<(String, NtPayload)>, String>> + Send {
        let name_owned = name.to_string();
        let value_owned = value.clone();
        async move {
            let channel = self
                .channel(&name_owned)
                .await
                .ok_or_else(|| format!("PV not found: {name_owned}"))?;

            let pv = decoded_to_pv_structure(&value_owned, channel.channel_name());
            channel.put(&pv).await.map_err(|e| e.to_string())?;

            // The monitor bridge delivers follow-up notifications; the
            // synchronous PUT response returns no inline changes.
            Ok(Vec::new())
        }
    }

    fn is_writable(&self, name: &str) -> impl Future<Output = bool> + Send {
        let provider = self.provider.clone();
        let name = name.to_string();
        async move { provider.channel_find(&name).await }
    }

    fn list_pvs(&self) -> impl Future<Output = Vec<String>> + Send {
        let provider = self.provider.clone();
        async move { provider.channel_list().await }
    }

    fn subscribe(
        &self,
        name: &str,
    ) -> impl Future<Output = Option<mpsc::Receiver<NtPayload>>> + Send {
        let name_owned = name.to_string();
        async move {
            let channel = self.channel(&name_owned).await?;
            let mut monitor = match channel.create_monitor().await {
                Ok(m) => m,
                Err(e) => {
                    tracing::debug!("qsrv subscribe({name_owned}) create_monitor: {e}");
                    return None;
                }
            };
            if let Err(e) = monitor.start().await {
                tracing::debug!("qsrv subscribe({name_owned}) start: {e}");
                return None;
            }
            let (tx, rx) = mpsc::channel::<NtPayload>(64);
            tokio::spawn(monitor_bridge_loop(monitor, tx, name_owned));
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

// ── PvStructure → NtPayload / NtStructure / StructureDesc ────────────────

fn pv_structure_to_nt_payload(pv: &PvStructure) -> NtPayload {
    NtPayload::Structure(pv_structure_to_nt_structure(pv))
}

fn pv_structure_to_nt_structure(pv: &PvStructure) -> NtStructure {
    let struct_id = if pv.struct_id.is_empty() {
        None
    } else {
        Some(pv.struct_id.clone())
    };
    let fields = pv
        .fields
        .iter()
        .map(|(name, field)| (name.clone(), pv_field_to_nt_field(field)))
        .collect();
    NtStructure { struct_id, fields }
}

fn pv_field_to_nt_field(field: &PvField) -> NtField {
    match field {
        PvField::Scalar(sv) => NtField::Scalar(pva_scalar_to_spvirit(sv)),
        PvField::ScalarArray(items) => NtField::ScalarArray(pva_array_to_spvirit(items)),
        PvField::Structure(nested) => NtField::Structure(pv_structure_to_nt_structure(nested)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pv_scalar_struct_to_nt_structure_preserves_struct_id() {
        let mut pv = PvStructure::new("epics:nt/NTScalar:1.0");
        pv.fields.push((
            "value".into(),
            PvField::Scalar(PvaScalarValue::Double(3.125)),
        ));

        let nt = pv_structure_to_nt_structure(&pv);
        assert_eq!(nt.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));
        assert_eq!(nt.fields.len(), 1);
        match &nt.fields[0].1 {
            NtField::Scalar(ScalarValue::F64(v)) => assert_eq!(*v, 3.125),
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
    fn nested_pv_structure_nests_nt_structure() {
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

        let nt = pv_structure_to_nt_structure(&outer);
        match &nt.fields[1].1 {
            NtField::Structure(inner) => {
                assert_eq!(inner.struct_id.as_deref(), Some("alarm_t"));
                assert_eq!(inner.fields.len(), 1);
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
