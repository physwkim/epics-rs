//! Selected ports of pvxs's `test/testxcode.cpp` to our codec.
//!
//! Focuses on byte-exact wire-format checks: string encoding,
//! `to_wire_valid` (= encode_pv_field_with_bitset) for NTScalar with
//! various bitset patterns, and partial structure decode with bitset.

#![cfg(test)]

use std::io::Cursor;

use epics_pva_rs::proto::{decode_string, encode_string, BitSet, ByteOrder};
use epics_pva_rs::pvdata::encode::{
    encode_pv_field, encode_pv_field_with_bitset, encode_type_desc,
};
use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};

// ── String wire format (testDeserializeString) ─────────────────────

#[test]
fn pvxs_string_size_zero_is_empty() {
    // pvxs: from_wire("\x00") → ""
    let bytes: &[u8] = b"\x00";
    let mut cur = Cursor::new(bytes);
    let s = decode_string(&mut cur, ByteOrder::Big).unwrap();
    assert_eq!(s.as_deref(), Some(""));
}

#[test]
fn pvxs_string_null_marker_decodes_as_none() {
    // pvxs: from_wire("\xff") → "" (their dut wasn't reset; they treat it
    // as null and leave the result empty). Our decoder surfaces null via
    // Option::None.
    let bytes: &[u8] = b"\xff";
    let mut cur = Cursor::new(bytes);
    let s = decode_string(&mut cur, ByteOrder::Big).unwrap();
    assert_eq!(s, None);
}

#[test]
fn pvxs_string_round_trips_short_ascii() {
    let bytes: &[u8] = b"\x0bhello world";
    let mut cur = Cursor::new(bytes);
    let s = decode_string(&mut cur, ByteOrder::Big).unwrap();
    assert_eq!(s.as_deref(), Some("hello world"));
    let re = encode_string("hello world", ByteOrder::Big);
    assert_eq!(re.as_slice(), bytes);
}

// ── Helper: NTScalar(UInt32) descriptor matching pvxs nt::NTScalar ─

fn nt_scalar_uint32_desc() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "epics:nt/NTScalar:1.0".into(),
        fields: vec![
            ("value".into(), FieldDesc::Scalar(ScalarType::UInt)),
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
                        (
                            "secondsPastEpoch".into(),
                            FieldDesc::Scalar(ScalarType::Long),
                        ),
                        ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
                    ],
                },
            ),
        ],
    }
}

fn nt_scalar_uint32_value() -> PvField {
    let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
    s.fields
        .push(("value".into(), PvField::Scalar(ScalarValue::UInt(0))));
    let mut alarm = PvStructure::new("alarm_t");
    alarm
        .fields
        .push(("severity".into(), PvField::Scalar(ScalarValue::Int(0))));
    alarm
        .fields
        .push(("status".into(), PvField::Scalar(ScalarValue::Int(0))));
    alarm.fields.push((
        "message".into(),
        PvField::Scalar(ScalarValue::String(String::new())),
    ));
    s.fields.push(("alarm".into(), PvField::Structure(alarm)));
    let mut ts = PvStructure::new("time_t");
    ts.fields.push((
        "secondsPastEpoch".into(),
        PvField::Scalar(ScalarValue::Long(0)),
    ));
    ts.fields
        .push(("nanoseconds".into(), PvField::Scalar(ScalarValue::Int(0))));
    ts.fields
        .push(("userTag".into(), PvField::Scalar(ScalarValue::Int(0))));
    s.fields
        .push(("timeStamp".into(), PvField::Structure(ts)));
    PvField::Structure(s)
}

// ── testSerialize1: NTScalar(UInt32) wire encoding ─────────────────
//
// Ported from pvxs testxcode.cpp. Each sub-case is its own #[test]
// so failures isolate cleanly.

#[test]
fn pvxs_serialize_full_default_uint32() {
    // pvxs: to_wire_full(default NTScalar UInt32) = 29 zero bytes
    //   value: 4
    //   alarm.{severity:4, status:4, message:1 (empty string size byte)} = 9
    //   timeStamp.{secondsPastEpoch:8, nanoseconds:4, userTag:4} = 16
    //   total = 4 + 9 + 16 = 29
    let desc = nt_scalar_uint32_desc();
    let value = nt_scalar_uint32_value();
    let mut buf = Vec::new();
    encode_pv_field(&value, &desc, ByteOrder::Big, &mut buf);
    let expected = vec![0u8; 29];
    assert_eq!(buf, expected, "to_wire_full default NTScalar UInt32");
}

#[test]
fn pvxs_serialize_valid_empty_bitset() {
    // pvxs: to_wire_valid(default, empty bitset) = "\x00" (just the size byte)
    let desc = nt_scalar_uint32_desc();
    let value = nt_scalar_uint32_value();
    let bitset = BitSet::new();
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);
    assert_eq!(buf, b"\x00");
}

