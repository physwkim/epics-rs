---
sha: ad326751fa53d08929eb407143c2efc8f1bc10c2
short_sha: ad32675
date: 2025-04-17
author: Andrew Johnson
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [C23, cast, function-pointer, dbStaticLib, portability]
---
# dbStaticLib: add cast for DEVSUPFUN pointer in C23

## Root Cause
C23 tightened rules around pointer arithmetic on function pointers. In
`dbDumpDevice` (`dbStaticLib.c`), a `DEVSUPFUN *pfunc` pointer was initialized
by taking the address of a struct field (`&pdevSup->pdset->report`) and then
iterated with `++pfunc` to walk the device support function table. In C23 this
requires an explicit cast to `DEVSUPFUN*` because the source expression is
typed as a pointer to the specific struct field type, not `DEVSUPFUN*`.

## Symptoms
Compilation failure with C23-conformant compilers. No runtime behavior change.

## Fix
Add `(DEVSUPFUN*)` cast: `DEVSUPFUN *pfunc = (DEVSUPFUN*) &pdevSup->pdset->report;`

## Rust Applicability
Rust does not use C-style function pointer tables with pointer arithmetic.
Device support dispatch in `base-rs` uses trait objects or enum dispatch.
No equivalent pattern exists. Eliminated.

## Audit Recommendation
None — the C23 pointer-arithmetic-on-function-pointer pattern cannot arise
in Rust.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbDumpDevice` — add `(DEVSUPFUN*)` cast for C23 compliance
