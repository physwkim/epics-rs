---
sha: 012139638d6a212ba347c11cbedaedaf8875ab6f
short_sha: 0121396
date: 2024-06-11
author: Michael Davidsaver
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread, leak, EPERM, joinable, reference-count]
---
# Thread handle leak when joinable thread creation fails with EPERM

## Root Cause
`epicsThreadCreateOpt` uses an RT-priority probe strategy: it first tries
to create the thread with `SCHED_FIFO`, and if that fails with `EPERM`
(no realtime-priority permission), it retries without `SCHED_FIFO`.  For
joinable threads, the `epicsThreadOSD` struct has an extra reference
count that is decremented only by the eventual `epicsThreadMustJoin`
call.  On the retry-after-EPERM path, the first `pthreadInfo` allocation
(for the SCHED_FIFO attempt) was freed via `free_threadInfo` without
first decrementing the joinable reference count, causing the allocator to
see refcnt == 1 and assert-fail (or leak the allocation depending on
whether assertions are active).

## Symptoms
Memory leak of one `epicsThreadOSD` struct per joinable thread created on
systems where RT priorities are probed but not permitted.  With
assertions enabled this was an `assert(cnt==1)` failure on the retry path.

## Fix
Before calling `free_threadInfo(pthreadInfo)` on the EPERM retry path,
add `epicsAtomicDecrIntT(&pthreadInfo->refcnt)` when `pthreadInfo->joinable`
is set, and assert the decremented value is 1.  This mirrors the existing
fix in the `pthread_create` failure path.

## Rust Applicability
Eliminated.  Rust's `std::thread::spawn` returns a `JoinHandle` whose
ownership model enforces that the handle is either joined or detached;
no manual reference-counting is needed.  Tokio task handles use
Arc-based reference counting automatically.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — decrement joinable refcnt before free_threadInfo on EPERM retry path
