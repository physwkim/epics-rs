//! Integration tests for the PVA server bridge (PvDatabaseStore ↔ PvDatabase).
//!
//! These tests exercise the [`PvStore`] adapter without starting the network
//! stack, mirroring how epics-ca-rs tests exercise CaServer.get/.put directly.

use std::sync::Arc;

use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::types::EpicsValue;
use epics_pva_rs::server::bridge::{PvDatabaseStore, snapshot_to_nt_payload};
use spvirit_codec::spvd_decode::DecodedValue;
use spvirit_server::PvStore;
use spvirit_types::{NtPayload, ScalarArrayValue, ScalarValue};

// ── helpers ──────────────────────────────────────────────────────────────

async fn db_with_pvs(pvs: &[(&str, EpicsValue)]) -> (Arc<PvDatabase>, PvDatabaseStore) {
    let db = Arc::new(PvDatabase::new());
    for (name, val) in pvs {
        db.add_pv(name, val.clone()).await;
    }
    let store = PvDatabaseStore::new(db.clone());
    (db, store)
}

// ── has_pv ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn has_pv_returns_true_for_existing() {
    let (_db, store) = db_with_pvs(&[("TEST:VAL", EpicsValue::Double(1.0))]).await;
    assert!(store.has_pv("TEST:VAL").await);
}

#[tokio::test]
async fn has_pv_returns_false_for_missing() {
    let (_db, store) = db_with_pvs(&[("TEST:VAL", EpicsValue::Double(1.0))]).await;
    assert!(!store.has_pv("NONEXISTENT").await);
}

// ── list_pvs ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_pvs_empty_db() {
    let db = Arc::new(PvDatabase::new());
    let store = PvDatabaseStore::new(db);
    let pvs = store.list_pvs().await;
    assert!(pvs.is_empty());
}

// ── get_snapshot for Double ──────────────────────────────────────────────

