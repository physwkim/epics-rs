//! PvStore implementation backed by an epics-rs PvDatabase.
//!
//! Bridges the gap between [`PvDatabase`] (EPICS record engine) and
//! [`PvStore`] (PVA server protocol handler) by converting between
//! `EpicsValue` / `Snapshot` and `NtPayload` / `DecodedValue`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use spvirit_codec::spvd_decode::{DecodedValue, FieldDesc, FieldType, StructureDesc, TypeCode};
use spvirit_server::PvStore;
use spvirit_server::monitor::MonitorRegistry;
use spvirit_types::{NtPayload, NtScalar, NtScalarArray, ScalarArrayValue, ScalarValue};
use tokio::sync::mpsc;
use tracing::warn;

use epics_base_rs::server::database::{PvDatabase, PvEntry, parse_pv_name};
use epics_base_rs::server::snapshot::Snapshot;
use epics_base_rs::types::{DbFieldType, EpicsValue};

static NEXT_PVA_SID: AtomicU32 = AtomicU32::new(2_000_000);

fn next_sid() -> u32 {
    NEXT_PVA_SID.fetch_add(1, Ordering::Relaxed)
}

/// [`PvStore`] implementation backed by an epics-rs [`PvDatabase`].
///
/// Each PvStore method translates between PVA normative types and EPICS
/// record values. Monitor notifications from the record engine are bridged
/// separately via [`start_monitor_bridge`].
pub struct PvDatabaseStore {
    db: Arc<PvDatabase>,
}

impl PvDatabaseStore {
    pub fn new(db: Arc<PvDatabase>) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &Arc<PvDatabase> {
        &self.db
    }
}

impl PvStore for PvDatabaseStore {
    fn has_pv(&self, name: &str) -> impl Future<Output = bool> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        async move { db.has_name(&name).await }
    }

    fn get_snapshot(&self, name: &str) -> impl Future<Output = Option<NtPayload>> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        async move { snapshot_for_pv(&db, &name).await }
    }

    fn get_descriptor(&self, name: &str) -> impl Future<Output = Option<StructureDesc>> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        async move {
            let payload = snapshot_for_pv(&db, &name).await?;
            Some(descriptor_for_payload(&payload))
        }
    }

    fn put_value(
        &self,
        name: &str,
        value: &DecodedValue,
    ) -> impl Future<Output = Result<Vec<(String, NtPayload)>, String>> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        let value = value.clone();
        async move { put_pv_value(&db, &name, &value).await }
    }

    fn is_writable(&self, name: &str) -> impl Future<Output = bool> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        async move { db.has_name(&name).await }
    }

    fn list_pvs(&self) -> impl Future<Output = Vec<String>> + Send {
        let db = self.db.clone();
        async move {
            let mut names = db.all_record_names().await;
            names.extend(db.all_simple_pv_names().await);
            names
        }
    }

    fn subscribe(
        &self,
        name: &str,
    ) -> impl Future<Output = Option<mpsc::Receiver<NtPayload>>> + Send {
        let db = self.db.clone();
        let name = name.to_string();
        async move { subscribe_to_pv(&db, &name).await }
    }
}

// ── Snapshot → NtPayload ─────────────────────────────────────────────────

async fn snapshot_for_pv(db: &PvDatabase, name: &str) -> Option<NtPayload> {
    let (_base, field) = parse_pv_name(name);

    match db.find_entry(name).await? {
        PvEntry::Simple(pv) => {
            let snap = pv.snapshot().await;
            Some(snapshot_to_nt_payload(&snap))
        }
        PvEntry::Record(rec) => {
            let instance = rec.read().await;
            let snap = instance.snapshot_for_field(field)?;
            Some(snapshot_to_nt_payload(&snap))
        }
    }
}

