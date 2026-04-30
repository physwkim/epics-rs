---
sha: 333446e0feb14efef2730f8d2fba06fa4d4ef099
short_sha: 333446e
date: 2025-06-16
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: process_target
tags: [locking, lockset, db-link, race, debug-assert]
---

# dbDbLink: Assert lockset ownership before dbPutLink

## Root Cause
`processTarget()` in `dbDbLink.c` calls `dbPutLink` (which eventually calls
`db_process`) without verifying that the calling thread holds the lockset for
both source and destination records. If another code path invoked
`processTarget` without first acquiring `dbScanLock`, the destination record
could be processed by two threads concurrently, corrupting record fields.

The bug was latent: the lockset debug framework tracks the owning thread in
`lockSet::owner` but this field was never checked in the link processing path.

## Symptoms
- Under LOCKSET_DEBUG builds: silent race on destination record processing.
- In production: potential double-process of a linked record leading to
  inconsistent alarm state or wrong output values.

## Fix
Added a `#ifdef LOCKSET_DEBUG` block in `processTarget()` that asserts both
`psrc->lset->owner == self` and `pdst->lset->owner == self` before proceeding.
Also added the missing `#include "epicsThread.h"` to `dbLockPvt.h` for the
`epicsThreadGetIdSelf()` call.

## Rust Applicability
Partial. In base-rs, record processing is guarded by per-record async mutexes
(or scan-lock equivalents). A Rust implementation that links records together
must ensure both source and destination scan-locks are held before calling any
`process()` path that triggers side effects on the destination. The assertion
pattern translates to a `debug_assert!(lock.is_held_by_current_task())` idiom
in any async lock wrapper.

## Audit Recommendation
In `base-rs/src/server/database/db_link.rs`, check that `process_target` (or
its equivalent) acquires the destination record's scan-lock before invoking
`db_process`. Add a debug assertion or a structured lock guard type that
enforces this statically.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — added LOCKSET_DEBUG ownership assert
- `modules/database/src/ioc/db/dbLockPvt.h` — added epicsThread.h include for epicsThreadGetIdSelf
