//! pvData wire codec — encodes/decodes [`FieldDesc`] (introspection) and
//! [`PvField`] (values).
//!
//! Source: pvxs `dataencode.cpp` + `data.cpp`. Designed to be byte-exact with
//! `spvirit_codec::spvd_encode`/`spvd_decode` for the shapes our `FieldDesc`
//! covers (scalars, scalar arrays, structures, structure arrays, unions,
//! union arrays, variants, bounded strings).
//!
//! Wire-format type tags (from pvxs `dataencode.cpp`):
//!
//! | tag    | meaning                                  |
//! |--------|------------------------------------------|
//! | 0x00   | boolean                                  |
//! | 0x20-7 | signed/unsigned ints (Byte..ULong)       |
//! | 0x42-3 | float / double                           |
//! | 0x60   | string                                   |
//! | 0x80   | structure (followed by descriptor body)  |
//! | 0x81   | union     (followed by descriptor body)  |
//! | 0x82   | variant ("any")                          |
//! | 0x83   | bounded string + size word               |
//! | 0x88   | structure array (followed by 0x80 + body)|
//! | 0x89   | union array     (followed by 0x81 + body)|
//! | 0x8A   | variant array                            |
//! | scalar | tag (above) | 0x08 → scalar array        |

use std::io::Cursor;

use super::{
    FieldDesc, PvField, PvStructure, ScalarType, ScalarValue, UnionItem, VariantValue,
};
use crate::proto::{
    decode_size, decode_string, encode_size_into, encode_string_into, ByteOrder, DecodeError,
    ReadExt, WriteExt,
};

