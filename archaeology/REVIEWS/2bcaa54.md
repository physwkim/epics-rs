---
short_sha: 2bcaa54
status: not-applicable
files_changed: []
---
The C bug is `memcpy(pbufmsg + 1, pExt, extsize)` with `if (extsize)` only — pExt could be null while extsize > 0, causing a null deref. ca-rs has no equivalent `push_datagram_msg`; UDP frames are assembled in `epics-ca-rs/src/protocol.rs::CaHeader::to_bytes_extended` and callers append payloads using safe `Vec<u8>::extend_from_slice` on `&[u8]` slices, never raw pointer + length pairs. The Rust type system makes this class of null-deref impossible: there is no way to have a non-zero-length slice with a null data pointer in safe Rust. No fix needed.
