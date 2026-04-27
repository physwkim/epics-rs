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
