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

// ── testRegressBadBitMask (pvxs testxcode.cpp:1165) ────────────────
//
// pvxs `from_wire(buf, BitMask)` rejects the null marker (0xff): a
// bitset can be empty but not null. Our `BitSet::decode` does the same.

#[test]
fn pvxs_regress_bad_bitmask_null_marker_rejected() {
    let bytes: &[u8] = b"\xff";
    let mut cur = Cursor::new(bytes);
    let err = BitSet::decode(&mut cur, ByteOrder::Little).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("null") || msg.contains("size"),
        "expected null-related error, got {msg}"
    );
}

#[test]
fn pvxs_regress_bad_bitmask_zero_size_is_empty() {
    // pvxs: empty bitmask is valid (size=0)
    let bytes: &[u8] = b"\x00";
    let mut cur = Cursor::new(bytes);
    let bs = BitSet::decode(&mut cur, ByteOrder::Little).unwrap();
    assert!(bs.is_empty());
}

// ── testBadFieldName (pvxs testxcode.cpp:961) ──────────────────────
//
// pvxs round-trips a struct whose field name contains an invalid
// identifier character. The wire format permits this; only field
// access by name is restricted. We just verify wire round-trip.

#[test]
fn pvxs_bad_field_name_round_trips() {
    // 0x80 (struct) + size(0) (empty struct_id) + size(1) (1 field)
    // + size(8) "in-valid" + 0x26 (UInt scalar)
    let wire: &[u8] = b"\x80\x00\x01\x08in-valid\x26";
    let mut cur = Cursor::new(wire);
    let desc = epics_pva_rs::pvdata::encode::decode_type_desc(&mut cur, ByteOrder::Big)
        .expect("decode");
    if let FieldDesc::Structure {
        ref struct_id,
        ref fields,
    } = desc
    {
        assert_eq!(struct_id, "");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "in-valid");
        assert!(matches!(fields[0].1, FieldDesc::Scalar(ScalarType::UInt)));
    } else {
        panic!("expected struct, got {desc:?}");
    }
    let mut re = Vec::new();
    encode_type_desc(&desc, ByteOrder::Big, &mut re);
    assert_eq!(re.as_slice(), wire);
}

// ── testPartialXCode (pvxs testxcode.cpp:1218, time_t-only subset) ─
//
// pvxs encodes a standalone time_t with `secondsPastEpoch` set.
// Wire (re-rooted at time_t): bitset bit 1 → "\x01\x02" + 8 BE bytes.

fn time_t_desc() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "time_t".into(),
        fields: vec![
            ("secondsPastEpoch".into(), FieldDesc::Scalar(ScalarType::Long)),
            ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
            ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
        ],
    }
}

fn time_t_value(seconds: i64) -> PvField {
    let mut s = PvStructure::new("time_t");
    s.fields.push((
        "secondsPastEpoch".into(),
        PvField::Scalar(ScalarValue::Long(seconds)),
    ));
    s.fields.push((
        "nanoseconds".into(),
        PvField::Scalar(ScalarValue::Int(0)),
    ));
    s.fields.push((
        "userTag".into(),
        PvField::Scalar(ScalarValue::Int(0)),
    ));
    PvField::Structure(s)
}

#[test]
fn pvxs_partial_xcode_time_t_descriptor_only() {
    // pvxs: to_wire(desc(time)) =
    //   "\x80\x06time_t\x03\x10secondsPastEpoch\x23\x0bnanoseconds\x22\x07userTag\x22"
    let desc = time_t_desc();
    let mut buf = Vec::new();
    encode_type_desc(&desc, ByteOrder::Big, &mut buf);
    let expected: &[u8] =
        b"\x80\x06time_t\x03\x10secondsPastEpoch\x23\x0bnanoseconds\x22\x07userTag\x22";
    assert_eq!(buf, expected);
}

#[test]
fn pvxs_partial_xcode_time_t_full_value() {
    // pvxs: to_wire_full(time, secondsPastEpoch=0x10203040) = 16 bytes
    //   long 0x10203040 BE + int 0 BE + int 0 BE
    let desc = time_t_desc();
    let value = time_t_value(0x10203040);
    let mut buf = Vec::new();
    encode_pv_field(&value, &desc, ByteOrder::Big, &mut buf);
    let expected: &[u8] =
        b"\x00\x00\x00\x00\x10\x20\x30\x40\x00\x00\x00\x00\x00\x00\x00\x00";
    assert_eq!(buf, expected);
}

#[test]
fn pvxs_partial_xcode_time_t_valid_only_seconds() {
    // pvxs (time_t-rooted): to_wire_valid with bit 1 set
    //   = "\x01\x02" + 8 BE bytes
    let desc = time_t_desc();
    let value = time_t_value(0x10203040);
    let mut bitset = BitSet::new();
    bitset.set(1); // secondsPastEpoch
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);
    let expected: &[u8] = b"\x01\x02\x00\x00\x00\x00\x10\x20\x30\x40";
    assert_eq!(buf, expected);
}

