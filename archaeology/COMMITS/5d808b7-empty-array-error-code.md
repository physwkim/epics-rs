---
sha: 5d808b7c0242486f0dc4e18fa62cb2af3a40a75c
short_sha: 5d808b7
date: 2020-05-07
author: Dirk Zimoch
category: bounds
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [empty-array, error-code, bounds, distinguishable-errors, S_db_emptyArray]
---

# Introduce distinct error code for zero-element array reads

## Root Cause
Empty-array conditions were previously returned as `S_db_badField`, which
is a generic field-format error. Callers could not distinguish "field
type mismatch" from "array is empty". The same code was used for: (1)
`dbGet` when the filter log reports zero elements, and (2) `dbDbGetValue`
when the channel's final element count is zero or the post-filter
`*pnRequest` is zero.

## Symptoms
Callers that needed to treat empty arrays specially (e.g., setting a LINK
alarm without logging an error) could not differentiate from genuine bad
field type errors, leading to either spurious errors or missed alarm
states.

## Fix
Added `S_db_emptyArray` (status code 69) to `dbAccessDefs.h` and
replaced all `S_db_badField` returns for empty-array conditions in
`dbAccess.c` and `dbDbLink.c`.

## Rust Applicability
In base-rs, the Rust equivalent is likely an enum variant on a `DbError`
or `AccessError` type. If a single `BadField` variant covers both
type-mismatch and empty-array cases, callers in `db_access.rs` and
`links.rs` cannot apply distinct handling. A dedicated `EmptyArray`
variant enables callers to set LINK alarm without logging.

## Audit Recommendation
Search `src/server/database/db_access.rs` and `links.rs` for the error
enum returned when element count is zero. Verify there is a distinct
variant (e.g., `EmptyArray`) vs. field-type mismatch errors.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — returns `S_db_emptyArray` when `no_elements < 1`
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — returns `S_db_emptyArray` for zero elements from channel or post-filter
- `modules/database/src/ioc/db/dbAccessDefs.h` — defines `S_db_emptyArray` (M_dbAccess|69)
