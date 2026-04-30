---
sha: e542a22631969de80a9a9a366943531aa958fb3d
short_sha: e542a22
date: 2020-08-04
author: Andrew Johnson
category: timeout
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [monotonic-clock, Darwin, resolution, floating-point, timeout]
---

# Darwin epicsMonotonicResolution returns wrong value due to float multiply

## Root Cause
`epicsMonotonicResolution()` on Darwin computed the clock resolution as
`1e-9 * tbinfo.numer / tbinfo.denom`. The function return type is
`epicsUInt64` (nanoseconds), so the intent was integer division of
`numer / denom` (which gives nanoseconds-per-tick). Multiplying by `1e-9`
first converts to fractional seconds, then truncation to `uint64` gives
0 on any system where `numer/denom < 1e9` (i.e., almost all), or gives
a garbage large integer if the float is < 1.0.

## Symptoms
`epicsMonotonicResolution()` returns 0 on macOS, causing any timeout
calculation that uses the resolution to compute deadline adjustments to
malfunction (divide-by-zero, zero-duration timeouts, infinite loops).

## Fix
Removed `1e-9 *` — the correct expression is `tbinfo.numer / tbinfo.denom`
which gives the clock tick size in nanoseconds (typically 1 for modern
Apple Silicon).

## Rust Applicability
Rust code uses `std::time::Instant` or `tokio::time::Instant` for
monotonic timing, which are platform-correct by construction. `tokio`
does not rely on `epicsMonotonicResolution`. This is fully eliminated.

## Audit Recommendation
No audit needed. If epics-rs has a monotonic-clock FFI binding that
calls `epicsMonotonicResolution`, remove it in favor of
`std::time::Instant::now()`. Otherwise fully eliminated.

## C Locations
- `modules/libcom/src/osi/os/Darwin/osdMonotonic.c:epicsMonotonicResolution` — removed erroneous `1e-9 *` float multiply
