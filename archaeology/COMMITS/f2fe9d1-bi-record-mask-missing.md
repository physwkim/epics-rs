---
sha: f2fe9d12032569a8b1d3f09ddf7e07f2ce8c04dd
short_sha: f2fe9d1
date: 2023-11-02
author: Michael Davidsaver
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dev/bi_soft_raw.rs
    function: read_locked
tags: [bi-record, MASK, RVAL, device-support, record-processing]
---

# bi "Raw Soft Channel" did not apply MASK to RVAL

## Root Cause
The `devBiSoftRaw` device support for the binary input record read a raw value into `RVAL` via a soft channel link but never applied the `MASK` field. The standard CA `bi` record processing pipeline applies MASK during hardware device support reads, but the "Raw Soft Channel" path skipped this step. The result: if an operator configured a non-zero MASK, it would be silently ignored, causing the record to process the full raw integer value rather than the masked subset of bits.

## Symptoms
A `bi` record configured with `DTYP="Raw Soft Channel"` and a non-zero `MASK` would see all bits of the linked RVAL rather than only the masked bits. This would cause incorrect `VAL`/alarm evaluation whenever the source value had bits set outside the mask.

## Fix
Added `if (prec->mask) prec->rval &= prec->mask;` immediately after the soft-channel link read in `readLocked()`, before timestamp processing. This aligns "Raw Soft Channel" behavior with hardware device support.

## Rust Applicability
Applies. `base-rs` implements device support for standard record types. If `bi` "Raw Soft Channel" device support exists in `base-rs`, it must apply `MASK` to `RVAL` after the link read. This is a correctness requirement for protocol-correct record processing.

## Audit Recommendation
In `base-rs/src/server/database/dev/bi_soft_raw.rs` (or equivalent), verify that `rval &= mask` is applied when `mask != 0` after reading the raw value from the input link. This must happen before the standard convert-RVAL-to-VAL pipeline.

## C Locations
- `modules/database/src/std/dev/devBiSoftRaw.c:readLocked` — added `if (prec->mask) prec->rval &= prec->mask`
