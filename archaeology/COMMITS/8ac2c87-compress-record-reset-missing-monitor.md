---
sha: 8ac2c871563fefc07e757c099ae35a82a00148db
short_sha: 8ac2c87
date: 2025-01-07
author: Érico Nogueira
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/records/compress.rs
    function: special
tags: [compress-record, monitor, RES-field, event-post, array-reset]
---
# compressRecord: post monitor event when reset via RES field

## Root Cause
`compressRecord.c::special` handled the `SPC_RESET` special-processing type
(triggered by writing to the `RES` field) by calling `reset(prec)` which
zeroed the internal array and set `NORD=0`. However, it did NOT call
`monitor(prec)` afterwards. CA monitor subscribers watching `VAL` or `NORD`
therefore received no notification that the buffer was cleared.

## Symptoms
Clients monitoring a compress record's `VAL` field via CA subscriptions did not
receive an update (empty array) when the record was reset by writing to `RES`.
The record's internal state was correct, but subscribers saw stale data until
the next normal processing cycle posted a monitor event.

## Fix
Add `monitor(prec)` call immediately after `reset(prec)` in the `SPC_RESET`
branch of `special()`. This posts a `DBE_VALUE | DBE_LOG` event with the
now-empty array so all subscribers receive the reset notification.

## Rust Applicability
In `base-rs`, the compress record's equivalent of `special()` must call the
event-post mechanism (equivalent of `db_post_events`) after resetting the
internal buffer. If the Rust implementation handles `SPC_RESET` and omits the
post, CA monitor clients will see the same stale-data symptom.

## Audit Recommendation
In `base-rs/src/server/database/records/compress.rs::special`, verify that
writing to the reset field:
1. Clears the internal buffer and sets `NORD=0`.
2. Calls the event-post function with `DBE_VALUE | DBE_LOG` mask immediately
   after the reset, without waiting for the next normal scan cycle.

## C Locations
- `modules/database/src/std/rec/compressRecord.c:special` — add `monitor(prec)` call after `reset(prec)` in `SPC_RESET` branch
