//! Port of pvxs's `test/testudp.cpp` — beacon + search frame parsing.
//!
//! pvxs sends a hand-crafted 46-byte BEACON UDP frame and verifies its
//! UDPManager parses GUID + server addr correctly. Same for SEARCH.
//! We don't have a public UDPManager-equivalent, so we inline the
//! wire-format parsing using `proto::*` primitives. This still
//! exercises the same byte-layout invariants pvxs depends on.

#![cfg(test)]

use std::io::Cursor;

use epics_pva_rs::proto::{
    decode_size, decode_string, ip_from_bytes, ByteOrder, Command, PvaHeader, ReadExt,
};

// ── testBeacon (pvxs testudp.cpp:25) ───────────────────────────────
//
// pvxs's reference frame is the full 46-byte BEACON. Bytes:
//   0xCA, version, flags, CMD_BEACON, length:u32
//   GUID:12, change_count:u4 (ignored), addr:16, port:u16
//   "tcp" string, then optional status

fn build_beacon_bytes(be: bool) -> Vec<u8> {
    let mut msg: Vec<u8> = vec![
        // header (filled below)
        0xCA, 2, 0, Command::Beacon.code(),
        0, 0, 0, 0, // length
        // GUID 0x01..0x0c
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        // unused/ignored: 4 bytes
        0, 0, 0, 0,
        // server addr (IPv4 in IPv6-mapped form ::ffff:0.0.0.0)
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 0, 0,
        // port placeholder
        0, 0,
        // protocol "tcp"
        3, b't', b'c', b'p',
    ];
    let total = msg.len();
    let payload_len = total - 8;
    if be {
        msg[2] |= 0x80; // BE flag
        msg[4] = ((payload_len >> 24) & 0xFF) as u8;
        msg[5] = ((payload_len >> 16) & 0xFF) as u8;
        msg[6] = ((payload_len >> 8) & 0xFF) as u8;
        msg[7] = (payload_len & 0xFF) as u8;
        msg[40] = 0x12;
        msg[41] = 0x34;
    } else {
        msg[4] = (payload_len & 0xFF) as u8;
        msg[5] = ((payload_len >> 8) & 0xFF) as u8;
        msg[6] = ((payload_len >> 16) & 0xFF) as u8;
        msg[7] = ((payload_len >> 24) & 0xFF) as u8;
        msg[40] = 0x34;
        msg[41] = 0x12;
    }
    msg
}

fn parse_beacon_bytes(frame: &[u8]) -> Option<([u8; 12], std::net::SocketAddr)> {
    let mut cur = Cursor::new(frame);
    let header = PvaHeader::decode(&mut cur).ok()?;
    if header.command != Command::Beacon.code() {
        return None;
    }
    let order = header.flags.byte_order();

    let guid_bytes = cur.get_bytes(12).ok()?;
    let mut guid = [0u8; 12];
    guid.copy_from_slice(&guid_bytes);

    // pvxs udp_collector.cpp skips 4 bytes here (flags+seq+change).
    cur.get_u8().ok()?; // flags
    cur.get_u8().ok()?; // seq (u8 in pvxs)
    cur.get_u16(order).ok()?; // change

    let addr_bytes = cur.get_bytes(16).ok()?;
    let mut addr_arr = [0u8; 16];
    addr_arr.copy_from_slice(&addr_bytes);
    let ip = ip_from_bytes(&addr_arr)?;
    let port = cur.get_u16(order).ok()?;

    let _proto = decode_string(&mut cur, order).ok()?;
    Some((guid, std::net::SocketAddr::new(ip, port)))
}

#[test]
fn pvxs_beacon_le_round_trip() {
    let frame = build_beacon_bytes(false);
    let (guid, server) = parse_beacon_bytes(&frame).expect("parse beacon");
    let expect_guid = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    assert_eq!(guid, expect_guid);
    assert_eq!(server.port(), 0x1234);
}

#[test]
fn pvxs_beacon_be_round_trip() {
    let frame = build_beacon_bytes(true);
    let (guid, server) = parse_beacon_bytes(&frame).expect("parse beacon");
    let expect_guid = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    assert_eq!(guid, expect_guid);
    assert_eq!(server.port(), 0x1234);
}

