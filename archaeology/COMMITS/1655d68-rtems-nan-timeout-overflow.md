---
sha: 1655d68ec4ff044de069e986060d6bed2bff8ce1
short_sha: 1655d68
date: 2022-01-05
author: Andrew Johnson
category: timeout
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [NaN, overflow, timeout, RTEMS, epicsEvent]
---

# Fix NaN/overflow timeout in RTEMS-score osdEvent epicsEventWaitWithTimeout

## Root Cause
`epicsEventWaitWithTimeout()` in the RTEMS-score OSI backend computed the tick
delay as:

```c
delay = timeout * rtemsTicksPerSecond_double;
if (delay == 0) delay++;
```

If `timeout` is `NaN`, the multiplication produces `NaN`, which when assigned
to an integer (`rtems_interval` = `uint32_t`) produces undefined behavior
(typically `0` or `UINT32_MAX` depending on hardware FPU behavior). If
`timeout` is very large (beyond `UINT32_MAX / rtemsTicksPerSecond_double`),
the multiplication overflows, also producing an incorrect tick count.

The RTEMS-posix backend already had the correct guard (`timeout < UINT32_MAX /
rate`) but the RTEMS-score backend was missing it.

## Symptoms
- On RTEMS-score targets: passing `NaN` timeout causes unpredictable wait
  duration (immediate return or near-infinite wait depending on FPU).
- Very large timeout (e.g. `1e300`) overflows the tick counter, wrapping to a
  short wait, causing premature timeout.
- Inconsistent behavior between RTEMS-posix and RTEMS-score backends.

## Fix
Added the same guard as RTEMS-posix and vxWorks:

```c
if (timeout < (double) UINT32_MAX / rtemsTicksPerSecond_double) {
    delay = timeout * rtemsTicksPerSecond_double;
    if (delay == 0) delay++;
} else {
    delay = RTEMS_NO_TIMEOUT;  /* NaN or overflow → wait forever */
}
```

## Rust Applicability
`tokio::time::timeout()` and `std::thread::park_timeout()` accept `Duration`,
which cannot represent NaN (it's a struct of u64+u32). Rust callers cannot
pass NaN to timeout APIs. This RTEMS-specific integer overflow pattern is
fully eliminated in Rust's async runtime.

## Audit Recommendation
None — eliminated by Rust's `Duration` type for timeouts.

## C Locations
- `modules/libcom/src/osi/os/RTEMS-score/osdEvent.c:epicsEventWaitWithTimeout` — NaN/overflow guard added