/// Convert an epics-base [`Snapshot`] to an [`NtPayload`].
pub fn snapshot_to_nt_payload(snap: &Snapshot) -> NtPayload {
    match &snap.value {
        EpicsValue::Double(v) => scalar_payload(ScalarValue::F64(*v), snap),
        EpicsValue::Float(v) => scalar_payload(ScalarValue::F32(*v), snap),
        EpicsValue::Long(v) => scalar_payload(ScalarValue::I32(*v), snap),
        EpicsValue::Short(v) => scalar_payload(ScalarValue::I16(*v), snap),
        EpicsValue::Char(v) => scalar_payload(ScalarValue::U8(*v), snap),
        EpicsValue::Enum(v) => {
            let mut nt = NtScalar::from_value(ScalarValue::I32(*v as i32));
            if let Some(ref ei) = snap.enums {
                nt.display_form_choices = ei.strings.clone();
            }
            populate_nt_metadata(&mut nt, snap);
            NtPayload::Scalar(nt)
        }
        EpicsValue::String(s) => scalar_payload(ScalarValue::Str(s.clone()), snap),

        EpicsValue::DoubleArray(v) => array_payload(ScalarArrayValue::F64(v.clone())),
        EpicsValue::FloatArray(v) => array_payload(ScalarArrayValue::F32(v.clone())),
        EpicsValue::LongArray(v) => array_payload(ScalarArrayValue::I32(v.clone())),
        EpicsValue::ShortArray(v) => array_payload(ScalarArrayValue::I16(v.clone())),
        EpicsValue::CharArray(v) => array_payload(ScalarArrayValue::U8(v.clone())),
        EpicsValue::EnumArray(v) => array_payload(ScalarArrayValue::U16(v.clone())),
    }
}

fn scalar_payload(sv: ScalarValue, snap: &Snapshot) -> NtPayload {
    let mut nt = NtScalar::from_value(sv);
    populate_nt_metadata(&mut nt, snap);
    NtPayload::Scalar(nt)
}

fn array_payload(sav: ScalarArrayValue) -> NtPayload {
    NtPayload::ScalarArray(NtScalarArray::from_value(sav))
}

fn populate_nt_metadata(nt: &mut NtScalar, snap: &Snapshot) {
    nt.alarm_severity = snap.alarm.severity as i32;
    nt.alarm_status = snap.alarm.status as i32;

    if let Some(ref disp) = snap.display {
        nt.units = disp.units.clone();
        nt.display_precision = disp.precision as i32;
        nt.display_low = disp.lower_disp_limit;
        nt.display_high = disp.upper_disp_limit;
        nt.alarm_high = non_zero(disp.upper_warning_limit);
        nt.alarm_hihi = non_zero(disp.upper_alarm_limit);
        nt.alarm_low = non_zero(disp.lower_warning_limit);
        nt.alarm_lolo = non_zero(disp.lower_alarm_limit);
        // Mirror limits into value alarm
        nt.value_alarm_high_warning_limit = disp.upper_warning_limit;
        nt.value_alarm_high_alarm_limit = disp.upper_alarm_limit;
        nt.value_alarm_low_warning_limit = disp.lower_warning_limit;
        nt.value_alarm_low_alarm_limit = disp.lower_alarm_limit;
    }

    if let Some(ref ctrl) = snap.control {
        nt.control_low = ctrl.lower_ctrl_limit;
        nt.control_high = ctrl.upper_ctrl_limit;
    }
}

fn non_zero(v: f64) -> Option<f64> {
    if v != 0.0 { Some(v) } else { None }
}

// ── NtPayload → StructureDesc ────────────────────────────────────────────

fn descriptor_for_payload(payload: &NtPayload) -> StructureDesc {
    match payload {
        NtPayload::Scalar(nt) => nt_scalar_desc(&nt.value),
        NtPayload::ScalarArray(arr) => nt_scalar_array_desc(&arr.value),
        _ => StructureDesc::new(),
    }
}

fn value_type_code(sv: &ScalarValue) -> TypeCode {
    match sv {
        ScalarValue::Bool(_) => TypeCode::Boolean,
        ScalarValue::I8(_) => TypeCode::Int8,
        ScalarValue::I16(_) => TypeCode::Int16,
        ScalarValue::I32(_) => TypeCode::Int32,
        ScalarValue::I64(_) => TypeCode::Int64,
        ScalarValue::U8(_) => TypeCode::UInt8,
        ScalarValue::U16(_) => TypeCode::UInt16,
        ScalarValue::U32(_) => TypeCode::UInt32,
        ScalarValue::U64(_) => TypeCode::UInt64,
        ScalarValue::F32(_) => TypeCode::Float32,
        ScalarValue::F64(_) => TypeCode::Float64,
        ScalarValue::Str(_) => TypeCode::String,
    }
}

