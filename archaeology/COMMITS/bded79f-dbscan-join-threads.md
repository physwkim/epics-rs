---
sha: bded79f14dc7bde32df42478adcafa2c5546a407
short_sha: bded79f
date: 2022-07-30
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/scan.rs
    function: scan_stop
tags: [thread-join, shutdown, scan, periodic-task, lifecycle]
---
# dbScan: join periodic and once-scan threads on scanStop()

## Root Cause
In `dbScan.c:scanStop()`, after signaling all periodic scan threads to stop
(via `epicsEventSignal`/`epicsEventWait` for each), the function did NOT join
the threads. The threads' `joinable` flag was set to `0` (non-joinable), so
they were effectively detached — `scanStop()` returned as soon as the stop
event was acknowledged, but the threads might still be running.

Similarly for `onceTaskId` (the scanOnce thread): it was signaled to exit but
not joined.

Since `dbScan.c` threads access the IOC database, not joining them before
proceeding with database cleanup created use-after-free windows.

## Symptoms
- Race condition between scan thread teardown and database memory cleanup.
- Use-after-free crashes during shutdown, particularly under AddressSanitizer.
- Threads still referencing record structures that are being freed by
  `scanCleanup()` after `scanStop()` returns.

## Fix
1. Set `opts.joinable = 1` for both `onceTaskId` and `periodicTaskId[ind]`
   threads in `initOnce()` and `spawnPeriodic()`.
2. In `scanStop()`, after the existing stop-event loop, added:
   ```c
   for (i = 0; i < nPeriodic; i++) {
       epicsThreadMustJoin(periodicTaskId[i]);
   }
   epicsThreadMustJoin(onceTaskId);
   ```

## Rust Applicability
In a Rust scan implementation using `tokio::task`, scan tasks are
`JoinHandle`s. On shutdown, `handle.abort()` followed by `handle.await` (or
`.await` after a cancellation token signal) provides the equivalent of
`epicsThreadMustJoin`. The bug pattern — calling `.abort()` without awaiting
the `JoinHandle` — is possible in Rust. Detached tasks (spawned without
storing the handle) cannot be joined at all.

## Audit Recommendation
In `base-rs` scan task management: verify that all periodic scan task
`JoinHandle`s are stored and awaited (or aborted+awaited) during shutdown.
Confirm no `tokio::spawn` call for scan tasks is made without storing the
handle (i.e., no fire-and-forget scan tasks).

## C Locations
- `modules/database/src/ioc/db/dbScan.c:scanStop` — epicsThreadMustJoin added for periodic and once tasks
- `modules/database/src/ioc/db/dbScan.c:initOnce` — joinable=1 set
- `modules/database/src/ioc/db/dbScan.c:spawnPeriodic` — joinable=1 set
