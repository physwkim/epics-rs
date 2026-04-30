---
sha: 71e4635d348d56baf46daa3f32c0d7e87ca59c89
short_sha: 71e4635
date: 2025-10-17
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/db_ca.rs
    function: db_ca_sync
  - crate: ca-rs
    file: src/client/db_ca.rs
    function: testdb_ca_wait_for_event
tags: [race, dbCaSync, CA-context-sync, lock-order, epicsEvent-destroy]
---
# testdbCaWaitForEvent: race between event destroy and CA context flush

## Root Cause
`testdbCaWaitForEvent()` had two distinct race conditions:

**Race 1 — Wrong lock for event trigger**: The callback `testdbCaWaitForEventCB`
triggered `epicsEventMustTrigger(pvt->evt)` while holding `pca->lock`. The
caller's loop held `pca->lock` on the outer wait, but `epicsEventDestroy(evt)`
was called while `pca->lock` was held — *before* ensuring the callback had
returned from `epicsEventMustTrigger`. If the scheduler allowed the callback
to fire and reach `epicsEventMustTrigger` after `epicsEventDestroy` had freed
the event handle, the result was a use-after-free. Fix: switch the callback
to hold `workListLock` (not `pca->lock`) when triggering; hold `workListLock`
around `epicsEventDestroy` to guarantee the trigger has completed.

**Race 2 — dbCaSync called too early**: `dbCaSync()` was called before
`epicsEventDestroy()`, but `dbCaSync()` only drains the dbCa worker queue —
it did not synchronize with the CA context thread (which processes I/O in a
separate thread). A pending CA callback from the context thread could race
with `epicsEventDestroy`. Fix: added `dbCaSyncLocal()` which calls
`ca_client_context::sync()` → `cac::sync()` → `db_add_extra_labor_event` +
`db_flush_extra_labor_event`, flushing the CA context's local event queue
before proceeding.

## Symptoms
- Intermittent crash/assertion failure in tests calling
  `testdbCaWaitForEvent()`, especially on multi-core systems with a busy CA
  context thread.
- Use-after-free on a destroyed `epicsEventId` when the CA callback fires
  after the event handle is freed.

## Fix
1. Move `dbCaSync()` call to after `epicsMutexUnlock(pca->lock)` and before
   `epicsEventDestroy`.
2. Change `testdbCaWaitForEventCB` to lock `workListLock` (not `pca->lock`)
   around `epicsEventMustTrigger`.
3. Lock `workListLock` around `epicsEventDestroy` to ensure the trigger has
   fully returned.
4. Add `dbCaSyncLocal()` which flushes the CA context thread via
   `db_add_extra_labor_event` + `db_post_extra_labor` + `db_flush_extra_labor_event`.

## Rust Applicability
In `ca-rs`, any test utility that waits for a CA callback and then frees shared
state must ensure the callback has fully returned before the free. In Rust, the
equivalent is an `Arc<Mutex<WaitState>>` where the callback holds a weak ref:
drop order is guaranteed by Rust's borrow checker and Arc ref counting, making
the use-after-free impossible. However, the flush ordering (sync CA context
before test teardown) is still needed: if the CA client context has a separate
async task, the test must `join`/`abort` that task before asserting state.

## Audit Recommendation
In `ca-rs/src/client/db_ca.rs::db_ca_sync` (or its async equivalent): verify
it flushes both the dbCa worker queue AND the CA client context's I/O event
queue before returning. In any test utilities that wait for CA callbacks,
ensure the sync call precedes any shared state destruction (equivalent of
`epicsEventDestroy`). Use `Arc`/`Weak` to prevent use-after-free in callback
state instead of manual lock-around-destroy.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForEventCB` — lock changed from `pca->lock` to `workListLock`
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForEvent` — `dbCaSync()` moved after unlock; `workListLock` guards `epicsEventDestroy`; calls `dbCaSyncLocal()`
- `modules/database/src/ioc/db/dbCa.c:dbCaSync` — now calls `dbCaSyncLocal()` first
- `modules/ca/src/client/access.cpp:dbCaSyncLocal` — new: flushes CA context via `ca_client_context::sync()`
- `modules/database/src/ioc/db/dbContext.cpp:dbContext::sync` — new: `db_add_extra_labor_event` + flush to drain CA context queue
