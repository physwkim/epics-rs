---
sha: e11f88017da959e7ed5a2481819b93b2376f3f5b
short_sha: e11f880
date: 2022-10-18
author: Jure Varlec
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/filters/ts.rs
    function: replace_fl_value
tags: [db-field-log, api-change, destructor, filter, timestamp]
---
# ts Filter Uses Stale db_field_log API — dtor Field Moved Out of Union

## Root Cause
The `db_field_log` struct underwent an API change: the `dtor` (destructor)
function pointer was moved from inside the `u.r` union sub-struct
(`pfl->u.r.dtor`) to the top-level struct (`pfl->dtor`). The `ts.c` filter
was not updated and continued to read/write `pfl->u.r.dtor`, which after the
struct rearrangement referred to a different memory location — effectively
reading/writing an unrelated field.

This is a classic API migration bug where the old field name was not removed
or deprecated immediately, so the old access compiled but silently operated
on wrong memory.

## Symptoms
- `ts` filter's destructor for allocated timestamp arrays was registered at
  `pfl->u.r.dtor` but the runtime cleaned up via `pfl->dtor`, leaving the
  destructor unregistered.
- Allocated timestamp array memory was leaked when the field log was
  destroyed.
- The `channelRegisterPost` probe path also failed to call the dtor before
  replacing the value, leaking the probe's previous allocation.

## Fix
Replace all `pfl->u.r.dtor` and `probe->u.r.dtor` accesses with `pfl->dtor`
/ `probe->dtor` to match the new `db_field_log` struct layout.

## Rust Applicability
`partial` — If `base-rs` implements channel filter plugins including a
timestamp (`ts`) filter, the `DbFieldLog` struct must place the destructor
callback at the correct level (top-level, not inside a union sub-field).
Any Rust port of the `ts` filter must register the drop function via the
correct field. More broadly, any Rust `DbFieldLog` wrapper should ensure
field layout matches what the runtime cleans up.

## Audit Recommendation
In `base-rs`, if a `DbFieldLog` struct is defined with a destructor callback,
confirm the callback field is at the top struct level and not nested inside
a variant/union sub-struct. Verify ts filter equivalent registers its
cleanup via the same path the runtime uses to invoke cleanup.

## C Locations
- `modules/database/src/std/filters/ts.c:replace_fl_value` — `pfl->u.r.dtor` should be `pfl->dtor`
- `modules/database/src/std/filters/ts.c:ts_array` — same
- `modules/database/src/std/filters/ts.c:ts_string` — same
- `modules/database/src/std/filters/ts.c:channelRegisterPost` — probe dtor not cleaned up via new field
