---
sha: aff74638bd7cc6e894c7de78ddee11c851a6be60
short_sha: aff7463
date: 2023-03-08
author: Henrique Silva
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec/aai.rs
    function: process
  - crate: base-rs
    file: src/server/database/rec/aao.rs
    function: process
tags: [NORD, aai, aao, db_post_events, monitor, record-layer]
---
# aai and aao process: add NORD db_post_events when element count changes

## Root Cause
`aaiRecord.c:process()` and `aaoRecord.c:process()` did not post `db_post_events`
for the NORD field when device support changed the element count. For `aai`,
NORD was only posted in `put_array_info()` (the dbPut path), not in the
process/read path. For `aao`, NORD was never posted from `process()`. This
left CA monitors on NORD stale after every normal scan cycle.

This is the companion to 5d1f572 (remove NORD posting from device support),
which removed the duplicate posting from `devAaiSoft.c` and `devWfSoft.c`.
The fix sequence establishes correct layering: record support owns all event
notification, device support is notification-free.

## Symptoms
- Monitors subscribed to `aai.NORD` see stale (initial) values during normal
  IOC operation; NORD only updates when written via `dbPut` (the `put_array_info`
  path) rather than when the device support read changes the element count.
- `aao.NORD` monitors receive no events at all from the process path.

## Fix
In `aaiRecord.c:process()`: snapshot `nord = prec->nord` before calling device
support, then post `db_post_events(&prec->nord, DBE_VALUE|DBE_LOG)` if the
count changed.
In `aaoRecord.c:process()`: same snapshot + conditional post.
The `put_array_info()` path in `aaiRecord.c` already had correct NORD posting
and is unchanged.

## Rust Applicability
In `base-rs/src/server/database/rec/aai.rs::process` and `rec/aao.rs::process`:
snapshot NORD before calling device support, then post NORD change events after.
Pattern: `let old_nord = rec.nord; device_support.read(rec)?; if rec.nord !=
old_nord { post_events(&rec.nord, DBE_VALUE | DBE_LOG); }`. This is a universal
pattern for all array records (aai, aao, subArray, waveform).

## Audit Recommendation
Check all array-type record `process()` implementations in
`base-rs/src/server/database/rec/`: verify each one snapshots NORD before
device support call and posts NORD events if changed. The four records
affected by this commit cluster (51c5b8f, 64011ba, 5d1f572, aff7463) are:
subArray, aai, aao, waveform.

## C Locations
- `modules/database/src/std/rec/aaiRecord.c:process` — snapshot `nord`, post `db_post_events` if changed
- `modules/database/src/std/rec/aaoRecord.c:process` — snapshot `nord`, post `db_post_events` if changed
