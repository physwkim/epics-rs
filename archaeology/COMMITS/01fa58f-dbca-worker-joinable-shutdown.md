---
sha: 01fa58f37b4ab74e355fb1bc74b1b1fcaaa96d1a
short_sha: 01fa58f
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, dbCa, shutdown, joinable, lifecycle]
---
# dbCa: join worker thread at shutdown instead of signal-only

## Root Cause
`dbCaShutdown()` signalled the worker via `dbCaCtl = ctlExit` then waited for
`startStopEvent` (which the worker signals when it begins its exit path), but
never joined the thread. The thread could still be running (cleaning up CA
context, flushing queues) after `dbCaShutdown()` returned, creating a
use-after-free window if the caller deallocated shared state.

## Symptoms
- CA link worker thread may outlive IOC shutdown, accessing freed memory.
- Shutdown ordering issues on multi-threaded teardown.

## Fix
Store the worker `epicsThreadId` in `dbCaWorker`. Create the thread with
`epicsThreadCreateOpt()` and `joinable=1`. After `epicsEventMustWait(startStopEvent)`,
call `epicsThreadMustJoin(dbCaWorker)` to guarantee the worker has fully exited
before `dbCaShutdown()` returns.

## Rust Applicability
Eliminated. The dbCa link worker in base-rs is a tokio task with a
`JoinHandle`. Shutdown calls `handle.abort()` and can `.await` the handle to
confirm exit. No manual semaphore/join dance needed.

## Audit Recommendation
Verify that base-rs CA-link worker task shutdown awaits the `JoinHandle` (not
just aborts it) before tearing down shared state like the CA client context.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaShutdown` — added `epicsThreadMustJoin(dbCaWorker)`
- `modules/database/src/ioc/db/dbCa.c:dbCaLinkInitImpl` — switched to `epicsThreadCreateOpt` with `joinable=1`
