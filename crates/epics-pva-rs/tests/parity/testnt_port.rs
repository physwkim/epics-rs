//! Port of pvxs `test/testnt.cpp` (subset).
//!
//! pvxs verifies the structure-id and core fields of each NT builder.
//! We mirror the same assertions on our nt:: builders.

#![cfg(test)]

use epics_pva_rs::nt::{NTAttribute, NTEnum, NTScalar, NTTable, NTURI};
use epics_pva_rs::pvdata::{FieldDesc, ScalarType};

// ── testNTScalar (pvxs testnt.cpp:18) ──────────────────────────────

#[test]
fn pvxs_nt_scalar_int32_id_starts_with_ntscalar() {
    let desc = NTScalar::new(ScalarType::Int).build();
    if let FieldDesc::Structure { struct_id, .. } = desc {
        assert!(struct_id.starts_with("epics:nt/NTScalar:"));
    } else {
        panic!("expected struct");
    }
}

#[test]
fn pvxs_nt_scalar_default_has_no_display() {
    let desc = NTScalar::new(ScalarType::Int).build();
    if let FieldDesc::Structure { fields, .. } = desc {
        assert!(!fields.iter().any(|(n, _)| n == "display"));
        assert!(!fields.iter().any(|(n, _)| n == "control"));
        assert!(!fields.iter().any(|(n, _)| n == "valueAlarm"));
    }
}

#[test]
fn pvxs_nt_scalar_double_with_display_only() {
    let desc = NTScalar::new(ScalarType::Double).with_display().build();
    if let FieldDesc::Structure { fields, .. } = desc {
        let display = fields
            .iter()
            .find_map(|(n, d)| if n == "display" { Some(d) } else { None })
            .expect("display");
        if let FieldDesc::Structure {
            fields: display_fields,
            ..
        } = display
        {
            assert!(display_fields.iter().any(|(n, _)| n == "limitLow"));
        }
        assert!(!fields.iter().any(|(n, _)| n == "control"));
    }
}

#[test]
fn pvxs_nt_scalar_double_with_everything() {
    let desc = NTScalar::new(ScalarType::Double)
        .with_display()
        .with_control()
        .with_value_alarm()
        .build();
    if let FieldDesc::Structure { fields, .. } = desc {
        assert!(fields.iter().any(|(n, _)| n == "display"));
        assert!(fields.iter().any(|(n, _)| n == "control"));
        assert!(fields.iter().any(|(n, _)| n == "valueAlarm"));
    }
}

// ── testNTEnum (pvxs testnt.cpp:82) ────────────────────────────────

#[test]
fn pvxs_nt_enum_id_starts_with_ntenum() {
    let desc = NTEnum::new().build();
    if let FieldDesc::Structure { struct_id, .. } = desc {
        assert!(struct_id.starts_with("epics:nt/NTEnum:"));
    }
}

// ── testNTTable (pvxs testnt.cpp:91) ───────────────────────────────

#[test]
fn pvxs_nt_table_columns_int_string_string() {
    let desc = NTTable::new()
        .add_column(ScalarType::Int, "A", Some("Col A"))
        .add_column(ScalarType::String, "C", Some("Col C"))
        .add_column(ScalarType::String, "B", Some("Col B"))
        .build();
    if let FieldDesc::Structure { struct_id, fields } = desc {
        assert_eq!(struct_id, "epics:nt/NTTable:1.0");
        let value = fields
            .iter()
            .find_map(|(n, d)| if n == "value" { Some(d) } else { None })
            .expect("value");
        if let FieldDesc::Structure { fields: cols, .. } = value {
            // pvxs: value.A int32_t[], value.B string[]
            let by_name: std::collections::HashMap<&str, &FieldDesc> =
                cols.iter().map(|(n, d)| (n.as_str(), d)).collect();
            assert!(matches!(
                by_name.get("A"),
                Some(FieldDesc::ScalarArray(ScalarType::Int))
            ));
            assert!(matches!(
                by_name.get("B"),
                Some(FieldDesc::ScalarArray(ScalarType::String))
            ));
        }
    }
}

// ── testNTURI (pvxs testnt.cpp:64) ─────────────────────────────────

#[test]
fn pvxs_nt_uri_id_starts_with_nturi() {
    let desc = NTURI::new()
        .arg_scalar("arg1", ScalarType::UInt)
        .arg_scalar("arg2", ScalarType::String)
        .build();
    if let FieldDesc::Structure { struct_id, .. } = desc {
        assert!(struct_id.starts_with("epics:nt/NTURI:"));
    }
}

// ── NTAttribute ────────────────────────────────────────────────────

#[test]
fn pvxs_nt_attribute_struct_id() {
    let desc = NTAttribute::build();
    if let FieldDesc::Structure { struct_id, .. } = desc {
        assert!(struct_id.starts_with("epics:nt/NTAttribute:"));
    }
}
