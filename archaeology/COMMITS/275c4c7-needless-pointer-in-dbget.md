---
sha: 275c4c7cf42c3a5bad3c38a4ef0a2d6d8e6bf98a
short_sha: 275c4c7
date: 2020-05-07
author: Dirk Zimoch
category: bounds
severity: low
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
tags: [pointer-indirection, empty-array, bounds, dbAccess, off-by-one]
---

# Wrong pointer deref in empty-array guard in dbGet

## Root Cause
`dbGet()` checked `pfl->no_elements < 1` to guard against empty-array
processing, but `pfl` at that code point is the filter-log pointer whose
`no_elements` may differ from the local `no_elements` variable that
reflects the actual element count after filtering. The correct variable
is the local `no_elements`.

## Symptoms
When a channel filter reduced the element count to zero, `dbGet` would
read element count from the wrong struct field and potentially continue
into array processing with zero elements, causing undefined behavior or
crashes downstream.

## Fix
Replace `pfl->no_elements` with the local `no_elements` variable in the
empty-array early-exit guard inside `dbGet`.

## Rust Applicability
In base-rs `db_access.rs`, any `db_get`-equivalent that derives element
count from a field-log intermediate struct and then applies the guard
must use the locally-computed count, not a struct field. A stale or
wrong source of `no_elements` causes identical silent wrong-count
processing.

## Audit Recommendation
Grep for `no_elements` in `src/server/database/db_access.rs`. Verify
that the empty-array guard after filter application uses the
locally-scoped count variable, not a struct field from the filter log.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — replaced `pfl->no_elements` with local `no_elements`
