---
sha: 3506d115583282bc22c025818603a53dc7970659
short_sha: 3506d11
date: 2020-08-03
author: Andrew Johnson
category: timeout
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [macos, clock, posix, time, osd]
---

# Speed up osdTimeGetCurrent() on recent macOS using POSIX clock_gettime

## Root Cause
The macOS time provider used the Mach kernel clock service (`clock_get_time(host_clock, &mts)`) unconditionally. On macOS 10.12+ `CLOCK_REALTIME` is available via POSIX `clock_gettime()`, which is significantly faster than the Mach IPC round-trip. The code lacked a compile-time check for `CLOCK_REALTIME`.

## Symptoms
CA timeout accuracy degrades on macOS hosts: `convertDoubleToWakeTime()` and `osdTimeGetCurrent()` both called `clock_get_time()`, incurring unnecessary Mach IPC overhead that slows CA timer resolution on modern macOS, potentially causing spurious connection timeouts.

## Fix
Wrapped the Mach clock path in `#ifndef CLOCK_REALTIME` and added a macro-based dispatch so modern macOS uses `clock_gettime(CLOCK_REALTIME, &ts)` directly. Both `osdTimeGetCurrent()` and `convertDoubleToWakeTime()` now use the faster path when available.

## Rust Applicability
Rust uses `std::time::SystemTime::now()` (which calls `clock_gettime(CLOCK_REALTIME)` on all modern Unix platforms) and Tokio uses `Instant::now()`. No OS-specific time abstraction layer exists in epics-rs; this entire class of bug is structurally eliminated.

## Audit Recommendation
No audit needed. Rust/Tokio time primitives are always POSIX-based on macOS.

## C Locations
- `modules/libcom/src/osi/os/Darwin/osdTime.cpp:osdTimeGetCurrent` — Mach vs POSIX clock dispatch
- `modules/libcom/src/osi/os/Darwin/osdTime.cpp:convertDoubleToWakeTime` — same issue in timeout helper
