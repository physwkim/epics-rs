---
sha: 3b3261c877a8b66cb68203bbc399dff08b03a751
short_sha: 3b3261c
date: 2020-05-22
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: dbGet
  - crate: base-rs
    file: src/server/database/db_db_link.rs
    function: dbDbGetValue
tags: [empty-array, error-code, S_db_badField, dbGet, protocol-compat]
---
# Revert S_db_emptyArray — empty array must return S_db_badField for compatibility

## Root Cause
A previous commit introduced a new error code `S_db_emptyArray` for the case
where `dbGet` is called on an array field with zero elements, and `dbDbGetValue`
would return this code when `*pnRequest <= 0`. While semantically distinct,
this new code broke downstream code that checked specifically for `S_db_badField`
to detect the empty-array condition. The new code was also never widely adopted
and its introduction was not coordinated across the whole codebase.

## Symptoms
Any code path that calls `dbGet` or `dbDbGetValue` on an empty array and checks
the return status for `S_db_badField` would silently succeed (the check would
not match), causing incorrect behavior downstream. Link filter tests had been
updated to expect `S_db_emptyArray`, but production code had not been fully
updated.

## Fix
Revert the error code back to `S_db_badField` in both `dbGet` (when
`no_elements < 1`) and `dbDbLink.c::dbDbGetValue` (when `*pnRequest <= 0`).
Remove the `S_db_emptyArray` define from `dbAccessDefs.h`.

## Rust Applicability
In base-rs, the equivalent of `dbGet` on an empty array should return the
`BadField` error variant (not a separate `EmptyArray` variant). If a Rust
`DbError::EmptyArray` was ever introduced to mirror the reverted C code, it
must be removed or aliased to `DbError::BadField`. Any match arm checking for
empty-array via a distinct variant would be wrong.

## Audit Recommendation
Search base-rs for any `EmptyArray` or separate empty-array error path in
`db_access.rs` and `db_db_link.rs`. Ensure `dbGet` returns `Err(DbError::BadField)`
when element count is zero, matching the canonical C behavior post-revert.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — revert `S_db_emptyArray` → `S_db_badField` for zero-element arrays
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — revert `S_db_emptyArray` → `S_db_badField` for zero-count request
- `modules/database/src/ioc/db/dbAccessDefs.h` — remove `S_db_emptyArray` define
