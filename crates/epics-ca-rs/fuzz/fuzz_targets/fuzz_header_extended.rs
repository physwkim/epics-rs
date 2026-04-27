#![no_main]
//! Fuzz the extended-header parser. Hostile inputs typically advertise
//! impossibly large `extended_postsize` to trigger an allocation panic
//! — must be cleanly rejected with Err instead.

use epics_ca_rs::protocol::CaHeader;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = CaHeader::from_bytes_extended(data);
});
