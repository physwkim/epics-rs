---
sha: 6e7a7153805088ae6115e707f36a4c1d8a37655a
short_sha: 6e7a715
date: 2022-08-16
author: Andrew Johnson
category: bounds
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_fast_link_conv.rs
    function: cvt_device_st
tags: [null-deref, devSup, DTYP, record-type, field-convert]
---
# Getting .DTYP from rectype with no devSup returns empty string instead of crash

## Root Cause
`cvt_device_st` in `dbFastLinkConv.c` converted all four guard conditions
(`!paddr`, `!pdbFldDes`, `!pdbDeviceMenu`, `*from>=nChoice`) into one
compound `if`. When a record type has no device support (`ftPvt == NULL`),
the old code fell through to `recGblDbaddrError(S_db_badChoice)` instead of
returning an empty string — which is the correct semantics. The `NULL`
dereference was also possible if `papChoice[*from]` was NULL, but the primary
correctness bug was treating "no devSup" as an error rather than a valid
empty-string case.

## Symptoms
Reading `.DTYP` from a record type that has no device support produced a
spurious `S_db_badChoice` error instead of returning an empty string `""`.
Clients observing `.DTYP` via CA/PVA on such record types received an alarm
or confusing error response.

## Fix
Split the monolithic null-chain into three separate checks:
1. `!paddr || !pdbFldDes` → `S_db_errArg` (hard error, bad call site).
2. `!pdbDeviceMenu` (ftPvt is NULL) → write `'\0'` and return 0 (valid, no devSup).
3. `*from >= nChoice || !papChoice || !pchoice` → `S_db_badChoice` as before.

## Rust Applicability
In base-rs, any function that converts a DTYP field value to a string must
guard against a record having no device support (`devSup` list empty). The
Rust equivalent of `ftPvt` is typically an `Option<Arc<DevSup>>`. Code that
unwraps this without checking will panic; code that maps `None` to an error
status replicates the pre-fix C bug.

## Audit Recommendation
Audit `cvt_device_st` Rust equivalent in `db_fast_link_conv.rs`. Ensure that
`pdbDeviceMenu == NULL` (i.e., `Option::None` for device menu) is handled by
returning an empty string, not an error. Any `unwrap()` or `?` on a
`Option<DeviceMenu>` in a field-conversion path is suspect.

## C Locations
- `modules/database/src/ioc/db/dbFastLinkConv.c:cvt_device_st` — split null guard; "no devSup" now returns empty string
