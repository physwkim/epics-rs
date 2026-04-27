//! BitSet for monitor-delta encoding.
//!
//! Implements the exact wire format used by pvxs `bitmask.cpp::to_wire/from_wire`,
//! which differs from a "fixed-size byte vector" in two important ways:
//!
//! 1. **LSB-first within each byte / word.** Bit `i` lives in byte `i/8` at
//!    position `i%8`. (pvxs internally uses `u64` words, but the wire format
//!    is just a sequence of bytes so we use `Vec<u8>` here.)
//! 2. **Trailing-zero trimming on encode.** Writes only enough bytes to cover
//!    the highest set bit. A BitSet with no bits set encodes as a single
//!    `Size{0}` byte (`0x00`).
//!
//! Field-bit numbering on a `PvStructure` follows pvData spec §5.4: the
//! root structure occupies bit 0, then nested fields are numbered depth-first
//! in declaration order. This module is purely the bit container; the field-
//! numbering logic lives next to `pvdata::FieldDesc`.

use std::io::Cursor;

use super::buffer::{ByteOrder, DecodeError, ReadExt};
use super::size::{decode_size, encode_size_into};

/// Compact bit container with LSB-first packing matching pvxs `bitmask.cpp`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BitSet {
    bytes: Vec<u8>,
}

impl BitSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a BitSet large enough to hold `nbits` and pre-fill nothing.
    pub fn with_capacity(nbits: usize) -> Self {
        Self {
            bytes: Vec::with_capacity((nbits + 7) / 8),
        }
    }

    /// All bits in `0..nbits` set. Used for "first monitor event" delta where
    /// the entire structure is new.
    pub fn all_set(nbits: usize) -> Self {
        let nbytes = (nbits + 7) / 8;
        let mut bytes = vec![0xFFu8; nbytes];
        // Mask the unused high bits in the final byte.
        let extra = nbytes * 8 - nbits;
        if extra > 0 {
            let last = bytes.len() - 1;
            bytes[last] &= 0xFFu8 >> extra;
        }
        Self { bytes }
    }

    /// Set bit `i`. Grows storage as needed.
    pub fn set(&mut self, i: usize) {
        let need = i / 8 + 1;
        if self.bytes.len() < need {
            self.bytes.resize(need, 0);
        }
        self.bytes[i / 8] |= 1 << (i % 8);
    }

    /// Clear bit `i`. Does NOT shrink storage; trailing zeros are trimmed
    /// only at encode time.
    pub fn clear(&mut self, i: usize) {
        let byte_idx = i / 8;
        if byte_idx < self.bytes.len() {
            self.bytes[byte_idx] &= !(1 << (i % 8));
        }
    }

    /// Test bit `i`.
    pub fn get(&self, i: usize) -> bool {
        let byte_idx = i / 8;
        if byte_idx >= self.bytes.len() {
            return false;
        }
        self.bytes[byte_idx] & (1 << (i % 8)) != 0
    }

    /// True iff no bits are set.
    pub fn is_empty(&self) -> bool {
        self.bytes.iter().all(|&b| b == 0)
    }

    /// Iterator over set bit positions in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.bytes.iter().enumerate().flat_map(|(byte_idx, &byte)| {
            (0..8).filter_map(move |bit_idx| {
                if byte & (1 << bit_idx) != 0 {
                    Some(byte_idx * 8 + bit_idx)
                } else {
                    None
                }
            })
        })
    }

    /// Number of bits set.
    pub fn count(&self) -> usize {
        self.bytes.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// Encode in pvxs-compatible wire format.
    pub fn encode(&self, order: ByteOrder) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_into(order, &mut out);
        out
    }

    /// Append the encoded form to `buf`. Trims trailing zero bytes per pvxs
    /// `bitmask.cpp::to_wire`.
    pub fn write_into(&self, order: ByteOrder, buf: &mut Vec<u8>) {
        let mut nbytes = self.bytes.len();
        while nbytes > 0 && self.bytes[nbytes - 1] == 0 {
            nbytes -= 1;
        }
        encode_size_into(nbytes as u32, order, buf);
        buf.extend_from_slice(&self.bytes[..nbytes]);
    }

    /// Decode the next BitSet from `cur`.
    pub fn decode(cur: &mut Cursor<&[u8]>, order: ByteOrder) -> Result<Self, DecodeError> {
        let nbytes = decode_size(cur, order)?
            .ok_or_else(|| DecodeError("bitset size cannot be null".into()))?
            as usize;
        let bytes = cur.get_bytes(nbytes)?;
        Ok(Self { bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bitset_encodes_as_zero_size() {
        let bs = BitSet::new();
        assert!(bs.is_empty());
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x00]);
    }

    #[test]
    fn single_bit_set() {
        let mut bs = BitSet::new();
        bs.set(0);
        assert!(bs.get(0));
        assert!(!bs.get(1));
        // size=1, byte=0x01
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x01, 0x01]);
    }

    #[test]
    fn high_bit_grows_storage() {
        let mut bs = BitSet::new();
        bs.set(20); // byte 2, bit 4 → bytes = [0x00, 0x00, 0x10]
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x03, 0x00, 0x00, 0x10]);
        assert!(bs.get(20));
        assert!(!bs.get(19));
        assert!(!bs.get(21));
    }

    #[test]
    fn clear_then_trim() {
        let mut bs = BitSet::new();
        bs.set(20);
        bs.set(0);
        bs.clear(20);
        // Trailing zeros must be trimmed even though storage still has 3 bytes.
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x01, 0x01]);
    }

    #[test]
    fn all_set_full_byte() {
        let bs = BitSet::all_set(8);
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x01, 0xFF]);
        for i in 0..8 {
            assert!(bs.get(i));
        }
        assert!(!bs.get(8));
    }

    #[test]
    fn all_set_partial_byte_masked() {
        let bs = BitSet::all_set(5);
        // 5 bits → only bottom 5 bits should be set
        assert_eq!(bs.encode(ByteOrder::Little), vec![0x01, 0b0001_1111]);
    }

    #[test]
    fn round_trip_random() {
        let mut bs = BitSet::new();
        for &i in &[0usize, 7, 8, 9, 63, 64, 100, 200] {
            bs.set(i);
        }
        let encoded = bs.encode(ByteOrder::Little);
        let mut cur = Cursor::new(encoded.as_slice());
        let decoded = BitSet::decode(&mut cur, ByteOrder::Little).unwrap();
        assert_eq!(bs, decoded);
        let set: Vec<usize> = decoded.iter().collect();
        assert_eq!(set, vec![0, 7, 8, 9, 63, 64, 100, 200]);
        assert_eq!(decoded.count(), 8);
    }

    #[test]
    fn iter_sorted_ascending() {
        let mut bs = BitSet::new();
        for &i in &[100usize, 0, 50, 7, 8] {
            bs.set(i);
        }
        let collected: Vec<_> = bs.iter().collect();
        assert_eq!(collected, vec![0, 7, 8, 50, 100]);
    }
}
