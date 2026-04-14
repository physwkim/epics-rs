use spvirit_codec::spvirit_encode::{
    encode_connection_validated, encode_create_channel_request, encode_get_field_request,
    encode_get_request, encode_monitor_request, encode_op_request, encode_put_request,
    encode_search_request, ip_to_bytes,
};

use std::net::{IpAddr, Ipv4Addr};

/// PVA protocol version used in headers.
pub const PVA_VERSION: u8 = 2;

// PVA command codes
pub const CMD_SEARCH: u8 = 3;
pub const CMD_SEARCH_RESPONSE: u8 = 4;
pub const CMD_CREATE_CHANNEL: u8 = 7;
pub const CMD_CONNECTION_VALIDATED: u8 = 9;
pub const CMD_GET: u8 = 10;
pub const CMD_PUT: u8 = 11;
pub const CMD_MONITOR: u8 = 13;
pub const CMD_DESTROY_REQUEST: u8 = 15;
pub const CMD_GET_FIELD: u8 = 17;

// QoS / subcommand flags
pub const QOS_INIT: u8 = 0x08;

/// PVA message codec — manages byte order and provides message building helpers.
///
/// All encoding now delegates to `spvirit_codec::spvirit_encode`.
pub struct PvaCodec {
    pub big_endian: bool,
}

impl PvaCodec {
    pub fn new() -> Self {
        Self { big_endian: false }
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
        let be = self.big_endian;
        let flags: u8 = if unicast { 0x80 } else { 0x00 };
        let addr: [u8; 16] = ip_to_bytes(IpAddr::V4(Ipv4Addr::from(response_addr)))
            .try_into()
            .expect("ip_to_bytes returns 16 bytes");
        encode_search_request(
            sequence_id,
            flags,
            response_port,
            addr,
            &[(search_id, channel_name)],
            PVA_VERSION,
            be,
        )
    }

    // ─── Connection validation response ──────────────────────────────────

    pub fn build_connection_validated(&self) -> Vec<u8> {
        encode_connection_validated(false, PVA_VERSION, self.big_endian)
    }

    // ─── Create channel ──────────────────────────────────────────────────

    pub fn build_create_channel(&self, client_channel_id: u32, channel_name: &str) -> Vec<u8> {
        encode_create_channel_request(
            client_channel_id,
            channel_name,
            PVA_VERSION,
            self.big_endian,
        )
    }

    // ─── GET ─────────────────────────────────────────────────────────────

    pub fn build_get_init(&self, server_channel_id: u32, ioid: u32, pv_request: &[u8]) -> Vec<u8> {
        encode_get_request(
            server_channel_id,
            ioid,
            QOS_INIT,
            pv_request,
            PVA_VERSION,
            self.big_endian,
        )
    }

    pub fn build_get(&self, server_channel_id: u32, ioid: u32) -> Vec<u8> {
        encode_get_request(
            server_channel_id,
            ioid,
            0x00,
            &[],
            PVA_VERSION,
            self.big_endian,
        )
    }

    // ─── PUT ─────────────────────────────────────────────────────────────

    pub fn build_put_init(&self, server_channel_id: u32, ioid: u32, pv_request: &[u8]) -> Vec<u8> {
        encode_put_request(
            server_channel_id,
            ioid,
            QOS_INIT,
            pv_request,
            PVA_VERSION,
            self.big_endian,
        )
    }

    pub fn build_put(&self, server_channel_id: u32, ioid: u32, value_data: &[u8]) -> Vec<u8> {
        encode_put_request(
            server_channel_id,
            ioid,
            0x00,
            value_data,
            PVA_VERSION,
            self.big_endian,
        )
    }

    // ─── MONITOR ─────────────────────────────────────────────────────────

    pub fn build_monitor_init(
        &self,
        server_channel_id: u32,
        ioid: u32,
        pv_request: &[u8],
    ) -> Vec<u8> {
        encode_monitor_request(
            server_channel_id,
            ioid,
            QOS_INIT,
            pv_request,
            PVA_VERSION,
            self.big_endian,
        )
    }

    pub fn build_monitor_start(
        &self,
        server_channel_id: u32,
        ioid: u32,
        pipeline_size: u32,
    ) -> Vec<u8> {
        let be = self.big_endian;
        let extra = if be {
            pipeline_size.to_be_bytes()
        } else {
            pipeline_size.to_le_bytes()
        };
        encode_monitor_request(server_channel_id, ioid, 0x80, &extra, PVA_VERSION, be)
    }

    // ─── GET_FIELD (info) ────────────────────────────────────────────────

    pub fn build_get_field(&self, server_channel_id: u32, ioid: u32, subfield: &str) -> Vec<u8> {
        let sub = if subfield.is_empty() {
            None
        } else {
            Some(subfield)
        };
        encode_get_field_request(server_channel_id, ioid, sub, PVA_VERSION, self.big_endian)
    }

    // ─── DESTROY_REQUEST ─────────────────────────────────────────────────

    pub fn build_destroy_request(&self, server_channel_id: u32, ioid: u32) -> Vec<u8> {
        encode_op_request(
            CMD_DESTROY_REQUEST,
            server_channel_id,
            ioid,
            0x00,
            &[],
            PVA_VERSION,
            self.big_endian,
        )
    }
}
