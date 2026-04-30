---
sha: 2340c6e6c1a3b34526108fc84f0220fadd052146
short_sha: 2340c6e
date: 2021-02-25
author: Krisztián Löki
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/record_support.rs
    function: cvt_dbaddr
  - crate: base-rs
    file: src/server/database/record_support.rs
    function: get_array_info
tags: [bptr, cvt-dbaddr, get-array-info, segfault, array-record]
---
# Array records: move bptr assignment from cvt_dbaddr to get_array_info

## Root Cause
For compress, histogram, and subArray records, `cvt_dbaddr()` was setting
`paddr->pfield = prec->bptr` at link-resolution time (pass 0/1).  If the aai
record that links to one of these records initialized first (pass 0, before
the source record allocated its buffer), `bptr` was NULL at the time
`cvt_dbaddr` ran.  The NULL was stored into `paddr->pfield` and never updated.
Subsequent reads through the dbAddr produced NULL dereferences → segfault.

The `monitor()` functions in these records also passed `prec->bptr` to
`db_post_events` instead of `(void*)&prec->val`, causing the same
wrong-field-address issue as in `4a0f488`.

## Symptoms
Segfault on first scan or CA/PVA get when an aai record is linked to a
compress/histogram/subArray record and the aai initializes first in pass 0.
Also: monitor callbacks deliver wrong data (same bptr-vs-field-addr bug).

## Fix
1. Removed `paddr->pfield = prec->bptr` from `cvt_dbaddr()` in all three
   record types.  `cvt_dbaddr` still sets `no_elements`, `field_type`,
   `field_size`, and `dbr_field_size` — static fields that do not change.
2. Added `paddr->pfield = prec->bptr` to `get_array_info()` in all three
   record types.  `get_array_info` is called at access time (after
   initialization), at which point `bptr` is guaranteed to be set.
3. Changed `db_post_events(prec, prec->bptr, mask)` →
   `db_post_events(prec, (void*)&prec->val, mask)` in `monitor()` for
   compress, histogram, and subArray.

## Rust Applicability
In `base-rs`, if dbAddr-style field descriptors are resolved at IOC
initialization time, ensure the buffer pointer is resolved lazily at access
time, not at descriptor setup time.  If field resolution is always at access
time (natural in async Rust), this bug is structurally eliminated.

## Audit Recommendation
In `base-rs/src/server/database/record_support.rs`, for array-type records,
verify that the field buffer pointer is resolved at access time (equivalent to
`get_array_info`), not cached at descriptor-creation time (`cvt_dbaddr`).
Check compress, histogram, and subArray record equivalents.

## C Locations
- `modules/database/src/std/rec/compressRecord.c:cvt_dbaddr` — removed paddr->pfield assignment
- `modules/database/src/std/rec/compressRecord.c:get_array_info` — added paddr->pfield = prec->bptr
- `modules/database/src/std/rec/compressRecord.c:monitor` — bptr → &prec->val
- `modules/database/src/std/rec/histogramRecord.c:cvt_dbaddr` — removed paddr->pfield
- `modules/database/src/std/rec/histogramRecord.c:get_array_info` — added paddr->pfield
- `modules/database/src/std/rec/histogramRecord.c:monitor` — bptr → &prec->val
- `modules/database/src/std/rec/subArrayRecord.c:cvt_dbaddr` — removed paddr->pfield
- `modules/database/src/std/rec/subArrayRecord.c:get_array_info` — added paddr->pfield
- `modules/database/src/std/rec/subArrayRecord.c:monitor` — bptr → &prec->val
