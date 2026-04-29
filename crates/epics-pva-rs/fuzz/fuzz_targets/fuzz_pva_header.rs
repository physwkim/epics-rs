#![no_main]
//! Fuzz the 8-byte PVA header parser. Should never panic on arbitrary
//! input — including truncated buffers, garbage version bytes, or
//! out-of-range payload_length fields. The header decoder is the very
//! first decode step a peer can drive, so any panic here is a remote
//! denial-of-service vector.

use std::io::Cursor;

use epics_pva_rs::proto::PvaHeader;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut cur = Cursor::new(data);
    let _ = PvaHeader::decode(&mut cur);
});
