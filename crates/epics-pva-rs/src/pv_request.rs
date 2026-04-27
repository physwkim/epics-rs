//! pvRequest builders.
//!
//! A pvRequest is sent inside an INIT operation to filter which fields the
//! server will return. Wire format: a single `0x80` (structure tag) byte
//! followed by an `encode_structure_desc` body for a structure shaped like
//!
//! ```text
//! structure
//!     structure field
//!         structure value      (empty)
//!         structure alarm      (empty)
//!         structure timeStamp  (empty)
//! ```
//!
//! Empty sub-structures carry no value bytes — only the descriptor — so the
//! caller need not append anything after the body.

use crate::proto::ByteOrder;
use crate::pvdata::encode::encode_type_desc;
use crate::pvdata::FieldDesc;

/// Build a pvRequest selecting `fields` at the top level of "field(...)".
fn build(fields: &[&str], order: ByteOrder) -> Vec<u8> {
    let inner = FieldDesc::Structure {
        struct_id: String::new(),
        fields: fields
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    FieldDesc::Structure {
                        struct_id: String::new(),
                        fields: Vec::new(),
                    },
                )
            })
            .collect(),
    };
    let pv_request = FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![("field".to_string(), inner)],
    };
    // pvRequest wire format begins with the 0x80 type tag (the rest of the
    // structure body follows). encode_type_desc emits both the tag and the
    // body so the result is exactly what the wire expects.
    let mut out = Vec::new();
    encode_type_desc(&pv_request, order, &mut out);
    out
}

/// Build the standard pvRequest: `field(value,alarm,timeStamp)`.
pub fn build_pv_request(big_endian: bool) -> Vec<u8> {
    let order = if big_endian { ByteOrder::Big } else { ByteOrder::Little };
    build(&["value", "alarm", "timeStamp"], order)
}

/// Build a minimal pvRequest for PUT: `field(value)`.
pub fn build_pv_request_value_only(big_endian: bool) -> Vec<u8> {
    let order = if big_endian { ByteOrder::Big } else { ByteOrder::Little };
    build(&["value"], order)
}

/// Build a pvRequest selecting an arbitrary list of top-level fields,
/// equivalent to `field(<f1>,<f2>,...)`.
pub fn build_pv_request_fields(fields: &[&str], big_endian: bool) -> Vec<u8> {
    let order = if big_endian { ByteOrder::Big } else { ByteOrder::Little };
    build(fields, order)
}

/// Convert a pvRequest *structure* (rooted at `request_desc`) into a
/// `BitSet` over the fields of `value_desc`, using pvData spec §5.4
/// depth-first bit numbering. Mirrors pvxs `request2mask`.
///
/// Rules:
/// - The pvRequest has shape `structure { structure field { ... } }`.
///   Each direct child of `field` selects the matching top-level field
///   in `value_desc` and (recursively) its sub-fields named.
/// - An empty `field {}` (no children) selects *every* bit (root + all
///   descendants).
/// - Names in pvRequest that don't exist in `value_desc` are silently
///   skipped, *unless* no field at all matched — in which case
///   `Err(EmptyMask)` is returned.
/// - The root bit (bit 0) is always set when at least one descendant is
///   selected.
pub fn request_to_mask(
    value_desc: &crate::pvdata::FieldDesc,
    request_desc: &crate::pvdata::FieldDesc,
) -> Result<crate::proto::BitSet, RequestMaskError> {
    use crate::pvdata::FieldDesc;
    let mut mask = crate::proto::BitSet::new();

    // Find the top-level "field" sub-structure inside the pvRequest.
    let request_field = match request_desc {
        FieldDesc::Structure { fields, .. } => fields.iter().find(|(n, _)| n == "field"),
        _ => None,
    };
    let request_field = match request_field {
        Some((_, FieldDesc::Structure { fields, .. })) => fields,
        _ => {
            // No `field` sub-structure → select root only.
            mask.set(0);
            return Ok(mask);
        }
    };

    // Empty `field {}` → all fields set.
    if request_field.is_empty() {
        let total = value_desc.total_bits();
        for i in 0..total {
            mask.set(i);
        }
        return Ok(mask);
    }

    // Walk each requested top-level name and recursively select bits.
    let mut any_matched = false;
    if let FieldDesc::Structure { fields, .. } = value_desc {
        let mut child_bit = 1usize;
        for (name, child_desc) in fields {
            if let Some((_, sub_request)) = request_field.iter().find(|(n, _)| n == name) {
                any_matched = true;
                // Mark this field and recurse.
                mark_path(&mut mask, child_bit, child_desc, sub_request);
            }
            child_bit += child_desc.total_bits();
        }
    }

    if !any_matched {
        return Err(RequestMaskError::EmptyMask);
    }
    mask.set(0); // root
    Ok(mask)
}

/// Recursively mark `value_desc`'s bit (at `bit_offset`) plus any
/// requested sub-fields as defined by `sub_request`.
fn mark_path(
    mask: &mut crate::proto::BitSet,
    bit_offset: usize,
    value_desc: &crate::pvdata::FieldDesc,
    sub_request: &crate::pvdata::FieldDesc,
) {
    use crate::pvdata::FieldDesc;
    mask.set(bit_offset);

    // Pick out the named sub-fields requested.
    let sub_fields = match sub_request {
        FieldDesc::Structure { fields, .. } => fields,
        _ => return,
    };
    if sub_fields.is_empty() {
        // Empty {} selects this entire sub-tree.
        let total = value_desc.total_bits();
        for i in 0..total {
            mask.set(bit_offset + i);
        }
        return;
    }

    if let FieldDesc::Structure { fields, .. } = value_desc {
        let mut child_bit = bit_offset + 1;
        for (name, child_desc) in fields {
            if let Some((_, sub2)) = sub_fields.iter().find(|(n, _)| n == name) {
                mark_path(mask, child_bit, child_desc, sub2);
            }
            child_bit += child_desc.total_bits();
        }
    }
}

/// Errors from [`request_to_mask`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RequestMaskError {
    /// The pvRequest selected no existing fields.
    #[error("pvRequest selected no existing fields")]
    EmptyMask,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pv_request_starts_with_structure_tag() {
        let bytes = build_pv_request(false);
        assert_eq!(bytes[0], 0x80);
    }

    #[test]
    fn value_only_request_is_shorter() {
        let full = build_pv_request(false);
        let value_only = build_pv_request_value_only(false);
        assert!(value_only.len() < full.len());
    }
}
