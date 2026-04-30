---
sha: 832abbd3b1fcbf5d15f1beb7da1a4bf3d8a4f0b5
short_sha: 832abbd
date: 2022-12-20
author: Brendan Chandler
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/records/sub_record.rs
    function: process
tags: [error-propagation, silent-failure, subrecord, bad-link, process]
---
# subRecord: propagate error from bad INP links instead of silently succeeding

## Root Cause
In `subRecord.c:process()`, the function called `fetch()` to load each INPx
link into the corresponding field. `fetch()` returns a non-zero status on
error (e.g., link to a non-existent PV). The `process()` function accumulated
the status from each `fetch()` call into a local `status` variable, then
continued processing regardless. At the end, it called the subroutine
function (`psubroutine`) only if `status == 0`, but then **always returned 0**
(success) to the caller, discarding the accumulated error status.

This meant that a `dbPut()` to a subRecord's PROC field with a bad INPx link
would appear to succeed (`dbPut` returned 0) even though the subroutine never
ran and PROC was left modified as if it ran.

## Symptoms
- Silent failure: a write to a subRecord with an invalid INPx link appears
  to succeed (`dbPut()` returns 0).
- The subroutine is never executed, and no error is visible to the operator.
- The test case demonstrates: `testdbPutFieldFail(-1, "InvalidINPARec.PROC",
  DBF_LONG, 1)` previously passed (incorrectly indicating success).

## Fix
Changed `return 0;` to `return status;` at the end of `process()`. The
accumulated `status` from `fetch()` calls is now propagated to the caller,
causing `dbPut()` to return an error when any INPx link fails.

## Rust Applicability
In a Rust record implementation, `process()` would return `Result<(), Error>`.
Each link fetch would use `?` to propagate errors immediately. Silent
discarding of `Result` values produces a compiler warning (`unused Result`),
so the bug pattern is harder to introduce silently. However:
- The specific question of whether `process()` should abort early on first
  bad link or collect all errors is a design decision to audit.
- Ensure the Rust subRecord equivalent propagates link-fetch errors back to
  the `dbPut` caller rather than swallowing them.

## Audit Recommendation
In `base-rs` subRecord (or equivalent): verify that the `process` function
propagates `Err` from each input link fetch to the caller rather than always
returning `Ok(())`.

## C Locations
- `modules/database/src/std/rec/subRecord.c:process` — `return 0` changed to `return status`
