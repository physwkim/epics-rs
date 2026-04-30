---
sha: d0ef45acc3f75f86c8533e6285af4a9e042b0e35
short_sha: d0ef45a
date: 2020-02-25
author: Dirk Zimoch
category: lifecycle
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [freelist, init-order, dbEvent, dbChannel, lifecycle]
---

# Free-list not initialized before dbChannel use, causing NULL deref

## Root Cause
`dbChannelInit()` initializes channel and filter free lists but did not
call `db_init_event_freelists()`. The event free lists
(`dbevEventUserFreeList`, `dbevFieldLogFreeList`, etc.) were only
initialized lazily inside `db_init_events()`. If any code path called
`db_create_read_log()` (which uses `dbevFieldLogFreeList`) before
`db_init_events()` was called — which can happen during PINI processing
before the event task starts — the free list was NULL, causing a crash
or `freeListFree` on a NULL list at task exit.

## Symptoms
Crash or assertion failure when `db_create_read_log()` is called during
early IOC initialization before the event subsystem has been started.
The `event_task` exit path also hits `freeListFree(dbevEventUserFreeList,
evUser)` on a NULL pointer if the free lists were never initialized.

## Fix
Extracted free-list initialization from `db_init_events()` into a new
`db_init_event_freelists()` function (idempotent, safe to call multiple
times) and called it from `dbChannelInit()` to guarantee lists are
ready before any channel operations.

## Rust Applicability
In Rust, free lists are replaced by the allocator. The init-order hazard
does not apply because Rust's allocator is always available. However, if
base-rs uses a pool allocator (e.g., `slab` crate) for `FieldLog`
objects, check that the pool is initialized before the first
`db_create_read_log` equivalent is called. Likely eliminated in practice.

## Audit Recommendation
No direct audit needed for Rust. If epics-base-rs uses an explicit pool
for field logs, verify it is constructed at startup before any channel
or link operations. Otherwise this is fully eliminated by Rust's allocator.

## C Locations
- `modules/database/src/ioc/db/dbChannel.c:dbChannelInit` — now calls `db_init_event_freelists()`
- `modules/database/src/ioc/db/dbEvent.c:db_init_event_freelists` — new idempotent init function
- `modules/database/src/ioc/db/dbEvent.c:event_task` — guarded `freeListFree` against NULL list
