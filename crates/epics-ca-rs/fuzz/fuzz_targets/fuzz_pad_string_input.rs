#![no_main]
//! Fuzz `pad_string` with arbitrary UTF-8 strings to verify it never
//! panics on adversarial inputs (long strings, multi-byte chars, etc.).

use epics_ca_rs::protocol::pad_string;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = pad_string(s);
    }
});
