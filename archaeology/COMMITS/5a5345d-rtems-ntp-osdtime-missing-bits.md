---
sha: 5a5345d44a92547282754e16c18dc7a1dd633c79
short_sha: 5a5345d
date: 2020-09-10
author: Michael Davidsaver
category: other
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [RTEMS, NTP, time, osdTime, missing-symbols]
---
# RTEMS: missing osdNTPGet/osdTickGet symbols needed by osiNTPTime.c

## Root Cause
The `osiNTPTime.c` driver (RTEMS NTP synchronization) required platform-
specific implementations of `osdNTPGet`, `osdTickGet`, `osdTickRateGet`, and
`osdNTPReport`. When porting to RTEMS libbsd (as opposed to the legacy stack),
these symbols were missing from `osdTime.cpp`, causing link errors. The NTP
synchronization call in `rtems_init.c` (`epicsNtpGetTime`) also required a
wrapper that matched the expected signature for `osdNTPGet`.

## Symptoms
Link error on RTEMS libbsd builds: undefined symbols `osdNTPGet`,
`osdTickGet`, `osdTickRateGet`, `osdNTPReport`. IOC would fail to link.
Without NTP sync, EPICS timestamps would use the RTEMS boot time as epoch,
causing incorrect timestamps on all PV events.

## Fix
Added to `osdTime.cpp` under `#ifdef __rtems__`:
- `osdTickGet()`: returns `epicsMonotonicGet()` (nanoseconds)
- `osdTickRateGet()`: returns `1000000000` (ns/tick)
- `osdNTPReport()`: empty stub

Added to `rtems_init.c`:
- `osdNTPGet(struct timespec *now)`: wrapper calling `epicsNtpGetTime`

Added declarations to `osdTime.h` under `#ifdef __rtems__`.

## Rust Applicability
RTEMS-specific. Tokio uses `std::time::Instant` / `SystemTime` which rely on
the OS clock — there is no NTP driver layer to implement. Eliminated.

## Audit Recommendation
No audit needed. RTEMS BSP time-provider stubs with no Rust analog.

## C Locations
- `modules/libcom/src/osi/os/posix/osdTime.cpp` — adds osdTickGet, osdTickRateGet, osdNTPReport for RTEMS
- `modules/libcom/src/osi/os/posix/osdTime.h` — declares RTEMS NTP/tick interfaces
- `modules/libcom/RTEMS/posix/rtems_init.c:osdNTPGet` — new wrapper for epicsNtpGetTime
