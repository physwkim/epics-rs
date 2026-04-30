---
sha: 4f2228fb1d7527fb5ebc8b2d747c309f1dd7698d
short_sha: 4f2228f
date: 2018-12-23
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [monotonic-clock, timeout, time-skew, wall-clock, ca-client]
---
# Replace getCurrent with getMonotonic for all time-delta computations

## Root Cause
All timeout and elapsed-time calculations in the CA client (and related
libcom timer infrastructure) used `epicsTime::getCurrent()`, which returns
wall-clock time.  Wall-clock time can jump backward (NTP step, leap second,
operator adjustment) or forward by large amounts.  When the clock jumps
backward, computed deltas become negative; when it jumps forward, timeouts
expire instantly.  Both cause connection drops, spurious channel re-searches,
or hung `ca_pend_io` / `ca_pend_event` calls.

Affected call sites included:
- `CASG::block` — synchronous group timeout
- `ca_client_context::pendIO`, `pendEvent` — pend timeout loops
- `cac` constructor — `programBeginTime` reference point
- `searchTimer` — beacon interval tracking
- `tcpiiu`, `udpiiu`, `casw` — connection and beacon timing

## Symptoms
Under NTP clock adjustments: CA channels disconnect and reconnect spuriously;
`ca_pend_io` returns `ECA_TIMEOUT` prematurely or blocks indefinitely;
search timers fire at wrong intervals.

## Fix
Replaced all `epicsTime::getCurrent()` call sites that compute time *deltas*
with `epicsTime::getMonotonic()`, introduced in companion commit `5087c4cb`.
Monotonic time never steps backward and is immune to wall-clock adjustments.

## Rust Applicability
Eliminated — Rust's `tokio::time::Instant` is monotonic by construction and
`std::time::Instant` is also monotonic on all supported platforms.  The CA
client in `ca-rs` should use `tokio::time::Instant` (not `SystemTime`) for
all timeout/deadline logic.  No direct audit needed; just verify no code
accidentally uses `SystemTime::now()` for timeout arithmetic.

## Audit Recommendation
Grep `ca-rs/src/client/` for any `SystemTime::now()` usage in timeout
contexts; replace with `Instant::now()`.

## C Locations
- `modules/ca/src/client/CASG.cpp:CASG::block` — two getCurrent → getMonotonic
- `modules/ca/src/client/ca_client_context.cpp:pendIO` — beg_time + delay measurement
- `modules/ca/src/client/ca_client_context.cpp:pendEvent` — elapsed time
- `modules/ca/src/client/cac.cpp:cac::cac` — programBeginTime
- `modules/ca/src/client/searchTimer.cpp:searchTimer::searchTimer` — timeAtLastSend
- `modules/ca/src/client/tcpiiu.cpp` — connection timing
- `modules/ca/src/client/udpiiu.cpp` — beacon timing
