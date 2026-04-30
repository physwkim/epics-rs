---
sha: 51c5b8f1ee744c65244bc4d1a4cbea99210b2f42
short_sha: 51c5b8f
date: 2023-03-09
author: Henrique Silva
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec/subarray.rs
    function: process
tags: [NORD, subArray, db_post_events, monitor, record-process]
---
# subArray process: missing NORD db_post_events when element count changes

## Root Cause
`subArrayRecord`'s `process()` function called device support `read_sa()` which
could change `prec->nord` (number of elements read), but `process()` never
called `db_post_events(&prec->nord, DBE_VALUE|DBE_LOG)` to notify monitors of
the NORD field change. This was the record layer's responsibility.

Meanwhile, `devSASoft.c:read_sa()` (the companion commit 64011ba) had
duplicated this event posting in the device support layer, which is
architecturally wrong: event posting belongs in record support, not device
support, so that all device support implementations get consistent behaviour.

## Symptoms
- Monitors subscribed to `record.NORD` do not receive updates when the subArray
  reads a different number of elements than the previous cycle.
- CA clients watching `.NORD` see a frozen value even as the array data changes
  in length.

## Fix
Snapshot `nord = prec->nord` before calling device support, then after
`read_sa()` and status check, compare and post `DBE_VALUE|DBE_LOG` if changed.
This mirrors the fix applied simultaneously to `aaiRecord` and `aaoRecord`.
The companion commit (64011ba) removes the now-duplicate posting from
`devSASoft.c`.

## Rust Applicability
In `base-rs`, any array record's `process()` function that calls device support
to read data must compare NORD before and after the read and call
`db_post_events` (or the Rust equivalent monitor notification) on the NORD
field if it changed. This must be in the record layer, not delegated to device
support. The pattern: `let old_nord = prec.nord; dev_support.read(prec)?;
if prec.nord != old_nord { post_events(&prec.nord, DBE_VALUE | DBE_LOG); }`.

## Audit Recommendation
In `base-rs/src/server/database/rec/subarray.rs::process`,
`rec/aai.rs::process`, `rec/aao.rs::process`, and `rec/waveform.rs::process`:
confirm NORD change detection and event posting is in the record layer.
Verify device support implementations do NOT duplicate NORD event posting.

## C Locations
- `modules/database/src/std/rec/subArrayRecord.c:process` — snapshot `nord`, post `db_post_events` if changed after `read_sa()`
