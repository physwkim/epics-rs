---
sha: 7890e67d378ba3a0909e44d2be11fbe1b5e58711
short_sha: 7890e67
date: 2023-12-24
author: Andrew Johnson
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [warning-cleanup, pointer-cast, debug-guard, cast_server, dbLink]
---

# Miscellaneous Warning Fixes: Pointer Cast and Debug Guard

## Root Cause
Several minor issues accumulated:
1. `registerAllRecordDeviceDrivers.cpp`: `lhs.sizeOffset < rhs.sizeOffset`
   compared function pointers directly (pre-uintptr_t fix), fixed here to
   `(char *)` cast (intermediate step before the later uintptr_t fix).
2. `cast_server.c`: Debug print guarded by `#ifdef DEBUG` macro, but the code
   used `CASDEBUG` runtime variable. The `#ifdef DEBUG` guard was incorrect —
   it was a compile-time symbol that was never defined in normal builds, so the
   log was silently suppressed. Fixed to use `if (CASDEBUG>1 ...)`.
3. `dbLink.h`: Documentation comment typo ("where to put the value" for a put
   function, should be "where the data is").
4. `dbStressLock.c`: Silenced unused-variable warning for `sum`.

## Symptoms
For cast_server: expiry log messages for timed-out CA channels were never
printed even when `CASDEBUG` was set, hiding operational problems.

## Fix
- Replaced `#ifdef DEBUG` with `if (CASDEBUG>1 && ndelete)` in `clean_addrq`.
- Added `(char *)` cast for pointer comparison (intermediate step).
- Fixed doc comment and silenced test unused-variable warning.

## Rust Applicability
Eliminated. Rust does not use C preprocessor debug guards; logging uses the
`tracing` crate with runtime-controlled levels. No equivalent pattern exists.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/rsrv/cast_server.c:clean_addrq` — `#ifdef DEBUG` suppressed CA channel expiry log
- `modules/database/src/ioc/db/dbLink.h` — doc comment error
