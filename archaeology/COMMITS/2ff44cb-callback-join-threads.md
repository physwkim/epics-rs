---
sha: 2ff44cb38649b201ea62a69028ecc48f924fc92d
short_sha: 2ff44cb
date: 2022-07-30
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callback_stop
tags: [thread-join, shutdown, callback, lifecycle, resource-leak]
---
# callback.c: join callback threads on callbackStop()

## Root Cause
In `callback.c:callbackStop()`, after waiting for `threadsRunning` to drop to
zero (via a busy-wait + timeout loop), the function did NOT join the callback
threads. The threads were created with `joinable = 0`, so they were detached.
`callbackStop()` returned as soon as `threadsRunning == 0`, but the threads
might still be in the process of exiting (running stack cleanup code and
touching shared resources).

Furthermore, thread IDs were not saved anywhere — `callbackInit()` discarded
the `tid` returned by `epicsThreadCreateOpt`.

## Symptoms
- Use-after-free during `callbackCleanup()`: the `epicsRingPointerDelete` and
  `epicsMutexDestroy` calls in cleanup can race with still-running callback
  thread teardown.
- Non-deterministic crash under AddressSanitizer or Valgrind during IOC
  shutdown.

## Fix
1. Added `epicsThreadId *threads` array to `cbQueueSet` to store all thread
   IDs for each priority level.
2. In `callbackInit()`: set `joinable = 1`, allocated `threads` array via
   `callocMustSucceed`, stored each `tid` in `threads[j]`.
3. In `callbackStop()`: after the running-count loop, added:
   ```c
   for(j=0; j<mySet->threadsConfigured; j++) {
       epicsThreadMustJoin(mySet->threads[j]);
   }
   ```
4. In `callbackCleanup()`: added `free(mySet->threads); mySet->threads = NULL;`

## Rust Applicability
In a Rust callback system using `tokio::task`, callback worker tasks are
`JoinHandle`s (one per priority level, potentially multiple handles per
priority). The same pattern applies: if handles are dropped without `.await`
(or `.abort().await`), the tasks may still run after cleanup. The `threads`
array corresponds to a `Vec<JoinHandle<()>>` per priority.

## Audit Recommendation
In `base-rs` callback dispatch: verify that callback worker task `JoinHandle`s
are stored per priority level and awaited (or aborted+awaited) in the shutdown
path. Ensure no `tokio::spawn` for callback workers is fire-and-forget.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackStop` — threads joined after running-count loop
- `modules/database/src/ioc/db/callback.c:callbackInit` — threads array allocated, joinable=1 set
- `modules/database/src/ioc/db/callback.c:callbackCleanup` — threads array freed
