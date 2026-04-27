//! Byte-exact cross-check: native `proto::*` encoders ↔ `spvirit_codec` encoders.
//!
//! This is the Phase 0 baseline. While both implementations exist side-by-side,
//! these tests assert that swapping in our native primitives is a no-op at the
//! byte level — which is what we need before Phase 2 can rip out
//! `spvirit-codec` without breaking on-the-wire compatibility.
//!
//! Once `spvirit-codec` is removed (Phase 6) these tests will be deleted; until
//! then they run as part of `cargo test -p epics-pva-rs`.

use std::net::{IpAddr, Ipv4Addr};

use epics_pva_rs::proto::{
    encode_size, encode_string, ip_to_bytes, BitSet, ByteOrder, PvaHeader, Status, NULL_MARKER,
    PVA_VERSION,
};

#[test]
fn size_matches_spvirit() {
    use spvirit_codec::encode_common::encode_size as spv;
    for v in [0u32, 1, 50, 253, 254, 255, 1024, 0x1_0000, u32::MAX] {
        for be in [false, true] {
            let order = if be { ByteOrder::Big } else { ByteOrder::Little };
            let ours = encode_size(v, order);
            let theirs = spv(v as usize, be);
            assert_eq!(ours, theirs, "size={v}, be={be}");
        }
    }
}

#[test]
fn string_matches_spvirit() {
    use spvirit_codec::encode_common::encode_string as spv;
    for s in ["", "MY:PV", "long".repeat(80).as_str(), "한글", "."] {
        for be in [false, true] {
            let order = if be { ByteOrder::Big } else { ByteOrder::Little };
            let ours = encode_string(s, order);
            let theirs = spv(s, be);
            assert_eq!(ours, theirs, "s={s:?}, be={be}");
        }
    }
}

#[test]
fn ip_to_bytes_matches_spvirit() {
    use spvirit_codec::spvirit_encode::ip_to_bytes as spv;
    let v4 = IpAddr::V4(Ipv4Addr::new(192, 168, 7, 42));
    assert_eq!(ip_to_bytes(v4), spv(v4));
    let unspec = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    assert_eq!(ip_to_bytes(unspec), spv(unspec));
}

#[test]
fn header_matches_spvirit_application() {
    use spvirit_codec::spvirit_encode::encode_header as spv_header;
    // Application message: server=false, control=false, le, cmd=7, len=42
    for (server, be, control, cmd, payload) in [
        (false, false, false, 7u8, 42u32),
        (true, true, false, 4, 0x100),
        (false, false, true, 2, 0xDEADBEEF),
        (true, false, false, 17, 0),
    ] {
        let order = if be { ByteOrder::Big } else { ByteOrder::Little };
        let ours = if control {
            PvaHeader::control(server, order, cmd, payload).encode().to_vec()
        } else {
            PvaHeader::application(server, order, cmd, payload).encode().to_vec()
        };
        let theirs = spv_header(server, be, control, PVA_VERSION, cmd, payload);
        assert_eq!(
            ours, theirs,
            "server={server} be={be} ctrl={control} cmd={cmd} len={payload}"
        );
    }
}

#[test]
fn status_ok_matches_spvirit() {
    // spvirit's encode_status_ok = vec![0xFF]
    assert_eq!(Status::ok().encode(ByteOrder::Little), vec![0xFF]);
    assert_eq!(Status::ok().encode(ByteOrder::Big), vec![0xFF]);
}

#[test]
fn null_marker_constant() {
    assert_eq!(NULL_MARKER, 0xFF);
}

#[test]
fn field_desc_matches_spvirit_structure_desc() {
    // Build the same NTScalar(double) descriptor in both representations
    // and assert byte-exact equality.
    use epics_pva_rs::proto::ByteOrder;
    use epics_pva_rs::pvdata::encode::encode_field_desc as our_encode;
    use epics_pva_rs::pvdata::{FieldDesc, ScalarType};
    use spvirit_codec::spvd_decode::{
        FieldDesc as SpvFieldDesc, FieldType as SpvFieldType, StructureDesc as SpvStructureDesc,
        TypeCode as SpvTypeCode,
    };
    use spvirit_codec::spvd_encode::encode_structure_desc as spv_encode;

    // Our model
    let ours = FieldDesc::Structure {
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
    };

    // Equivalent spvirit model
    let theirs = SpvStructureDesc {
        struct_id: Some("epics:nt/NTScalar:1.0".into()),
        fields: vec![
            SpvFieldDesc {
                name: "value".into(),
                field_type: SpvFieldType::Scalar(SpvTypeCode::Float64),
            },
            SpvFieldDesc {
                name: "alarm".into(),
                field_type: SpvFieldType::Structure(SpvStructureDesc {
                    struct_id: Some("alarm_t".into()),
                    fields: vec![
                        SpvFieldDesc {
                            name: "severity".into(),
                            field_type: SpvFieldType::Scalar(SpvTypeCode::Int32),
                        },
                        SpvFieldDesc {
                            name: "status".into(),
                            field_type: SpvFieldType::Scalar(SpvTypeCode::Int32),
                        },
                        SpvFieldDesc {
                            name: "message".into(),
                            field_type: SpvFieldType::String,
                        },
                    ],
                }),
            },
        ],
    };

    for (be, order) in [(false, ByteOrder::Little), (true, ByteOrder::Big)] {
        // Our encode_field_desc with empty top-level name produces:
        //   <empty-name string> 0x80 <structure body>
        // spvirit's encode_structure_desc produces just:
        //   <structure body>  (no leading name byte, no leading type tag)
        // To compare byte-exact we encode just the structure body via our
        // encode_type_desc and skip the type tag.
        let mut our_buf = Vec::new();
        our_encode("", &ours, order, &mut our_buf);
        // Skip empty-name leading byte (single 0x00) and the 0x80 type tag
        assert_eq!(our_buf[0], 0x00);
        assert_eq!(our_buf[1], 0x80);
        let our_body = &our_buf[2..];

        let their_body = spv_encode(&theirs, be);
        assert_eq!(
            our_body, their_body,
            "FieldDesc body mismatch (be={be})"
        );
    }
}

