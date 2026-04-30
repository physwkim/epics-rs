---
sha: d019e9787a2092db3f038d5feefb3c9cd78a7808
short_sha: d019e97
date: 2022-05-03
author: Michael Davidsaver
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [pthread, thread-local, cleanup, leak, posix]
---

# posix: use pthread_key_create destructor to clean up epicsThreadOSD

## Root Cause
`epicsThreadOSD` (per-thread info struct, reference-counted) was only freed in
two explicit call sites: `start_routine()` (EPICS-managed threads) and
`epicsThreadExitMain()`. Threads created outside of EPICS (e.g. by third-party
libraries or the C runtime itself) that nevertheless called into EPICS APIs
would adopt a `pthreadInfo` struct via `init_threadInfo()` but never release
it, because neither of the two explicit free sites would run for these threads.

The fix: pass `&free_threadInfo` as the destructor to `pthread_key_create()`,
so the POSIX TLS system automatically calls it when any thread exits, regardless
of how the thread was created.

## Symptoms
- Per-thread `epicsThreadOSD` structs leaked for every non-EPICS thread that
  touched EPICS thread APIs (thread name lookup, priority get, etc.).
- On long-running IOCs with many short-lived threads (e.g. from CA client
  contexts or third-party plugins), this could accumulate significant heap usage.

## Fix
- Change `free_threadInfo` signature from `(epicsThreadOSD*)` to `(void*)` to
  match the POSIX destructor signature.
- Pass `&free_threadInfo` to `pthread_key_create()` instead of `0` (NULL).
- Remove the two explicit `free_threadInfo()` calls from `start_routine()` and
  `epicsThreadExitMain()` — POSIX TLS now handles all cases.

## Rust Applicability
Tokio and std::thread both guarantee thread-local destructors run on thread
exit (via `std::thread_local!` drop). In Rust, there is no equivalent of
forgetting to free thread-local state — the type system ensures Drop runs.
This C pattern is entirely eliminated by Rust's ownership model.

## Audit Recommendation
None — eliminated by Rust's thread-local drop guarantees.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:once` — key creation w/ destructor
- `modules/libcom/src/osi/os/posix/osdThread.c:start_routine` — explicit free removed
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadExitMain` — explicit free removed
- `modules/libcom/src/osi/os/posix/osdThread.c:free_threadInfo` — signature changed to void*
