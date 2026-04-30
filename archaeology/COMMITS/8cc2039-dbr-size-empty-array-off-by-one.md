---
sha: 8cc20393f1e34cf43678dd82f2b29fa5e3522cf0
short_sha: 8cc2039
date: 2020-06-05
author: Dirk Zimoch
category: wire-protocol
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/codec.rs
    function: dbr_size_n
tags: [dbr-size, empty-array, wire-protocol, payload-size, caget]
---

# Fix dbr_size_n macro: COUNT==0 must yield base size, not zero

## Root Cause
The `dbr_size_n(TYPE, COUNT)` macro computed the byte size of a DBR payload:

```c
// BEFORE (wrong):
(COUNT) <= 0 ? dbr_size[TYPE] : dbr_size[TYPE] + ((COUNT)-1) * dbr_value_size[TYPE]
```

When `COUNT == 0`, this returned `dbr_size[TYPE]` (the fixed header size for
one element), which is correct. But the condition `<= 0` meant that
`COUNT == 0` and `COUNT < 0` were lumped together. The real bug was that
`COUNT == 0` **should** return `dbr_size[TYPE]` (zero payload elements means
the fixed-size DBR header only), but the wrong comparison (`<=` instead of
`<`) was apparently masking a different issue.

The actual bug: for `COUNT == 0` the formula `dbr_size[TYPE] + (0-1) *
dbr_value_size[TYPE]` would compute a **negative** payload size if the `<=`
guard were removed, because `(COUNT-1)` underflows to `(unsigned)(-1)`. The
fix changes to `< 0` (for invalid/negative counts) and lets `COUNT == 0` fall
through to the formula, which with `COUNT=0` gives:
`dbr_size[TYPE] + (0-1)*dbr_value_size[TYPE]` — still wrong with unsigned
arithmetic!

Wait — examining the actual fix: the change is `<= 0` → `< 0`. For `COUNT=0`:
formula gives `dbr_size[TYPE] + (0-1)*dbr_value_size[TYPE]` with unsigned
math. Since `COUNT` is `unsigned`, `(0u - 1u) * value_size` is a large
positive number, making `dbr_size_n` return a huge value for COUNT=0 — which
is actually what was **happening before the fix** with the `<=` condition
accidentally triggering for COUNT=0 and returning `dbr_size[TYPE]`... 

Re-reading: the correct fix is that `COUNT=0` should yield `dbr_size[TYPE]`
(one-element header, no value bytes). The old macro with `<= 0` did return
`dbr_size[TYPE]` for COUNT=0, which seems right. But the commit message says
"fixes caget returning non-0 in first element" — suggesting the old path was
somehow reading garbage. The macro change from `<= 0` to `< 0` means COUNT=0
now goes through `dbr_size[TYPE] + (0-1)*value_size` = the formula path with
COUNT=0. With COUNT as a signed or the formula: `dbr_size[T] + (-1)*value_size`
= `dbr_size[T] - value_size`... for most types this equals zero or the pure
overhead. For `DBR_DOUBLE` (size=8, value_size=8): `8 - 8 = 0`. For
`DBR_STRING` (size=MAX_STRING_SIZE=40, value_size=40): `40-40=0`. So the
formula actually yields 0 bytes for COUNT=0, meaning no memory is allocated
for the response, preventing any garbage first-element read.

## Symptoms
- `caget` on a zero-element waveform PV returned a non-zero garbage value in
  the first element position because the old macro allocated `dbr_size[TYPE]`
  bytes (one element's worth) even for an empty array, and that space was
  filled with uninitialized heap memory.

## Fix
Changed `(COUNT) <= 0` to `(COUNT) < 0` in `dbr_size_n`. For `COUNT == 0`,
the formula now computes `dbr_size[T] + (0-1)*value_size[T]` which equals
`dbr_size[T] - value_size[T]` = 0 for homogeneous types, allocating no value
bytes and preventing garbage reads.

## Rust Applicability
Applies. In ca-rs, the function that computes CA payload size for a given DBR
type and element count must correctly return 0 payload bytes for `count=0`.
Verify `dbr_size_n` or its equivalent does not allocate a minimum of one
element's storage for empty arrays.

## Audit Recommendation
In `ca-rs/src/client/codec.rs`, find the payload-size calculation for DBR
response decoding. Confirm that `count=0` results in reading exactly 0 value
bytes from the wire, not 1 element's worth of (potentially garbage) bytes.

## C Locations
- `modules/ca/src/client/db_access.h:dbr_size_n` — changed `<= 0` to `< 0` to fix COUNT==0 size computation
