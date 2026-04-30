---
sha: 5a8b6e4111b869e8913b7bfaff4c56b546fb8879
short_sha: 5a8b6e4
date: 2020-11-23
author: Dirk Zimoch
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [mutex, priority-inheritance, posix, real-time, lock-ordering]
---
# POSIX epicsMutex lacked priority-inheritance, enabling priority inversion

## Root Cause
The old POSIX `epicsMutexOsdCreate` created `pthread_mutex_t` with
`pthread_mutexattr_setprotocol(..., PTHREAD_PRIO_INHERIT)` conditioned on
`_POSIX_THREAD_PRIO_INHERIT > 0`, but earlier in the file the code
unconditionally `#undef _POSIX_THREAD_PRIO_INHERIT` and
`#undef _POSIX_THREAD_PROCESS_SHARED` with the comment "leave until these
can be demonstrated to work" (referencing Solaris 8 failures). This meant PI
was permanently disabled regardless of platform capability, causing classic
priority inversion under `SCHED_FIFO`: a low-priority task holding a mutex
could block high-priority tasks indefinitely.

The old code also stored a `pthread_mutexattr_t` per-mutex (wasteful —
`mutexAttr` embedded in every `epicsMutexOSD`), and had a complex fallback
implementation for pre-POSIX-2001 systems using a `pthread_cond_t` loop.

## Symptoms
On real-time Linux systems running EPICS IOCs with `SCHED_FIFO`, a
low-priority thread (e.g. scan thread) holding an `epicsMutex` while a
high-priority callback thread waited would cause the high-priority thread to
be blocked by the low-priority thread without the kernel boosting the
low-priority thread's scheduling priority. This is the textbook priority
inversion scenario; Mars Pathfinder was affected by an analogous bug.

## Fix
Refactored to use global `pthread_mutexattr_t` objects initialized once via
`pthread_once` (`globalAttrDefault`, `globalAttrRecursive`). The init
function `globalAttrInit` probes for `PTHREAD_PRIO_INHERIT` support by
actually creating a test mutex — if the kernel rejects it, it falls back to
`PTHREAD_PRIO_NONE`. Per-mutex `mutexAttr` storage was eliminated. The
pre-POSIX-2001 fallback implementation (cond+loop emulation of recursive
mutex) was dropped. A new `osdPosixMutexInit(pthread_mutex_t*, int type)`
helper is now shared by both `osdMutex.c` and `osdEvent.c` (events also use
a `pthread_mutex_t` internally).

## Rust Applicability
Rust's `std::sync::Mutex` uses the OS mutex primitives but does not expose
`PTHREAD_PRIO_INHERIT` control. Tokio tasks run on a thread pool managed by
`tokio::runtime`, which does not set `SCHED_FIFO` or PI attributes. For
real-time EPICS deployments, the C IOC remains the appropriate host for
hard-RT tasks. No Rust analog to audit.

## Audit Recommendation
No audit needed. Feature is OS/C-specific. If ca-rs or base-rs are deployed
in a real-time context, document that PI mutex semantics are not available.

## C Locations
- `modules/libcom/src/osi/os/posix/osdMutex.c:globalAttrInit` — new shared PI mutex attr init
- `modules/libcom/src/osi/os/posix/osdMutex.c:osdPosixMutexInit` — new helper replaces per-mutex attr
- `modules/libcom/src/osi/os/posix/osdMutex.c:epicsMutexOsdCreate` — simplified, no embedded mutexAttr
- `modules/libcom/src/osi/os/posix/osdEvent.c:epicsEventCreate` — now uses osdPosixMutexInit
- `modules/libcom/src/osi/os/posix/osdPosixMutexPriv.h` — new shared header declaring osdPosixMutexInit