#[test]
fn pvxs_beacon_truncated_returns_none() {
    let frame = build_beacon_bytes(false);
    let truncated = &frame[..frame.len() - 2];
    assert!(parse_beacon_bytes(truncated).is_none());
}

// ── testSearch (pvxs testudp.cpp:86) ───────────────────────────────
//
// pvxs builds a SEARCH frame referencing N name lookups, sends it,
// and verifies the names come back in order with sequential IDs 1..N.
// We do the same byte-exact construction and parse the resulting
// frame.

fn build_search_bytes(be: bool, names: &[&str]) -> Vec<u8> {
    use epics_pva_rs::proto::{encode_size_into, encode_string_into, WriteExt};

    let order = if be { ByteOrder::Big } else { ByteOrder::Little };

    let mut payload: Vec<u8> = Vec::new();
    payload.put_u32(0x12345678, order); // search seq
    payload.put_u8(0x80); // unicast flag (matches pvxs pva_search_flags::Unicast)
    payload.put_u8(0); // reserved
    payload.put_u8(0);
    payload.put_u8(0);
    // reply addr (16 bytes IPv6-mapped 0.0.0.0) + port 0x1020
    // IPv4 0.0.0.0 in IPv6-mapped form: 10 zeros + 0xff 0xff + 4 zeros
    payload.extend_from_slice(&[0u8; 10]);
    payload.extend_from_slice(&[0xff, 0xff]);
    payload.extend_from_slice(&[0u8; 4]);
    payload.put_u16(0x1020, order);
    // 1 protocol "tcp"
    encode_size_into(1, order, &mut payload);
    encode_string_into("tcp", order, &mut payload);
    // N names
    payload.put_u16(names.len() as u16, order);
    for (i, name) in names.iter().enumerate() {
        payload.put_u32((i + 1) as u32, order);
        encode_string_into(name, order, &mut payload);
    }

    let header = PvaHeader::application(true, order, Command::Search.code(), payload.len() as u32);
    let mut out = Vec::new();
    header.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

fn parse_search_names(frame: &[u8]) -> Option<Vec<(u32, String)>> {
    let mut cur = Cursor::new(frame);
    let header = PvaHeader::decode(&mut cur).ok()?;
    if header.command != Command::Search.code() {
        return None;
    }
    let order = header.flags.byte_order();

    cur.get_u32(order).ok()?; // seq
    cur.get_u8().ok()?; // flags
    cur.get_u8().ok()?;
    cur.get_u8().ok()?;
    cur.get_u8().ok()?;
    cur.get_bytes(16).ok()?; // reply addr
    cur.get_u16(order).ok()?; // reply port
    let n_proto = decode_size(&mut cur, order).ok()??;
    for _ in 0..n_proto {
        decode_string(&mut cur, order).ok()??;
    }
    let n_names = cur.get_u16(order).ok()? as usize;
    let mut out = Vec::with_capacity(n_names);
    for _ in 0..n_names {
        let id = cur.get_u32(order).ok()?;
        let name = decode_string(&mut cur, order).ok()??;
        out.push((id, name));
    }
    Some(out)
}

#[test]
fn pvxs_search_le_with_one_name() {
    let frame = build_search_bytes(false, &["pv:one"]);
    let names = parse_search_names(&frame).expect("parse search");
    assert_eq!(names, vec![(1u32, "pv:one".to_string())]);
}

#[test]
fn pvxs_search_be_with_one_name() {
    let frame = build_search_bytes(true, &["pv:one"]);
    let names = parse_search_names(&frame).expect("parse search");
    assert_eq!(names, vec![(1u32, "pv:one".to_string())]);
}

#[test]
fn pvxs_search_three_names_get_sequential_ids() {
    let frame = build_search_bytes(false, &["a", "b", "c"]);
    let names = parse_search_names(&frame).expect("parse search");
    assert_eq!(
        names,
        vec![
            (1u32, "a".to_string()),
            (2u32, "b".to_string()),
            (3u32, "c".to_string()),
        ]
    );
}
