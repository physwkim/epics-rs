---
sha: c5012d9f73b80bb55ddba2a8785d5c5841e268ec
short_sha: c5012d9
date: 2021-12-17
author: Dirk Zimoch
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/com_buf.rs
    function: push_int8
  - crate: ca-rs
    file: src/client/com_que_recv.rs
    function: copy_out_bytes
  - crate: base-rs
    file: src/server/database/db_types.rs
    function: null
tags: [epicsInt8, signed-char, DBF_CHAR, wire-protocol, type-safety]
---

# Make sure epicsInt8 is signed on all architectures

## Root Cause
`epicsInt8` was defined as `typedef char epicsInt8;`. On architectures where
`char` is unsigned by default (many PPC/PowerPC targets), `epicsInt8` was
therefore unsigned. This caused `DBF_CHAR` (which maps to `epicsInt8`) to have
different sign semantics between x86 (signed char) and PPC (unsigned char):

- Converting a `DBF_CHAR` value of `0xFF` to `epicsInt16`/`epicsFloat32`:
  - x86: sign-extends to `-1` (correct for signed byte)
  - PPC: zero-extends to `255` (wrong — treats as unsigned)

The CA wire protocol encodes `DBF_CHAR` as raw bytes. The `comBuf::push()`
and `comQueRecv::copyOutBytes()` were typed on `epicsInt8*`, so on PPC they
were effectively operating on `unsigned char*`, causing wrong sign propagation
when the buffer contents were used in arithmetic.

## Symptoms
- On PPC architectures: negative `DBF_CHAR` values in input links appeared
  as large positive values when read by connected records.
- `DBF_CHAR` waveform reads from CA returned wrong values on PPC hosts.
- Downstream conversions (e.g., DBF_CHAR → DBF_FLOAT) produced incorrect results.

## Fix
- Changed `typedef char epicsInt8` to `typedef signed char epicsInt8` in
  `epicsTypes.h`.
- Added a `comBuf::push(const char*, unsigned)` overload that calls
  `copyInBytes` — to keep the CA wire buffer API accepting raw `char*` without
  breaking caller code that uses unqualified `char` arrays.
- Changed `comQueRecv::copyOutBytes()` signature from `epicsInt8*` to `char*`
  for the same reason.

## Rust Applicability
Rust's `i8` is always signed; there is no "char" ambiguity. However, `ca-rs`
must correctly handle the wire encoding of `DBF_CHAR` (CA type code 4):
- Wire bytes must be interpreted as `i8` (signed), not `u8`.
- When decoding a `DBF_CHAR` array from the CA wire buffer, use `i8::from_ne_bytes`
  or cast the received `u8` slice to `&[i8]` before numeric conversion.
- When converting `i8` → `f64` for `DBR_FLOAT` responses, sign extension must
  occur (Rust's `i8 as f64` is correct; `u8 as f64` would be wrong).

## Audit Recommendation
- In `ca-rs/src/client/com_buf.rs`: verify `push_dbf_char` / analogous function
  uses `i8` not `u8` when writing CA type 4 data.
- In `ca-rs/src/client/com_que_recv.rs`: verify `copy_out_bytes` for DBF_CHAR
  reads back as `i8`.
- In `base-rs/src/server/database/db_types.rs`: verify `DBF_CHAR` field is
  stored as `i8` (or a newtype over `i8`), not `u8`.

## C Locations
- `modules/libcom/src/misc/epicsTypes.h:epicsInt8` — changed to `signed char`
- `modules/ca/src/client/comBuf.h:comBuf::push` — added `const char*` overload
- `modules/ca/src/client/comQueRecv.cpp:comQueRecv::copyOutBytes` — `char*` signature
- `modules/ca/src/client/comQueRecv.h:comQueRecv::copyOutBytes` — `char*` declaration
