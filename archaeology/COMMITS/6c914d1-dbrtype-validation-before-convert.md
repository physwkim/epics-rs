---
sha: 6c914d19c3cfa2d183e44b7fe2c6211ab8c24a58
short_sha: 6c914d1
date: 2020-06-01
author: Michael Davidsaver
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: ca_get_callback
  - crate: base-rs
    file: src/server/database/db_convert_json.rs
    function: convert
tags: [dbrtype, validation, out-of-bounds, table-lookup, type-check]
---

# Validate dbrType before indexing conversion table to prevent OOB access

## Root Cause
Multiple functions in the database layer (`dbGet`, `dbCa`, `dbConvertJSON`,
and several link type implementations: `lnkCalc`, `lnkConst`, `lnkState`)
indexed directly into `dbFastGetConvertRoutine[dbrType]` or
`dbFastPutConvertRoutine[dbrType]` without first verifying that `dbrType` was
in the valid range `[0, DBR_ENUM]`. A caller passing an out-of-range `dbrType`
(e.g., a large positive value from a malformed CA request or a bug in a record
support module) would read a function pointer from beyond the end of the table,
causing a segmentation fault or executing an arbitrary function pointer.

## Symptoms
- Crash (`SIGSEGV` or equivalent) when any link type's `getValue`/`loadScalar`/
  `loadArray` is called with an invalid `dbrType`.
- Potential arbitrary code execution if the out-of-bounds table read fetches a
  controllable function pointer (remote code execution via malformed CA PUT).

## Fix
Added `if(INVALID_DB_REQ(dbrType)) return S_db_badDbrtype;` guard at the top
of each affected function before any use of `dbrType` as a table index.
`INVALID_DB_REQ` is the existing macro that checks `(dbrType) < 0 || (dbrType) > LAST_BUFFER_TYPE`.
Also moved the `dbFastGetConvertRoutine` table lookup *after* the guard in
`lnkCalc` and `lnkState` to prevent use of the local variable before initialization.

## Rust Applicability
Applies. In base-rs, any function that accepts a `dbrType` (or Rust enum
equivalent) and indexes into a conversion dispatch table must validate the input
before dispatch. If `dbrType` is represented as a raw integer (e.g., from a CA
protocol message), the conversion to a typed Rust enum must happen at the
network boundary with explicit error handling, not inside the hot path.

## Audit Recommendation
- In `base-rs/src/server/database/db_access.rs:db_get`, verify that `dbrType`
  received from a CA message is validated against known types before any
  conversion dispatch.
- In `base-rs/src/server/database/db_ca.rs:ca_get_callback`, same check.
- In any JSON link or const-link implementation: ensure `dbrType` validation
  occurs before array indexing.
- Prefer wrapping raw type codes in a `TryFrom`-validated enum at parse time.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — INVALID_DB_REQ guard added
- `modules/database/src/ioc/db/dbCa.c` — INVALID_DB_REQ guard added
- `modules/database/src/ioc/db/dbConvertJSON.c` — INVALID_DB_REQ guard added
- `modules/database/src/std/link/lnkCalc.c:lnkCalc_getValue` — guard + deferred table lookup
- `modules/database/src/std/link/lnkConst.c:lnkConst_loadScalar,loadArray` — INVALID_DB_REQ guard
- `modules/database/src/std/link/lnkState.c:lnkState_getValue` — guard + deferred table lookup
