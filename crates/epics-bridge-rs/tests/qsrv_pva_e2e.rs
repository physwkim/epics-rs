//! End-to-end tests for the qsrv ↔ spvirit PvStore adapter.
//!
//! These exercise `QsrvPvStore` directly against a populated `PvDatabase`,
//! verifying that the adapter produces wire-shape `NtPayload` snapshots and
//! correctly routes `put_value`/`subscribe` through qsrv channels. They
//! skip the TCP/UDP plumbing — that is covered by manual runs of
//! `qsrv-rs` against `spget`/`spput`/`spmonitor`.

#![cfg(feature = "qsrv")]

use std::sync::Arc;

use spvirit_codec::spvd_decode::{DecodedValue, FieldType, TypeCode};
use spvirit_server::PvStore;
use spvirit_types::{NtField, NtPayload, ScalarValue};
use tokio::time::{Duration, timeout};

use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::records::{ai::AiRecord, ao::AoRecord, bi::BiRecord};
use epics_bridge_rs::qsrv::{BridgeProvider, QsrvPvStore};

fn structure_payload(payload: &NtPayload) -> &spvirit_types::NtStructure {
    match payload {
        NtPayload::Structure(s) => s,
        other => panic!("expected NtPayload::Structure, got {other:?}"),
    }
}

async fn store_with_record<R>(name: &str, record: R) -> Arc<QsrvPvStore>
where
    R: epics_base_rs::server::record::Record + 'static,
{
    let db = Arc::new(PvDatabase::new());
    db.add_record(name, Box::new(record)).await;
    let provider = Arc::new(BridgeProvider::new(db));
    Arc::new(QsrvPvStore::new(provider))
}

#[tokio::test]
async fn get_snapshot_nt_scalar_double() {
    let store = store_with_record("TEST:AI", AiRecord::new(3.14)).await;

    let payload = store.get_snapshot("TEST:AI").await.expect("snapshot");
    let s = structure_payload(&payload);
    assert_eq!(s.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));

    let value_field = s.field("value").expect("value field");
    match value_field {
        NtField::Scalar(ScalarValue::F64(v)) => assert!((v - 3.14).abs() < 1e-10),
        other => panic!("expected F64 scalar, got {other:?}"),
    }

    assert!(matches!(s.field("alarm"), Some(NtField::Structure(_))));
    assert!(matches!(s.field("timeStamp"), Some(NtField::Structure(_))));
}

#[tokio::test]
async fn get_snapshot_nt_enum_for_bi_record() {
    let store = store_with_record("TEST:BI", BiRecord::default()).await;

    let payload = store.get_snapshot("TEST:BI").await.expect("snapshot");
    let s = structure_payload(&payload);
    assert_eq!(s.struct_id.as_deref(), Some("epics:nt/NTEnum:1.0"));

    // NTEnum value is itself a structure { index, choices }.
    match s.field("value").expect("value") {
        NtField::Structure(inner) => {
            assert!(inner.field("index").is_some(), "expected enum index field");
            assert!(inner.field("choices").is_some(), "expected enum choices field");
        }
        other => panic!("expected nested structure for NTEnum value, got {other:?}"),
    }
}

#[tokio::test]
async fn get_descriptor_matches_nt_scalar_shape() {
    let store = store_with_record("TEST:AI", AiRecord::new(0.0)).await;
    let desc = store.get_descriptor("TEST:AI").await.expect("descriptor");
    assert_eq!(desc.struct_id.as_deref(), Some("epics:nt/NTScalar:1.0"));

    let value = desc.fields.iter().find(|f| f.name == "value").expect("value field");
    match &value.field_type {
        FieldType::Scalar(TypeCode::Float64) => {}
        other => panic!("expected Float64 value field, got {other:?}"),
    }
    assert!(desc.fields.iter().any(|f| f.name == "alarm"));
    assert!(desc.fields.iter().any(|f| f.name == "timeStamp"));
}

#[tokio::test]
async fn has_pv_and_list_pvs_report_record() {
    let store = store_with_record("TEST:AI", AiRecord::new(0.0)).await;
    assert!(store.has_pv("TEST:AI").await);
    assert!(!store.has_pv("NOT:REAL").await);

    let all = store.list_pvs().await;
    assert!(
        all.iter().any(|n| n == "TEST:AI"),
        "expected TEST:AI in list_pvs, got {all:?}"
    );
}

#[tokio::test]
async fn put_value_scalar_updates_the_record() {
    let store = store_with_record("TEST:AO", AoRecord::default()).await;

    let decoded = DecodedValue::Structure(vec![(
        "value".to_string(),
        DecodedValue::Float64(99.5),
    )]);
    store.put_value("TEST:AO", &decoded).await.expect("put");

    let payload = store.get_snapshot("TEST:AO").await.expect("readback");
    let s = structure_payload(&payload);
    match s.field("value").expect("value") {
        NtField::Scalar(ScalarValue::F64(v)) => assert!((v - 99.5).abs() < 1e-10),
        other => panic!("expected F64 value, got {other:?}"),
    }
}

#[tokio::test]
async fn put_value_bare_scalar_also_works() {
    let store = store_with_record("TEST:AO", AoRecord::default()).await;
    let decoded = DecodedValue::Float64(7.5);
    store.put_value("TEST:AO", &decoded).await.expect("put");

    let payload = store.get_snapshot("TEST:AO").await.expect("readback");
    let s = structure_payload(&payload);
    match s.field("value").expect("value") {
        NtField::Scalar(ScalarValue::F64(v)) => assert!((v - 7.5).abs() < 1e-10),
        other => panic!("expected F64 value, got {other:?}"),
    }
}

#[tokio::test]
async fn subscribe_delivers_post_put_update() {
    let store = store_with_record("TEST:AO", AoRecord::default()).await;

    let mut rx = store.subscribe("TEST:AO").await.expect("subscribe");

    // Drain the initial snapshot that monitors emit on connect (mirrors
    // the C++ BaseMonitor::connect behavior). Tolerate either presence
    // or absence — the contract is "first event is the current value".
    let _ = timeout(Duration::from_millis(200), rx.recv()).await;

    let decoded = DecodedValue::Float64(123.0);
    store.put_value("TEST:AO", &decoded).await.expect("put");

    // Loop a few times in case the initial snapshot wasn't drained or
    // an intermediate value (e.g. simulation processing) sneaks through.
    let mut last_value: Option<f64> = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        let Ok(Some(payload)) = timeout(Duration::from_millis(200), rx.recv()).await else {
            continue;
        };
        let s = structure_payload(&payload);
        if let Some(NtField::Scalar(ScalarValue::F64(v))) = s.field("value") {
            last_value = Some(*v);
            if (*v - 123.0).abs() < 1e-10 {
                return;
            }
        }
    }
    panic!("did not see post-put value 123.0; last seen = {last_value:?}");
}

#[tokio::test]
async fn unknown_pv_returns_none() {
    let db = Arc::new(PvDatabase::new());
    let store = Arc::new(QsrvPvStore::new(Arc::new(BridgeProvider::new(db))));
    assert!(store.get_snapshot("MISSING").await.is_none());
    assert!(store.get_descriptor("MISSING").await.is_none());
    assert!(store.subscribe("MISSING").await.is_none());
    assert!(!store.has_pv("MISSING").await);
}