fn array_type_code(sav: &ScalarArrayValue) -> TypeCode {
    match sav {
        ScalarArrayValue::Bool(_) => TypeCode::Boolean,
        ScalarArrayValue::I8(_) => TypeCode::Int8,
        ScalarArrayValue::I16(_) => TypeCode::Int16,
        ScalarArrayValue::I32(_) => TypeCode::Int32,
        ScalarArrayValue::I64(_) => TypeCode::Int64,
        ScalarArrayValue::U8(_) => TypeCode::UInt8,
        ScalarArrayValue::U16(_) => TypeCode::UInt16,
        ScalarArrayValue::U32(_) => TypeCode::UInt32,
        ScalarArrayValue::U64(_) => TypeCode::UInt64,
        ScalarArrayValue::F32(_) => TypeCode::Float32,
        ScalarArrayValue::F64(_) => TypeCode::Float64,
        ScalarArrayValue::Str(_) => TypeCode::String,
    }
}

fn nt_scalar_desc(sv: &ScalarValue) -> StructureDesc {
    let tc = value_type_code(sv);
    StructureDesc {
        struct_id: Some("epics:nt/NTScalar:1.0".to_string()),
        fields: vec![
            FieldDesc {
                name: "value".to_string(),
                field_type: FieldType::Scalar(tc),
            },
            FieldDesc {
                name: "alarm".to_string(),
                field_type: FieldType::Structure(alarm_desc()),
            },
            FieldDesc {
                name: "timeStamp".to_string(),
                field_type: FieldType::Structure(timestamp_desc()),
            },
            FieldDesc {
                name: "display".to_string(),
                field_type: FieldType::Structure(display_desc()),
            },
            FieldDesc {
                name: "control".to_string(),
                field_type: FieldType::Structure(control_desc()),
            },
            FieldDesc {
                name: "valueAlarm".to_string(),
                field_type: FieldType::Structure(value_alarm_desc()),
            },
        ],
    }
}

fn nt_scalar_array_desc(sav: &ScalarArrayValue) -> StructureDesc {
    let tc = array_type_code(sav);
    StructureDesc {
        struct_id: Some("epics:nt/NTScalarArray:1.0".to_string()),
        fields: vec![
            FieldDesc {
                name: "value".to_string(),
                field_type: FieldType::ScalarArray(tc),
            },
            FieldDesc {
                name: "alarm".to_string(),
                field_type: FieldType::Structure(alarm_desc()),
            },
            FieldDesc {
                name: "timeStamp".to_string(),
                field_type: FieldType::Structure(timestamp_desc()),
            },
            FieldDesc {
                name: "display".to_string(),
                field_type: FieldType::Structure(display_desc()),
            },
            FieldDesc {
                name: "control".to_string(),
                field_type: FieldType::Structure(control_desc()),
            },
        ],
    }
}

fn alarm_desc() -> StructureDesc {
    StructureDesc {
        struct_id: Some("alarm_t".to_string()),
        fields: vec![
            FieldDesc {
                name: "severity".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "status".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "message".to_string(),
                field_type: FieldType::String,
            },
        ],
    }
}

fn timestamp_desc() -> StructureDesc {
    StructureDesc {
        struct_id: Some("time_t".to_string()),
        fields: vec![
            FieldDesc {
                name: "secondsPastEpoch".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int64),
            },
            FieldDesc {
                name: "nanoseconds".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "userTag".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
        ],
    }
}

