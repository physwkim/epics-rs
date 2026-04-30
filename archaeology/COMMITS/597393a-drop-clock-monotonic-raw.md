---
sha: 597393a8eeb0d41dd07fd0170672c2a95156cf12
short_sha: 597393a
date: 2019-03-25
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [clock, monotonic, performance, linux, portability]
---
# libCom: drop CLOCK_MONOTONIC_RAW (10x slower than CLOCK_MONOTONIC)

## Root Cause
`osdMonotonicInit()` probed `CLOCK_MONOTONIC_RAW` (Linux-specific) first in the
clock-preference list. `CLOCK_MONOTONIC_RAW` is a hardware counter not adjusted
by NTP, which sounds attractive for a monotonic clock. However it is
approximately 10× slower to query than `CLOCK_MONOTONIC` on Linux because it
bypasses the vDSO fast path and requires a kernel syscall each time.
`CLOCK_MONOTONIC` goes through vDSO and is nearly free.

## Symptoms
- Significant performance regression on Linux systems that use
  `osdTimeGetCurrent` in hot paths (timer queues, CA timestamps, etc.).
- `CLOCK_MONOTONIC_RAW` gives no accuracy benefit for EPICS use cases since
  EPICS does not require true hardware-rate monotonicity.

## Fix
Remove `CLOCK_MONOTONIC_RAW` from the probed-clock list entirely. The fallback
`CLOCK_MONOTONIC` provides adequate monotonicity through vDSO.

## Rust Applicability
Eliminated. Rust/tokio uses `std::time::Instant` which on Linux maps to
`CLOCK_MONOTONIC` (vDSO-backed), not `CLOCK_MONOTONIC_RAW`. No action needed.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/os/posix/osdMonotonic.c:osdMonotonicInit` — removed `CLOCK_MONOTONIC_RAW` from probe list
