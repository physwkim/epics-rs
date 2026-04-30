---
sha: 7ae3e4c2dfc258ea3a872013aad1dcfc3f6e1c35
short_sha: 7ae3e4c
date: 2025-11-19
author: Michael Davidsaver
category: timeout
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [timer, early-expiry, test, TIMER_QUANTUM_BIAS, accuracy]
---
# epicsTimerTest: assert timers do not expire earlier than requested

## Root Cause
The `epicsTimerTest` test harness checked only the absolute magnitude of
the timing error (`fabs(measuredError) < minError`), which allowed timers
to expire arbitrarily early. On non-RTOS platforms where `TIMER_QUANTUM_BIAS`
is not defined (see companion commit 01360b2), early expiration is a latent
bug because the rounding subtraction was removed. The test did not catch
early expiration at all.

## Symptoms
Tests would pass even when a timer fired before its deadline. This masked
real regressions in timer accuracy.

## Fix
Split the error bound into a directional lower and upper bound:
- On `TIMER_QUANTUM_BIAS` platforms (vxWorks/RTEMS): lower bound is
  `-quantum/2` (one tick early is still acceptable).
- On non-RTOS platforms: lower bound is `0.0` (no early expiration allowed).
- Upper bound remains the platform-specific imprecision tolerance.

The test now uses `measuredError >= lowerBound && measuredError < upperBound`.

## Rust Applicability
`eliminated` — tokio's `time::sleep` is backed by OS timers (epoll/kqueue
timerfd) and does not use a quantum-bias rounding trick. Rust timer accuracy
is validated by tokio's own test suite. No equivalent audit needed in epics-rs.

## Audit Recommendation
No audit needed in epics-rs. Note: if base-rs wraps a legacy EPICS timer
queue, verify that early-expiration cannot occur when the OS timer fires
slightly before the deadline due to scheduler jitter.

## C Locations
- `modules/libcom/test/epicsTimerTest.cpp:delayVerify::checkError` — absolute error check replaced by directional bounds