#[test]
fn codec_search_matches_spvirit() {
    use epics_pva_rs::codec::PvaCodec;
    use spvirit_codec::spvirit_encode::encode_search_request;

    for big_endian in [false, true] {
        let codec = PvaCodec { big_endian };
        let ours = codec.build_search(7, 42, "MY:PV", [192, 168, 1, 7], 5076, false);

        let addr_v4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 7));
        let addr16 = spvirit_codec::spvirit_encode::ip_to_bytes(addr_v4);
        let theirs = encode_search_request(
            7,
            0x00,
            5076,
            addr16,
            &[(42u32, "MY:PV")],
            PVA_VERSION,
            big_endian,
        );
        assert_eq!(ours, theirs, "build_search be={big_endian}");
    }
}

#[test]
fn codec_create_channel_matches_spvirit() {
    use epics_pva_rs::codec::PvaCodec;
    use spvirit_codec::spvirit_encode::encode_create_channel_request;

    for big_endian in [false, true] {
        let codec = PvaCodec { big_endian };
        let ours = codec.build_create_channel(99, "INSTR:PRESSURE");
        let theirs =
            encode_create_channel_request(99, "INSTR:PRESSURE", PVA_VERSION, big_endian);
        assert_eq!(ours, theirs, "build_create_channel be={big_endian}");
    }
}

#[test]
fn codec_op_requests_match_spvirit() {
    use epics_pva_rs::codec::PvaCodec;
    use spvirit_codec::spvirit_encode::{
        encode_get_field_request, encode_get_request, encode_monitor_request, encode_op_request,
        encode_put_request,
    };

    for big_endian in [false, true] {
        let codec = PvaCodec { big_endian };
        let pv_req = epics_pva_rs::pv_request::build_pv_request(big_endian);

        // GET INIT
        assert_eq!(
            codec.build_get_init(11, 22, &pv_req),
            encode_get_request(11, 22, 0x08, &pv_req, PVA_VERSION, big_endian),
        );
        // GET (no extra)
        assert_eq!(
            codec.build_get(11, 22),
            encode_get_request(11, 22, 0x00, &[], PVA_VERSION, big_endian),
        );
        // PUT INIT
        assert_eq!(
            codec.build_put_init(11, 22, &pv_req),
            encode_put_request(11, 22, 0x08, &pv_req, PVA_VERSION, big_endian),
        );
        // MONITOR INIT
        assert_eq!(
            codec.build_monitor_init(11, 22, &pv_req),
            encode_monitor_request(11, 22, 0x08, &pv_req, PVA_VERSION, big_endian),
        );
        // GET_FIELD
        assert_eq!(
            codec.build_get_field(11, 22, "value"),
            encode_get_field_request(11, 22, Some("value"), PVA_VERSION, big_endian),
        );
        // GET_FIELD with empty subfield
        assert_eq!(
            codec.build_get_field(11, 22, ""),
            encode_get_field_request(11, 22, None, PVA_VERSION, big_endian),
        );
        // DESTROY_REQUEST
        assert_eq!(
            codec.build_destroy_request(11, 22),
            encode_op_request(15, 11, 22, 0x00, &[], PVA_VERSION, big_endian),
        );
    }
}

#[test]
fn pv_request_matches_spvirit() {
    use epics_pva_rs::pv_request::{build_pv_request, build_pv_request_value_only};
    use spvirit_codec::spvd_encode::encode_pv_request as spv_pv_req;

    for be in [false, true] {
        assert_eq!(
            build_pv_request(be),
            spv_pv_req(&["value", "alarm", "timeStamp"], be),
            "full pvRequest be={be}",
        );
        assert_eq!(
            build_pv_request_value_only(be),
            spv_pv_req(&["value"], be),
            "value-only pvRequest be={be}",
        );
    }
}

#[test]
fn codec_connection_validated_matches_spvirit() {
    use epics_pva_rs::codec::PvaCodec;
    use spvirit_codec::spvirit_encode::encode_connection_validated;

    for big_endian in [false, true] {
        let codec = PvaCodec { big_endian };
        let ours = codec.build_connection_validated();
        let theirs = encode_connection_validated(false, PVA_VERSION, big_endian);
        assert_eq!(ours, theirs, "build_connection_validated be={big_endian}");
    }
}

#[test]
fn bitset_decodes_spvirit_first_event_payload() {
    // spvirit's encode_structure_bitset(desc) for a 5-field structure
    // (root + 4 nested) produces:
    //   total_bits = 1 + 4 = 5
    //   bitset_size = (5 + 7) / 8 = 1
    //   bitset = [0b0001_1111]
    //   wire = encode_size(1) ++ [0x1F]
    let wire = vec![0x01, 0b0001_1111];
    let mut cur = std::io::Cursor::new(wire.as_slice());
    let bs = BitSet::decode(&mut cur, ByteOrder::Little).unwrap();
    for i in 0..5 {
        assert!(bs.get(i), "bit {i} should be set");
    }
    assert!(!bs.get(5));
}
