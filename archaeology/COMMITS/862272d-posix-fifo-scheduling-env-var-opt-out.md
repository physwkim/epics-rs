---
sha: 862272d6665b174c178b472236214c45e8bbb0bc
short_sha: 862272d
date: 2025-11-10
author: Ralph Lange
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [posix, FIFO-scheduling, env-var, thread-priority, configuration]
---
# libCom/posix: add env var to opt out of FIFO real-time scheduling

## Root Cause
EPICS threads on POSIX systems were unconditionally started with `SCHED_FIFO`
real-time scheduling when the OS permitted it (i.e., when
`maxPriority > minPriority`). Containers and cloud environments typically do
not grant `CAP_SYS_NICE`, causing `pthread_create` to fail with `EPERM`.
While the code had an EPERM retry path, the absence of a pre-flight opt-out
was a usability and correctness gap for containerized deployments.

## Symptoms
In constrained environments without `CAP_SYS_NICE`: spurious `EPERM` retries
logged at startup; in rare cases (see companion 214b5d9) the EPERM path had a
refcount bug leading to freed-memory access.

## Fix
Introduce `EPICS_ALLOW_POSIX_THREAD_PRIORITY_SCHEDULING` (default `YES`).
Read via `envGetBoolConfigParam` during `once()` into a `wantPrioScheduling`
static. When `NO`, skip `setSchedulingPolicy(pthreadInfo, SCHED_FIFO)` and
`isRealTimeScheduled = 1` during thread creation.

## Rust Applicability
`eliminated` — epics-rs uses tokio's thread pool which does not attempt
SCHED_FIFO by default. Containerized deployments need no workaround.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — unconditional `setSchedulingPolicy(SCHED_FIFO)`
- `modules/libcom/src/osi/os/posix/osdThread.c:once` — new `wantPrioScheduling` read from env