const TAG_STRUCTURE: u8 = 0x80;
const TAG_UNION: u8 = 0x81;
const TAG_VARIANT: u8 = 0x82;
const TAG_BOUNDED_STRING: u8 = 0x83;
const TAG_STRUCTURE_ARRAY: u8 = 0x88;
const TAG_UNION_ARRAY: u8 = 0x89;
const TAG_VARIANT_ARRAY: u8 = 0x8A;

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
            out.put_u8(TAG_STRUCTURE);
            encode_structure_body(struct_id, fields, order, out);
        }
        FieldDesc::StructureArray { struct_id, fields } => {
            out.put_u8(TAG_STRUCTURE_ARRAY);
            out.put_u8(TAG_STRUCTURE);
            encode_structure_body(struct_id, fields, order, out);
        }
        FieldDesc::Union {
            struct_id,
            variants,
        } => {
            out.put_u8(TAG_UNION);
            encode_structure_body(struct_id, variants, order, out);
        }
        FieldDesc::UnionArray {
            struct_id,
            variants,
        } => {
            out.put_u8(TAG_UNION_ARRAY);
            out.put_u8(TAG_UNION);
            encode_structure_body(struct_id, variants, order, out);
        }
        FieldDesc::Variant => out.put_u8(TAG_VARIANT),
        FieldDesc::VariantArray => out.put_u8(TAG_VARIANT_ARRAY),
        FieldDesc::BoundedString(bound) => {
            out.put_u8(TAG_BOUNDED_STRING);
            encode_size_into(*bound, order, out);
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
        TAG_STRUCTURE => decode_structure_body(cur, order, false),
        TAG_UNION => decode_union_body(cur, order, false),
        TAG_STRUCTURE_ARRAY => {
            let inner = cur.get_u8()?;
            if inner != TAG_STRUCTURE {
                return Err(DecodeError(format!(
                    "structure-array element tag 0x{inner:02X} (expected 0x80)"
                )));
            }
            decode_structure_body(cur, order, true)
        }
        TAG_UNION_ARRAY => {
            let inner = cur.get_u8()?;
            if inner != TAG_UNION {
                return Err(DecodeError(format!(
                    "union-array element tag 0x{inner:02X} (expected 0x81)"
                )));
            }
            decode_union_body(cur, order, true)
        }
        TAG_VARIANT => Ok(FieldDesc::Variant),
        TAG_VARIANT_ARRAY => Ok(FieldDesc::VariantArray),
        TAG_BOUNDED_STRING => {
            let bound = decode_size(cur, order)?
                .ok_or_else(|| DecodeError("bounded string size cannot be null".into()))?;
            Ok(FieldDesc::BoundedString(bound))
        }
        b if b & 0x08 != 0 => {
            // Scalar array — top bit zone for scalar codes (`0x20..0x60`)
            // OR'd with `0x08`. `0x80+` codes are handled above.
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
    is_array: bool,
) -> Result<FieldDesc, DecodeError> {
    let struct_id = decode_string(cur, order)?.unwrap_or_default();
    let n = decode_size(cur, order)?
        .ok_or_else(|| DecodeError("structure field count cannot be null".into()))?
        as usize;
    let mut fields = Vec::with_capacity(n);
    for _ in 0..n {
        fields.push(decode_field_desc(cur, order)?);
    }
    Ok(if is_array {
        FieldDesc::StructureArray { struct_id, fields }
    } else {
        FieldDesc::Structure { struct_id, fields }
    })
}

fn decode_union_body(
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
    is_array: bool,
) -> Result<FieldDesc, DecodeError> {
    let struct_id = decode_string(cur, order)?.unwrap_or_default();
    let n = decode_size(cur, order)?
        .ok_or_else(|| DecodeError("union variant count cannot be null".into()))?
        as usize;
    let mut variants = Vec::with_capacity(n);
    for _ in 0..n {
        variants.push(decode_field_desc(cur, order)?);
    }
    Ok(if is_array {
        FieldDesc::UnionArray {
            struct_id,
            variants,
        }
    } else {
        FieldDesc::Union {
            struct_id,
            variants,
        }
    })
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

// ── PvField encode/decode (full value; descriptor-driven) ────────────────

/// Encode the value bytes for a `PvField` given its descriptor.
pub fn encode_pv_field(value: &PvField, desc: &FieldDesc, order: ByteOrder, out: &mut Vec<u8>) {
    match (desc, value) {
        (FieldDesc::Scalar(_), PvField::Scalar(sv)) => encode_scalar_value(sv, order, out),
        (FieldDesc::ScalarArray(_), PvField::ScalarArray(items)) => {
            encode_size_into(items.len() as u32, order, out);
            for sv in items {
                encode_scalar_value(sv, order, out);
            }
        }
        (FieldDesc::Structure { fields, .. }, PvField::Structure(s)) => {
            for (name, child_desc) in fields {
                let child_val = s
                    .get_field(name)
                    .cloned()
                    .unwrap_or_else(|| default_value_for(child_desc));
                encode_pv_field(&child_val, child_desc, order, out);
            }
        }
        (FieldDesc::StructureArray { fields, .. }, PvField::StructureArray(items)) => {
            encode_size_into(items.len() as u32, order, out);
            // Each element is preceded by a presence byte: 0x01 = element
            // follows, 0x00 = null. We always emit non-null.
            for s in items {
                out.put_u8(0x01);
                let element_desc = FieldDesc::Structure {
                    struct_id: s.struct_id.clone(),
                    fields: fields.clone(),
                };
                encode_pv_field(&PvField::Structure(s.clone()), &element_desc, order, out);
            }
        }
        (FieldDesc::Union { variants, .. }, PvField::Union { selector, value, .. }) => {
            // Union: selector (Size) followed by the chosen variant's value.
            // -1 → null marker (0xFF).
            if *selector < 0 {
                out.put_u8(0xFF);
            } else {
                encode_size_into(*selector as u32, order, out);
                if let Some((_, vdesc)) = variants.get(*selector as usize) {
                    encode_pv_field(value, vdesc, order, out);
                }
            }
        }
        (FieldDesc::UnionArray { variants, .. }, PvField::UnionArray(items)) => {
            encode_size_into(items.len() as u32, order, out);
            for it in items {
                if it.selector < 0 {
                    out.put_u8(0xFF);
                } else {
                    encode_size_into(it.selector as u32, order, out);
                    if let Some((_, vdesc)) = variants.get(it.selector as usize) {
                        encode_pv_field(&it.value, vdesc, order, out);
                    }
                }
            }
        }
        (FieldDesc::Variant, PvField::Variant(v)) => match &v.desc {
            None => out.put_u8(0xFF),
            Some(d) => {
                encode_type_desc(d, order, out);
                encode_pv_field(&v.value, d, order, out);
            }
        },
        (FieldDesc::VariantArray, PvField::VariantArray(items)) => {
            encode_size_into(items.len() as u32, order, out);
            for it in items {
                match &it.desc {
                    None => out.put_u8(0xFF),
                    Some(d) => {
                        encode_type_desc(d, order, out);
                        encode_pv_field(&it.value, d, order, out);
                    }
                }
            }
        }
        (FieldDesc::BoundedString(_), PvField::Scalar(ScalarValue::String(s))) => {
            encode_string_into(s, order, out);
        }
        // Fallback: write zero bytes for "missing" / mismatched values. Real
        // callers should ensure value/desc match; this just keeps encoding
        // total when they don't.
        _ => {}
    }
}

/// Decode a `PvField` matching `desc`, consulting `bitset` to know which
/// fields are actually present on the wire.
///
/// pvxs / pvData encode only the fields whose bit (or whose descendants'
/// bits) is set; the rest are omitted entirely. Callers that want
/// "everything is present" semantics can pass a fully-set bitset of size
/// `desc.total_bits()`.
///
/// `bit_offset` is the bit position assigned to `desc` in the parent's
/// bitset numbering scheme — pvData spec §5.4 depth-first.
pub fn decode_pv_field_with_bitset(
    desc: &FieldDesc,
    bitset: &crate::proto::BitSet,
    bit_offset: usize,
    cur: &mut Cursor<&[u8]>,
    order: ByteOrder,
) -> Result<PvField, DecodeError> {
    // Helper: true iff the bit at `pos` is set, OR any descendant of
    // a structure starting at `pos` (with `desc_local`) is set.
    fn any_descendant_set(
        bitset: &crate::proto::BitSet,
        pos: usize,
        desc_local: &FieldDesc,
    ) -> bool {
        let total = desc_local.total_bits();
        for i in 0..total {
            if bitset.get(pos + i) {
                return true;
            }
        }
        false
    }

    // The bit at `bit_offset` represents this descriptor itself.
    // For Structure, recurse into children. For scalar/leaf, decode iff
    // the bit is set.
    match desc {
        FieldDesc::Scalar(_)
        | FieldDesc::ScalarArray(_)
        | FieldDesc::Variant
        | FieldDesc::VariantArray
        | FieldDesc::BoundedString(_)
        | FieldDesc::Union { .. }
        | FieldDesc::UnionArray { .. }
        | FieldDesc::StructureArray { .. } => {
            if bitset.get(bit_offset) {
                decode_pv_field(desc, cur, order)
            } else {
                Ok(default_value_for(desc))
            }
        }
        FieldDesc::Structure { struct_id, fields } => {
            // The root struct is "present" if its own bit OR any descendant
            // is set. If neither, return a default-filled structure.
            if !any_descendant_set(bitset, bit_offset, desc) {
                return Ok(default_value_for(desc));
            }
            let mut s = PvStructure::new(struct_id);
            // First child bit = root bit + 1.
            let mut child_bit = bit_offset + 1;
            for (name, child) in fields {
                let v = decode_pv_field_with_bitset(child, bitset, child_bit, cur, order)?;
                s.fields.push((name.clone(), v));
                child_bit += child.total_bits();
            }
            Ok(PvField::Structure(s))
        }
    }
}

/// Decode a `PvField` matching `desc` — assumes every field is present
/// on the wire (i.e. bitset is "all bits set"). Used for cases like
/// CONNECTION_VALIDATION authnz where there's no bitset.
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
        FieldDesc::StructureArray { struct_id, fields } => {
            let n = decode_size(cur, order)?
                .ok_or_else(|| DecodeError("structure array length cannot be null".into()))?
                as usize;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                // 0x01 = element present, 0x00 = null. Some pvxs builds emit
                // the legacy "always present" form without the byte; we accept
                // both by peeking.
                let presence = cur.get_u8()?;
                if presence == 0x00 || presence == 0xFF {
                    out.push(PvStructure::new(struct_id));
                    continue;
                }
                if presence != 0x01 {
                    // Treat as already-consumed first byte of the element body.
                    // Reset cursor back by 1 — only safe because the element
                    // starts with a structure body. This branch should not
                    // normally hit pvxs-generated wire.
                    let pos = cur.position();
                    cur.set_position(pos - 1);
                }
                let element_desc = FieldDesc::Structure {
                    struct_id: struct_id.clone(),
                    fields: fields.clone(),
                };
                if let PvField::Structure(s) = decode_pv_field(&element_desc, cur, order)? {
                    out.push(s);
                }
            }
            PvField::StructureArray(out)
        }
        FieldDesc::Union { variants, .. } => {
            let selector_byte = cur.get_u8()?;
            if selector_byte == 0xFF {
                PvField::Union {
                    selector: -1,
                    variant_name: String::new(),
                    value: Box::new(PvField::Null),
                }
            } else {
                // Push back, then decode as size.
                let pos = cur.position();
                cur.set_position(pos - 1);
                let sel = decode_size(cur, order)?
                    .ok_or_else(|| DecodeError("union selector".into()))?
                    as i32;
                let (variant_name, vdesc) = variants
                    .get(sel as usize)
                    .ok_or_else(|| DecodeError(format!("union selector {sel} out of range")))?
                    .clone();
                let value = decode_pv_field(&vdesc, cur, order)?;
                PvField::Union {
                    selector: sel,
                    variant_name,
                    value: Box::new(value),
                }
            }
        }
        FieldDesc::UnionArray { variants, .. } => {
            let n = decode_size(cur, order)?
                .ok_or_else(|| DecodeError("union array length cannot be null".into()))?
                as usize;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                let sel_byte = cur.get_u8()?;
                if sel_byte == 0xFF {
                    items.push(UnionItem {
                        selector: -1,
                        variant_name: String::new(),
                        value: PvField::Null,
                    });
                    continue;
                }
                let pos = cur.position();
                cur.set_position(pos - 1);
                let sel = decode_size(cur, order)?
                    .ok_or_else(|| DecodeError("union array selector".into()))?
                    as i32;
                let (variant_name, vdesc) = variants
                    .get(sel as usize)
                    .ok_or_else(|| DecodeError(format!("union array selector {sel} out of range")))?
                    .clone();
                let value = decode_pv_field(&vdesc, cur, order)?;
                items.push(UnionItem {
                    selector: sel,
                    variant_name,
                    value,
                });
            }
            PvField::UnionArray(items)
        }
        FieldDesc::Variant => {
            // First byte is the type tag of the carried value, OR 0xFF for null.
            let peek = cur.get_u8()?;
            if peek == 0xFF {
                PvField::Variant(Box::new(VariantValue {
                    desc: None,
                    value: PvField::Null,
                }))
            } else {
                let pos = cur.position();
                cur.set_position(pos - 1);
                let inner = decode_type_desc(cur, order)?;
                let value = decode_pv_field(&inner, cur, order)?;
                PvField::Variant(Box::new(VariantValue {
                    desc: Some(inner),
                    value,
                }))
            }
        }
        FieldDesc::VariantArray => {
            let n = decode_size(cur, order)?
                .ok_or_else(|| DecodeError("variant array length".into()))?
                as usize;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                let peek = cur.get_u8()?;
                if peek == 0xFF {
                    items.push(VariantValue {
                        desc: None,
                        value: PvField::Null,
                    });
                    continue;
                }
                let pos = cur.position();
                cur.set_position(pos - 1);
                let inner = decode_type_desc(cur, order)?;
                let value = decode_pv_field(&inner, cur, order)?;
                items.push(VariantValue {
                    desc: Some(inner),
                    value,
                });
            }
            PvField::VariantArray(items)
        }
        FieldDesc::BoundedString(_) => {
            let s = decode_string(cur, order)?.unwrap_or_default();
            PvField::Scalar(ScalarValue::String(s))
        }
    })
}

