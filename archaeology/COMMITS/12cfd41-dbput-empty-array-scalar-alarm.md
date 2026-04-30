---
sha: 12cfd418d62e066e1ea08ef2af6603d27168999d
short_sha: 12cfd41
date: 2020-07-06
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_put
tags: [empty-array, scalar, alarm, LINK_ALARM, dbPut]
---

# dbPut raises LINK/INVALID alarm when writing empty array into scalar field

## Root Cause
`dbPut()` restructured the scalar vs. array dispatch logic. Previously, a put of `nRequest=0` elements into a scalar field fell through to the fast scalar put path (`dbFastPutConvertRoutine`) with `nRequest=1`, silently using uninitialized/stale buffer data. The fix restructures the branches so that the array path (guarded by `SPC_DBADDR`) handles `nRequest < 1` by raising a `LINK_ALARM / INVALID_ALARM` instead of converting garbage data.

## Symptoms
Writing an empty array (`nRequest=0`) to a scalar-backed field would silently call `dbFastPutConvertRoutine` with 0 elements, converting whatever happened to be at the head of the buffer as if it were a valid value. No alarm was raised; the scalar PV would receive a corrupt value.

## Fix
Restructured `dbPut()` so the array branch (`SPC_DBADDR`) checks `nRequest < 1` before conversion. If zero elements, calls `recGblSetSevr(precord, LINK_ALARM, INVALID_ALARM)` and skips the conversion entirely.

## Rust Applicability
In base-rs `db_put`, the element count check before conversion must guard against zero-length input to scalar fields. This is an explicit logic guard that Rust's type system cannot enforce automatically (the element count is a runtime value). Audit the scalar-put fast path for zero-element handling.

## Audit Recommendation
In `base-rs/src/server/database/db_access.rs::db_put` (or equivalent), verify that when `n_request == 0` and the target is a scalar field (no array support), the code sets a LINK/INVALID alarm and returns without calling the fast-put converter.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPut` — restructured scalar/array branch; empty-array-to-scalar now sets LINK_ALARM
