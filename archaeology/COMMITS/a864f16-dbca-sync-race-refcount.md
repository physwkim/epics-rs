---
sha: a864f16318beef781555925b50f9c21326643b6e
short_sha: a864f16
date: 2024-06-11
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/link.rs
    function: testdb_ca_wait_for_event
  - crate: ca-rs
    file: src/client/link.rs
    function: db_ca_sync
tags: [race, refcount, dbCa, sync, caLink, lock-ordering]
---

# dbCa Test Sync Race: Missing Refcount and Wrong Lock Release Order

## Root Cause
Two related races in `dbCa.c`:

**Race 1 — Missing refcount in `testdbCaWaitForEvent`:**
The function grabbed `pca` from `plink->value.pv_link.pvt` without incrementing
the reference count (`caLinkInc`). If the worker thread processed a disconnect
event between reading `pca` and locking `pca->lock`, the `caLink` could be
freed while `testdbCaWaitForEvent` still held a pointer to it. Added
`caLinkInc(pca)` at entry and `caLinkDec(pca)` at exit.

**Race 2 — `dbCaSync` unlocked workListLock too early:**
```c
epicsMutexMustLock(workListLock);
epicsMutexUnlock(workListLock);   // ← too early
assert(templink.refcount==1);
epicsMutexDestroy(templink.lock);
epicsEventDestroy(wake);
```
The worker holds `workListLock` when it triggers the wake event. The intent was
to cycle through `workListLock` to ensure the worker had finished. But releasing
before `assert` + destroy created a window where another thread could re-acquire
`workListLock` and modify `templink` before the assertions and destroys ran.
Fixed by moving `epicsMutexUnlock` to after the destroy calls.

**Race 3 — Missing `dbCaSync()` in wait loop:**
After `epicsEventMustWait(evt)`, the worker might still be executing the
callback. Added `dbCaSync()` call to ensure the worker has fully returned
before re-checking state.

## Symptoms
- Test flakiness: `testdbCaWaitForEvent` could use-after-free `pca`.
- `dbCaSync` assertion failures or double-free of `templink.lock`/`wake` event.
- CA link tests could hang or crash under high concurrency.

## Fix
- Add `caLinkInc`/`caLinkDec` around `testdbCaWaitForEvent` body.
- Move `epicsMutexUnlock(workListLock)` to after destroy calls in `dbCaSync`.
- Add `dbCaSync()` after `epicsEventMustWait(evt)` in the wait loop.

## Rust Applicability
Applies. In ca-rs, the CA link subsystem uses `Arc<Mutex<CaLink>>` for shared
link state. The analogous patterns to audit:
1. Any place that reads a `CaLink` pointer from a link struct without holding
   an Arc clone — check that `Arc::clone` is called before releasing the
   parent lock.
2. Any sync/drain function that signals a condvar and then releases a lock —
   verify the lock is held until all associated resources are destroyed/dropped.
3. Test helpers that wait for CA events — ensure they hold an Arc for the
   duration of the wait.

## Audit Recommendation
In `ca-rs/src/client/link.rs`: audit all functions that (a) read a `CaLink`
reference from a shared structure, (b) release the parent lock, and (c)
subsequently use the reference — ensure an Arc clone is held across the gap.
Also audit `db_ca_sync` equivalent for correct lock release ordering around
resource cleanup.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForEvent` — missing `caLinkInc`/`caLinkDec`
- `modules/database/src/ioc/db/dbCa.c:dbCaSync` — `workListLock` released before resource destroy