/// Default-zero value for a descriptor — used to fill missing fields when a
/// caller-supplied `PvStructure` is sparser than its descriptor.
pub fn default_value_for(desc: &FieldDesc) -> PvField {
    match desc {
        FieldDesc::Scalar(st) => PvField::Scalar(zero_scalar(*st)),
        FieldDesc::ScalarArray(_) => PvField::ScalarArray(Vec::new()),
        FieldDesc::Structure { struct_id, fields } => {
            let mut s = PvStructure::new(struct_id);
            for (name, child) in fields {
                s.fields.push((name.clone(), default_value_for(child)));
            }
            PvField::Structure(s)
        }
        FieldDesc::StructureArray { .. } => PvField::StructureArray(Vec::new()),
        FieldDesc::Union { .. } => PvField::Union {
            selector: -1,
            variant_name: String::new(),
            value: Box::new(PvField::Null),
        },
        FieldDesc::UnionArray { .. } => PvField::UnionArray(Vec::new()),
        FieldDesc::Variant => PvField::Variant(Box::new(VariantValue {
            desc: None,
            value: PvField::Null,
        })),
        FieldDesc::VariantArray => PvField::VariantArray(Vec::new()),
        FieldDesc::BoundedString(_) => PvField::Scalar(ScalarValue::String(String::new())),
    }
}

