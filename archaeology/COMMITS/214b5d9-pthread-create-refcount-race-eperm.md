---
sha: 214b5d935b6fd46b59517d682e143a78279c05b0
short_sha: 214b5d9
date: 2025-11-12
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [pthread, refcount, race, EPERM, thread-create]
---
# posix: pthread_create refcount mis-count and use-after-free race in epicsThreadCreateOpt

## Root Cause
Two bugs combined in `epicsThreadCreateOpt()`:

**Bug 1 — refcount mis-count on EPERM retry:**
A previous commit added a `epicsAtomicDecrIntT(&pthreadInfo->refcnt)` in the
`EPERM` error path (when the first `pthread_create` attempt with `SCHED_FIFO`
fails), but forgot to add the matching `epicsAtomicIncrIntT` for the joinable
path. After the decrement, `refcnt` reached zero, `free_threadInfo` freed the
struct, and the subsequent retry operated on freed memory.

**Bug 2 — use-after-free data race:**
If `pthread_create()` succeeded in starting a very short-lived thread, the
new thread could complete its entire body and call `free_threadInfo` (dropping
the last reference) before `pthread_create()` returned and wrote
`pthreadInfo->tid`. The creator would then write to freed memory.

## Symptoms
- With EPERM retry: double-free or corruption of `pthreadInfo` on systems
  where `SCHED_FIFO` is not permitted.
- With short-lived thread: write to freed heap memory, potential crash or
  silent corruption of `pthreadInfo->tid`.

## Fix
Add a temporary "creator" reference before `pthread_create()`:
```c
epicsAtomicIncrIntT(&pthreadInfo->refcnt); // temp ref for creator
status = pthread_create(&new_tid, ...);
free_threadInfo(pthreadInfo); // release temp ref
```
This ensures `pthreadInfo` stays alive until after `pthread_create` has
written `new_tid`, regardless of how fast the new thread runs. The EPERM
retry path is corrected to also hold a temporary reference and properly
manage the joinable reference.

## Rust Applicability
`eliminated` — `std::thread::spawn` and tokio's `task::spawn` manage thread
lifetime through `Arc`-based handles. There is no manual reference count on
a thread descriptor struct.

## Audit Recommendation
No audit needed in epics-rs.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — missing temp ref before pthread_create; EPERM path missing inc-ref
