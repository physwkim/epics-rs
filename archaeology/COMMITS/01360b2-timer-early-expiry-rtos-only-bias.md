---
sha: 01360b2a6917d2723b6b3219973a573680a687f9
short_sha: 01360b2
date: 2022-09-29
author: Michael Davidsaver
category: timeout
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [timer, early-expiry, RTOS, quantum-bias, accuracy]
---
# Avoid early timer expiration on non-RTOS by removing quantum bias subtraction

## Root Cause
`timer::privateStart()` unconditionally subtracted half a sleep quantum from
the requested expiry time (`exp = expire - quantum/2`). This was a
historical workaround for vxWorks/RTEMS tick-based timers: by rounding down
by half a tick, periodic timers could fire on every tick rather than every
other tick. On Linux/macOS/Windows, where the OS provides high-resolution
timers, this subtraction caused every timer to fire slightly earlier than
requested — up to ~50 ms on systems with 100 Hz tick rates. This affected
`calcout.ODLY`, scan period timers, and CA timeouts.

## Symptoms
Timers expire early on non-RTOS platforms by up to half the OS tick quantum.
`calcout.ODLY` records fire too early. CA connection timeout fires before the
configured deadline.

## Fix
Wrap the quantum-bias subtraction in `#ifdef TIMER_QUANTUM_BIAS`, defined
only for `vxWorks` and `RTEMS`:
```cpp
this->exp = expire
#ifdef TIMER_QUANTUM_BIAS
        - ( this->queue.notify.quantum () / 2.0 )
#endif
        ;
```

## Rust Applicability
`eliminated` — tokio uses `tokio::time::sleep` backed by `timerfd` (Linux)
or `kqueue` (macOS), which do not use a quantum subtraction. epics-rs timers
should not implement this bias. No audit needed.

## Audit Recommendation
No audit needed. If epics-rs has any hand-rolled timer queue
(e.g., for scan-period implementation), verify it does not subtract a
fraction of the clock quantum from the deadline.

## C Locations
- `modules/libcom/src/timer/timer.cpp:timer::privateStart` — unconditional quantum subtraction
- `modules/libcom/src/timer/timerPrivate.h` — `TIMER_QUANTUM_BIAS` macro added for RTOS-only
