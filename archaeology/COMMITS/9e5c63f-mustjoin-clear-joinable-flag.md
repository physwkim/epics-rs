---
sha: 9e5c63fb9158f0262abfc9e6be282dcb542a914c
short_sha: 9e5c63f
date: 2019-06-23
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, double-join, joinable-flag, race, lifecycle]
---
# epicsThreadMustJoin: clear joinable flag after join to prevent double-join UAF

## Root Cause
After `epicsThreadMustJoin()` completed successfully it did not clear the
`joinable` flag on the thread descriptor. A second call to
`epicsThreadMustJoin()` on the same (now-freed or recycled) `id` would find
`joinable=1` and attempt a second `pthread_join` / `WaitForSingleObject`, which
is undefined behaviour (double-join on POSIX, invalid handle on WIN32).

For the self-join case specifically (where the descriptor is not freed), the
double self-join should degrade gracefully rather than crashing.

## Symptoms
- Double `epicsThreadMustJoin()` on the same id → undefined behaviour: deadlock,
  crash, or silent corruption depending on whether the OS reused the thread id.

## Fix
All three backends: set `id->joinable = 0` (or `v->joinable = 0`) immediately
after the join/wait completes, before calling `free_threadInfo`. This ensures
a second call hits the `!joinable` branch and either warns (self-join) or
aborts (cross-thread).

## Rust Applicability
Eliminated. `JoinHandle` is consumed by `.await` / `.join()` — it is not
`Copy` or `Clone`, so a second join attempt is a compile-time error.

## Audit Recommendation
None required. This is a C-layer portability fix with no Rust equivalent needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadMustJoin` — `id->joinable = 0` before free
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsThreadMustJoin` — `pParmWIN32->joinable = 0`
- `modules/libcom/src/osi/os/RTEMS/osdThread.c:epicsThreadMustJoin` — `v->joinable = 0`
