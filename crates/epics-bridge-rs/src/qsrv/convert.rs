use epics_base_rs::types::{DbFieldType, EpicsValue};
use epics_pva_rs::pvdata::{PvField, ScalarType, ScalarValue};

/// Convert EPICS DBF type to PVA ScalarType.
pub fn dbf_to_scalar_type(dbf: DbFieldType) -> ScalarType {
    match dbf {
        DbFieldType::String => ScalarType::String,
        DbFieldType::Short => ScalarType::Short,
        DbFieldType::Float => ScalarType::Float,
        DbFieldType::Enum => ScalarType::UShort, // C++ maps DBR_ENUM to pvUShort
        DbFieldType::Char => ScalarType::UByte,
        DbFieldType::Long => ScalarType::Int,
        DbFieldType::Double => ScalarType::Double,
    }
}

/// Convert EpicsValue to PVA ScalarValue.
pub fn epics_to_scalar(val: &EpicsValue) -> ScalarValue {
    match val {
        EpicsValue::String(s) => ScalarValue::String(s.clone()),
        EpicsValue::Short(v) => ScalarValue::Short(*v),
        EpicsValue::Float(v) => ScalarValue::Float(*v),
        EpicsValue::Enum(v) => ScalarValue::UShort(*v), // C++: pvUShort
        EpicsValue::Char(v) => ScalarValue::UByte(*v),
        EpicsValue::Long(v) => ScalarValue::Int(*v),
        EpicsValue::Double(v) => ScalarValue::Double(*v),
        // Arrays: take first element or default
        EpicsValue::ShortArray(a) => ScalarValue::Short(a.first().copied().unwrap_or(0)),
        EpicsValue::FloatArray(a) => ScalarValue::Float(a.first().copied().unwrap_or(0.0)),
        EpicsValue::EnumArray(a) => ScalarValue::UShort(a.first().copied().unwrap_or(0)),
        EpicsValue::DoubleArray(a) => ScalarValue::Double(a.first().copied().unwrap_or(0.0)),
        EpicsValue::LongArray(a) => ScalarValue::Int(a.first().copied().unwrap_or(0)),
        EpicsValue::CharArray(a) => ScalarValue::UByte(a.first().copied().unwrap_or(0)),
    }
}

/// Convert PVA ScalarValue back to EpicsValue (context-free fallback).
///
/// Prefer `scalar_to_epics_typed()` when the target DBF type is known.
pub fn scalar_to_epics(val: &ScalarValue) -> EpicsValue {
    match val {
        ScalarValue::String(s) => EpicsValue::String(s.clone()),
        ScalarValue::Short(v) => EpicsValue::Short(*v),
        ScalarValue::Float(v) => EpicsValue::Float(*v),
        ScalarValue::Double(v) => EpicsValue::Double(*v),
        ScalarValue::Int(v) => EpicsValue::Long(*v),
        ScalarValue::Long(v) => EpicsValue::Double(*v as f64),
        ScalarValue::Byte(v) => EpicsValue::Short(*v as i16),
        ScalarValue::UByte(v) => EpicsValue::Char(*v),
        ScalarValue::UShort(v) => EpicsValue::Enum(*v),
        ScalarValue::UInt(v) => EpicsValue::Long(*v as i32),
        ScalarValue::ULong(v) => EpicsValue::Double(*v as f64),
        ScalarValue::Boolean(v) => EpicsValue::Short(if *v { 1 } else { 0 }),
    }
}

