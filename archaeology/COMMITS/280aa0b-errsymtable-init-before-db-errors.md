---
sha: 280aa0b399b3484fc72637570d45658616f09385
short_sha: 280aa0b
date: 2025-10-08
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/static_lib.rs
    function: db_read_database
tags: [errlog, errSymTable, init-ordering, dbReadDatabase, lifecycle]
---
# Initialize errSymTable before database errors can occur in dbReadCOM

## Root Cause
`dbReadCOM()` (called from `dbReadDatabase` / `dbReadDatabaseFP`) begins
parsing the database definition file immediately, and any parse error
calls `errlogPrintf` which in turn tries to look up error symbols in
`errSymTable`. However, `errlogInit` (which populates `errSymTable`) was
not guaranteed to have been called before `dbReadCOM` was invoked. In
particular, `dbReadDatabase` can be called from `iocsh` before `iocInit`,
at which point `errSymTable` is uninitialized, causing either a null-ptr
dereference or silent loss of the error symbol lookup.

## Symptoms
Early database load errors (e.g., malformed `.db` file before `iocInit`)
produce garbled or missing error symbols in the log output. In the worst
case, `errlogPrintf` dereferences an uninitialized pointer, crashing the
IOC.

## Fix
Add `errlogInit(0)` at the very top of `dbReadCOM`, before
`dbAllocBase()` and before any parsing begins. This is idempotent — if
`errlogInit` was already called, the call is a no-op.

## Rust Applicability
`partial` — base-rs's database loader (`db_read_database` or equivalent)
should ensure the error/logging subsystem is initialized before the first
parse error can be emitted. In Rust this is less critical because `tracing`
/ `log` crates are always safe to call, but any EPICS-specific error
symbol table that is lazily initialized should be checked for the same
ordering gap.

## Audit Recommendation
Audit `base-rs/src/server/database/static_lib.rs::db_read_database` (or the
equivalent entry point for loading `.db` files) to verify that the error
reporting subsystem is fully initialized before parsing begins.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbReadCOM` — `errlogInit(0)` added before any parsing
