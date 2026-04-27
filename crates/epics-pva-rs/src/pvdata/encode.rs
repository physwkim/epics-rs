//! pvData wire codec — encodes/decodes [`FieldDesc`] (introspection) and
//! [`PvField`] (values).
//!
//! Source: pvxs `dataencode.cpp` + `data.cpp`. Designed to be byte-exact with
//! `spvirit_codec::spvd_encode`/`spvd_decode` for the subset of shapes our
//! `FieldDesc` covers (scalars, scalar arrays, structures). Unions, variants,
//! and bounded strings are not yet modelled in [`FieldDesc`] and will be
//! added when we port [`crate::nt::NdArray`] et al.

use std::io::Cursor;

use super::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};
use crate::proto::{
    decode_size, decode_string, encode_size_into, encode_string_into, ByteOrder, DecodeError,
    ReadExt, WriteExt,
};

// ── FieldDesc encode ─────────────────────────────────────────────────────

/// Encode a top-level `FieldDesc`. The output starts with a name field
/// (top-level descriptors carry an empty name) followed by the type
/// description; this matches the pvData "field" wire format used by
/// `pvRequest` and operation INIT responses.
pub fn encode_field_desc(name: &str, desc: &FieldDesc, order: ByteOrder, out: &mut Vec<u8>) {
    encode_string_into(name, order, out);
    encode_type_desc(desc, order, out);
}

/// Encode just the type-tag portion (no name) of a `FieldDesc`.
pub fn encode_type_desc(desc: &FieldDesc, order: ByteOrder, out: &mut Vec<u8>) {
    match desc {
        FieldDesc::Scalar(st) => out.put_u8(st.type_code()),
        FieldDesc::ScalarArray(st) => out.put_u8(st.array_type_code()),
        FieldDesc::Structure { struct_id, fields } => {
            out.put_u8(0x80);
            encode_structure_body(struct_id, fields, order, out);
        }
    }
}

fn encode_structure_body(
    struct_id: &str,
    fields: &[(String, FieldDesc)],
    order: ByteOrder,
    out: &mut Vec<u8>,
) {
    encode_string_into(struct_id, order, out);
    encode_size_into(fields.len() as u32, order, out);
    for (name, child) in fields {
        encode_field_desc(name, child, order, out);
    }
}

// ── FieldDesc decode ─────────────────────────────────────────────────────

