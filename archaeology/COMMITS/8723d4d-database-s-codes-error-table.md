---
sha: 8723d4d9cb90dfd4894ddb16a5695d8166093aaa
short_sha: 8723d4d
date: 2021-02-05
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [error-codes, errSymTbl, database, S_db, build-system]
---
# Database S_* error codes missing from errSymTbl error string table

## Root Cause
The `errSymTbl.c` error string table (generated from `ERR_S_FILES` in the
`libcom/src/error/Makefile`) did not include the database module headers
`dbAccessDefs.h` and `dbStaticLib.h`. As a result, `errSymLookup()` could
not resolve `S_db_*` or `S_dbLib_*` error codes to human-readable strings —
it would return a hex string instead.

Additionally, `dbAccessDefs.h` used a `#ifndef INCerrMdefh` guard around the
`#include "errMdef.h"` that was unnecessary and potentially caused the
include to be skipped if `errMdef.h` had been seen via a different path.

## Symptoms
Error messages from database operations (e.g., `S_db_badDbrtype`,
`S_db_notFound`) would display as numeric codes rather than descriptive
strings when passed through `errSymLookup`. The fix in `27918cb` that adds
`S_db_badChoice` to error output would only show numbers without this.

## Fix
Added `dbAccessDefs.h` and `dbStaticLib.h` to `ERR_S_FILES` in the error
Makefile so their `S_*` definitions are scanned into `errSymTbl.c`. Removed
the redundant `#ifndef INCerrMdefh` guard from `dbAccessDefs.h` and made
`#include "errMdef.h"` unconditional (it is idempotent via its own include
guard).

## Rust Applicability
Rust error types are defined in the type system and displayed via `Display`
trait — no separate string table is needed. This pattern is eliminated.

## Audit Recommendation
No audit needed. Build-system / error-table registration is a C-specific
pattern with no Rust analog.

## C Locations
- `modules/libcom/src/error/Makefile` — adds dbAccessDefs.h + dbStaticLib.h to ERR_S_FILES
- `modules/database/src/ioc/db/dbAccessDefs.h` — removes redundant #ifndef guard around errMdef.h include
