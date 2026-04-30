---
sha: 5efce9ecc056e5de24c7c8a8cbc2fba7d7f4ef67
short_sha: 5efce9e
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [rename, thread-join, api-contract, lifecycle, portability]
---
# Rename epicsThreadJoin() to epicsThreadMustJoin() to enforce join contract

## Root Cause
`epicsThreadJoin()` had an implicit contract that the thread must be joinable
(created with `joinable=1`) but the name did not signal this. Callers could
accidentally call it on non-joinable threads, leading to `cantProceed()` aborts.
The rename to `epicsThreadMustJoin()` makes the "must" contract explicit, in
the same style as `epicsMutexMustLock`, `epicsEventMustCreate`, etc.

## Symptoms
- Soft: no runtime bug introduced. The rename prevents future API misuse by
  making the requirement visible in the name.

## Fix
Rename `epicsThreadJoin` → `epicsThreadMustJoin` in all four backends
(POSIX, WIN32, RTEMS, vxWorks), the public header, and the C++ wrapper
(`epicsThread::exitWait`).

## Rust Applicability
Eliminated. Rust's type system enforces join-once via `JoinHandle` ownership.
No naming convention needed.

## Audit Recommendation
None required. Pure rename/API-contract change in the C threading layer.

## C Locations
- `modules/libcom/src/osi/epicsThread.h` — declaration rename
- `modules/libcom/src/osi/epicsThread.cpp:epicsThread::exitWait` — call-site update
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadMustJoin` — definition rename
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsThreadMustJoin` — definition rename
- `modules/libcom/src/osi/os/RTEMS/osdThread.c:epicsThreadMustJoin` — definition rename
- `modules/libcom/src/osi/os/vxWorks/osdThread.c:epicsThreadMustJoin` — definition rename
