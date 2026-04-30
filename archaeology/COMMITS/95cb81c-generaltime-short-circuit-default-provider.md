---
sha: 95cb81c286258e4534afc8426bba53cbdddfd787
short_sha: 95cb81c
date: 2019-03-10
author: Michael Davidsaver
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [time, performance, short-circuit, provider, race]
---
# generalTime: short-circuit epicsTimeGetCurrent to OS clock when only default registered

## Root Cause
`epicsTimeGetCurrent()` and `generalTimeGetExceptPriority()` always took the
full path: call `generalTime_Init()`, acquire a lock, iterate the `timeProviders`
list. Since the overwhelming majority of calls happen when only the default OS
clock provider is registered, this lock+list traversal was wasted overhead on
every timestamp acquisition.

A secondary correctness concern: `gtMoreThanDefault` is a plain `int` read and
written without a memory barrier. The short-circuit read happens before
`generalTime_Init()`, which means on the first call the init might not have run.
However, `gtMoreThanDefault` is only ever set from `insertProvider()` under the
lock; since the OS default is registered during init, and additional providers
are registered only after init, the unsynchronized read is safe as a one-way
latch (0 → 1 transition only, monotone).

## Symptoms
- Excessive lock contention on `epicsTimeGetCurrent()` in systems with many
  threads timestamping events (CA monitors, scan tasks, etc.).

## Fix
Add `gtMoreThanDefault` flag (set in `insertProvider()` when a non-default
provider is added). In `epicsTimeGetCurrent()` and `generalTimeGetExceptPriority()`,
check `!gtMoreThanDefault` first and call `osdTimeGetCurrent()` directly if
true, bypassing the lock and provider list.

## Rust Applicability
Eliminated. Rust/tokio uses `std::time::SystemTime::now()` or `Instant::now()`
directly; no provider-list abstraction. No hot-path lock to bypass.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/epicsGeneralTime.c:epicsTimeGetCurrent` — short-circuit on `!gtMoreThanDefault`
- `modules/libcom/src/osi/epicsGeneralTime.c:generalTimeGetExceptPriority` — same
- `modules/libcom/src/osi/epicsGeneralTime.c:insertProvider` — sets `gtMoreThanDefault`
- `modules/libcom/src/osi/osiClockTime.c:ClockTimeGetCurrent` — renamed to `osdTimeGetCurrent`
