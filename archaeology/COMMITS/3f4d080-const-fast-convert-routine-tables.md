---
sha: 3f4d080260b094e63b27ea804948a2308857e37c
short_sha: 3f4d080
date: 2024-07-10
author: Andrew Johnson
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [const-correctness, function-pointer-table, dbFastGetConvertRoutine, dbFastPutConvertRoutine]
---

# Non-const Fast Convert Routine Tables Allow Silent Mutation

## Root Cause
`dbFastGetConvertRoutine` and `dbFastPutConvertRoutine` were declared as
non-`const` arrays of function pointers. Any code with access to these
global arrays could overwrite a function pointer slot, silently replacing
a conversion routine with a bad or malicious function pointer. There was no
enforcement that these tables are read-only after initialization.

## Symptoms
No crash was reported — this is a latent safety/correctness issue. A buggy
module or test that accidentally wrote to these arrays would silently corrupt
all conversions of the affected type pair for the entire process lifetime.

## Fix
Added `const` to both array declarations in `dbConvertFast.h` and their
definitions in `dbFastLinkConv.c`, making the tables immutable after
program initialization. Also added Doxygen documentation.

## Rust Applicability
Eliminated. Rust's conversion dispatch in base-rs uses `match` statements or
const lookup tables that are immutable by default (`static` items are `const`
in effect). There are no mutable global function pointer tables.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbConvertFast.h` — declaration missing `const`
- `modules/database/src/ioc/db/dbFastLinkConv.c:dbFastGetConvertRoutine` — definition missing `const`
- `modules/database/src/ioc/db/dbFastLinkConv.c:dbFastPutConvertRoutine` — definition missing `const`
