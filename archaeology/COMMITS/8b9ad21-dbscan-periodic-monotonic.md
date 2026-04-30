---
sha: 8b9ad212c4eb43dca632bcc08d47677081d6fa70
short_sha: 8b9ad21
date: 2018-12-23
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [monotonic-clock, periodic-scan, dbScan, time-skew, scheduler]
---
# Periodic scan task must use monotonic clock for interval timing

## Root Cause
The `periodicTask()` scan loop in `dbScan.c` used `epicsTimeGetCurrent()` to
set the `next` reference point and to compute the `delay` until the next scan
interval.  Wall-clock time can step backward (NTP) or forward, causing the
delay computation `delay = next - now` to be negative (scan fires immediately
in a busy-loop) or very large (scan stalls for many intervals).

## Symptoms
Under NTP adjustments: periodic scan records fire too frequently (CPU spike)
or stop scanning entirely until the wall-clock catches up to the previously
computed `next` deadline.  The `overtime` detection logic also misfired.

## Fix
Changed both `epicsTimeGetCurrent` calls in `periodicTask` to
`epicsTimeGetMonotonic`, which is provided by the companion commit `5087c4cb`.
Initial `next` is now set from monotonic time; delay is measured using
monotonic time; the overtime reporting reference (`reported`) was already
derived from the same `next` value so it also becomes monotonic.

## Rust Applicability
Eliminated — `base-rs` periodic scan tasks will be driven by
`tokio::time::interval` / `tokio::time::sleep_until`, both of which use the
monotonic `Instant` internally.  No manual clock calls needed.

## Audit Recommendation
Verify that any periodic timer in `base-rs/src/server/database/db_scan.rs`
uses `tokio::time::interval` rather than `SystemTime`.

## C Locations
- `modules/database/src/ioc/db/dbScan.c:periodicTask` — two epicsTimeGetCurrent → epicsTimeGetMonotonic