fn zero_scalar(st: ScalarType) -> ScalarValue {
    match st {
        ScalarType::Boolean => ScalarValue::Boolean(false),
        ScalarType::Byte => ScalarValue::Byte(0),
        ScalarType::Short => ScalarValue::Short(0),
        ScalarType::Int => ScalarValue::Int(0),
        ScalarType::Long => ScalarValue::Long(0),
        ScalarType::UByte => ScalarValue::UByte(0),
        ScalarType::UShort => ScalarValue::UShort(0),
        ScalarType::UInt => ScalarValue::UInt(0),
        ScalarType::ULong => ScalarValue::ULong(0),
        ScalarType::Float => ScalarValue::Float(0.0),
        ScalarType::Double => ScalarValue::Double(0.0),
        ScalarType::String => ScalarValue::String(String::new()),
    }
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
        for v in [
            ScalarValue::Boolean(true),
            ScalarValue::Int(-12345),
            ScalarValue::ULong(u64::MAX - 1),
            ScalarValue::Double(2.71828),
            ScalarValue::String("hello".into()),
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
    fn field_desc_round_trip_structure() {
        let desc = nt_scalar_double_desc();
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut buf = Vec::new();
            encode_field_desc("", &desc, order, &mut buf);
            let mut cur = Cursor::new(buf.as_slice());
            let (name, dec) = decode_field_desc(&mut cur, order).unwrap();
            assert_eq!(name, "");
            assert_eq!(format!("{dec}"), format!("{desc}"));
        }
    }

    #[test]
    fn field_desc_round_trip_union() {
        let desc = FieldDesc::Union {
            struct_id: String::new(),
            variants: vec![
                ("doubleValue".into(), FieldDesc::ScalarArray(ScalarType::Double)),
                ("intValue".into(), FieldDesc::ScalarArray(ScalarType::Int)),
            ],
        };
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut buf = Vec::new();
            encode_type_desc(&desc, order, &mut buf);
            let mut cur = Cursor::new(buf.as_slice());
            let dec = decode_type_desc(&mut cur, order).unwrap();
            assert_eq!(format!("{dec}"), format!("{desc}"));
        }
    }

    #[test]
    fn field_desc_round_trip_structure_array() {
        let desc = FieldDesc::StructureArray {
            struct_id: "epics:nt/NTAttribute:1.0".into(),
            fields: vec![
                ("name".into(), FieldDesc::Scalar(ScalarType::String)),
                ("value".into(), FieldDesc::Variant),
            ],
        };
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut buf = Vec::new();
            encode_type_desc(&desc, order, &mut buf);
            let mut cur = Cursor::new(buf.as_slice());
            let dec = decode_type_desc(&mut cur, order).unwrap();
            assert_eq!(format!("{dec}"), format!("{desc}"));
        }
    }

    #[test]
    fn pv_field_round_trip_through_desc() {
        let desc = nt_scalar_double_desc();
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.set("value", PvField::Scalar(ScalarValue::Double(42.5)));
        let mut alarm = PvStructure::new("alarm_t");
        alarm.set("severity", PvField::Scalar(ScalarValue::Int(0)));
        alarm.set("status", PvField::Scalar(ScalarValue::Int(0)));
        alarm.set(
            "message",
            PvField::Scalar(ScalarValue::String("OK".into())),
        );
        s.set("alarm", PvField::Structure(alarm));

        let value = PvField::Structure(s);
        let mut buf = Vec::new();
        encode_pv_field(&value, &desc, ByteOrder::Little, &mut buf);
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
        encode_pv_field(&v, &desc, ByteOrder::Little, &mut buf);
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

    #[test]
    fn union_round_trip() {
        let desc = FieldDesc::Union {
            struct_id: String::new(),
            variants: vec![
                ("intValue".into(), FieldDesc::Scalar(ScalarType::Int)),
                ("doubleValue".into(), FieldDesc::Scalar(ScalarType::Double)),
            ],
        };
        let v = PvField::Union {
            selector: 1,
            variant_name: "doubleValue".into(),
            value: Box::new(PvField::Scalar(ScalarValue::Double(1.5))),
        };
        let mut buf = Vec::new();
        encode_pv_field(&v, &desc, ByteOrder::Little, &mut buf);
        let mut cur = Cursor::new(buf.as_slice());
        let dec = decode_pv_field(&desc, &mut cur, ByteOrder::Little).unwrap();
        match dec {
            PvField::Union {
                selector,
                variant_name,
                value,
            } => {
                assert_eq!(selector, 1);
                assert_eq!(variant_name, "doubleValue");
                assert_eq!(*value, PvField::Scalar(ScalarValue::Double(1.5)));
            }
            other => panic!("expected union, got {other:?}"),
        }
    }

    #[test]
    fn variant_round_trip() {
        let desc = FieldDesc::Variant;
        let v = PvField::Variant(Box::new(VariantValue {
            desc: Some(FieldDesc::Scalar(ScalarType::Int)),
            value: PvField::Scalar(ScalarValue::Int(42)),
        }));
        let mut buf = Vec::new();
        encode_pv_field(&v, &desc, ByteOrder::Little, &mut buf);
        let mut cur = Cursor::new(buf.as_slice());
        let dec = decode_pv_field(&desc, &mut cur, ByteOrder::Little).unwrap();
        match dec {
            PvField::Variant(vv) => {
                assert!(matches!(vv.value, PvField::Scalar(ScalarValue::Int(42))));
            }
            other => panic!("expected variant, got {other:?}"),
        }
    }
}
