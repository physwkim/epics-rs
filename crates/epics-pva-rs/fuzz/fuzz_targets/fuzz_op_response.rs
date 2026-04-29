#![no_main]
//! End-to-end fuzz of the wire-decode pipeline:
//! `bytes → try_parse_frame → decode_op_response`. Covers the GET /
//! PUT / MONITOR / RPC INIT-and-Data branches, with introspection
//! provided either implicitly (Variant fallback when None) or via the
//! per-connection TypeCache embedded inside `decode_op_response`.

use epics_pva_rs::client_native::decode::{decode_op_response, try_parse_frame};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(Some((frame, _))) = try_parse_frame(data) {
        let _ = decode_op_response(&frame, None);
    }
});
