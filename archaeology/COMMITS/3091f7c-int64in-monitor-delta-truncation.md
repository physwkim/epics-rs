---
sha: 3091f7c56f360d2704e37a76064ade4bdd9fef4c
short_sha: 3091f7c
date: 2021-07-29
author: Kay Kasemir
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/int64in_record.rs
    function: monitor
tags: [int64, monitor-delta, type-truncation, mdel, adel]
---

# int64in: Monitor Delta Comparison Truncated to 32 Bits

## Root Cause
The `DELTA` macro in `int64inRecord.c` computed the absolute difference
between two `epicsInt64` values but cast the result to `epicsUInt32`:

```c
#define DELTA(last, val) \
    ((epicsUInt32) ((last) > (val) ? (last) - (val) : (val) - (last)))
```

The comparison `DELTA(...) > (epicsUInt32) prec->mdel` also cast `mdel` to
`epicsUInt32`. Any difference larger than `2^32 - 1` (≈ 4.3 billion) would
wrap to a small 32-bit value, making the monitor fire (false positive) or not
fire (false negative) incorrectly for large int64 changes.

## Symptoms
For an `int64in` record where the value changes by more than ~4 billion but
the configured `MDEL` or `ADEL` threshold is in the same range, monitors may
fail to post when they should (difference wraps below threshold) or may post
spuriously when they should not (difference wraps above threshold). Bug
reported as: https://bugs.launchpad.net/epics-base/+bug/1938459

## Fix
Change `DELTA` to cast to `epicsUInt64` and compare against `(epicsUInt64)
prec->mdel` / `(epicsUInt64) prec->adel`.

## Rust Applicability
Applies. A Rust implementation of the `int64in` monitor function must compute
the absolute delta as `u64` (or `i64` with `abs()` and `as u64`), not `u32`.
If the threshold fields (`mdel`, `adel`) are stored as `i64` but compared as
`u64`, the cast must be done carefully to avoid sign-extension bugs.

## Audit Recommendation
In `base-rs/src/server/database/int64in_record.rs`, check the `monitor()`
function's delta computation. Ensure the DELTA equivalent uses `u64`/`i64`
arithmetic throughout, and that no intermediate cast to `u32` or `i32` occurs.
Also verify `int64out` record if it exists and has the same MDEL logic.

## C Locations
- `modules/database/src/std/rec/int64inRecord.c:monitor` — changed `DELTA` macro cast from `epicsUInt32` to `epicsUInt64`, comparison operand from `epicsUInt32` to `epicsUInt64`
