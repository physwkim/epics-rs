---
sha: 11a4bed9aa6007f3c8617e2eaeac155b30f02dae
short_sha: 11a4bed
date: 2022-05-11
author: Simon Rose
category: bounds
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/records/compress_record.rs
    function: compress_scalar
tags: [accumulation-bug, average, compress-record, inx, partial-buffer]
---
# compressRecord: compress_scalar average computation is incorrect

## Root Cause
In `compressRecord.c:compress_scalar()`, the N-to-1 average algorithm was
implemented incorrectly for the accumulation case:

```c
if (inx == 0)
    *pdest = value;
else {
    *pdest += value;
    if (inx + 1 >= prec->n)
        *pdest = *pdest / (inx + 1);
}
```

This computes a running sum, then divides only when `inx + 1 >= prec->n`.
However:
1. When `PBUF=YES` (push-buffer / partial output enabled), `put_value()` is
   called before `inx + 1 >= prec->n`, but the sum has not been divided yet
   — so the partial output is the raw sum, not the average.
2. The `inx` reset after partial output was `prec->inx = 0` unconditionally,
   discarding the current sample even when PBUF triggered mid-accumulation.

The fix replaces the sum+conditional-divide with an incremental mean formula:
```c
*pdest = (inx * (*pdest) + value) / (inx + 1);
```
This maintains a running mean at every step, so any intermediate `put_value()`
produces the correct partial average.

Also fixed: `prec->inx` reset:
- Old: always `prec->inx = 0`
- New: `prec->inx = (inx >= prec->n) ? 0 : inx` — when PBUF triggers mid-
  accumulation (inx < n), the index continues from where it left off.

## Symptoms
- N-to-1 Average compress record with `PBUF=YES` outputs running sum instead
  of running average on each intermediate put.
- After a full N samples, the output is correct (sum/N), but intermediate
  pushed values are wrong (multiplied by the sample count).
- Example (N=4, partial output): inputs 1,2,3,4 → expected 1,1.5,2,2.5;
  actual (before fix): 1,3,6,10 (then 2.5 on final flush).

## Fix
Replace sum+divide with incremental mean. Fix `prec->inx` reset to retain
position when PBUF triggers before N samples are accumulated.

## Rust Applicability
A Rust compressRecord implementation would implement the same N-to-1 average
logic. The incremental mean formula `(n * mean + x) / (n + 1)` should be
verified for floating-point precision (potential for cancellation at large `n`).
The `inx` state must be correctly preserved on partial flushes.

## Audit Recommendation
In `base-rs` compress record: verify that the N-to-1 scalar average uses the
incremental mean formula, and that `inx` is not unconditionally reset to 0
when a partial push is triggered.

## C Locations
- `modules/database/src/std/rec/compressRecord.c:compress_scalar` — running sum replaced with incremental mean; inx reset conditionalized
