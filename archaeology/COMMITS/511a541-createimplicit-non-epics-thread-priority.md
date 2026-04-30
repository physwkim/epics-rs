---
sha: 511a541f31f2efd9a7b56a359ef522f6ff637dd5
short_sha: 511a541
date: 2019-03-14
author: till straumann
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-priority, createImplicit, posix, sched-policy, race]
---
# posix: createImplicit assigns correct priority for non-EPICS threads

## Root Cause
`createImplicit()` creates an EPICS thread descriptor for a non-EPICS thread
that calls an EPICS API (e.g., `epicsThreadGetIdSelf()`). The priority
computation divided the `sched_priority` by the EPICS range, but did not check
whether the thread's scheduling *policy* matched the EPICS policy. If a
non-EPICS thread used `SCHED_OTHER` (priority 0) while EPICS used `SCHED_FIFO`,
the raw `sched_priority` (0) would be divided by EPICS's FIFO range, giving an
incorrect nonzero result — or worse, an out-of-range result if the ranges differed.

Launchpad bug #1816841.

## Symptoms
- `epicsThreadGetPriority(epicsThreadGetIdSelf())` returns wrong value for a
  non-EPICS thread using a different scheduling policy.
- Threads created with `SCHED_OTHER` and priority 0 might be assigned EPICS
  priority > 0, causing incorrect priority display or assertions.

## Fix
Store `schedPolicy` and `schedParam` directly in `pthreadInfo`. Before computing
the EPICS priority, check `pthreadInfo->schedPolicy == pcommonAttr->schedPolicy`
and that `pcommonAttr->usePolicy` is set. If not, leave `osiPriority = 0`.

## Rust Applicability
Eliminated. Rust threads and tokio tasks do not expose `sched_priority` mapping.
Thread priority in epics-rs is handled at the OS thread level only for
real-time contexts, and non-EPICS thread adoption doesn't apply.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:createImplicit` — check schedPolicy before computing osiPriority