fn display_desc() -> StructureDesc {
    StructureDesc {
        struct_id: Some("display_t".to_string()),
        fields: vec![
            FieldDesc {
                name: "limitLow".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "limitHigh".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "description".to_string(),
                field_type: FieldType::String,
            },
            FieldDesc {
                name: "units".to_string(),
                field_type: FieldType::String,
            },
            FieldDesc {
                name: "precision".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "form".to_string(),
                field_type: FieldType::Structure(StructureDesc {
                    struct_id: Some("enum_t".to_string()),
                    fields: vec![
                        FieldDesc {
                            name: "index".to_string(),
                            field_type: FieldType::Scalar(TypeCode::Int32),
                        },
                        FieldDesc {
                            name: "choices".to_string(),
                            field_type: FieldType::StringArray,
                        },
                    ],
                }),
            },
        ],
    }
}

fn control_desc() -> StructureDesc {
    StructureDesc {
        struct_id: Some("control_t".to_string()),
        fields: vec![
            FieldDesc {
                name: "limitLow".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "limitHigh".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "minStep".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
        ],
    }
}

fn value_alarm_desc() -> StructureDesc {
    StructureDesc {
        struct_id: Some("valueAlarm_t".to_string()),
        fields: vec![
            FieldDesc {
                name: "active".to_string(),
                field_type: FieldType::Scalar(TypeCode::Boolean),
            },
            FieldDesc {
                name: "lowAlarmLimit".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "lowWarningLimit".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "highWarningLimit".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "highAlarmLimit".to_string(),
                field_type: FieldType::Scalar(TypeCode::Float64),
            },
            FieldDesc {
                name: "lowAlarmSeverity".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "lowWarningSeverity".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "highWarningSeverity".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "highAlarmSeverity".to_string(),
                field_type: FieldType::Scalar(TypeCode::Int32),
            },
            FieldDesc {
                name: "hysteresis".to_string(),
                field_type: FieldType::Scalar(TypeCode::UInt8),
            },
        ],
    }
}

// ── PUT: DecodedValue → EpicsValue ───────────────────────────────────────

async fn put_pv_value(
    db: &PvDatabase,
    name: &str,
    value: &DecodedValue,
) -> Result<Vec<(String, NtPayload)>, String> {
    let (base, field) = parse_pv_name(name);

    let inner = extract_pva_value(value);
    let epics_value = decoded_to_epics(inner)
        .ok_or_else(|| format!("cannot convert decoded value for '{}'", name))?;

    // Try simple PV put first; fall back to record field put.
    match db.put_pv(name, epics_value.clone()).await {
        Ok(()) => {}
        Err(_) => {
            db.put_record_field_from_ca(base, field, epics_value)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    // Return empty: the monitor bridge handles notifications.
    Ok(vec![])
}

fn extract_pva_value(value: &DecodedValue) -> &DecodedValue {
    if let DecodedValue::Structure(fields) = value {
        if let Some((_, inner)) = fields.iter().find(|(name, _)| name == "value") {
            return inner;
        }
    }
    value
}

fn decoded_to_epics(value: &DecodedValue) -> Option<EpicsValue> {
    match value {
        DecodedValue::Float64(v) => Some(EpicsValue::Double(*v)),
        DecodedValue::Float32(v) => Some(EpicsValue::Float(*v)),
        DecodedValue::Int32(v) => Some(EpicsValue::Long(*v)),
        DecodedValue::Int16(v) => Some(EpicsValue::Short(*v)),
        DecodedValue::Int8(v) => Some(EpicsValue::Char(*v as u8)),
        DecodedValue::UInt8(v) => Some(EpicsValue::Char(*v)),
        DecodedValue::UInt16(v) => Some(EpicsValue::Enum(*v)),
        DecodedValue::Int64(v) => Some(EpicsValue::Long(*v as i32)),
        DecodedValue::UInt32(v) => Some(EpicsValue::Long(*v as i32)),
        DecodedValue::UInt64(v) => Some(EpicsValue::Long(*v as i32)),
        DecodedValue::String(s) => Some(EpicsValue::String(s.clone())),
        DecodedValue::Boolean(b) => Some(EpicsValue::Char(if *b { 1 } else { 0 })),
        _ => None,
    }
}

// ── Subscribe ────────────────────────────────────────────────────────────

async fn subscribe_to_pv(db: &PvDatabase, name: &str) -> Option<mpsc::Receiver<NtPayload>> {
    let (_base, field) = parse_pv_name(name);
    let (tx, rx) = mpsc::channel(64);

    match db.find_entry(name).await? {
        PvEntry::Simple(pv) => {
            let sid = next_sid();
            let mut event_rx = pv.add_subscriber(sid, DbFieldType::Double, 0x07).await;
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let payload = snapshot_to_nt_payload(&event.snapshot);
                    if tx.send(payload).await.is_err() {
                        break;
                    }
                }
            });
        }
        PvEntry::Record(rec) => {
            let sid = next_sid();
            let event_rx = {
                let mut instance = rec.write().await;
                instance.add_subscriber(field, sid, DbFieldType::Double, 0x07)
            };
            let mut event_rx = event_rx;
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let payload = snapshot_to_nt_payload(&event.snapshot);
                    if tx.send(payload).await.is_err() {
                        break;
                    }
                }
            });
        }
    }

    Some(rx)
}

