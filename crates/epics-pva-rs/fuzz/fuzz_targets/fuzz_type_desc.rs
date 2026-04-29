#![no_main]
//! Fuzz the wire-format type-descriptor decoder. Covers the recursive
//! descent through nested Structure / StructureArray / Union /
//! UnionArray entries. The decode path consults the per-connection
//! TypeCache for 0xFD/0xFE marker resolution; we use a fresh cache
//! each iteration so the fuzzer can't get stuck in cache-only states.

use std::io::Cursor;

use epics_pva_rs::proto::ByteOrder;
use epics_pva_rs::pvdata::encode::decode_type_desc;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    // Drive both byte orders with the same input, since the dispatch
    // through u32/u16 readers picks up subtly different code paths.
    let order = if data[0] & 1 == 0 {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    };
    let body = &data[1..];
    let mut cur = Cursor::new(body);
    let _ = decode_type_desc(&mut cur, order);
});
