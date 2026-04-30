---
sha: 7b6e48f4e0cffc50b121fb0c5521c634261665ee
short_sha: 7b6e48f
date: 2020-02-11
author: Michael Davidsaver
category: timeout
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/bin/casw.rs
    function: main
tags: [monotonic-clock, wall-clock, beacon, casw, timing]
---

# casw uses monotonic clock for beacon timestamps — wrong clock domain

## Root Cause
`casw` (CA server watcher / beacon analyzer) used `epicsTime::getMonotonic()`
for both `programBeginTime` and `currentTime` when recording beacon arrival
times and computing inter-beacon intervals. CA beacons carry a
**wall-clock** timestamp (the IOC's system clock); comparing a monotonic
arrival time to a wall-clock beacon timestamp produces nonsensical deltas.
Additionally, beacon anomaly detection compares relative time to epoch-based
absolute times stored in the hash table — using a monotonic epoch makes this
comparison incorrect unless the system never suspends.

## Symptoms
- Spurious beacon anomaly reports (or missed real anomalies) in `casw`
  output when the system clock and monotonic clock diverge (NTP step,
  hibernation, VM migration).
- Wrong inter-beacon interval calculations.

## Fix
Replace both `epicsTime::getMonotonic()` calls with `epicsTime::getCurrent()`
(wall clock).

## Rust Applicability
In ca-rs, any beacon timing code must use `std::time::SystemTime` (wall clock)
rather than `std::time::Instant` (monotonic). `Instant` is appropriate for
measuring elapsed durations; `SystemTime` is required when timestamps must
correlate with CA beacon payloads or EPICS wall-clock timestamps.

## Audit Recommendation
Audit `ca-rs/src/bin/casw.rs::main` and any beacon receive/process path:
1. Confirm beacon arrival timestamps use `SystemTime::now()` not `Instant::now()`.
2. Check beacon hash-table entries — verify stored times are wall-clock.
3. If inter-beacon interval is computed for anomaly detection, ensure both
   endpoints are from the same clock domain.

## C Locations
- `modules/ca/src/client/casw.cpp:main` — two `getMonotonic()` replaced by `getCurrent()`
