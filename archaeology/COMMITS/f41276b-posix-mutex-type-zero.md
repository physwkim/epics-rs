---
sha: f41276bef8e1a98da18f146428ad2c1ada6a9b6b
short_sha: f41276b
date: 2021-02-28
author: Brendan Chandler
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [mutex, POSIX, platform-portability, RTEMS, initialization]
---
# Replace literal 0 with PTHREAD_MUTEX_DEFAULT in osdPosixMutexInit calls

## Root Cause
`osdPosixMutexInit()` accepted an integer `mutextype` parameter.  All call
sites outside `osdMutex.c` passed the literal `0`, assuming that the value `0`
equals `PTHREAD_MUTEX_DEFAULT`.  On Linux/macOS this holds, but on RTEMS 5
`PTHREAD_MUTEX_DEFAULT` is defined to a different numeric value.  Passing `0`
on RTEMS 5 selects an unintended mutex type, which can exhibit different
locking semantics or fail initialization, leading to silent correctness bugs
or crashes.

Affected call sites: `osdEvent.c` (event semaphore backing mutex),
`osdSpin.c` (spinlock), `osdThread.c` (two global thread-list mutexes
`onceLock` and `listLock`).

## Symptoms
On RTEMS 5: event semaphores or spinlocks initialized with wrong mutex type.
Locking behavior undefined; potential deadlock or non-atomicity.  On
Linux/macOS: no visible effect (0 == PTHREAD_MUTEX_DEFAULT there).

## Fix
Changed all four call sites from `osdPosixMutexInit(&x, 0)` to
`osdPosixMutexInit(&x, PTHREAD_MUTEX_DEFAULT)`.  Updated the comment in
`osdPosixMutexPriv.h` to document the symbolic constant rather than the
magic number.

Note: SHA `79242da5` is an identical cherry-pick of this same change to a
different branch; both are covered by this file.

## Rust Applicability
Eliminated — `std::sync::Mutex` and `tokio::sync::Mutex` use the platform's
correct mutex type by construction.  Rust's POSIX backend always passes
`PTHREAD_MUTEX_DEFAULT` via `libc::pthread_mutexattr_settype`.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdEvent.c:epicsEventCreate` — literal 0 → PTHREAD_MUTEX_DEFAULT
- `modules/libcom/src/osi/os/posix/osdSpin.c:epicsSpinCreate` — literal 0 → PTHREAD_MUTEX_DEFAULT
- `modules/libcom/src/osi/os/posix/osdThread.c:once` — onceLock and listLock
- `modules/libcom/src/osi/os/posix/osdPosixMutexPriv.h` — comment updated