#[tokio::test]
async fn get_snapshot_double() {
    let (_db, store) = db_with_pvs(&[("SIG:A", EpicsValue::Double(3.125))]).await;
    let payload = store.get_snapshot("SIG:A").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::F64(v) => assert!((v - 3.125).abs() < 1e-10),
            other => panic!("expected F64, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_float() {
    let (_db, store) = db_with_pvs(&[("SIG:F", EpicsValue::Float(2.5))]).await;
    let payload = store.get_snapshot("SIG:F").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::F32(v) => assert!((v - 2.5).abs() < 1e-5),
            other => panic!("expected F32, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_long() {
    let (_db, store) = db_with_pvs(&[("SIG:L", EpicsValue::Long(42))]).await;
    let payload = store.get_snapshot("SIG:L").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::I32(v) => assert_eq!(v, 42),
            other => panic!("expected I32, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_short() {
    let (_db, store) = db_with_pvs(&[("SIG:S", EpicsValue::Short(7))]).await;
    let payload = store.get_snapshot("SIG:S").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::I16(v) => assert_eq!(v, 7),
            other => panic!("expected I16, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_char() {
    let (_db, store) = db_with_pvs(&[("SIG:C", EpicsValue::Char(65))]).await;
    let payload = store.get_snapshot("SIG:C").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::U8(v) => assert_eq!(v, 65),
            other => panic!("expected U8, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_string() {
    let (_db, store) = db_with_pvs(&[("SIG:STR", EpicsValue::String("hello".into()))]).await;
    let payload = store.get_snapshot("SIG:STR").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::Str(ref s) => assert_eq!(s, "hello"),
            other => panic!("expected Str, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_enum() {
    let (_db, store) = db_with_pvs(&[("SIG:E", EpicsValue::Enum(3))]).await;
    let payload = store.get_snapshot("SIG:E").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::I32(v) => assert_eq!(v, 3),
            other => panic!("expected I32 (enum), got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_nonexistent_returns_none() {
    let (_db, store) = db_with_pvs(&[("SIG:A", EpicsValue::Double(1.0))]).await;
    assert!(store.get_snapshot("NO:SUCH:PV").await.is_none());
}

// ── get_snapshot for arrays ──────────────────────────────────────────────

#[tokio::test]
async fn get_snapshot_double_array() {
    let vals = vec![1.0, 2.0, 3.0];
    let (_db, store) = db_with_pvs(&[("ARR:D", EpicsValue::DoubleArray(vals.clone()))]).await;
    let payload = store.get_snapshot("ARR:D").await.unwrap();
    match payload {
        NtPayload::ScalarArray(arr) => match arr.value {
            ScalarArrayValue::F64(v) => assert_eq!(v, vals),
            other => panic!("expected F64 array, got {other:?}"),
        },
        other => panic!("expected ScalarArray, got {other:?}"),
    }
}

#[tokio::test]
async fn get_snapshot_long_array() {
    let vals = vec![10, 20, 30];
    let (_db, store) = db_with_pvs(&[("ARR:L", EpicsValue::LongArray(vals.clone()))]).await;
    let payload = store.get_snapshot("ARR:L").await.unwrap();
    match payload {
        NtPayload::ScalarArray(arr) => match arr.value {
            ScalarArrayValue::I32(v) => assert_eq!(v, vals),
            other => panic!("expected I32 array, got {other:?}"),
        },
        other => panic!("expected ScalarArray, got {other:?}"),
    }
}

// ── get_descriptor ───────────────────────────────────────────────────────

#[tokio::test]
async fn get_descriptor_double_has_nt_scalar_id() {
    let (_db, store) = db_with_pvs(&[("DESC:D", EpicsValue::Double(0.0))]).await;
    let desc = store.get_descriptor("DESC:D").await.unwrap();
    assert_eq!(desc.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));
    // Should have value, alarm, timeStamp, display, control, valueAlarm fields
    let names: Vec<&str> = desc.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"value"), "missing value field");
    assert!(names.contains(&"alarm"), "missing alarm field");
    assert!(names.contains(&"timeStamp"), "missing timeStamp field");
    assert!(names.contains(&"display"), "missing display field");
    assert!(names.contains(&"control"), "missing control field");
    assert!(names.contains(&"valueAlarm"), "missing valueAlarm field");
}

#[tokio::test]
async fn get_descriptor_double_array_has_nt_scalar_array_id() {
    let (_db, store) = db_with_pvs(&[("DESC:ARR", EpicsValue::DoubleArray(vec![1.0]))]).await;
    let desc = store.get_descriptor("DESC:ARR").await.unwrap();
    assert_eq!(
        desc.struct_id.as_deref(),
        Some("epics:nt/NTScalarArray:1.0")
    );
}

#[tokio::test]
async fn get_descriptor_nonexistent_returns_none() {
    let (_db, store) = db_with_pvs(&[]).await;
    assert!(store.get_descriptor("NOPE").await.is_none());
}

// ── put_value ────────────────────────────────────────────────────────────

#[tokio::test]
async fn put_value_double() {
    let (db, store) = db_with_pvs(&[("PUT:D", EpicsValue::Double(0.0))]).await;
    let val = DecodedValue::Float64(99.9);
    let result = store.put_value("PUT:D", &val).await;
    assert!(result.is_ok());
    // Verify via database
    let readback = db.get_pv("PUT:D").await.unwrap();
    assert_eq!(readback, EpicsValue::Double(99.9));
}

#[tokio::test]
async fn put_value_long() {
    let (db, store) = db_with_pvs(&[("PUT:L", EpicsValue::Long(0))]).await;
    let val = DecodedValue::Int32(42);
    let result = store.put_value("PUT:L", &val).await;
    assert!(result.is_ok());
    let readback = db.get_pv("PUT:L").await.unwrap();
    assert_eq!(readback, EpicsValue::Long(42));
}

#[tokio::test]
async fn put_value_string() {
    let (db, store) = db_with_pvs(&[("PUT:S", EpicsValue::String("".into()))]).await;
    let val = DecodedValue::String("updated".into());
    let result = store.put_value("PUT:S", &val).await;
    assert!(result.is_ok());
    let readback = db.get_pv("PUT:S").await.unwrap();
    assert_eq!(readback, EpicsValue::String("updated".into()));
}

#[tokio::test]
async fn put_value_wrapped_in_structure() {
    // PVA PUT messages wrap the value in Structure { value: <val> }
    let (db, store) = db_with_pvs(&[("PUT:W", EpicsValue::Double(0.0))]).await;
    let val = DecodedValue::Structure(vec![("value".to_string(), DecodedValue::Float64(55.5))]);
    let result = store.put_value("PUT:W", &val).await;
    assert!(result.is_ok());
    let readback = db.get_pv("PUT:W").await.unwrap();
    assert_eq!(readback, EpicsValue::Double(55.5));
}

#[tokio::test]
async fn put_value_returns_empty_changed_vec() {
    // Monitor bridge handles notifications, so put_value returns empty vec
    let (_db, store) = db_with_pvs(&[("PUT:E", EpicsValue::Double(0.0))]).await;
    let val = DecodedValue::Float64(1.0);
    let changed = store.put_value("PUT:E", &val).await.unwrap();
    assert!(changed.is_empty());
}

#[tokio::test]
async fn put_value_nonexistent_returns_error() {
    let (_db, store) = db_with_pvs(&[]).await;
    let val = DecodedValue::Float64(1.0);
    let result = store.put_value("NO:PV", &val).await;
    assert!(result.is_err());
}

// ── is_writable ──────────────────────────────────────────────────────────

#[tokio::test]
async fn is_writable_existing() {
    let (_db, store) = db_with_pvs(&[("W:PV", EpicsValue::Double(0.0))]).await;
    assert!(store.is_writable("W:PV").await);
}

#[tokio::test]
async fn is_writable_nonexistent() {
    let (_db, store) = db_with_pvs(&[]).await;
    assert!(!store.is_writable("NOPE").await);
}

// ── subscribe ────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscribe_existing_pv_returns_receiver() {
    let (_db, store) = db_with_pvs(&[("SUB:PV", EpicsValue::Double(1.0))]).await;
    let rx = store.subscribe("SUB:PV").await;
    assert!(rx.is_some());
}

#[tokio::test]
async fn subscribe_nonexistent_returns_none() {
    let (_db, store) = db_with_pvs(&[]).await;
    let rx = store.subscribe("NOPE").await;
    assert!(rx.is_none());
}

// ── snapshot_to_nt_payload unit tests ────────────────────────────────────

#[test]
fn snapshot_double_to_nt_scalar() {
    use epics_base_rs::server::snapshot::Snapshot;
    use std::time::SystemTime;
    let snap = Snapshot::new(EpicsValue::Double(2.75), 0, 0, SystemTime::now());
    let payload = snapshot_to_nt_payload(&snap);
    match payload {
        NtPayload::Scalar(nt) => {
            assert!(matches!(nt.value, ScalarValue::F64(v) if (v - 2.75).abs() < 1e-10));
        }
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[test]
fn snapshot_string_to_nt_scalar() {
    use epics_base_rs::server::snapshot::Snapshot;
    use std::time::SystemTime;
    let snap = Snapshot::new(EpicsValue::String("test".into()), 0, 0, SystemTime::now());
    let payload = snapshot_to_nt_payload(&snap);
    match payload {
        NtPayload::Scalar(nt) => {
            assert!(matches!(&nt.value, ScalarValue::Str(s) if s == "test"));
        }
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[test]
fn snapshot_double_array_to_nt_scalar_array() {
    use epics_base_rs::server::snapshot::Snapshot;
    use std::time::SystemTime;
    let snap = Snapshot::new(
        EpicsValue::DoubleArray(vec![1.0, 2.0]),
        0,
        0,
        SystemTime::now(),
    );
    let payload = snapshot_to_nt_payload(&snap);
    match payload {
        NtPayload::ScalarArray(arr) => {
            assert!(matches!(&arr.value, ScalarArrayValue::F64(v) if v == &[1.0, 2.0]));
        }
        other => panic!("expected ScalarArray, got {other:?}"),
    }
}

// ── put + get roundtrip ──────────────────────────────────────────────────

#[tokio::test]
async fn put_then_get_roundtrip() {
    let (_db, store) = db_with_pvs(&[("RT:PV", EpicsValue::Double(0.0))]).await;

    // Put via PvStore
    let val = DecodedValue::Float64(42.0);
    store.put_value("RT:PV", &val).await.unwrap();

    // Get via PvStore
    let payload = store.get_snapshot("RT:PV").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match nt.value {
            ScalarValue::F64(v) => assert!((v - 42.0).abs() < 1e-10),
            other => panic!("expected F64, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

#[tokio::test]
async fn put_then_get_roundtrip_string() {
    let (_db, store) = db_with_pvs(&[("RT:S", EpicsValue::String("".into()))]).await;

    store
        .put_value("RT:S", &DecodedValue::String("round-trip".into()))
        .await
        .unwrap();

    let payload = store.get_snapshot("RT:S").await.unwrap();
    match payload {
        NtPayload::Scalar(nt) => match &nt.value {
            ScalarValue::Str(s) => assert_eq!(s, "round-trip"),
            other => panic!("expected Str, got {other:?}"),
        },
        other => panic!("expected Scalar, got {other:?}"),
    }
}

// ── multiple PVs ─────────────────────────────────────────────────────────

#[tokio::test]
async fn multiple_pv_types_coexist() {
    let (_db, store) = db_with_pvs(&[
        ("M:D", EpicsValue::Double(1.1)),
        ("M:L", EpicsValue::Long(-5)),
        ("M:S", EpicsValue::String("abc".into())),
        ("M:F", EpicsValue::Float(3.0)),
    ])
    .await;

    assert!(store.has_pv("M:D").await);
    assert!(store.has_pv("M:L").await);
    assert!(store.has_pv("M:S").await);
    assert!(store.has_pv("M:F").await);
    assert!(!store.has_pv("M:NOPE").await);

    // Verify values
    match store.get_snapshot("M:D").await.unwrap() {
        NtPayload::Scalar(nt) => {
            assert!(matches!(nt.value, ScalarValue::F64(v) if (v - 1.1).abs() < 1e-10))
        }
        other => panic!("expected Scalar, got {other:?}"),
    }
    match store.get_snapshot("M:L").await.unwrap() {
        NtPayload::Scalar(nt) => assert!(matches!(nt.value, ScalarValue::I32(-5))),
        other => panic!("expected Scalar, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PvaServer builder tests — mirrors epics-ca-rs CaServer builder tests
// ═══════════════════════════════════════════════════════════════════════════

use epics_pva_rs::server::PvaServer;

#[tokio::test]
async fn pva_server_builder_with_simple_pvs() {
    let server = PvaServer::builder()
        .pv("TEST:DOUBLE", EpicsValue::Double(3.15))
        .pv("TEST:STRING", EpicsValue::String("hello".into()))
        .pv("TEST:SHORT", EpicsValue::Short(42))
        .pv("TEST:ENUM", EpicsValue::Enum(2))
        .build()
        .await
        .unwrap();

    assert_eq!(
        server.get("TEST:DOUBLE").await.unwrap(),
        EpicsValue::Double(3.15)
    );
    assert_eq!(
        server.get("TEST:STRING").await.unwrap(),
        EpicsValue::String("hello".into())
    );
    assert_eq!(
        server.get("TEST:SHORT").await.unwrap(),
        EpicsValue::Short(42)
    );
    assert_eq!(server.get("TEST:ENUM").await.unwrap(), EpicsValue::Enum(2));
}

#[tokio::test]
async fn pva_server_put_and_get_double() {
    let server = PvaServer::builder()
        .pv("SRV:D", EpicsValue::Double(0.0))
        .build()
        .await
        .unwrap();

    server.put("SRV:D", EpicsValue::Double(99.9)).await.unwrap();
    assert_eq!(server.get("SRV:D").await.unwrap(), EpicsValue::Double(99.9));
}

#[tokio::test]
async fn pva_server_put_and_get_string() {
    let server = PvaServer::builder()
        .pv("SRV:S", EpicsValue::String("initial".into()))
        .build()
        .await
        .unwrap();

    server
        .put("SRV:S", EpicsValue::String("updated".into()))
        .await
        .unwrap();
    assert_eq!(
        server.get("SRV:S").await.unwrap(),
        EpicsValue::String("updated".into())
    );
}

#[tokio::test]
async fn pva_server_get_nonexistent() {
    let server = PvaServer::builder()
        .pv("REAL:PV", EpicsValue::Double(1.0))
        .build()
        .await
        .unwrap();

    assert!(server.get("DOES:NOT:EXIST").await.is_err());
}

#[tokio::test]
async fn pva_server_add_pv_at_runtime() {
    let server = PvaServer::builder().build().await.unwrap();
    assert!(server.get("RUNTIME:PV").await.is_err());

    server.add_pv("RUNTIME:PV", EpicsValue::Double(42.0)).await;
    assert_eq!(
        server.get("RUNTIME:PV").await.unwrap(),
        EpicsValue::Double(42.0)
    );
}

#[tokio::test]
async fn pva_server_db_string_ai_record() {
    let db_text = r#"
record(ai, "TEMP:READING") {
    field(VAL, "25.0")
}
"#;
    let macros = std::collections::HashMap::new();
    let server = PvaServer::builder()
        .db_string(db_text, &macros)
        .unwrap()
        .build()
        .await
        .unwrap();

    let val = server.get("TEMP:READING").await.unwrap();
    assert_eq!(val, EpicsValue::Double(25.0));
}

#[tokio::test]
async fn pva_server_db_string_with_macros() {
    let db_text = r#"
record(ai, "$(PREFIX):SETPOINT") {
    field(VAL, "100.0")
}
"#;
    let mut macros = std::collections::HashMap::new();
    macros.insert("PREFIX".to_string(), "MTR01".to_string());
    let server = PvaServer::builder()
        .db_string(db_text, &macros)
        .unwrap()
        .build()
        .await
        .unwrap();

    let val = server.get("MTR01:SETPOINT").await.unwrap();
    assert_eq!(val, EpicsValue::Double(100.0));
}

#[tokio::test]
async fn pva_server_database_accessor() {
    let server = PvaServer::builder()
        .pv("DB:ACCESS", EpicsValue::Double(7.7))
        .build()
        .await
        .unwrap();

    let db = server.database();
    assert!(db.has_name("DB:ACCESS").await);
    assert!(!db.has_name("NONEXISTENT").await);
}

#[tokio::test]
async fn pva_server_custom_port() {
    let server = PvaServer::builder()
        .port(9999)
        .pv("PORT:TEST", EpicsValue::Double(1.0))
        .build()
        .await
        .unwrap();

    assert_eq!(
        server.get("PORT:TEST").await.unwrap(),
        EpicsValue::Double(1.0)
    );
}
