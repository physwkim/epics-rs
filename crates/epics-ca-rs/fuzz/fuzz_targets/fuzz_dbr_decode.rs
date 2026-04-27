#![no_main]
//! Fuzz `decode_dbr(dbr_type, data, count)` over arbitrary inputs.
//! The first 4 bytes of the fuzz buffer encode the dbr_type (u16) and
//! count (u16); the rest is the payload.

use epics_base_rs::types::decode_dbr;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let dbr_type = u16::from_be_bytes([data[0], data[1]]);
    let count = u16::from_be_bytes([data[2], data[3]]) as usize;
    let payload = &data[4..];
    let _ = decode_dbr(dbr_type, payload, count);
});