// ── Monitor bridge ───────────────────────────────────────────────────────

/// Generic monitor bridge: for every PV the store exposes, spawn a task that
/// forwards [`PvStore::subscribe`] updates into the spvirit [`MonitorRegistry`].
///
/// This replaces the PvDatabase-specific bridge for custom stores such as
/// `QsrvPvStore`. The specialized [`start_monitor_bridge`] below remains for
/// the default [`PvDatabaseStore`] path — it is slightly cheaper because it
/// avoids the channel creation roundtrip inside `QsrvPvStore::subscribe`.
pub async fn start_store_monitor_bridge<S: PvStore + 'static>(
    store: Arc<S>,
    registry: Arc<MonitorRegistry>,
) {
    let pvs = store.list_pvs().await;
    for pv in pvs {
        let store = store.clone();
        let registry = registry.clone();
        tokio::spawn(async move {
            let Some(mut rx) = store.subscribe(&pv).await else {
                warn!("store monitor bridge: subscribe('{}') returned None", pv);
                return;
            };
            while let Some(payload) = rx.recv().await {
                registry.notify_monitors(&pv, &payload).await;
            }
        });
    }
}

/// Start background tasks that bridge PvDatabase record change events to the
/// PVA [`MonitorRegistry`].
///
/// For each record in the database, a subscriber is added. Whenever a record
/// is processed and its value changes, the subscriber converts the EPICS
/// [`Snapshot`] to an [`NtPayload`] and pushes it to all active PVA monitors
/// via the registry.
///
/// Simple PVs (non-record) are also bridged so that direct `put_pv` calls
/// propagate to PVA monitor clients.
pub async fn start_monitor_bridge(db: Arc<PvDatabase>, registry: Arc<MonitorRegistry>) {
    // Bridge records.
    let record_names = db.all_record_names().await;
    for name in record_names {
        let db = db.clone();
        let registry = registry.clone();
        let pv_name = name.clone();
        tokio::spawn(async move {
            let Some(rec) = db.get_record(&pv_name).await else {
                warn!("monitor bridge: record '{}' not found", pv_name);
                return;
            };
            let sid = next_sid();
            let event_rx = {
                let mut instance = rec.write().await;
                instance.add_subscriber("VAL", sid, DbFieldType::Double, 0x07)
            };
            let mut event_rx = event_rx;
            while let Some(event) = event_rx.recv().await {
                let payload = snapshot_to_nt_payload(&event.snapshot);
                registry.notify_monitors(&pv_name, &payload).await;
            }
        });
    }

    // Bridge simple PVs.
    let simple_names = db.all_simple_pv_names().await;
    for name in simple_names {
        let db = db.clone();
        let registry = registry.clone();
        let pv_name = name.clone();
        tokio::spawn(async move {
            let Some(pv) = db.find_pv(&pv_name).await else {
                warn!("monitor bridge: simple PV '{}' not found", pv_name);
                return;
            };
            let sid = next_sid();
            let mut event_rx = pv.add_subscriber(sid, DbFieldType::Double, 0x07).await;
            while let Some(event) = event_rx.recv().await {
                let payload = snapshot_to_nt_payload(&event.snapshot);
                registry.notify_monitors(&pv_name, &payload).await;
            }
        });
    }
}
