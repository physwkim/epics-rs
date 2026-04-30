---
sha: 99be9a86a0bb2a16eac334aa8e1d509e5558cb6c
short_sha: 99be9a8
date: 2019-07-02
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, macro, VxWorks, RTEMS, portability]
---
# EPICS_THREAD_CAN_JOIN macro broken: VxWorks < 6.9 evaluates to 0 triggering cantProceed

## Root Cause
`EPICS_THREAD_CAN_JOIN` was previously defined as `(0)` on VxWorks < 6.9
(and positive `(1)` elsewhere), and `epicsThreadMustJoin` used `#if EPICS_THREAD_CAN_JOIN`
to decide behavior. On VxWorks < 6.9, `EPICS_THREAD_CAN_JOIN` was `0`, which
is truthy in a preprocessor `#if`, so the macro did not evaluate as "not
supported". The VxWorks implementation then called `cantProceed()` unconditionally.
Additionally, RTEMS `osdThread.h` was missing an `extern "C"` wrapper required
for C++ compilation.

## Symptoms
On VxWorks < 6.9, calling `epicsThreadMustJoin` would invoke `cantProceed()`
(fatal abort) instead of being a no-op. Any IOC using joinable threads on
older VxWorks would crash on shutdown.

## Fix
- Define `EPICS_THREAD_CAN_JOIN` (without a value) in the public `epicsThread.h`.
- In VxWorks `osdThread.h`, `#undef EPICS_THREAD_CAN_JOIN` when the VxWorks
  version does not support it (< 6.9). Switch all `#if EPICS_THREAD_CAN_JOIN`
  checks to `#ifdef EPICS_THREAD_CAN_JOIN`.
- Remove per-platform `#define EPICS_THREAD_CAN_JOIN (1)` from Linux, POSIX,
  WIN32 headers (the default in `epicsThread.h` now covers them).
- Fix RTEMS `osdThread.h` missing `extern "C"` wrapper.
- On VxWorks < 6.9, `epicsThreadMustJoin` becomes a no-op instead of aborting.

## Rust Applicability
Rust uses `tokio::task::JoinHandle` and `std::thread::JoinHandle` which are
always joinable (or detachable via `drop`). The platform-conditional join
support is entirely eliminated. No equivalent bug exists in ca-rs or base-rs.

## Audit Recommendation
No direct Rust audit needed. If any bridge code wraps a C `epicsThreadMustJoin`,
verify it is not called from platforms where join was unsupported.

## C Locations
- `modules/libcom/src/osi/epicsThread.h` — define `EPICS_THREAD_CAN_JOIN` unconditionally, VxWorks `#undef`s it
- `modules/libcom/src/osi/os/vxWorks/osdThread.h` — `#undef` on VxWorks < 6.9
- `modules/libcom/src/osi/os/vxWorks/osdThread.c:epicsThreadMustJoin` — switch to `#ifdef`; remove `cantProceed` on unsupported
- `modules/libcom/src/osi/os/RTEMS/osdThread.h` — add `extern "C"` guard
