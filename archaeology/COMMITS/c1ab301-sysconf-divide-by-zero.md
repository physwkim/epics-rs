---
sha: c1ab30142abbfe3d354efdb08c07ef28fa3036e8
short_sha: c1ab301
date: 2019-06-26
author: Michael Davidsaver
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [divide-by-zero, sysconf, sleep-quantum, paranoia, posix]
---
# epicsThreadSleepQuantum divides by sysconf result without guarding against zero

## Root Cause
`epicsThreadSleepQuantum()` in `osdThread.c` calls `sysconf(_SC_CLK_TCK)` to
get the system clock frequency, then computes `1.0 / hz`. The guard was
`if (hz < 0) return 0.0` — but if `sysconf` returns 0 (theoretically possible,
though pathological), the division `1.0 / 0.0` would produce `+Inf` rather
than 0, leading to callers receiving an infinite sleep quantum.

## Symptoms
If `sysconf(_SC_CLK_TCK)` returns 0 (undefined/broken system config),
`epicsThreadSleepQuantum()` returns `+Inf`. Any timer using this value as a
sleep duration would block forever.

## Fix
Change `if (hz < 0)` to `if (hz <= 0)` so that both error (-1) and
pathological-zero cases return `0.0` (no sleep quantum).

## Rust Applicability
In Rust, `tokio::time` and `std::time::Duration` handle sleep durations with
type safety — duration of zero is well-defined (immediate), and infinity is not
representable in `Duration`. There is no `sysconf` call; the Tokio runtime
manages its own timer resolution. This bug is eliminated by the platform
abstraction.

## Audit Recommendation
No Rust code change required. If ca-rs or base-rs has any code calling
`libc::sysconf` directly and dividing by the result, add a `<= 0` guard.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadSleepQuantum` — guard `hz <= 0` instead of `hz < 0`
