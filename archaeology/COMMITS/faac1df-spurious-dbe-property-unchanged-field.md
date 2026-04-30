---
sha: faac1df1c6cff2a62a030b854d442507799997b6
short_sha: faac1df
date: 2024-08-30
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: dbPut
tags: [DBE_PROPERTY, spurious-event, property-field, CA-monitor, memcmp]
---
# Spurious DBE_PROPERTY events posted even when property field value unchanged

## Root Cause
`dbPut` previously posted `DBE_PROPERTY` unconditionally for any write to
a field marked as a property field (`pfldDes->prop != 0`), even if the
new value was identical to the existing value.  This caused unnecessary
`DBE_PROPERTY` events on every write (e.g., a caput of the current EGU
value), causing all subscribing clients to re-fetch the entire property
set.

## Symptoms
Clients subscribed to `DBE_PROPERTY` on high-write-rate records would
receive excessive re-notifications, causing unnecessary network traffic
and client-side re-rendering.

## Fix
Added a comparison step for property field writes: when the field fits
within `MAX_STRING_SIZE`, the new value is first written into a temporary
stack buffer, then compared against the current field value with
`memcmp`.  If the content is unchanged, `propertyUpdate` is cleared and
no `DBE_PROPERTY` event is posted.  Only changed property values trigger
the event.  For fields larger than `MAX_STRING_SIZE`, the write proceeds
unconditionally (rare case).

## Rust Applicability
Applies.  In base-rs the write path must suppress `DBE_PROPERTY` events
when the property field value is unchanged.  Implement a compare-before-
write for property fields analogous to this C fix.

## Audit Recommendation
In `db_access.rs::dbPut`, verify that a `propertyUpdate` flag is only
set when the property field content actually changes.  A `memcmp` (or
`PartialEq` check in Rust) of old vs. new value must gate the
`DBE_PROPERTY` post.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPut` — added propertyUpdate flag with memcmp check; suppresses DBE_PROPERTY if unchanged
