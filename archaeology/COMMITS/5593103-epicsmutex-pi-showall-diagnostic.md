---
sha: 5593103c11157aada4e40c5f00c6b033387fb57e
short_sha: 5593103
date: 2021-02-21
author: Michael Davidsaver
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [mutex, priority-inheritance, diagnostic, posix, platform]
---
# epicsMutexShowAll: missing PI-enabled diagnostic across platforms

## Root Cause
After enabling priority-inheritance (PI) mutexes (sha 5a8b6e4), there was no
way to verify at runtime that the kernel and libc actually honored
`PTHREAD_PRIO_INHERIT`. The `epicsMutexShowAll()` function printed mutex list
statistics but never confirmed PI status. On systems where
`pthread_mutexattr_setprotocol` silently fails (non-RT kernel, musl, old
glibc), all mutexes would be created without PI with no diagnostic.

## Symptoms
Operators running `epicsMutexShowAll` (via IOC shell) had no confirmation
whether PI was active. Priority inversion could still occur silently on
systems that do not support `PTHREAD_PRIO_INHERIT`, degrading real-time
performance without any indication.

## Fix
Added `epicsMutexOsdShowAll()` — a platform-specific hook called from
`epicsMutexShowAll()` before iterating the mutex list. On POSIX, it calls
`pthread_mutexattr_getprotocol(&globalAttrRecursive, &proto)` and prints
`"PI is enabled"` or `"PI is not enabled"`. Stub no-op implementations added
for RTEMS, WIN32, vxWorks. The function is gated behind `EPICS_PRIVATE_API`.

## Rust Applicability
Rust's `std::sync::Mutex` (and tokio's equivalents) do not expose
priority-inheritance control. In a real-time Rust EPICS server, PI would be
handled by the OS scheduler (`SCHED_FIFO`) or a custom PI-aware mutex crate.
There is no Rust analog to audit; the feature is eliminated.

## Audit Recommendation
No audit needed. Diagnostic-only addition for C/POSIX-specific PI mutex
infrastructure. Rust async tasks use cooperative scheduling.

## C Locations
- `modules/libcom/src/osi/epicsMutex.cpp:epicsMutexShowAll` — calls new epicsMutexOsdShowAll()
- `modules/libcom/src/osi/os/posix/osdMutex.c:epicsMutexOsdShowAll` — queries globalAttrRecursive protocol
- `modules/libcom/src/osi/os/RTEMS/osdMutex.c:epicsMutexOsdShowAll` — stub
- `modules/libcom/src/osi/os/WIN32/osdMutex.c:epicsMutexOsdShowAll` — stub
- `modules/libcom/src/osi/os/vxWorks/osdMutex.c:epicsMutexOsdShowAll` — stub
