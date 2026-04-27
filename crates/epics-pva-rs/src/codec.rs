//! Application-level PVA message builders.
//!
//! This module is a thin layer over [`crate::proto`] that produces the byte
//! sequences expected by clients (`build_search`, `build_get_init`, ...) and
//! servers (`build_connection_validated`).
//!
//! It is byte-exact compatible with `spvirit_codec::spvirit_encode` for the
//! commands we emit; see `tests/proto_spvirit_parity.rs` for the cross-check.

use std::net::{IpAddr, Ipv4Addr};

use crate::proto::{
    ByteOrder, Command, PvaHeader, QosFlags, Status, WriteExt, encode_size_into,
    encode_string_into, ip_to_bytes,
};

// Public constants (kept for backward compatibility with downstream callers).
pub use crate::proto::PVA_VERSION;
pub const CMD_SEARCH: u8 = Command::Search as u8;
pub const CMD_SEARCH_RESPONSE: u8 = Command::SearchResponse as u8;
pub const CMD_CREATE_CHANNEL: u8 = Command::CreateChannel as u8;
pub const CMD_CONNECTION_VALIDATED: u8 = Command::ConnectionValidated as u8;
pub const CMD_GET: u8 = Command::Get as u8;
pub const CMD_PUT: u8 = Command::Put as u8;
pub const CMD_MONITOR: u8 = Command::Monitor as u8;
pub const CMD_DESTROY_REQUEST: u8 = Command::DestroyRequest as u8;
pub const CMD_GET_FIELD: u8 = Command::GetField as u8;
pub const QOS_INIT: u8 = QosFlags::INIT;

/// PVA message codec — manages byte order and provides message building helpers.
///
/// All encoding is fully native — no `spvirit_codec` dependency.
pub struct PvaCodec {
    pub big_endian: bool,
}

impl PvaCodec {
    pub fn new() -> Self {
        Self { big_endian: false }
    }

    fn order(&self) -> ByteOrder {
        if self.big_endian {
            ByteOrder::Big
        } else {
            ByteOrder::Little
        }
    }

    fn frame(&self, server: bool, command: u8, payload: Vec<u8>) -> Vec<u8> {
        let header = PvaHeader::application(server, self.order(), command, payload.len() as u32);
        let mut out = Vec::with_capacity(PvaHeader::SIZE + payload.len());
        header.write_into(&mut out);
        out.extend_from_slice(&payload);
        out
    }

    fn op_payload(sid: u32, ioid: u32, subcmd: u8, extra: &[u8], order: ByteOrder) -> Vec<u8> {
        let mut p = Vec::with_capacity(9 + extra.len());
        p.put_u32(sid, order);
        p.put_u32(ioid, order);
        p.put_u8(subcmd);
        p.put_bytes(extra);
        p
    }

    // ─── Search message (UDP) ────────────────────────────────────────────

    pub fn build_search(
        &self,
        sequence_id: u32,
        search_id: u32,
        channel_name: &str,
        response_addr: [u8; 4],
        response_port: u16,
        unicast: bool,
    ) -> Vec<u8> {
        let order = self.order();
        let flags: u8 = if unicast { 0x80 } else { 0x00 };
        let addr = ip_to_bytes(IpAddr::V4(Ipv4Addr::from(response_addr)));

        let mut p = Vec::new();
        p.put_u32(sequence_id, order);
        p.put_u8(flags);
        p.extend_from_slice(&[0u8; 3]); // reserved
        p.extend_from_slice(&addr);
        p.put_u16(response_port, order);
        // Single supported protocol: "tcp"
        encode_size_into(1, order, &mut p);
        encode_string_into("tcp", order, &mut p);
        // Channel list: count (u16) + (cid, name) entries
        p.put_u16(1, order);
        p.put_u32(search_id, order);
        encode_string_into(channel_name, order, &mut p);

        self.frame(false, CMD_SEARCH, p)
    }

    // ─── Connection validation response ──────────────────────────────────

    pub fn build_connection_validated(&self) -> Vec<u8> {
        let payload = Status::ok().encode(self.order());
        self.frame(false, CMD_CONNECTION_VALIDATED, payload)
    }

    // ─── Create channel ──────────────────────────────────────────────────

    pub fn build_create_channel(&self, client_channel_id: u32, channel_name: &str) -> Vec<u8> {
        let order = self.order();
        let mut p = Vec::new();
        p.put_u16(1, order); // channel count
        p.put_u32(client_channel_id, order);
        encode_string_into(channel_name, order, &mut p);
        self.frame(false, CMD_CREATE_CHANNEL, p)
    }

    // ─── GET ─────────────────────────────────────────────────────────────

