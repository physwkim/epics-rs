---
sha: 0916cf985c20a35ed7accfb732ebe904b9ddcd33
short_sha: 0916cf9
date: 2025-11-10
author: Ralph Lange
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [posix, memlock, FIFO-scheduling, mlockall, thread-priority]
---
# libCom/posix: skip mlockall when FIFO scheduling is opted out via env var

## Root Cause
`epicsThreadRealtimeLock()` called `mlockall(MCL_CURRENT | MCL_FUTURE)` on
any system where `maxPriority > minPriority`, regardless of whether the
application had opted out of FIFO scheduling via the newly added
`EPICS_ALLOW_POSIX_THREAD_PRIORITY_SCHEDULING=NO` environment variable.
This meant that even when the user disabled real-time scheduling, the process
still locked all its virtual memory pages, consuming physical memory and
potentially preventing other processes from getting RAM.

## Symptoms
On systems where the operator sets
`EPICS_ALLOW_POSIX_THREAD_PRIORITY_SCHEDULING=NO`, the IOC process still
locks all its memory, wasting physical RAM and potentially triggering OOM.

## Fix
Add `&& wantPrioScheduling` to the `mlockall` guard:
```c
if (pcommonAttr->maxPriority > pcommonAttr->minPriority && wantPrioScheduling) {
    mlockall(MCL_CURRENT | MCL_FUTURE);
}
```

## Rust Applicability
`eliminated` — epics-rs uses tokio's async runtime and does not call
`mlockall`. Thread scheduling is delegated to the OS. No audit needed.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadRealtimeLock` — `mlockall` not gated on `wantPrioScheduling`
