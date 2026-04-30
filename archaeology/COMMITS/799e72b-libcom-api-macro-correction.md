---
sha: 799e72b1e338ffb79e1c1f4699a3d56f0cd43706
short_sha: 799e72b
date: 2020-05-20
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [api-macro, shared-library, symbol-export, libcom, abi]
---

# Replace deprecated epicsShareFunc/epicsShareAPI with LIBCOM_API/epicsStdCall

## Root Cause
EPICS historically used `epicsShareFunc` / `epicsShareAPI` macros for shared
library symbol export/import. These macros were deprecated in favor of the
more explicit `LIBCOM_API` / `DB_API` / `LIBCA_API` per-module macros that
correctly express both export (`__declspec(dllexport)`) and import
(`__declspec(dllimport)`) semantics on Windows. Using the wrong macro causes
symbols to be exported with default visibility when they should be imported
(or vice versa), resulting in link errors or silent symbol resolution to the
wrong DLL on Windows.

## Symptoms
- Link errors on Windows when mixing DLLs that use deprecated `epicsShareFunc`
  with those that use `LIBCOM_API`.
- Potential ABI mismatch if calling convention macros (`epicsShareAPI` vs
  `epicsStdCall`) differ between caller and callee translation units.

## Fix
Mass replacement of `epicsShareFunc epicsShareAPI` with `LIBCOM_API epicsStdCall`
across `libcom`, `database/callback`, `database/dbAccess`, `dbConvert`,
`dbFastLinkConv`, `recGbl`, and header files. Also removed stray
`#define epicsExportSharedSymbols` from several `.cpp` files in the timer
subsystem that were accidentally re-exporting symbols.

## Rust Applicability
Eliminated. Rust's `pub` / `pub(crate)` / `#[no_mangle] pub extern "C"`
visibility is explicit and type-checked by the compiler. No
`epicsShareFunc`-equivalent macro pattern exists.

## Audit Recommendation
No action required.

## C Locations
- `modules/database/src/ioc/db/callback.c` — epicsShareFunc → DB_API
- `modules/database/src/ioc/db/dbAccess.c` — epicsShareFunc → DB_API
- `modules/libcom/src/as/asLib.h` — bulk macro replacement
- `modules/libcom/src/timer/*.cpp` — removed epicsExportSharedSymbols defines
- (many other files — see commit diff)
