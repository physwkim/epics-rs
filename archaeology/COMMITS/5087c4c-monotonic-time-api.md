---
sha: 5087c4cb2f9897828829f5b15076ab29d8106ac3
short_sha: 5087c4c
date: 2018-12-23
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [monotonic-clock, epicsTime, time-api, libcom, infrastructure]
---
# Add epicsTimeGetMonotonic API bridging epicsMonotonicGet to epicsTime

## Root Cause
`epicsMonotonicGet()` (from `epicsMonotonic.h`) returned a raw `epicsUInt64`
nanosecond counter but had no bridge into the `epicsTimeStamp` / `epicsTime`
type system used by record processing, timers, and the CA client.  Without
such a bridge, call sites defaulted to `epicsTimeGetCurrent()` (wall-clock)
for time-delta calculations, exposing them to clock steps.

## Symptoms
This is an infrastructure gap commit, not a user-visible bug on its own.
Without `epicsTimeGetMonotonic`, companions `4f2228f` and `8b9ad21` could
not have made their changes.

## Fix
- Added `epicsTimeGetMonotonic(epicsTimeStamp *)` to `epicsGeneralTime.c`:
  calls `epicsMonotonicGet()`, stores result as `secPastEpoch` + `nsec`.
- Added `epicsTime::getMonotonic()` static method to `epicsTime.cpp`/`.h`:
  wraps the C function into the C++ `epicsTime` class.
- Declared `epicsTimeGetMonotonic` in `epicsTime.h` for C callers.

## Rust Applicability
Eliminated — Rust `std::time::Instant` and `tokio::time::Instant` are
monotonic by definition.  No wrapper layer needed.

## Audit Recommendation
No direct audit needed.  The Rust ecosystem provides monotonic timing
natively through `std::time::Instant`.

## C Locations
- `modules/libcom/src/osi/epicsGeneralTime.c:epicsTimeGetMonotonic` — new function
- `modules/libcom/src/osi/epicsTime.cpp:epicsTime::getMonotonic` — new C++ wrapper
- `modules/libcom/src/osi/epicsTime.h` — declaration added
