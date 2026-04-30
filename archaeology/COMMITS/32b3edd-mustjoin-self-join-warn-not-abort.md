---
sha: 32b3eddb94cf4f47494ff2cc58d6f5e0f3f71d21
short_sha: 32b3edd
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, self-join, unjoinable, warning, lifecycle]
---
# epicsThreadMustJoin: warn (not abort) on self-join of unjoinable thread

## Root Cause
`epicsThreadMustJoin()` called `cantProceed()` (fatal abort) if the thread was
not joinable, regardless of whether the caller was the thread itself. A thread
calling `epicsThreadMustJoin(epicsThreadGetIdSelf())` on itself when it was not
created joinable would crash the process, even though self-join is already a
no-op in correct code (used as a "make joinable flag stick" idiom).

## Symptoms
- Process abort on defensively-written code that calls `epicsThreadMustJoin(self)`
  from within the thread body as a no-op marker, when the thread was not
  created with `joinable=1`.

## Fix
All three backends (POSIX, WIN32, RTEMS): if `!joinable && caller == target`,
emit `errlogPrintf("Warning: ...")` and return. Only if the caller is a
*different* thread and the target is not joinable do we `cantProceed()`.

Also adds a test (`testSelfJoin`) that verifies a double self-join emits a
warning but does not crash.

## Rust Applicability
Eliminated. Rust tokio `JoinHandle` has defined semantics: you can `await` it
exactly once; a second `await` returns immediately with `Err(JoinError::Cancelled)`.
There is no unjoinable/joinable distinction at the task level in the same way.

## Audit Recommendation
None required. This is a portability fix in the C thread abstraction layer.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadMustJoin` — warn on self-join of unjoinable
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsThreadMustJoin` — same
- `modules/libcom/src/osi/os/RTEMS/osdThread.c:epicsThreadMustJoin` — same
