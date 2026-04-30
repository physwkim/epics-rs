---
sha: e9e576f4bb66b1e57ec0327789e5650187fb3005
short_sha: e9e576f
date: 2021-11-02
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: db_ca_sync
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: db_ca_task
tags: [race, sync, CA-link, work-queue, dbCaSync]
---

# Fix dbCaSync() and add testdbCaWaitForUpdateCount()

## Root Cause
`dbCaSync()` was supposed to wait until the CA link worker thread (`dbCaTask`)
had processed all previously queued actions. The implementation used
`CA_SYNC` as a sentinel action: it queued a `CA_SYNC` entry and waited on an
event. However, the trigger logic in `dbCaTask` was:

```c
if (link_action & CA_SYNC)
    epicsEventMustTrigger((epicsEventId)pca->userPvt);
    /* comment: dbCaSync() requires workListLock to be held here */
```

The event was triggered **before** the work item was fully consumed — specifically,
the trigger happened at dequeue time, not after the work queue was empty. This
meant `dbCaSync()` could return while other work items after the sentinel were
still being processed. The "requires workListLock to be held" comment was present
but the code was not actually correct — the trigger should only happen when the
queue is truly empty (i.e., after the inner `ellGet` returns NULL).

Additionally, `dbCaGetUpdateCount()` did not hold `dbScanLock` while reading
`pca` from `plink->value.pv_link.pvt`, creating a use-after-free race with
concurrent link clearing.

## Symptoms
- `dbCaSync()` returning before all queued CA link work was complete.
- Test infrastructure proceeding before the CA worker had processed put/connect
  actions, causing intermittent test failures.
- `dbCaGetUpdateCount()` potentially reading a freed `pca` pointer.

## Fix
- Deferred the `requestSync` event trigger to after the work queue is empty:
  introduced `epicsEventId requestSync = NULL` local variable; when a `CA_SYNC`
  action is dequeued, store the event in `requestSync` instead of triggering
  immediately; trigger only when `ellGet` returns NULL (queue empty).
- Fixed `dbCaGetUpdateCount()` to acquire `dbScanLock` before reading `pca`.
- Added `testdbCaWaitForUpdateCount()` — a properly synchronized helper that
  registers a callback on `pca->monitor` and waits for `nUpdate` to reach the
  target count.

## Rust Applicability
In `base-rs`, a `db_ca_sync()` equivalent must ensure that the CA work channel
(e.g., `mpsc::Sender<CaAction>`) is fully drained before returning. The correct
pattern:
- Send a `CaAction::Sync(oneshot::Sender)` to the worker.
- The worker responds on the `oneshot` only after it sees an empty queue
  (i.e., after processing all items before the Sync).
- The `Sync` sender waits on the `oneshot::Receiver`.

Verify that `base-rs/db_ca.rs` does not trigger the sync response at dequeue
time (like the C bug), but only after the queue is empty.

## Audit Recommendation
- In `base-rs/src/server/database/db_ca.rs:db_ca_task`: verify that the sync
  response is sent only when the work channel is empty (e.g., after
  `channel.try_recv()` returns `Empty`), not at dequeue time.
- In `db_ca_sync()`: verify it awaits the oneshot before returning.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaTask` — deferred requestSync trigger to queue-empty
- `modules/database/src/ioc/db/dbCa.c:dbCaGetUpdateCount` — added dbScanLock
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForUpdateCount` — new synchronized API
