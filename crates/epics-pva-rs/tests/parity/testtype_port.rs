//! Port of pvxs's `test/testtype.cpp` (selected, type-API-portable parts).
//!
//! pvxs's testtype.cpp is mostly about the `TypeDef` / `TypeCode` /
//! `Member` C++ DSL which we don't replicate. The portable parts are
//! TypeCode element sizes (`testCode`) and field-descriptor display
//! formatting (`testFormat` first cases).

#![cfg(test)]

use epics_pva_rs::pvdata::{FieldDesc, ScalarType};

// ── testCode (pvxs testtype.cpp:31) ────────────────────────────────
//
// pvxs verifies `TypeCode::size()` returns 1/2/4/8 bytes for each
// fixed-width scalar (and the matching arrayOf variants).

#[test]
fn pvxs_type_code_sizes_signed() {
    assert_eq!(ScalarType::Byte.element_size(), 1);
    assert_eq!(ScalarType::Short.element_size(), 2);
    assert_eq!(ScalarType::Int.element_size(), 4);
    assert_eq!(ScalarType::Long.element_size(), 8);
}

#[test]
fn pvxs_type_code_sizes_unsigned() {
    assert_eq!(ScalarType::UByte.element_size(), 1);
    assert_eq!(ScalarType::UShort.element_size(), 2);
    assert_eq!(ScalarType::UInt.element_size(), 4);
    assert_eq!(ScalarType::ULong.element_size(), 8);
}

#[test]
fn pvxs_type_code_sizes_float() {
    assert_eq!(ScalarType::Float.element_size(), 4);
    assert_eq!(ScalarType::Double.element_size(), 8);
}

#[test]
fn pvxs_type_code_array_type_code_high_bit() {
    // pvxs: arrayOf(TypeCode::T) sets the 0x08 array bit.
    for st in [
        ScalarType::Byte,
        ScalarType::Short,
        ScalarType::Int,
        ScalarType::Long,
        ScalarType::UByte,
        ScalarType::UShort,
        ScalarType::UInt,
        ScalarType::ULong,
        ScalarType::Float,
        ScalarType::Double,
        ScalarType::String,
    ] {
        let scalar_code = st.type_code();
        let array_code = st.array_type_code();
        assert_eq!(array_code, scalar_code | 0x08);
        assert_eq!(ScalarType::from_array_type_code(array_code), Some(st));
        // Element size matches the scalar version.
        assert_eq!(
            FieldDesc::ScalarArray(st).total_bits(),
            FieldDesc::Scalar(st).total_bits()
        );
    }
}

#[test]
fn pvxs_type_code_round_trips() {
    // Every scalar code decodes back to itself.
    for st in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Short,
        ScalarType::Int,
        ScalarType::Long,
        ScalarType::UByte,
        ScalarType::UShort,
        ScalarType::UInt,
        ScalarType::ULong,
        ScalarType::Float,
        ScalarType::Double,
        ScalarType::String,
    ] {
        assert_eq!(ScalarType::from_type_code(st.type_code()), Some(st));
    }
}

// ── testFormat (pvxs testtype.cpp:507, simple TypeDef cases) ───────
//
// pvxs expects `struct "id" { ... }` style output. Our Display matches
// the pvxs format closely enough for the common shapes.

#[test]
fn pvxs_format_simple_struct() {
    let desc = FieldDesc::Structure {
        struct_id: "simple_t".into(),
        fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Double))],
    };
    let s = format!("{desc}");
    assert!(s.contains("struct"), "actual: {s}");
    assert!(s.contains("simple_t"), "actual: {s}");
    assert!(s.contains("value"), "actual: {s}");
    assert!(s.contains("double"), "actual: {s}");
}

#[test]
fn pvxs_format_empty_struct() {
    let desc = FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![],
    };
    let s = format!("{desc}");
    assert!(s.contains("struct"), "actual: {s}");
}

#[test]
fn pvxs_format_scalar_array() {
    let desc = FieldDesc::ScalarArray(ScalarType::Double);
    let s = format!("{desc}");
    assert_eq!(s, "double[]");
}