#[test]
fn pvxs_serialize_valid_value_only() {
    // value = 0xdeadbeef; only "value" bit set → bitset = bit 1
    // Wire: bitset (size=1, byte=0x02) + value u32 BE (4 bytes)
    let desc = nt_scalar_uint32_desc();
    let mut value = nt_scalar_uint32_value();
    if let PvField::Structure(ref mut s) = value {
        for (name, v) in &mut s.fields {
            if name == "value" {
                *v = PvField::Scalar(ScalarValue::UInt(0xdeadbeef));
            }
        }
    }
    let mut bitset = BitSet::new();
    bitset.set(1); // value
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);
    // pvxs expected: \x01\x02 \xde\xad\xbe\xef
    assert_eq!(buf, b"\x01\x02\xde\xad\xbe\xef");
}

#[test]
fn pvxs_serialize_valid_unmarked_value() {
    // After unmarking, bitset is empty → just "\x00" again
    let desc = nt_scalar_uint32_desc();
    let value = nt_scalar_uint32_value();
    let bitset = BitSet::new();
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);
    assert_eq!(buf, b"\x00");
}

#[test]
fn pvxs_serialize_valid_alarm_message_and_nanoseconds() {
    // Set alarm.message = "hello world" and timeStamp.nanoseconds = 0xab.
    // pvxs computes bitset bits 5 (alarm.message) and 8 (timeStamp.nanoseconds).
    // bitset bytes (LSB-first): byte 0 = 0b00100000 = 0x20, byte 1 = 0b00000001 = 0x01
    // Wire: \x02 (size) \x20 \x01 then alarm.message string (\x0bhello world)
    //   then timeStamp.nanoseconds u32 BE (\x00\x00\x00\xab)
    let desc = nt_scalar_uint32_desc();
    let mut value = nt_scalar_uint32_value();

    // Set the two scalar fields.
    if let PvField::Structure(ref mut root) = value {
        for (name, v) in &mut root.fields {
            if name == "alarm" {
                if let PvField::Structure(alarm) = v {
                    for (n2, v2) in &mut alarm.fields {
                        if n2 == "message" {
                            *v2 = PvField::Scalar(ScalarValue::String(
                                "hello world".to_string(),
                            ));
                        }
                    }
                }
            }
            if name == "timeStamp" {
                if let PvField::Structure(ts) = v {
                    for (n2, v2) in &mut ts.fields {
                        if n2 == "nanoseconds" {
                            *v2 = PvField::Scalar(ScalarValue::Int(0xab));
                        }
                    }
                }
            }
        }
    }

    let mut bitset = BitSet::new();
    let alarm_msg_bit = desc.bit_for_path("alarm.message").unwrap();
    let ts_ns_bit = desc.bit_for_path("timeStamp.nanoseconds").unwrap();
    assert_eq!(alarm_msg_bit, 5);
    assert_eq!(ts_ns_bit, 8);
    bitset.set(alarm_msg_bit);
    bitset.set(ts_ns_bit);

    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);

    let expected: &[u8] = b"\x02\x20\x01\x0bhello world\x00\x00\x00\xab";
    assert_eq!(buf, expected);
}

// ── Type-descriptor wire round-trip (testSerialize2 small subset) ──

#[test]
fn pvxs_descriptor_nt_scalar_uint32_round_trip() {
    let desc = nt_scalar_uint32_desc();
    let mut buf = Vec::new();
    encode_type_desc(&desc, ByteOrder::Big, &mut buf);

    // Decode back.
    let mut cur = Cursor::new(buf.as_slice());
    let decoded = epics_pva_rs::pvdata::encode::decode_type_desc(&mut cur, ByteOrder::Big)
        .expect("decode");

    // Quick structural identity check via Display.
    assert_eq!(format!("{decoded}"), format!("{desc}"));
}

// ── Bitset-driven partial decode (testRegressBadBitMask + testPartialXCode) ─
//
// pvxs accepts a partial structure where some fields are missing; the
// decoder fills them with defaults. Our `decode_pv_field_with_bitset`
// already does this; verify against a constructed wire stream.

#[test]
fn pvxs_partial_decode_value_only_keeps_alarm_default() {
    let desc = nt_scalar_uint32_desc();

    // Wire with only `value` bit set.
    let mut bitset = BitSet::new();
    bitset.set(1);
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    // value u32 BE = 42
    buf.extend_from_slice(&42u32.to_be_bytes());

    let mut cur = Cursor::new(buf.as_slice());
    let bs = BitSet::decode(&mut cur, ByteOrder::Big).unwrap();
    let v = epics_pva_rs::pvdata::encode::decode_pv_field_with_bitset(
        &desc,
        &bs,
        0,
        &mut cur,
        ByteOrder::Big,
    )
    .unwrap();
    if let PvField::Structure(s) = v {
        match s.get_value() {
            Some(ScalarValue::UInt(n)) => assert_eq!(*n, 42),
            other => panic!("value: {other:?}"),
        }
        // alarm sub-structure must default-initialise.
        if let Some(PvField::Structure(alarm)) = s.get_field("alarm") {
            if let Some(PvField::Scalar(ScalarValue::Int(sev))) =
                alarm.get_field("severity")
            {
                assert_eq!(*sev, 0);
            }
        }
    } else {
        panic!("expected NTScalar structure");
    }
}