/// Decode a top-level `FieldDesc` (`name` + type description).
pub fn decode_field_desc(
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<(String, FieldDesc), DecodeError> {
    let name = decode_string(cur, order)?.unwrap_or_default();
    let desc = decode_type_desc(cur, order)?;
    Ok((name, desc))
}

/// Decode just the type-tag portion of a descriptor.
pub fn decode_type_desc(
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<FieldDesc, DecodeError> {
    let tag = cur.get_u8()?;
    match tag {
        0x80 => decode_structure_body(cur, order),
        // Scalar array: high bit 0x08
        b if b & 0xF8 == 0x08 || b & 0xF8 == 0x28 || b & 0xF8 == 0x48 || b == 0x68 => {
            let scalar = ScalarType::from_array_type_code(b)
                .ok_or_else(|| DecodeError(format!("unknown scalar array tag 0x{b:02X}")))?;
            Ok(FieldDesc::ScalarArray(scalar))
        }
        b => {
            let scalar = ScalarType::from_type_code(b)
                .ok_or_else(|| DecodeError(format!("unknown type tag 0x{b:02X}")))?;
            Ok(FieldDesc::Scalar(scalar))
        }
    }
}

fn decode_structure_body(
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<FieldDesc, DecodeError> {
    let struct_id = decode_string(cur, order)?.unwrap_or_default();
    let n = decode_size(cur, order)?
        .ok_or_else(|| DecodeError("structure field count cannot be null".into()))?
        as usize;
    let mut fields = Vec::with_capacity(n);
    for _ in 0..n {
        fields.push(decode_field_desc(cur, order)?);
    }
    Ok(FieldDesc::Structure { struct_id, fields })
}

// ── ScalarValue encode/decode ────────────────────────────────────────────

pub fn encode_scalar_value(v: &ScalarValue, order: ByteOrder, out: &mut Vec<u8>) {
    match v {
        ScalarValue::Boolean(b) => out.put_u8(if *b { 1 } else { 0 }),
        ScalarValue::Byte(x) => out.put_i8(*x),
        ScalarValue::UByte(x) => out.put_u8(*x),
        ScalarValue::Short(x) => out.put_i16(*x, order),
        ScalarValue::UShort(x) => out.put_u16(*x, order),
        ScalarValue::Int(x) => out.put_i32(*x, order),
        ScalarValue::UInt(x) => out.put_u32(*x, order),
        ScalarValue::Long(x) => out.put_i64(*x, order),
        ScalarValue::ULong(x) => out.put_u64(*x, order),
        ScalarValue::Float(x) => out.put_f32(*x, order),
        ScalarValue::Double(x) => out.put_f64(*x, order),
        ScalarValue::String(s) => encode_string_into(s, order, out),
    }
}

pub fn decode_scalar_value(
    st: ScalarType,
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<ScalarValue, DecodeError> {
    Ok(match st {
        ScalarType::Boolean => ScalarValue::Boolean(cur.get_u8()? != 0),
        ScalarType::Byte => ScalarValue::Byte(cur.get_i8()?),
        ScalarType::UByte => ScalarValue::UByte(cur.get_u8()?),
        ScalarType::Short => ScalarValue::Short(cur.get_i16(order)?),
        ScalarType::UShort => ScalarValue::UShort(cur.get_u16(order)?),
        ScalarType::Int => ScalarValue::Int(cur.get_i32(order)?),
        ScalarType::UInt => ScalarValue::UInt(cur.get_u32(order)?),
        ScalarType::Long => ScalarValue::Long(cur.get_i64(order)?),
        ScalarType::ULong => ScalarValue::ULong(cur.get_u64(order)?),
        ScalarType::Float => ScalarValue::Float(cur.get_f32(order)?),
        ScalarType::Double => ScalarValue::Double(cur.get_f64(order)?),
        ScalarType::String => {
            ScalarValue::String(decode_string(cur, order)?.unwrap_or_default())
        }
    })
}

// ── PvField encode/decode (full value, no bitset filter) ────────────────

/// Encode the value bytes for a `PvField` given its descriptor. The two
/// must agree in shape — caller is responsible for that invariant (it's
/// always true when both come from the same `pvdata` source).
pub fn encode_pv_field(value: &PvField, order: ByteOrder, out: &mut Vec<u8>) {
    match value {
        PvField::Scalar(sv) => encode_scalar_value(sv, order, out),
        PvField::ScalarArray(items) => {
            encode_size_into(items.len() as u32, order, out);
            for sv in items {
                encode_scalar_value(sv, order, out);
            }
        }
        PvField::Structure(s) => {
            for (_, child) in &s.fields {
                encode_pv_field(child, order, out);
            }
        }
    }
}

/// Decode a `PvField` matching `desc`.
pub fn decode_pv_field(
    desc: &FieldDesc,
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<PvField, DecodeError> {
    Ok(match desc {
        FieldDesc::Scalar(st) => PvField::Scalar(decode_scalar_value(*st, cur, order)?),
        FieldDesc::ScalarArray(st) => {
            let n = decode_size(cur, order)?
                .ok_or_else(|| DecodeError("scalar array length cannot be null".into()))?
                as usize;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                items.push(decode_scalar_value(*st, cur, order)?);
            }
            PvField::ScalarArray(items)
        }
        FieldDesc::Structure { struct_id, fields } => {
            let mut s = PvStructure::new(struct_id);
            for (name, child_desc) in fields {
                let v = decode_pv_field(child_desc, cur, order)?;
                s.fields.push((name.clone(), v));
            }
            PvField::Structure(s)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nt_scalar_double_desc() -> FieldDesc {
        FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![
                ("value".into(), FieldDesc::Scalar(ScalarType::Double)),
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
            ],
        }
    }

    #[test]
    fn scalar_value_round_trip() {
        for (v, _st) in [
            (ScalarValue::Boolean(true), ScalarType::Boolean),
            (ScalarValue::Int(-12345), ScalarType::Int),
            (ScalarValue::ULong(u64::MAX - 1), ScalarType::ULong),
            (ScalarValue::Double(2.71828), ScalarType::Double),
            (ScalarValue::String("hello".into()), ScalarType::String),
        ] {
            for order in [ByteOrder::Little, ByteOrder::Big] {
                let mut buf = Vec::new();
                encode_scalar_value(&v, order, &mut buf);
                let mut cur = Cursor::new(buf.as_slice());
                let decoded = decode_scalar_value(v.scalar_type(), &mut cur, order).unwrap();
                assert_eq!(decoded, v, "{:?} order={:?}", v, order);
                assert_eq!(cur.remaining(), 0);
            }
        }
    }

    #[test]
    fn field_desc_round_trip() {
        let desc = nt_scalar_double_desc();
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut buf = Vec::new();
            encode_field_desc("", &desc, order, &mut buf);
            let mut cur = Cursor::new(buf.as_slice());
            let (name, dec) = decode_field_desc(&mut cur, order).unwrap();
            assert_eq!(name, "");
            // Structural equality: convert via Display
            assert_eq!(format!("{dec}"), format!("{desc}"));
        }
    }

    #[test]
    fn pv_field_round_trip_through_desc() {
        let desc = nt_scalar_double_desc();
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Double(42.5))));
        let mut alarm = PvStructure::new("alarm_t");
        alarm
            .fields
            .push(("severity".into(), PvField::Scalar(ScalarValue::Int(0))));
        alarm
            .fields
            .push(("status".into(), PvField::Scalar(ScalarValue::Int(0))));
        alarm.fields.push((
            "message".into(),
            PvField::Scalar(ScalarValue::String("OK".into())),
        ));
        s.fields.push(("alarm".into(), PvField::Structure(alarm)));

        let value = PvField::Structure(s);
        let mut buf = Vec::new();
        encode_pv_field(&value, ByteOrder::Little, &mut buf);
        let mut cur = Cursor::new(buf.as_slice());
        let dec = decode_pv_field(&desc, &mut cur, ByteOrder::Little).unwrap();
        assert_eq!(format!("{dec}"), format!("{value}"));
        assert_eq!(cur.remaining(), 0);
    }

    #[test]
    fn scalar_array_round_trip() {
        let desc = FieldDesc::ScalarArray(ScalarType::Int);
        let v = PvField::ScalarArray(vec![
            ScalarValue::Int(1),
            ScalarValue::Int(2),
            ScalarValue::Int(3),
        ]);
        let mut buf = Vec::new();
        encode_pv_field(&v, ByteOrder::Little, &mut buf);
        let mut cur = Cursor::new(buf.as_slice());
        let dec = decode_pv_field(&desc, &mut cur, ByteOrder::Little).unwrap();
        if let PvField::ScalarArray(items) = dec {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], ScalarValue::Int(1));
            assert_eq!(items[2], ScalarValue::Int(3));
        } else {
            panic!("expected ScalarArray");
        }
    }
}
