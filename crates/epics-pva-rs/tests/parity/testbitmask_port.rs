//! Port of pvxs's `test/testbitmask.cpp` to our `proto::BitSet`.
//!
//! Translates each pvxs sub-test (testEmpty, testBasic1/2/3, testOp,
//! testSer) into Rust unit-test form. testExpr (operator overloading
//! for `!`/`&`/`|`) is omitted because we don't (yet) need bitwise
//! algebra on `BitSet` outside encode/decode.
//!
//! The `testSer` cases are the most important — they verify our
//! BitSet's wire format byte-for-byte against pvxs's reference vectors.

#![cfg(test)]

use epics_pva_rs::proto::{BitSet, ByteOrder};
use std::io::Cursor;

// ── testEmpty ──────────────────────────────────────────────────────

#[test]
fn pvxs_test_empty() {
    let empty = BitSet::new();
    assert!(empty.is_empty());
    assert_eq!(empty.size(), 0);
    assert_eq!(empty.byte_size(), 0);
    // findSet on an empty bitset returns size().
    assert_eq!(empty.find_set(0), 0);
    let collected: Vec<_> = empty.iter().collect();
    assert!(collected.is_empty());
}

// ── testBasic1: bits {1, 5, 3, 7} ──────────────────────────────────

#[test]
fn pvxs_test_basic_1() {
    let mut m = BitSet::new();
    for &b in &[1usize, 5, 3, 7] {
        m.set(b);
    }
    assert!(!m.is_empty());
    assert!(m.size() >= 8);
    assert_eq!(m.byte_size(), 1);

    assert_eq!(m.find_set(0), 1);
    assert_eq!(m.find_set(1), 1);
    assert_eq!(m.find_set(2), 3);
    assert_eq!(m.find_set(3), 3);
    assert_eq!(m.find_set(m.size()), m.size());

    let collected: Vec<_> = m.iter().collect();
    assert_eq!(collected, vec![1, 3, 5, 7]);
}

// ── testBasic2: bits {0, 2, 4, 6} ──────────────────────────────────

#[test]
fn pvxs_test_basic_2() {
    let mut m = BitSet::new();
    for &b in &[6usize, 0, 4, 2] {
        m.set(b);
    }
    assert!(!m.is_empty());
    assert!(m.size() >= 7);
    assert_eq!(m.byte_size(), 1);

    assert_eq!(m.find_set(0), 0);
    assert_eq!(m.find_set(1), 2);
    assert_eq!(m.find_set(2), 2);
    assert_eq!(m.find_set(3), 4);

    let collected: Vec<_> = m.iter().collect();
    assert_eq!(collected, vec![0, 2, 4, 6]);
}

// ── testBasic3: bits {63, 64, 67} — crosses a byte boundary ────────

#[test]
fn pvxs_test_basic_3() {
    let mut m = BitSet::new();
    for &b in &[63usize, 64, 67] {
        m.set(b);
    }
    assert!(!m.is_empty());
    assert!(m.size() >= 68);
    // pvxs uses u64 words (wsize=2 means 16 bytes). Our backing is u8.
    // Either way, 64+ bits requires ≥ 9 bytes.
    assert!(m.byte_size() >= 9);

    assert_eq!(m.find_set(0), 63);
    assert_eq!(m.find_set(62), 63);
    assert_eq!(m.find_set(63), 63);
    assert_eq!(m.find_set(64), 64);
    assert_eq!(m.find_set(65), 67);

    let collected: Vec<_> = m.iter().collect();
    assert_eq!(collected, vec![63, 64, 67]);
}

// ── testOp: per-bit set/clear via mutate ───────────────────────────

#[test]
fn pvxs_test_op() {
    let mut m = BitSet::new();
    for &b in &[1usize, 2, 4, 5] {
        m.set(b);
    }
    assert!(!m.is_empty());
    assert!(!m.get(0));
    assert!(m.get(1));
    m.set(0);
    m.clear(1);
    assert!(m.get(0));
    assert!(!m.get(1));
}

