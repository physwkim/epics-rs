---
sha: 82ec539e4943c2479e795f8c8e0940ef9bf3582a
short_sha: 82ec539
date: 2021-08-08
author: Michael Davidsaver
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_put
tags: [dbPut, SPC_DBADDR, long-string, array-info, CALC-field]
---
# dbPut: long-string (nRequest>1) skips get_array_info, corrupts write path

## Root Cause
`dbPut()` had two separate branches:
1. `SPC_DBADDR` fields (arrays): calls `get_array_info`, clips `nRequest`,
   calls `dbPutConvertRoutine`, then calls `put_array_info`.
2. Scalar fields (`else`): directly calls `dbPutConvertRoutine` with `offset=0`.

Long strings (`CALC$`, `LSTR`, etc.) use `nRequest > 1` with a scalar-looking
field, not `SPC_DBADDR`. Because the condition was `paddr->pfldDes->special ==
SPC_DBADDR`, these fields fell into the `else` branch, which did NOT call
`get_array_info` first and skipped the `no_elements` clip. An uninitialized
stack `offset` variable from the enclosing scope was used. After the fix was
removed (`long offset` was removed from the outer scope), this was a latent
use-of-uninitialized-value.

## Symptoms
- Writing a long string value to a `CALC$` field via `dbPut` with `nRequest > 1`
  used an uninitialized `offset`, potentially writing to the wrong buffer
  location or past the end.
- The `put_array_info` call was also skipped, leaving the array metadata
  (element count) stale after the write.

## Fix
The condition changed to `nRequest > 1 || SPC_DBADDR`:
- Any multi-element write (long string OR true array) enters the
  `get_array_info` path.
- `offset` is now declared and initialized to `0` inside this block.
- The `put_array_info` guard is tightened: only called when `SPC_DBADDR` is
  set AND `prset` is non-null.

## Rust Applicability
In `base-rs`'s `db_put()`, the branch selecting between scalar and array write
paths must be `nRequest > 1 || field.is_array_addr()`, not just
`field.is_array_addr()`. Long strings are multi-element scalar fields that
need array cursor handling. Ensure `offset` is initialized to `0` before
calling the convert routine, and that `put_array_info` is only called for true
`SPC_DBADDR` fields.

## Audit Recommendation
In `base-rs/src/server/database/db_access.rs::db_put`: check the branch
condition for entering the `get_array_info` path. Must cover `nRequest > 1`
in addition to `SPC_DBADDR`. Verify the `put_array_info` guard is not called
for long-string writes (it is only meaningful for true array fields). Add a
test: write `nRequest=MAX_STRING_SIZE` to a `CALC$` / `LSI` field.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPut` — condition `SPC_DBADDR` → `nRequest>1 || SPC_DBADDR`; `offset` moved inside block; `put_array_info` guard tightened
