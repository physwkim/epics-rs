---
sha: d989c8fade61a2fbff0823276f3a52449022a96c
short_sha: d989c8f
date: 2018-04-04
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, posix, epicsThread, joinable, refcount]
---

# Add epicsThreadJoin() and joinable flag to libCom thread API

## Root Cause
EPICS threads were created as detached pthreads by default, making it impossible to synchronize on thread exit at the OS level. The `epicsThread` class's `exitWait()` relied on an application-level flag and condition variable, which could race with thread teardown. No POSIX `pthread_join()` was ever called, leaving resources (pthread ID, stack) potentially reclaimed by the OS at undefined times.

## Symptoms
Valgrind/TSan reports of use-after-free on thread IDs; rare crashes on busy systems during shutdown when threads exited slightly after callers assumed they were done.

## Fix
Added `epicsThreadOpts::joinable` field, `epicsThreadJoin()` function, and POSIX `pthread_join()` implementation in `osdThread.c`. Non-POSIX targets (WIN32, RTEMS, vxWorks) get stub `epicsThreadJoin()` implementations. A reference count (`refcnt`) tracks shared ownership of the `epicsThreadOSD` between the thread itself and the joiner. Default `epicsThreadCreate()` still creates non-joinable threads for compatibility.

## Rust Applicability
Fully eliminated. All Rust threads (both `std::thread` and Tokio tasks) are joinable by default. The concept of optional joinability does not exist — handles always provide a join mechanism. Reference counting of thread descriptors is handled by the OS and Rust's ownership system.

## Audit Recommendation
None.

## C Locations
- `modules/libcom/src/osi/epicsThread.h` — added `epicsThreadOpts::joinable`, `epicsThreadJoin()` declaration
- `modules/libcom/src/osi/os/posix/osdThread.c` — full `epicsThreadJoin()` implementation with refcount and `pthread_join()`
- `modules/libcom/src/osi/os/WIN32/osdThread.c` — stub `epicsThreadJoin()`
- `modules/libcom/src/osi/os/RTEMS/osdThread.c` — stub `epicsThreadJoin()`
- `modules/libcom/src/osi/os/vxWorks/osdThread.c` — stub `epicsThreadJoin()`
