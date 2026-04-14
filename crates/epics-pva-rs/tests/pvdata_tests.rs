//! Tests for epics-pva-rs codec, pv_request, and spvirit-codec integration.

use epics_pva_rs::codec::PvaCodec;
use epics_pva_rs::pv_request::{build_pv_request, build_pv_request_value_only};
use spvirit_codec::PvaHeader;
use spvirit_codec::spvd_decode::{DecodedValue, FieldType, PvdDecoder, TypeCode};

// ---------------------------------------------------------------------------
// PvaCodec: header construction
// ---------------------------------------------------------------------------

#[test]
fn codec_search_produces_valid_header() {
    let codec = PvaCodec::new();
    let pkt = codec.build_search(1, 2, "TEST:PV", [0, 0, 0, 0], 5076, false);
    assert!(pkt.len() >= 8, "search packet too short");
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 3); // CMD_SEARCH
    assert!(!hdr.flags.is_control);
}

#[test]
fn codec_connection_validated_header() {
    let codec = PvaCodec::new();
    let pkt = codec.build_connection_validated();
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 9); // CMD_CONNECTION_VALIDATED
    assert!(!hdr.flags.is_server);
}

#[test]
fn codec_create_channel_header() {
    let codec = PvaCodec::new();
    let pkt = codec.build_create_channel(42, "MY:PV");
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 7); // CMD_CREATE_CHANNEL
}

#[test]
fn codec_get_init_header() {
    let codec = PvaCodec::new();
    let pvr = build_pv_request(false);
    let pkt = codec.build_get_init(1, 2, &pvr);
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 10); // CMD_GET
}

#[test]
fn codec_put_init_header() {
    let codec = PvaCodec::new();
    let pvr = build_pv_request_value_only(false);
    let pkt = codec.build_put_init(1, 2, &pvr);
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 11); // CMD_PUT
}

#[test]
fn codec_monitor_init_header() {
    let codec = PvaCodec::new();
    let pvr = build_pv_request(false);
    let pkt = codec.build_monitor_init(1, 2, &pvr);
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 13); // CMD_MONITOR
}

#[test]
fn codec_get_field_header() {
    let codec = PvaCodec::new();
    let pkt = codec.build_get_field(1, 2, "");
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 17); // CMD_GET_FIELD
}

#[test]
fn codec_destroy_request_header() {
    let codec = PvaCodec::new();
    let pkt = codec.build_destroy_request(1, 2);
    assert!(pkt.len() >= 8);
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.is_valid());
    assert_eq!(hdr.command, 15); // CMD_DESTROY_REQUEST
}

// ---------------------------------------------------------------------------
// PvaCodec: big endian toggling
// ---------------------------------------------------------------------------

#[test]
fn codec_big_endian_flag_in_header() {
    let mut codec = PvaCodec::new();
    codec.big_endian = true;
    let pkt = codec.build_connection_validated();
    let hdr = PvaHeader::new(&pkt[..8]);
    assert!(hdr.flags.is_msb, "expected big-endian flag set");
}

// ---------------------------------------------------------------------------
// pv_request builders
// ---------------------------------------------------------------------------

#[test]
fn pv_request_is_non_empty() {
    let req = build_pv_request(false);
    assert!(!req.is_empty());
    // Should start with 0x80 (structure type tag)
    assert_eq!(req[0], 0x80);
}

#[test]
fn pv_request_value_only_is_non_empty() {
    let req = build_pv_request_value_only(false);
    assert!(!req.is_empty());
    assert_eq!(req[0], 0x80);
}

// ---------------------------------------------------------------------------
// PvdDecoder: simple scalar roundtrips
// ---------------------------------------------------------------------------

#[test]
fn pvd_decode_int32() {
    let decoder = PvdDecoder::new(false);
    let data: [u8; 4] = 42i32.to_le_bytes();
    let ft = FieldType::Scalar(TypeCode::Int32);
    let (val, consumed) = decoder.decode_value(&data, &ft).unwrap();
    assert_eq!(consumed, 4);
    match val {
        DecodedValue::Int32(v) => assert_eq!(v, 42),
        other => panic!("expected Int32, got {other:?}"),
    }
}

#[test]
fn pvd_decode_float64() {
    let decoder = PvdDecoder::new(false);
    let test_val = 3.125_f64; // exact in binary, avoids clippy approx_constant
    let data: [u8; 8] = test_val.to_le_bytes();
    let ft = FieldType::Scalar(TypeCode::Float64);
    let (val, consumed) = decoder.decode_value(&data, &ft).unwrap();
    assert_eq!(consumed, 8);
    match val {
        DecodedValue::Float64(v) => assert!((v - test_val).abs() < 1e-10),
        other => panic!("expected Float64, got {other:?}"),
    }
}

#[test]
fn pvd_decode_boolean() {
    let decoder = PvdDecoder::new(false);
    let ft = FieldType::Scalar(TypeCode::Boolean);
    let (val, consumed) = decoder.decode_value(&[1], &ft).unwrap();
    assert_eq!(consumed, 1);
    match val {
        DecodedValue::Boolean(v) => assert!(v),
        other => panic!("expected Boolean, got {other:?}"),
    }
}

#[test]
fn pvd_decode_string() {
    let decoder = PvdDecoder::new(false);
    let s = "hello";
    let mut data = Vec::new();
    data.push(s.len() as u8); // size byte
    data.extend_from_slice(s.as_bytes());
    let ft = FieldType::String;
    let (val, consumed) = decoder.decode_value(&data, &ft).unwrap();
    assert_eq!(consumed, 6);
    match val {
        DecodedValue::String(v) => assert_eq!(v, "hello"),
        other => panic!("expected String, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// PvdDecoder: big endian
// ---------------------------------------------------------------------------

#[test]
fn pvd_decode_int32_big_endian() {
    let decoder = PvdDecoder::new(true);
    let data: [u8; 4] = 42i32.to_be_bytes();
    let ft = FieldType::Scalar(TypeCode::Int32);
    let (val, consumed) = decoder.decode_value(&data, &ft).unwrap();
    assert_eq!(consumed, 4);
    match val {
        DecodedValue::Int32(v) => assert_eq!(v, 42),
        other => panic!("expected Int32, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// DecodedValue Display
// ---------------------------------------------------------------------------

#[test]
fn decoded_value_display_int() {
    let val = DecodedValue::Int32(42);
    assert_eq!(format!("{val}"), "42");
}

#[test]
fn decoded_value_display_string() {
    let val = DecodedValue::String("hello".to_string());
    assert_eq!(format!("{val}"), "\"hello\"");
}

#[test]
fn decoded_value_display_structure() {
    let val = DecodedValue::Structure(vec![
        ("a".to_string(), DecodedValue::Int32(1)),
        ("b".to_string(), DecodedValue::Float64(2.5)),
    ]);
    let s = format!("{val}");
    assert!(s.contains("a=1"));
    assert!(s.contains("b="));
}

// ---------------------------------------------------------------------------
// Header parsing safety
// ---------------------------------------------------------------------------

#[test]
fn pva_header_valid_magic() {
    let mut buf = [0u8; 8];
    buf[0] = 0xCA; // magic byte 1
    buf[1] = 0x02; // version
    buf[2] = 0x00; // flags
    buf[3] = 0x03; // command (search)
    // payload length = 0
    let hdr = PvaHeader::new(&buf);
    assert!(hdr.is_valid());
}

#[test]
fn pva_header_invalid_magic() {
    let buf = [0u8; 8];
    let hdr = PvaHeader::new(&buf);
    assert!(!hdr.is_valid());
}
