---
sha: 3c46542630823a272001aaab4e6fc265c7e03046
short_sha: 3c46542
date: 2021-03-02
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [mutex, POSIX, priority-inheritance, compile-guard, portability]
---
# Guard epicsMutexOsdShowAll PI check with _POSIX_THREAD_PRIO_INHERIT

## Root Cause
`epicsMutexOsdShowAll()` called `pthread_mutexattr_getprotocol()` and
referenced `PTHREAD_PRIO_INHERIT` unconditionally.  These symbols are only
available when the platform defines `_POSIX_THREAD_PRIO_INHERIT`.  On
platforms without priority-inheritance support (some embedded RTOS targets)
the code would fail to compile.

## Symptoms
Compilation failure on platforms where
`_POSIX_THREAD_PRIO_INHERIT` is not defined (e.g. some RTEMS or vxWorks
configurations).

## Fix
Wrapped the PI-check block in `#if defined _POSIX_THREAD_PRIO_INHERIT` /
`#else` / `#endif`.  The `#else` branch prints `"PI not supported\n"`.

## Rust Applicability
Eliminated — Rust's standard library and tokio do not expose
priority-inheritance mutex APIs; diagnostic display of mutex attributes is
not part of the Rust EPICS codebase.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdMutex.c:epicsMutexOsdShowAll` — added compile-time PI guard
