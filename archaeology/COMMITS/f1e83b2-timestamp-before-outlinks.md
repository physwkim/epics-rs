---
sha: f1e83b22f24680e06422acd95bae2d30541b0918
short_sha: f1e83b2
date: 2017-02-18
author: Bruce Hill
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/records/ao.rs
    function: process
  - crate: base-rs
    file: src/server/records/bo.rs
    function: process
  - crate: base-rs
    file: src/server/records/calcout.rs
    function: process
  - crate: base-rs
    file: src/server/records/waveform.rs
    function: process
tags: [timestamp, outlink, TSEL, ordering, record-processing]
---

# Timestamp updated after outlinks: downstream TSEL reads stale timestamp

## Root Cause
Output record types (ao, bo, aao, calcout, int64out, longout, mbboDirect,
mbbo, stringout, aSub) called `recGblGetTimeStamp()` / 
`recGblGetTimeStampSimm()` AFTER processing outlinks. Any downstream
record with `TSEL` pointing to this record's TIME field would read the
old (pre-process) timestamp because the outlink processing had already
propagated the record scan before the timestamp was updated.

## Symptoms
Records downstream of output records that use `TSEL` to inherit the
timestamp receive an incorrect (previous-cycle) timestamp instead of the
current process timestamp. In time-synchronized data acquisition systems
this causes timestamp mismatches between the output record value and its
linked timestamp field.

## Fix
Moved `recGblGetTimeStamp()` call to occur BEFORE the outlink processing
phase (before `writeValue()` / `recGblFwdLink()`). For asynchronous
devices (where `pact` was already TRUE when process entered), the
timestamp is updated again after async completion to reflect actual write
time. Synchronous devices get a single pre-outlink timestamp update.

## Rust Applicability
In base-rs `records/ao.rs`, `bo.rs`, `calcout.rs`, `waveform.rs`, and
other output records, the `process()` function must call the timestamp
update step before invoking outlinks or forward links. If the Rust
record processing pipeline calls `get_timestamp()` at the end of
`process()`, downstream TSEL links see stale timestamps.

## Audit Recommendation
In `src/server/records/ao.rs` (and bo.rs, calcout.rs, waveform.rs):
find the `process()` function. Verify `recgbl_get_timestamp()` /
`get_timestamp()` is called BEFORE the `write_value()` / `fwd_link()`
calls. For async records (pact=true on re-entry), verify timestamp is
refreshed after async completion.

## C Locations
- `modules/database/src/std/rec/aoRecord.c:process` — timestamp before writeValue, again on async completion
- `modules/database/src/std/rec/boRecord.c:process` — same pattern
- `modules/database/src/std/rec/aaoRecord.c:process` — same pattern
- `modules/database/src/std/rec/calcoutRecord.c:process` — same pattern
- `modules/database/src/std/rec/int64outRecord.c:process` — same pattern
- `modules/database/src/std/rec/longoutRecord.c:process` — same pattern
- `modules/database/src/std/rec/mbboDirectRecord.c:process` — same pattern
- `modules/database/src/std/rec/mbboRecord.c:process` — same pattern
- `modules/database/src/std/rec/stringoutRecord.c:process` — same pattern
- `modules/database/src/std/rec/aSubRecord.c:process` — timestamp before outlink push