#[test]
fn pvxs_partial_xcode_time_t_valid_round_trip_decode() {
    // pvxs: from_wire_valid back into time decodes seconds=0x10203050
    let desc = time_t_desc();
    let wire: &[u8] = b"\x01\x02\x00\x00\x00\x00\x10\x20\x30\x50";
    let mut cur = Cursor::new(wire);
    let bs = BitSet::decode(&mut cur, ByteOrder::Big).unwrap();
    assert!(bs.get(1));
    let v = epics_pva_rs::pvdata::encode::decode_pv_field_with_bitset(
        &desc,
        &bs,
        0,
        &mut cur,
        ByteOrder::Big,
    )
    .unwrap();
    if let PvField::Structure(s) = v {
        match s.get_field("secondsPastEpoch") {
            Some(PvField::Scalar(ScalarValue::Long(n))) => assert_eq!(*n, 0x10203050),
            other => panic!("seconds: {other:?}"),
        }
    } else {
        panic!("expected struct");
    }
}

// ── testArrayXCode (pvxs testxcode.cpp:594) ────────────────────────
//
// `Struct { value: arrayOf<E> }` round-trip. The wire is bitset-driven
// (BE): bit 1 ("value") set → bitset = "\x01\x02" then array Size + raw
// elements. Empty array → just the size byte (=0).

fn struct_with_array_value(scalar_type: ScalarType) -> FieldDesc {
    FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![("value".into(), FieldDesc::ScalarArray(scalar_type))],
    }
}

fn struct_with_array_value_filled(items: Vec<ScalarValue>) -> PvField {
    let mut s = PvStructure::new("");
    s.fields
        .push(("value".into(), PvField::ScalarArray(items)));
    PvField::Structure(s)
}

fn encode_array_valid(scalar_type: ScalarType, items: Vec<ScalarValue>) -> Vec<u8> {
    let desc = struct_with_array_value(scalar_type);
    let value = struct_with_array_value_filled(items);
    let mut bitset = BitSet::new();
    bitset.set(1); // "value"
    let mut buf = Vec::new();
    bitset.write_into(ByteOrder::Big, &mut buf);
    encode_pv_field_with_bitset(&value, &desc, &bitset, 0, ByteOrder::Big, &mut buf);
    buf
}

fn decode_array_valid(scalar_type: ScalarType, wire: &[u8]) -> Vec<ScalarValue> {
    let desc = struct_with_array_value(scalar_type);
    let mut cur = Cursor::new(wire);
    let bs = BitSet::decode(&mut cur, ByteOrder::Big).unwrap();
    let v = epics_pva_rs::pvdata::encode::decode_pv_field_with_bitset(
        &desc,
        &bs,
        0,
        &mut cur,
        ByteOrder::Big,
    )
    .unwrap();
    let s = match v {
        PvField::Structure(s) => s,
        other => panic!("expected struct, got {other:?}"),
    };
    match s.get_field("value") {
        Some(PvField::ScalarArray(items)) => items.clone(),
        other => panic!("expected scalar array, got {other:?}"),
    }
}

#[test]
fn pvxs_array_uint32_empty() {
    // pvxs: testArrayXCodeT<uint32_t>("\x01\x02\x00", {});
    let wire = encode_array_valid(ScalarType::UInt, vec![]);
    assert_eq!(wire, b"\x01\x02\x00");
    assert_eq!(decode_array_valid(ScalarType::UInt, &wire), Vec::<ScalarValue>::new());
}

#[test]
fn pvxs_array_uint32_single() {
    // pvxs: "\x01\x02\x01\x12\x34\x56\x78", {0x12345678}
    let wire = encode_array_valid(
        ScalarType::UInt,
        vec![ScalarValue::UInt(0x12345678)],
    );
    assert_eq!(wire, b"\x01\x02\x01\x12\x34\x56\x78");
    let decoded = decode_array_valid(ScalarType::UInt, &wire);
    assert_eq!(decoded, vec![ScalarValue::UInt(0x12345678)]);
}

#[test]
fn pvxs_array_uint16_two_values() {
    // pvxs: testArrayXCodeT<uint16_t>("\x01\x02\x02\x00\x01\xff\xff", {1u, 0xffff});
    let wire = encode_array_valid(
        ScalarType::UShort,
        vec![ScalarValue::UShort(1), ScalarValue::UShort(0xffff)],
    );
    assert_eq!(wire, b"\x01\x02\x02\x00\x01\xff\xff");
    let decoded = decode_array_valid(ScalarType::UShort, &wire);
    assert_eq!(
        decoded,
        vec![ScalarValue::UShort(1), ScalarValue::UShort(0xffff)]
    );
}

#[test]
fn pvxs_array_double_one_point_zero() {
    // pvxs: testArrayXCodeT<double>("\x01\x02\x01?\xf0\x00\x00\x00\x00\x00\x00", {1.0});
    let wire = encode_array_valid(
        ScalarType::Double,
        vec![ScalarValue::Double(1.0)],
    );
    let mut expected = Vec::new();
    expected.extend_from_slice(b"\x01\x02\x01");
    expected.extend_from_slice(&1.0_f64.to_be_bytes());
    assert_eq!(wire, expected);
    let decoded = decode_array_valid(ScalarType::Double, &wire);
    assert_eq!(decoded, vec![ScalarValue::Double(1.0)]);
}

#[test]
fn pvxs_array_string_two_values() {
    // pvxs: "\x01\x02\x02\x05hello\x05world", {"hello", "world"}
    let wire = encode_array_valid(
        ScalarType::String,
        vec![
            ScalarValue::String("hello".into()),
            ScalarValue::String("world".into()),
        ],
    );
    assert_eq!(wire, b"\x01\x02\x02\x05hello\x05world");
    let decoded = decode_array_valid(ScalarType::String, &wire);
    assert_eq!(
        decoded,
        vec![
            ScalarValue::String("hello".into()),
            ScalarValue::String("world".into())
        ]
    );
}

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
