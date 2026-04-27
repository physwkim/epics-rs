//! Port of pvxs's `test/testdata.cpp` (selected parts).
//!
//! pvxs's testdata.cpp covers the C++ `Value` operator[] / from() /
//! mark/unmark API which we don't replicate. The portable parts are
//! field-iteration counts (`testIterStruct`), name lookup
//! (`testName`), and field-path traversal (`testTraverse`).

#![cfg(test)]

use epics_pva_rs::nt::NTScalar;
use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue, Value};

fn nt_scalar_string_desc() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "epics:nt/NTScalar:1.0".into(),
        fields: vec![
            ("value".into(), FieldDesc::Scalar(ScalarType::String)),
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
                        ("secondsPastEpoch".into(), FieldDesc::Scalar(ScalarType::Long)),
                        ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
                    ],
                },
            ),
        ],
    }
}

// ── testIterStruct (pvxs testdata.cpp:159) ─────────────────────────
//
// pvxs counts `iall` = 9 descendants and `ichildren` = 3 children for
// NTScalar(String). Those map to total_bits()-1 (excluding root) and
// field_count() respectively.

#[test]
fn pvxs_iter_struct_descendant_count() {
    let nt = nt_scalar_string_desc();
    // pvxs iall() = 9 descendants (excludes root = bit 0)
    assert_eq!(nt.total_bits() - 1, 9);
}

#[test]
fn pvxs_iter_struct_immediate_children_count() {
    let nt = nt_scalar_string_desc();
    // pvxs ichildren() = 3 (value, alarm, timeStamp)
    assert_eq!(nt.field_count(), 3);
}

#[test]
fn pvxs_iter_struct_alarm_subtree() {
    let nt = nt_scalar_string_desc();
    let alarm_bit = nt.bit_for_path("alarm").unwrap();
    // alarm bit + 3 leaves = 4 bits inhabited (matches pvxs "mark alarm sub-struct" case)
    let alarm = match &nt {
        FieldDesc::Structure { fields, .. } => fields.iter().find(|(n, _)| n == "alarm").unwrap().1.clone(),
        _ => unreachable!(),
    };
    assert_eq!(alarm_bit, 2);
    assert_eq!(alarm.total_bits(), 4); // alarm + severity + status + message
}

// ── testTraverse (pvxs testdata.cpp:24) ────────────────────────────
//
// pvxs uses `<` to traverse to parent. We test the simpler dotted-path
// lookup via `bit_for_path`.

#[test]
fn pvxs_traverse_alarm_severity_path() {
    let nt = nt_scalar_string_desc();
    assert_eq!(nt.bit_for_path("alarm.severity"), Some(3));
    assert_eq!(nt.bit_for_path("alarm.status"), Some(4));
    assert_eq!(nt.bit_for_path("alarm.message"), Some(5));
}

#[test]
fn pvxs_traverse_nonexistent_returns_none() {
    let nt = nt_scalar_string_desc();
    assert_eq!(nt.bit_for_path("missing"), None);
    assert_eq!(nt.bit_for_path("alarm.missing"), None);
    assert_eq!(nt.bit_for_path("missing.severity"), None);
}

#[test]
fn pvxs_traverse_root_path_is_bit_zero() {
    let nt = nt_scalar_string_desc();
    assert_eq!(nt.bit_for_path(""), Some(0));
}

// ── testName (pvxs testdata.cpp:144) ───────────────────────────────
//
// pvxs `val.nameOf(val["alarm.status"])` returns "alarm.status". Our
// `PvStructure::get_field` only handles single-segment names, so we
// verify field access at each level instead.

#[test]
fn pvxs_name_lookup_at_each_level() {
    let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
    s.fields.push((
        "value".into(),
        PvField::Scalar(ScalarValue::String("hi".into())),
    ));
    let mut alarm = PvStructure::new("alarm_t");
    alarm
        .fields
        .push(("severity".into(), PvField::Scalar(ScalarValue::Int(2))));
    alarm
        .fields
        .push(("status".into(), PvField::Scalar(ScalarValue::Int(1))));
    alarm.fields.push((
        "message".into(),
        PvField::Scalar(ScalarValue::String("low".into())),
    ));
    s.fields.push(("alarm".into(), PvField::Structure(alarm)));

    // Top-level lookup
    assert!(s.get_field("value").is_some());
    assert!(s.get_field("alarm").is_some());
    assert!(s.get_field("missing").is_none());

    // Drill into alarm
    if let Some(PvField::Structure(a)) = s.get_field("alarm") {
        match a.get_field("severity") {
            Some(PvField::Scalar(ScalarValue::Int(n))) => assert_eq!(*n, 2),
            other => panic!("severity: {other:?}"),
        }
        match a.get_field("status") {
            Some(PvField::Scalar(ScalarValue::Int(n))) => assert_eq!(*n, 1),
            other => panic!("status: {other:?}"),
        }
    } else {
        panic!("alarm not a struct");
    }
}

// ── testAssign + mark/unmark via Value (pvxs testdata.cpp:52,144) ──
//
// pvxs's Value provides operator[]/from()/as<T>()/mark()/unmark()/
// assign(). Our Value type ports the same patterns to typed methods.

#[test]
fn pvxs_value_set_and_get_with_coercion() {
    // pvxs: val["value"] = 4u; val["alarm.severity"] = 1;
    let mut v = Value::create_from(NTScalar::new(ScalarType::Int).build());
    v.set("value", 4i32).unwrap();
    v.set("alarm.severity", 1i32).unwrap();
    assert_eq!(v.get_as::<i32>("value").unwrap(), 4);
    assert_eq!(v.get_as::<i32>("alarm.severity").unwrap(), 1);
    assert!(v.is_marked("value"));
    assert!(v.is_marked("alarm.severity"));
}

#[test]
fn pvxs_value_assign_copies_marked_fields() {
    // pvxs: val1.assign(val2) — copies marked subset.
    let mut a = Value::create_from(NTScalar::new(ScalarType::Int).build());
    let mut b = Value::create_from(NTScalar::new(ScalarType::Int).build());
    b.set("value", 4i32).unwrap();
    b.set("alarm.severity", 1i32).unwrap();
    a.assign(&b).unwrap();
    assert_eq!(a.get_as::<i32>("value").unwrap(), 4);
    assert!(a.is_marked("value"));
    assert!(a.is_marked("alarm.severity"));
}

#[test]
fn pvxs_value_unmark_then_isnt_marked() {
    let mut v = Value::create_from(NTScalar::new(ScalarType::Int).build());
    v.mark("value").unwrap();
    assert!(v.is_marked("value"));
    v.unmark("value").unwrap();
    assert!(!v.is_marked("value"));
}

#[test]
fn pvxs_value_iter_marked_returns_marked_paths() {
    let mut v = Value::create_from(NTScalar::new(ScalarType::Int).build());
    v.mark("value").unwrap();
    v.mark("timeStamp.userTag").unwrap();
    let marked = v.iter_marked();
    assert_eq!(marked, vec!["value", "timeStamp.userTag"]);
}