/// Context-aware conversion: PVA ScalarValue → EpicsValue using target DBF type.
///
/// Unlike `scalar_to_epics()`, this uses the target field type to produce the
/// correct EpicsValue variant, matching C++ PVIF behavior where conversions are
/// guided by `dbChannelFinalFieldType()`.
pub fn scalar_to_epics_typed(val: &ScalarValue, target: DbFieldType) -> EpicsValue {
    match target {
        DbFieldType::Double => EpicsValue::Double(scalar_to_f64(val)),
        DbFieldType::Float => EpicsValue::Float(scalar_to_f64(val) as f32),
        DbFieldType::Long => EpicsValue::Long(scalar_to_i64(val) as i32),
        DbFieldType::Short => EpicsValue::Short(scalar_to_i64(val) as i16),
        DbFieldType::Char => EpicsValue::Char(scalar_to_i64(val) as u8),
        DbFieldType::Enum => EpicsValue::Enum(scalar_to_i64(val) as u16),
        DbFieldType::String => match val {
            ScalarValue::String(s) => EpicsValue::String(s.clone()),
            other => EpicsValue::String(other.to_string()),
        },
    }
}

/// Extract f64 from any ScalarValue.
fn scalar_to_f64(val: &ScalarValue) -> f64 {
    match val {
        ScalarValue::Double(v) => *v,
        ScalarValue::Float(v) => *v as f64,
        ScalarValue::Int(v) => *v as f64,
        ScalarValue::Long(v) => *v as f64,
        ScalarValue::Short(v) => *v as f64,
        ScalarValue::Byte(v) => *v as f64,
        ScalarValue::UByte(v) => *v as f64,
        ScalarValue::UShort(v) => *v as f64,
        ScalarValue::UInt(v) => *v as f64,
        ScalarValue::ULong(v) => *v as f64,
        ScalarValue::Boolean(v) => if *v { 1.0 } else { 0.0 },
        ScalarValue::String(s) => s.parse().unwrap_or(0.0),
    }
}

/// Extract i64 from any ScalarValue.
fn scalar_to_i64(val: &ScalarValue) -> i64 {
    match val {
        ScalarValue::Int(v) => *v as i64,
        ScalarValue::Long(v) => *v,
        ScalarValue::Short(v) => *v as i64,
        ScalarValue::Byte(v) => *v as i64,
        ScalarValue::UByte(v) => *v as i64,
        ScalarValue::UShort(v) => *v as i64,
        ScalarValue::UInt(v) => *v as i64,
        ScalarValue::ULong(v) => *v as i64,
        ScalarValue::Double(v) => *v as i64,
        ScalarValue::Float(v) => *v as i64,
        ScalarValue::Boolean(v) => if *v { 1 } else { 0 },
        ScalarValue::String(s) => s.parse().unwrap_or(0),
    }
}

/// Resolve an enum string to its index using a list of choice strings.
///
/// Corresponds to C++ dbf_copy.cpp enum string → index reverse lookup.
/// Returns None if the string doesn't match any choice.
pub fn enum_string_to_index(choices: &[String], name: &str) -> Option<u16> {
    choices
        .iter()
        .position(|s| s == name)
        .map(|i| i as u16)
}

/// Convert an enum index to its string representation.
pub fn enum_index_to_string(choices: &[String], index: u16) -> String {
    choices
        .get(index as usize)
        .cloned()
        .unwrap_or_else(|| format!("{index}"))
}

/// Convert EpicsValue to PvField (scalar or array).
pub fn epics_to_pv_field(val: &EpicsValue) -> PvField {
    match val {
        EpicsValue::ShortArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::Short(*v)).collect())
        }
        EpicsValue::FloatArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::Float(*v)).collect())
        }
        EpicsValue::EnumArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::UShort(*v)).collect())
        }
        EpicsValue::DoubleArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::Double(*v)).collect())
        }
        EpicsValue::LongArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::Int(*v)).collect())
        }
        EpicsValue::CharArray(a) => {
            PvField::ScalarArray(a.iter().map(|v| ScalarValue::UByte(*v)).collect())
        }
        other => PvField::Scalar(epics_to_scalar(other)),
    }
}

