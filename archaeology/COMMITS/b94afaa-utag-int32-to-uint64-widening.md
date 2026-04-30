---
sha: b94afaa0453b0b86a0d438e87758fb9a291c4416
short_sha: b94afaa
date: 2020-12-02
author: Michael Davidsaver
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: get_options
  - crate: base-rs
    file: src/server/database/db_field_log.rs
    function: null
tags: [utag, uint64, type-widening, alignment, wire-protocol]
---

# UTAG field widened from epicsInt32 to epicsUInt64

## Root Cause

The `UTAG` (timestamp tag) field in `dbCommon` was declared as `epicsInt32` with
a separate `epicsInt32 padTime` padding field inside `DBRtime`. When packed into
the `getOptions()` response buffer after the `epicsTimeStamp` (8 bytes), the utag
was written as a `epicsUInt32*` at offset +8, consuming only 4 bytes, followed by
4 bytes of explicit padding — giving a misaligned, sign-incorrect representation
of a conceptually unsigned tag value.

The fix widened the type to `epicsUTag` (64-bit unsigned), removed the padding
field, and changed the serialization to cast the buffer pointer to `epicsUInt64*`
before writing the tag, maintaining 8-byte alignment naturally.

## Symptoms

- Tags containing values above `INT32_MAX` (0x7FFFFFFF) would appear sign-extended
  or truncated depending on the consumer.
- Callers that stored the received utag back into a `epicsUTag` field would silently
  truncate 64-bit tag values.
- The 4-byte padding slot was unused wasted bandwidth in every timed CA/database
  response.

## Fix

- Changed `DBRtime` macro: removed `epicsInt32 utag` + `epicsInt32 padTime`; replaced
  with `epicsUTag utag` (typedef for `epicsUInt64`).
- In `getOptions()` (dbAccess.c): after writing `time.secPastEpoch` + `time.nsec`
  into a `epicsUInt32*` pointer, cast to `epicsUInt64*` and write `pcommon->utag`
  or `pfl->utag`.
- Updated `lset::getTimeStampTag` signature from `epicsInt32*` to `epicsUTag*`
  everywhere (dbDbLink.c, dbLink.c, dbLink.h).
- `dbCommon.dbd.pod`: field UTAG changed from `DBF_LONG` to `DBF_UINT64`.

## Rust Applicability

In base-rs, the equivalent of `getOptions()` serializes monitor/get response
payloads. If `utag` is modeled as `i32` or `u32`, values above 2^31-1 will be
silently truncated or sign-extended on the wire. The lset callback equivalent
(`get_timestamp_tag`) must also use `u64`.

Any `db_field_log` Rust struct that carries a `utag` field must declare it `u64`
(not `u32` or `i64`).

## Audit Recommendation

1. Search base-rs for any `utag` field typed as `i32`, `u32`, or `i64` — must be
   `u64`.
2. Verify the monitor-payload serialization writes utag as little-endian 8 bytes
   after the `nsec` field, with no additional padding.
3. Check that any link-layer callback signature for timestamp+tag uses `u64`.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:getOptions` — serializes utag as u64 after timestamp
- `modules/database/src/ioc/db/dbAccessDefs.h:DBRtime` — type declaration change
- `modules/database/src/ioc/db/db_field_log.h` — utag field type in event log struct
- `modules/database/src/ioc/db/dbLink.h:lset.getTimeStampTag` — callback signature
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetTimeStampTag` — implementation
