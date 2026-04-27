#![no_main]
//! Fuzz the standard 16-byte CA header parser. Should never panic on
//! arbitrary input.

use epics_ca_rs::protocol::CaHeader;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = CaHeader::from_bytes(data);
});
