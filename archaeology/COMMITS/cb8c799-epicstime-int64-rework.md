---
sha: cb8c7998b62701a849a6fa9c299cc1613f66a627
short_sha: cb8c799
date: 2020-09-18
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [timestamp, arithmetic, overflow, epicsTime, int64]
---

# epicsTime Reworked to epicsInt64 Arithmetic to Avoid Overflow

## Root Cause
The old `epicsTime` C++ class stored time internally as `(unsigned long secPastEpoch, unsigned long nSec)`
and performed epoch conversion via `double` arithmetic and `difftime()`. On
64-bit platforms, intermediate `double` conversions introduced precision loss
for fine-grained timestamps, and the `addNanoSec()` helper used `long nSecAdj`
which silently truncated negative adjustments (the guard `if (nSecAdj <= 0) return`
meant negative offsets were simply dropped rather than handled).

The cross-platform `time_t` handling via `double` was also fragile on systems
where `time_t` is not 1-second-per-tick.

## Symptoms
- Sub-microsecond timestamp precision loss on 64-bit systems using `double`
  intermediate representation
- Negative time adjustments silently discarded by `addNanoSec`
- Potential for epoch conversion errors on non-POSIX targets where
  `time_tSecPerTick != 1.0`

## Fix
Re-implemented `epicsTime` as a thin C++ wrapper around the C `epicsTimeStamp`
struct (`{epicsUInt32 secPastEpoch, epicsUInt32 nsec}`). All arithmetic now
uses `epicsInt64` rather than `double`. Removed the `l_fp` NTP conversion
struct and `epicsTimeLoadTimeInit` helper class. Introduced `throwError()` for
uniform error propagation.

## Rust Applicability
Eliminated. In Rust/epics-rs, timestamps are represented directly using
`std::time::Duration` / `std::time::Instant` or a newtype around
`(u32 sec_past_epics_epoch, u32 nsec)`. Integer arithmetic is used natively;
there is no double-conversion path. The specific bugs (silent negative-nsec
drop, double overflow) do not exist in idiomatic Rust.

## Audit Recommendation
No direct audit needed. Verify that any EPICS timestamp serialization/
deserialization in `base-rs` uses `u32` fields for both `secPastEpoch` and
`nsec` without intermediate `f64` conversion (e.g., no `as f64` cast in the
timestamp arithmetic path).

## C Locations
- `modules/libcom/src/osi/epicsTime.cpp:addNanoSec` — silent negative drop removed by full rework
- `modules/libcom/src/osi/epicsTime.cpp:epicsTimeLoadTimeInit` — double-based epoch conversion replaced
