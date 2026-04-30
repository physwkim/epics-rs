---
sha: 84f47716917b47d68ace3f3c63b64059ece56e58
short_sha: 84f4771
date: 2022-05-11
author: Simon Rose
category: bounds
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/records/compress_record.rs
    function: compress_array
tags: [early-return, compress-record, partial-buffer, array-average, input-count]
---
# compressRecord: compress_array rejects valid partial input when PBUF=YES

## Root Cause
In `compressRecord.c:compress_array()`, the early-return guard was:
```c
n = prec->n;
if (no_elements < n)
    return 1; /*dont do anything*/
```
This rejected processing whenever the number of input elements was less than
the configured `N` value — including when `PBUF=YES` (partial buffer output
enabled) where it should be valid to process partial arrays.

Also, after the guard, `n` was set to `prec->n`, but the downstream averaging
used `n` as the number of *input* elements to average over, which should be
`no_elements` when the input is smaller than `prec->n`.

## Symptoms
- With `PBUF=YES` and `ALG=Average`, if the source waveform has fewer elements
  than `N`, the compress record silently does nothing (returns early).
- Single-element input into an array-average compress record with PBUF was
  discarded.
- Test "Single input data test passes" confirms the bug: the record refused
  to process a 1-element array with N=2.

## Fix
1. Changed the guard to only skip when PBUF is not set:
   ```c
   if (no_elements < prec->n && prec->pbuf != menuYesNoYES)
       return 1;
   ```
2. Set `n = no_elements` (not `prec->n`) so the actual number of input
   elements drives the averaging computation.
3. Added `#include "menuYesNo.h"` to access `menuYesNoYES`.

Also in the same commit: `compress_scalar()` gained the same PBUF check:
```c
if (inx >= prec->n || prec->pbuf == menuYesNoYES)
```
so that partial scalar output is flushed even before N samples accumulate.

## Rust Applicability
A Rust compressRecord would implement the same early-return guard. The
condition must check whether partial-buffer mode is enabled before rejecting
sub-N input arrays. The Rust code should use an enum for `PBUF` / `BALG`
and the guard should explicitly handle the partial case.

## Audit Recommendation
In `base-rs` compress record: verify that `compress_array()` does not
unconditionally reject input arrays smaller than `N` when partial-buffer mode
is enabled. Check the `pbuf` / partial-output flag handling in both array and
scalar paths.

## C Locations
- `modules/database/src/std/rec/compressRecord.c:compress_array` — guard conditioned on !PBUF; n set to no_elements
- `modules/database/src/std/rec/compressRecord.c:compress_scalar` — PBUF added to flush condition
