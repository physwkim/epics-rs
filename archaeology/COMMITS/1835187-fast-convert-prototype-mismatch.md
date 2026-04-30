---
sha: 1835187a867afed3ad51792544b4a213544571ef
short_sha: 1835187
date: 2023-12-24
author: Andrew Johnson
category: type-system
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [function-prototype, void-pointer, UB, fast-convert, type-mismatch]
---

# Fast Convert Routines Called Through Wrong Prototype Cause UB

## Root Cause
All 200+ fast conversion routines in `dbFastLinkConv.c` were defined with
typed parameters (e.g., `char *from, char *to`) but stored in a table declared
as `long (*)()` (empty prototype — unspecified arguments). When called through
the table pointer, C passes the actual `const void *` / `void *` arguments as
per the `FASTCONVERTFUNC` typedef, but the callee receives them as `char *` or
`epicsInt8 *` etc. — this is a prototype mismatch and undefined behavior. On
most platforms this worked by accident, but it violates the C standard.

## Symptoms
On strict ABIs or with aggressive optimization, conversion functions could
receive incorrectly typed/promoted arguments, causing wrong data conversions or
crashes in `dbPutField` / `dbGetField` paths. Latent UB present in all EPICS
bases prior to this fix.

## Fix
Changed all ~200 conversion function signatures to use `const void *f, void *t`
matching the `FASTCONVERTFUNC` typedef, with explicit casts inside each function
body. This aligns the definition with the call-site prototype.

## Rust Applicability
Eliminated. Rust does not permit calling function pointers through a
differently-typed signature; all function pointer types carry full signatures.
The base-rs conversion layer uses generics and trait dispatch, not raw function
pointer tables.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbFastLinkConv.c` — all ~200 cvt_* functions had typed params not matching `FASTCONVERTFUNC`