/// Extract EpicsValue from a PvField.
pub fn pv_field_to_epics(field: &PvField) -> Option<EpicsValue> {
    match field {
        PvField::Scalar(sv) => Some(scalar_to_epics(sv)),
        PvField::ScalarArray(arr) => {
            if arr.is_empty() {
                return Some(EpicsValue::DoubleArray(vec![]));
            }
            match &arr[0] {
                ScalarValue::Double(_) => Some(EpicsValue::DoubleArray(
                    arr.iter().map(|v| scalar_to_f64(v)).collect(),
                )),
                ScalarValue::Float(_) => Some(EpicsValue::FloatArray(
                    arr.iter().map(|v| scalar_to_f64(v) as f32).collect(),
                )),
                ScalarValue::Short(_) => Some(EpicsValue::ShortArray(
                    arr.iter().map(|v| scalar_to_i64(v) as i16).collect(),
                )),
                ScalarValue::Int(_) => Some(EpicsValue::LongArray(
                    arr.iter().map(|v| scalar_to_i64(v) as i32).collect(),
                )),
                ScalarValue::UByte(_) => Some(EpicsValue::CharArray(
                    arr.iter().map(|v| scalar_to_i64(v) as u8).collect(),
                )),
                ScalarValue::UShort(_) => Some(EpicsValue::EnumArray(
                    arr.iter().map(|v| scalar_to_i64(v) as u16).collect(),
                )),
                _ => Some(EpicsValue::DoubleArray(
                    arr.iter().map(|v| scalar_to_f64(v)).collect(),
                )),
            }
        }
        PvField::Structure(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_double() {
        let orig = EpicsValue::Double(3.14);
        let sv = epics_to_scalar(&orig);
        let back = scalar_to_epics(&sv);
        assert_eq!(orig, back);
    }

    #[test]
    fn roundtrip_string() {
        let orig = EpicsValue::String("hello".into());
        let sv = epics_to_scalar(&orig);
        let back = scalar_to_epics(&sv);
        assert_eq!(orig, back);
    }

    #[test]
    fn roundtrip_short() {
        let orig = EpicsValue::Short(42);
        let sv = epics_to_scalar(&orig);
        let back = scalar_to_epics(&sv);
        assert_eq!(orig, back);
    }

    #[test]
    fn roundtrip_enum() {
        let orig = EpicsValue::Enum(3);
        let sv = epics_to_scalar(&orig);
        assert!(matches!(sv, ScalarValue::UShort(3)));
        let back = scalar_to_epics(&sv);
        assert_eq!(orig, back);
    }

    #[test]
    fn double_array_roundtrip() {
        let orig = EpicsValue::DoubleArray(vec![1.0, 2.0, 3.0]);
        let pf = epics_to_pv_field(&orig);
        let back = pv_field_to_epics(&pf).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn dbf_type_mapping() {
        assert_eq!(dbf_to_scalar_type(DbFieldType::Double), ScalarType::Double);
        assert_eq!(dbf_to_scalar_type(DbFieldType::String), ScalarType::String);
        assert_eq!(dbf_to_scalar_type(DbFieldType::Short), ScalarType::Short);
        assert_eq!(dbf_to_scalar_type(DbFieldType::Long), ScalarType::Int);
        assert_eq!(dbf_to_scalar_type(DbFieldType::Char), ScalarType::UByte);
        assert_eq!(dbf_to_scalar_type(DbFieldType::Enum), ScalarType::UShort);
    }

    #[test]
    fn typed_conversion_double() {
        let sv = ScalarValue::Int(42);
        let ev = scalar_to_epics_typed(&sv, DbFieldType::Double);
        assert_eq!(ev, EpicsValue::Double(42.0));
    }

    #[test]
    fn typed_conversion_enum() {
        let sv = ScalarValue::Int(5);
        let ev = scalar_to_epics_typed(&sv, DbFieldType::Enum);
        assert_eq!(ev, EpicsValue::Enum(5));
    }

    #[test]
    fn typed_conversion_string_from_numeric() {
        let sv = ScalarValue::Double(3.14);
        let ev = scalar_to_epics_typed(&sv, DbFieldType::String);
        assert!(matches!(ev, EpicsValue::String(_)));
    }
}