// ── testSer: byte-exact wire format checks ─────────────────────────

/// Verify that `bytes` decode to `expected_set_bits` and re-encode to
/// the same `bytes`. Mirrors pvxs `testSerCase`.
fn check_round_trip(order: ByteOrder, bytes: &[u8], expected_set_bits: &[usize]) {
    let mut cur = Cursor::new(bytes);
    let bs = BitSet::decode(&mut cur, order).expect("decode");
    let actual: Vec<usize> = bs.iter().collect();
    assert_eq!(
        actual, expected_set_bits,
        "decode mismatch order={order:?} bytes={bytes:?}"
    );

    let mut re = Vec::new();
    bs.write_into(order, &mut re);
    assert_eq!(
        re.as_slice(),
        bytes,
        "re-encode mismatch order={order:?} expected_set_bits={expected_set_bits:?}"
    );
}

#[test]
fn pvxs_test_ser_empty() {
    // size=0 → no bytes
    check_round_trip(ByteOrder::Big, &[0x00], &[]);
    check_round_trip(ByteOrder::Little, &[0x00], &[]);
}

#[test]
fn pvxs_test_ser_bit0() {
    check_round_trip(ByteOrder::Big, &[0x01, 0x01], &[0]);
    check_round_trip(ByteOrder::Little, &[0x01, 0x01], &[0]);
}

#[test]
fn pvxs_test_ser_bit1() {
    check_round_trip(ByteOrder::Big, &[0x01, 0x02], &[1]);
    check_round_trip(ByteOrder::Little, &[0x01, 0x02], &[1]);
}

#[test]
fn pvxs_test_ser_bit8() {
    // bit 8 is in the second byte, bit 0 of that byte
    check_round_trip(ByteOrder::Big, &[0x02, 0x00, 0x01], &[8]);
    check_round_trip(ByteOrder::Little, &[0x02, 0x00, 0x01], &[8]);
}

#[test]
fn pvxs_test_ser_bits_1_55() {
    // 7 bytes; bit 1 in byte 0, bit 55 = byte 6 bit 7
    let bytes: &[u8] = &[0x07, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80];
    check_round_trip(ByteOrder::Big, bytes, &[1, 55]);
    check_round_trip(ByteOrder::Little, bytes, &[1, 55]);
}

// pvxs distinguishes BE/LE here because crossing the 64-bit u64 word
// boundary changes byte order. Our impl is byte-array based so the
// wire encoding is the same in both orders for the actual byte stream.

#[test]
fn pvxs_test_ser_bits_1_63_be() {
    // pvxs BE form: word 0 high-byte first
    let bytes: &[u8] = &[0x08, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    check_round_trip(ByteOrder::Big, bytes, &[1, 63]);
}

#[test]
fn pvxs_test_ser_bits_1_63_le() {
    // pvxs LE: same set, byte order inside the 8-byte word reversed
    let bytes: &[u8] = &[0x08, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80];
    check_round_trip(ByteOrder::Little, bytes, &[1, 63]);
}

#[test]
fn pvxs_test_ser_bits_1_8_63_64_be() {
    let bytes: &[u8] = &[0x09, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x01];
    check_round_trip(ByteOrder::Big, bytes, &[1, 8, 63, 64]);
}

#[test]
fn pvxs_test_ser_bits_1_8_63_64_le() {
    let bytes: &[u8] = &[0x09, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01];
    check_round_trip(ByteOrder::Little, bytes, &[1, 8, 63, 64]);
}

#[test]
fn pvxs_test_ser_bits_1_63_64_126_be() {
    let bytes: &[u8] = &[
        0x10, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01,
    ];
    check_round_trip(ByteOrder::Big, bytes, &[1, 63, 64, 126]);
}

#[test]
fn pvxs_test_ser_bits_1_63_64_126_le() {
    let bytes: &[u8] = &[
        0x10, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x40,
    ];
    check_round_trip(ByteOrder::Little, bytes, &[1, 63, 64, 126]);
}
