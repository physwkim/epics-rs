---
sha: 3d70e706407072f7d19dfdba5edd02f1446888d3
short_sha: 3d70e70
date: 2024-06-14
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbCommonPvt, FORTIFY_SOURCE, type-cast, object-size, alignment]
---
# dbPvt2Rec cast needed to pacify _FORTIFY_SOURCE=3 object-size check

## Root Cause
`dbCommonPvt` contained an embedded `struct dbCommon common` field to
allow `CONTAINER()` macro navigation.  `_FORTIFY_SOURCE=3` uses
`__builtin_object_size(&precord->common)` to bound-check accesses via
`&precord->common`, but `__builtin_object_size` treats the embedded
`common` as having exactly `sizeof(dbCommon)` bytes rather than the
variable-length record struct that follows.  This caused false-positive
fortify errors when the server wrote into fields beyond `dbCommon` via
the `precord->common` address.  Additionally, `offsetof(dbCommonPvt, common)`
was used in the allocation size, which was fragile if padding was ever
added.

## Symptoms
False-positive `_FORTIFY_SOURCE=3` buffer-overrun errors when allocating
and accessing full record structs through the `dbCommonPvt` prefix.

## Fix
Removed the embedded `struct dbCommon common` field from `dbCommonPvt`;
replaced it with a comment noting it is followed by `dbCommon`.  Added
`dbPvt2Rec(pvt)` as the reverse of `dbRec2Pvt(prec)` — both use raw
`char*` pointer arithmetic to skip exactly `sizeof(dbCommonPvt)` bytes,
which the compiler can reason about correctly.  Added a
`dbCommonPvtAlignmentTest` static-assert to ensure no padding exists
between `dbCommonPvt` and the following `dbCommon`.  Allocation changed
from `offsetof(dbCommonPvt, common) + rec_size` to
`sizeof(dbCommonPvt) + rec_size`.

## Rust Applicability
Eliminated.  Rust's record prefix pattern would use a `#[repr(C)]`
struct with a prefix field, and the Rust compiler enforces layout
without needing `__builtin_object_size` workarounds.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbCommonPvt.h` — removed embedded dbCommon; added dbPvt2Rec(); alignment static-assert
- `modules/database/src/ioc/dbStatic/dbStaticRun.c:dbAllocRecord` — changed to sizeof(dbCommonPvt) + rec_size; use dbPvt2Rec
