//! Smoke test for `#[derive(NTScalar)]` + `pvget_typed` /
//! `pvput_typed` / `pvmonitor_typed`. Spins up an in-process
//! `PvaServer` with a single SharedPV, then exercises every
//! typed-NT entry point of `PvaClient`.

use std::sync::Arc;
use std::time::Duration;

use epics_pva_rs::client::PvaClient;
use epics_pva_rs::nt::derive::NTScalar;
use epics_pva_rs::nt::{Alarm, TimeStamp, TypedNT};
use epics_pva_rs::pvdata::{FieldDesc, PvField, ScalarType, ScalarValue};
use epics_pva_rs::server_native::{PvaServer, SharedPV, SharedSource};

#[derive(Debug, Clone, NTScalar, PartialEq)]
struct MotorPos {
    value: f64,
    #[nt(meta)]
    alarm: Alarm,
    #[nt(meta)]
    timestamp: TimeStamp,
}

#[test]
fn typed_nt_descriptor_shape() {
    let d = MotorPos::descriptor();
    match d {
        FieldDesc::Structure { struct_id, fields } => {
            assert_eq!(struct_id, "epics:nt/NTScalar:1.0");
            // value + alarm + timestamp
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].0, "value");
            assert!(matches!(fields[0].1, FieldDesc::Scalar(ScalarType::Double)));
            assert_eq!(fields[1].0, "alarm");
            assert_eq!(fields[2].0, "timestamp");
        }
        other => panic!("unexpected descriptor: {other:?}"),
    }
}

#[test]
fn typed_nt_round_trip_local() {
    let pos = MotorPos {
        value: 3.14,
        alarm: Alarm {
            severity: 1,
            status: 2,
            message: "near limit".into(),
        },
        timestamp: TimeStamp {
            seconds_past_epoch: 1_700_000_000,
            nanoseconds: 12345,
            user_tag: 7,
        },
    };
    let f = pos.to_pv_field();
    let back = MotorPos::from_pv_field(&f).expect("decode");
    assert_eq!(pos, back);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pvget_typed_against_local_server() {
    // Build a SharedPV holding a 3-field NTScalar (value + alarm +
    // timestamp). The descriptor we open with must match the
    // derived MotorPos descriptor exactly.
    let pv = SharedPV::new();
    pv.open(MotorPos::descriptor(), {
        let initial = MotorPos {
            value: 42.5,
            alarm: Alarm {
                severity: 0,
                status: 0,
                message: String::new(),
            },
            timestamp: TimeStamp::default(),
        };
        initial.to_pv_field()
    });
    let source = SharedSource::new();
    source.add("MOTOR:VAL", pv);
    let _server = PvaServer::isolated(Arc::new(source));
    let client = _server.client_config();
    let _ = &_server;

    let pos: MotorPos = tokio::time::timeout(
        Duration::from_secs(5),
        client.pvget_typed::<MotorPos>("MOTOR:VAL"),
    )
    .await
    .expect("timeout")
    .expect("typed get");
    assert_eq!(pos.value, 42.5);
    assert_eq!(pos.alarm.severity, 0);
    assert_eq!(pos.alarm.message, "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pvget_typed_primitive_f64() {
    // Bare f64 against a plain NTScalar<double> source.
    let pv = SharedPV::new();
    pv.open(
        f64::descriptor(),
        f64::to_pv_field(&7.5),
    );
    let source = SharedSource::new();
    source.add("OVEN:TEMP", pv);
    let _server = PvaServer::isolated(Arc::new(source));
    let client = _server.client_config();

    let temp: f64 = tokio::time::timeout(
        Duration::from_secs(5),
        client.pvget_typed::<f64>("OVEN:TEMP"),
    )
    .await
    .expect("timeout")
    .expect("typed get");
    assert_eq!(temp, 7.5);
}
