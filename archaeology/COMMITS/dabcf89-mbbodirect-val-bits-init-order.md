---
sha: dabcf893f7170325a22fd830a6caeec7048ba923
short_sha: dabcf89
date: 2021-10-03
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec/mbbodirect.rs
    function: init_record
tags: [mbboDirect, init, VAL, bits, record-init]
---
# mbboDirect: fix init priority — B0-B1F bits override VAL when VAL is UDF

## Root Cause
`init_record()` in `mbboDirectRecord.c` unconditionally called `bitsFromVAL()`
to populate `B0..B1F` from `VAL`. When the record was defined with explicit
bit-field values (`field(B0, 1)`, `field(B3, 1)`, etc.) but no `field(VAL, ...)`,
`VAL` remained zero and `UDF` was set. The unconditional `bitsFromVAL()` call
then zeroed all the user-specified bit fields before any processing occurred.

## Symptoms
- mbboDirect records configured with only `B*` fields would always initialize
  with all bits zero, ignoring the `field(B0..B1F, ...)` database settings.
- UDF alarm would be asserted despite the user having set meaningful bit values.
- The bug was silent: no error, just wrong initial state.

## Fix
Check `prec->udf` before calling `bitsFromVAL()`. If `UDF` is set (meaning
`VAL` was never explicitly set), scan the `B0..B1F` array instead. If any bit
is non-zero, reconstruct `VAL` from the bits, clear `UDF`, and continue. Only
if no bit is set does the record remain UDF. The old path (`bitsFromVAL()`) is
taken when `VAL` was explicitly provided.

## Rust Applicability
Any Rust implementation of `mbboDirect` record initialization must implement
this two-pass logic: if `udf` is set, try to reconstruct `val` from `b0..b1f`
before falling back to the UDF state. A simpler `val = bits_to_val(b_fields)`
at init would also work, as long as the result is checked against zero to
determine UDF status.

## Audit Recommendation
In `base-rs/src/server/database/rec/mbbodirect.rs::init_record`: confirm that
initialization checks whether `val` is UDF before deciding which direction to
sync (`VAL → bits` or `bits → VAL`). The correct precedence is:
`VAL` (if not UDF) → bits; else bits → `VAL`.

## C Locations
- `modules/database/src/std/rec/mbboDirectRecord.c:init_record` — unconditional `bitsFromVAL()` replaced with UDF-aware logic
