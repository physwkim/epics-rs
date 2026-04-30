---
sha: f1cbe93b6c0a8607a7e3cfb732c0eece7c64a029
short_sha: f1cbe93
date: 2020-04-23
author: Michael Davidsaver
category: timeout
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/search_timer.rs
    function: schedule_next
  - crate: ca-rs
    file: src/client/tcp_iiu.rs
    function: connection_timeout_check
  - crate: ca-rs
    file: src/client/cac.rs
    function: process_timers
tags: [monotonic-clock, getcurrent, timeout, timer, time-source]
---

# Revert getMonotonic() → getCurrent() in CA timers and timer queue

## Root Cause
A prior commit replaced most internal `epicsTime::getCurrent()` calls with
`epicsTime::getMonotonic()` in the timer infrastructure (`timerQueue`,
`timerQueueActive`, `searchTimer`, `tcpiiu`, `cac`, `CASG`,
`ca_client_context`, `udpiiu`, `fdManager`, `dbPutNotifyBlocker`,
`epicsThread`). The intent was to use a monotonic clock for timeouts to avoid
sensitivity to wall-clock jumps. However, `epicsTime::getMonotonic()` was not
universally available (or correctly implemented) on all EPICS target platforms
at the time, and it produced incorrect (zero or epoch) timestamps on some
systems, causing timers to fire immediately or never.

The revert kept `getMonotonic()` only in test code where it was safe.

## Symptoms
- CA search timers firing at time=0 (immediately) on platforms where
  `getMonotonic()` returned 0 before the first NTP sync.
- CA connection timeout watchdog triggering spuriously, disconnecting channels
  that had just connected.
- `CASG` (synchronous group) operations timing out instantly.
- `searchTimer` re-broadcasting at maximum rate, flooding the network.

## Fix
Reverted all production timer sites back to `epicsTime::getCurrent()`:
`timerQueue`, `timerQueueActive`, `searchTimer`, `tcpiiu::connectionTimeout`,
`cac::processTimers`, `CASG`, `ca_client_context`, `udpiiu`, `fdManager`,
`dbPutNotifyBlocker`, `epicsThread`. Test code retained `getMonotonic()` where
it was safe and intentional.

## Rust Applicability
Applies. In ca-rs, all timeout/timer computations (`searchTimer`, connection
watchdog, `SyncGroup` wait) must use `tokio::time::Instant` (which is
monotonic) rather than `std::time::SystemTime` (which can jump). However, the
risk is inverted from the C bug: Rust's `tokio::time::Instant` is correctly
monotonic everywhere, so accidental use of `SystemTime` would introduce the
*old* C bug (clock-jump sensitivity). Verify no timeout code uses
`SystemTime::now()`.

## Audit Recommendation
- Search `ca-rs/src/client/` for any `SystemTime::now()` or `chrono::Utc::now()`
  used as a timeout deadline — replace with `tokio::time::Instant::now()`.
- Verify `search_timer.rs`, `tcp_iiu.rs`, and `cac.rs` all use
  `tokio::time::sleep_until` / `tokio::time::Instant` for their retry intervals.

## C Locations
- `modules/ca/src/client/searchTimer.cpp:searchTimer::notifySearchResponse` — getCurrent reverted
- `modules/ca/src/client/tcpiiu.cpp:tcpiiu::connectionTimeout` — getCurrent reverted
- `modules/ca/src/client/cac.cpp:cac::processTimers` — getCurrent reverted
- `modules/ca/src/client/CASG.cpp` — getCurrent reverted
- `modules/ca/src/client/ca_client_context.cpp` — getCurrent reverted
- `modules/ca/src/client/udpiiu.cpp` — getCurrent reverted
- `modules/libcom/src/timer/timerQueue.cpp` — getCurrent reverted
- `modules/libcom/src/timer/timerQueueActive.cpp` — getCurrent reverted