    pub fn build_get_init(&self, server_channel_id: u32, ioid: u32, pv_request: &[u8]) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, QOS_INIT, pv_request, self.order());
        self.frame(false, CMD_GET, p)
    }

    pub fn build_get(&self, server_channel_id: u32, ioid: u32) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, 0x00, &[], self.order());
        self.frame(false, CMD_GET, p)
    }

    // ─── PUT ─────────────────────────────────────────────────────────────

    pub fn build_put_init(&self, server_channel_id: u32, ioid: u32, pv_request: &[u8]) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, QOS_INIT, pv_request, self.order());
        self.frame(false, CMD_PUT, p)
    }

    pub fn build_put(&self, server_channel_id: u32, ioid: u32, value_data: &[u8]) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, 0x00, value_data, self.order());
        self.frame(false, CMD_PUT, p)
    }

    // ─── MONITOR ─────────────────────────────────────────────────────────

    pub fn build_monitor_init(
        &self,
        server_channel_id: u32,
        ioid: u32,
        pv_request: &[u8],
    ) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, QOS_INIT, pv_request, self.order());
        self.frame(false, CMD_MONITOR, p)
    }

    pub fn build_monitor_start(
        &self,
        server_channel_id: u32,
        ioid: u32,
        pipeline_size: u32,
    ) -> Vec<u8> {
        let order = self.order();
        // pvxs `servermon.cpp` uses subcmd `0x44` (0x40 START | 0x04 PROCESS)
        // for the initial START, optionally followed by the pipeline window
        // size. Plain `0x40` works equivalently when no pipelining is
        // requested; we always send the 4-byte window for consistency.
        let extra = match order {
            ByteOrder::Big => pipeline_size.to_be_bytes(),
            ByteOrder::Little => pipeline_size.to_le_bytes(),
        };
        let p = Self::op_payload(server_channel_id, ioid, 0x44, &extra, order);
        self.frame(false, CMD_MONITOR, p)
    }

    /// Subsequent pipeline-ack message: subcmd `0x80` + ack count.
    pub fn build_monitor_ack(&self, server_channel_id: u32, ioid: u32, ack_count: u32) -> Vec<u8> {
        let order = self.order();
        let extra = match order {
            ByteOrder::Big => ack_count.to_be_bytes(),
            ByteOrder::Little => ack_count.to_le_bytes(),
        };
        let p = Self::op_payload(server_channel_id, ioid, 0x80, &extra, order);
        self.frame(false, CMD_MONITOR, p)
    }

    // ─── GET_FIELD (info) ────────────────────────────────────────────────

    pub fn build_get_field(&self, server_channel_id: u32, ioid: u32, subfield: &str) -> Vec<u8> {
        let order = self.order();
        let mut p = Vec::new();
        p.put_u32(server_channel_id, order);
        p.put_u32(ioid, order);
        encode_string_into(subfield, order, &mut p);
        self.frame(false, CMD_GET_FIELD, p)
    }

    // ─── DESTROY_REQUEST ─────────────────────────────────────────────────

    pub fn build_destroy_request(&self, server_channel_id: u32, ioid: u32) -> Vec<u8> {
        let p = Self::op_payload(server_channel_id, ioid, 0x00, &[], self.order());
        self.frame(false, CMD_DESTROY_REQUEST, p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec(big_endian: bool) -> PvaCodec {
        PvaCodec { big_endian }
    }

    #[test]
    fn search_message_has_pva_header() {
        let bytes = codec(false).build_search(1, 7, "MY:PV", [0, 0, 0, 0], 5076, false);
        assert_eq!(bytes[0], 0xCA);
        assert_eq!(bytes[1], PVA_VERSION);
        assert_eq!(bytes[2] & 0x80, 0); // little-endian
        assert_eq!(bytes[3], CMD_SEARCH);
    }

    #[test]
    fn create_channel_carries_pv_name() {
        let bytes = codec(false).build_create_channel(42, "MY:PV");
        assert_eq!(bytes[3], CMD_CREATE_CHANNEL);
        // Payload: channel_count (u16 LE) + cid (u32 LE) + string "MY:PV"
        let payload = &bytes[8..];
        assert_eq!(&payload[..2], &[0x01, 0x00]);
        assert_eq!(&payload[2..6], &[42, 0, 0, 0]);
        assert_eq!(payload[6] as usize, "MY:PV".len());
        assert_eq!(&payload[7..7 + 5], b"MY:PV");
    }

    #[test]
    fn destroy_request_payload_layout() {
        let bytes = codec(false).build_destroy_request(99, 17);
        assert_eq!(bytes[3], CMD_DESTROY_REQUEST);
        let payload = &bytes[8..];
        // sid (u32) + ioid (u32) + subcmd (u8)
        assert_eq!(&payload[..4], &[99, 0, 0, 0]);
        assert_eq!(&payload[4..8], &[17, 0, 0, 0]);
        assert_eq!(payload[8], 0x00);
    }
}
