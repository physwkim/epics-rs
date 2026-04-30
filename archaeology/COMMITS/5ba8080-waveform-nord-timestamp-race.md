---
sha: 5ba8080f6ed698da477cc33b54ee939f372fa031
short_sha: 5ba8080
date: 2022-05-13
author: Bruce Hill
category: race
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/waveform_record.rs
    function: process
tags: [waveform, NORD, timestamp, event-ordering, camonitor]
---
# Waveform NORD posted before timestamp update causes undefined timestamp on first CA monitor update

## Root Cause
In `waveformRecord.c`, `db_post_events` for NORD was called inside
`readValue()` (called from `process()`) before `recGblGetTimeStampSimm()` was
invoked in `process()`. On the very first CA monitor update, the posted event
carried an uninitialized (zero/epoch) timestamp because the record's timestamp
had not yet been set when the event was dispatched.

## Symptoms
The first CA monitor update for the `NORD` field of a waveform record showed
a timestamp of 1990-01-01 00:00:00 (EPICS epoch zero) or similar garbage. This
affected any CA client using `camonitor` or equivalent that subscribed to the
NORD field.

## Fix
- Capture `nord = prec->nord` at the top of `process()`, before calling device
  support (`readValue()`).
- Remove the `db_post_events(NORD)` call from inside `readValue()`.
- After `recGblGetTimeStampSimm()` completes in `process()`, compare saved
  `nord` with updated `prec->nord` and post the event then (with correct TS).
- This ensures the NORD event always carries the timestamp obtained during the
  same `process()` invocation.

## Rust Applicability
In base-rs waveform record processing, if `db_post_events` for NORD (or an
equivalent `nord_changed` notification) is triggered inside the device-support
read call before the timestamp has been obtained, the same stale-timestamp bug
applies. The pattern to audit: any `post_events` / `notify_subscribers` call
that happens before `get_timestamp()` in the record processing chain.

## Audit Recommendation
In `waveform_record.rs::process()`, verify that NORD-change notifications are
posted only after the record timestamp has been stamped (i.e., after the
equivalent of `recGblGetTimeStampSimm`). Snapshot `nord` before calling into
device support, then post the event after the timestamp is set.

## C Locations
- `modules/database/src/std/rec/waveformRecord.c:process` — capture nord before device support, post after timestamp
- `modules/database/src/std/rec/waveformRecord.c:readValue` — remove early `db_post_events(NORD)` call
