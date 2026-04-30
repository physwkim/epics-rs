---
sha: 152d306ad87dfd6fc8992330d6f29a86fb81533b
short_sha: 152d306
date: 2024-06-11
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [pthread_join, self-join, UB, thread, EDEADLK]
---
# Undefined behavior: pthread_join on own thread (self-join)

## Root Cause
`epicsThreadMustJoin` called `pthread_join(id->tid, &ret)` and relied on
the POSIX `EDEADLK` return to detect self-join.  However, POSIX only
requires that `pthread_join` may detect deadlock and return `EDEADLK`; it
does NOT mandate it.  On some platforms (notably glibc ≥ 2.34 with the
new `nptl` backend), `pthread_join` on the current thread produces
undefined behavior rather than `EDEADLK`, causing crashes or hangs.

## Symptoms
Crash or hang when a thread calls `epicsThreadMustJoin` on its own
thread ID.  Manifested on glibc-based Linux when joinable threads join
themselves on shutdown.

## Fix
Detect self-join explicitly before calling `pthread_join`: retrieve the
calling thread's ID with `epicsThreadGetIdSelf()` and compare it to the
join target.  If they match, call `pthread_detach` instead (semantics:
the thread releases its joinable reference without blocking).  A corrupt
`joinable` state (neither 0 nor 1) now calls `cantProceed` immediately.
The `EDEADLK` path is retained only as a fallback for indirect cycles.

## Rust Applicability
Eliminated.  Rust's `std::thread::JoinHandle` and Tokio's task handles
do not allow joining the current task from within itself.  The runtime
detects and panics/rejects such patterns at the API level.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadMustJoin` — explicit self-join detection via epicsThreadGetIdSelf(); use pthread_detach for self, cantProceed for corrupt state
