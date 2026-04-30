---
sha: 77092396369ce5655900817c299953c7f1a1465b
short_sha: 7709239
date: 2020-07-17
author: Dirk Zimoch
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_put
tags: [null-deref, rset, put-array-info, dbAccess, function-pointer]
---

# Null guard for put_array_info function pointer before calling in dbPut

## Root Cause
In `dbPut()`, after a successful array put conversion, the code unconditionally called `prset->put_array_info(paddr, nRequest)`. However, `put_array_info` is an optional function pointer in the record support entry table (RSET) — record types that do not implement it will have a NULL pointer. The prior code checked this only for scalar puts (guarded by `paddr->pfldDes->special == SPC_DBADDR`), but the restructured array path (from commit `12cfd418`) removed that outer guard and called `prset->put_array_info` unconditionally.

## Symptoms
Null pointer dereference (crash / SIGSEGV) when putting to an array field of a record type whose RSET does not provide `put_array_info`. Only triggered after a successful array conversion, so the crash is not immediate on all record types.

## Fix
Added `prset->put_array_info` null check before the call:
```c
if (!status && prset->put_array_info)
    status = prset->put_array_info(paddr, nRequest);
```

## Rust Applicability
In base-rs `db_put` (or equivalent), any call to an optional record-support callback must check for `None` before dispatching. If RSET callbacks are modeled as `Option<fn(...)>` in Rust, this is enforced by the type system — but if they are stored as raw function pointers or `unsafe fn` pointers with a sentinel null, the check must be explicit. Audit all call sites of optional RSET methods.

## Audit Recommendation
Audit `base-rs/src/server/database/db_access.rs::db_put` (or equivalent) for all RSET optional-callback dispatch sites. Confirm each is wrapped in `if let Some(f) = prset.put_array_info { f(...) }` or equivalent.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPut` — null guard added for `prset->put_array_info`
