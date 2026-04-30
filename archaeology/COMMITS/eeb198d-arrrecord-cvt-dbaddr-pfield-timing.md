---
sha: eeb198db15d9f55be63c84434e02d4959bb60eb8
short_sha: eeb198d
date: 2020-03-30
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: cvt_dbaddr
tags: [dbaddr, pfield, array, waveform, record-support]
---

# arrRecord: Move pfield assignment from cvt_dbaddr to get_array_info

## Root Cause
`arrRecord::cvt_dbaddr()` assigned `paddr->pfield = prec->bptr` — the pointer
to the array data buffer — at address-resolution time. However, `bptr` can be
reallocated when the record is processed (e.g., when the array size changes).
For waveform-like records the convention is that `pfield` must point to the
*current* buffer as of the moment `get_array_info` is called (which is
protected by the scan lock), not the buffer that existed at address-resolution
time (which may be stale by the time a read actually occurs).

This also fixes a test bug where `db_post_events` was called with `ptarg->bptr`
instead of `&ptarg->val`, which posted events against the raw array buffer
pointer rather than the VAL field descriptor, bypassing normal field-level
notification.

## Symptoms
- Stale buffer pointer in `paddr->pfield` if `bptr` is reallocated between
  `dbNameToAddr` (which calls `cvt_dbaddr`) and the actual `dbGet` call.
- Read of freed memory if the old buffer was deallocated.
- `db_post_events` in tests posting against a raw buffer address rather than
  the canonical VAL field, potentially missing monitors.

## Fix
Removed `paddr->pfield = prec->bptr` from `cvt_dbaddr()`. Added it to
`get_array_info()` instead, where it is called under scan lock at actual access
time. Fixed test code to use `&ptarg->val` instead of `ptarg->bptr` for
`db_post_events`.

## Rust Applicability
Partial. In base-rs, array record types that use a heap-allocated backing
buffer (equivalent to `bptr`) must expose the buffer pointer only at
access time (inside scan-lock), not at address-resolution time. If any
Rust record type stores a `*mut T` in a field that could be reallocated,
ensure that pointer is refreshed (not cached) each time data is accessed.

## Audit Recommendation
In `base-rs/src/server/database/db_access.rs:cvt_dbaddr`, verify that
`pfield` (or its Rust equivalent field reference) is not populated until
`get_array_info` time for array-type records. Prefer using an `Arc<Vec<T>>`
or snapshot reference obtained inside the scan-lock at read time.

## C Locations
- `modules/database/test/ioc/db/arrRecord.c:cvt_dbaddr` — removed pfield assignment (moved to get_array_info)
- `modules/database/test/ioc/db/arrRecord.c:get_array_info` — added pfield = prec->bptr
- `modules/database/test/ioc/db/dbCaLinkTest.c` — db_post_events fixed to use &ptarg->val
