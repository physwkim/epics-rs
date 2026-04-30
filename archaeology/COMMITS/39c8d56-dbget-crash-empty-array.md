---
sha: 39c8d5619a7cba9e95495cc99be9c25ab514f05c
short_sha: 39c8d56
date: 2020-02-13
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [empty-array, crash, bounds, dbGet, filter-chain]
---

# dbGet crashes on empty array: missing element-count guard before filter

## Root Cause
`dbGet()` did not check whether the filter-log `no_elements` was zero
before using the field-log to drive array operations. With empty-array
fields (e.g., a waveform with NORD=0), the post-filter field log carries
`no_elements=0`, and subsequent array processing (size calculation,
memcpy) operated on a zero-length buffer — undefined behavior or crash.

`dbDbLink.c::dbDbGetValue` had a mirrored bug: it called
`db_create_read_log` and ran filter chains on channels with zero final
elements, which is unsupported; no early-exit existed.

Additionally, spurious `errlogPrintf` warnings in `dbDbInitLink` and
`dbDbAddLink` fired for all zero-element channels at init time, even
though this is a valid runtime state (dynamic waveform that starts
empty). These were removed since the actual error is reported during get.

## Symptoms
IOC crash (segfault or assertion) when a DB link reads from an empty
array PV that has filters applied. Warning spam at IOC startup for any
zero-initial-element waveform.

## Fix
Added guard in `dbGet`: if `pfl->no_elements < 1`, return `S_db_badField`
and goto done before array processing. In `dbDbGetValue`: restructured
filter code so that when `dbChannelFinalElements() > 0`, run the filter
chain; otherwise return `S_db_badField` and set LINK alarm. Removed
pre-emptive warnings from init functions.

## Rust Applicability
In base-rs `db_access.rs`, the `db_get` equivalent must guard against
zero-element results from filter application before any slice or buffer
operations. In `links.rs`, `db_db_get_value` must check channel final
element count before running filter chains. Failing to guard will cause
a panic (index out of bounds) or silent empty-slice UB in unsafe code.

## Audit Recommendation
In `src/server/database/db_access.rs`: search for `no_elements` or
equivalent and verify a `< 1` / `== 0` guard exists after filter
application, before any buffer slicing. In `src/server/database/links.rs`:
find `run_pre_chain`/`run_post_chain` call sites and verify they are
guarded by a `final_elements > 0` check.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — added `pfl->no_elements < 1` guard
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — restructured to skip filter chain when channel has 0 elements
- `modules/database/src/ioc/db/dbDbLink.c:dbDbInitLink` — removed spurious zero-element warning
- `modules/database/src/ioc/db/dbDbLink.c:dbDbAddLink` — removed spurious zero-element warning
