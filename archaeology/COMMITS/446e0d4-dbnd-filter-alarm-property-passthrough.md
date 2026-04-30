---
sha: 446e0d4af8d539db69e57640dd19445311fc5b8c
short_sha: 446e0d4
date: 2021-10-03
author: Michael Davidsaver
category: flow-control
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/filters/dbnd.rs
    function: filter
tags: [dbnd, deadband-filter, DBE_ALARM, DBE_PROPERTY, event-mask, monitor]
---

# dbnd filter: pass through DBE_ALARM and DBE_PROPERTY events unconditionally

## Root Cause
The `dbnd` (deadband) filter in `dbnd.c` was designed to suppress `DBE_VALUE`
events when the value change was within the deadband. However, the filter code:

```c
send = 0;
recGblCheckDeadband(&my->last, val, my->hyst, &send, 1);
```

initialized `send = 0` unconditionally before calling `recGblCheckDeadband`.
This meant that if the event mask included `DBE_ALARM` or `DBE_PROPERTY` bits
(which should always pass through regardless of value change), those bits were
also suppressed when `recGblCheckDeadband` returned `send = 0` (within deadband).

Result: alarm state changes and property changes were silently dropped when a
`dbnd` filter was active on the subscription, even though the client explicitly
subscribed with `DBE_ALARM` or `DBE_PROPERTY`.

## Symptoms
- CA/PVA clients with a `dbnd` filter on a channel missed alarm state transitions
  when the value was within the deadband.
- `DBE_PROPERTY` events (metadata changes: EGU, HOPR/LOPR, etc.) were suppressed
  by the deadband filter, even though they have no numeric value to compare.
- A subscription `DBE_VALUE|DBE_ALARM` with dbnd could miss alarms when the
  value did not change enough to pass the deadband threshold.

## Fix
Changed the `send` initialization to preserve the non-VALUE/LOG bits:

```c
send = pfl->mask & ~(DBE_VALUE | DBE_LOG);
recGblCheckDeadband(&my->last, val, my->hyst, &send, pfl->mask & (DBE_VALUE | DBE_LOG));
```

- The third argument to `recGblCheckDeadband` (`arMask`) now tells it only to
  check the `DBE_VALUE|DBE_LOG` bits.
- `send` starts with the `DBE_ALARM | DBE_PROPERTY` bits already set, so they
  are never suppressed by the deadband check.
- The filter only applies deadband suppression to `DBE_VALUE` and `DBE_LOG` bits.

## Rust Applicability
In `base-rs`, if a deadband filter (equivalent of `dbnd`) is implemented, it
must not suppress events with `DBE_ALARM` or `DBE_PROPERTY` bits. The filter
should:
1. Extract `value_bits = field_log.mask & (DBE_VALUE | DBE_LOG)`.
2. Apply the deadband check only to `value_bits`.
3. Reconstruct the output mask as: `alarm_prop_bits | (value_bits if passed)`.

This is a semantic-correctness requirement for all filter implementations that
suppress based on value change.

## Audit Recommendation
- In `base-rs/src/server/database/filters/dbnd.rs:filter`: verify that
  `DBE_ALARM` and `DBE_PROPERTY` bits in `field_log.mask` are never cleared by
  the deadband check.
- Apply the same audit to any other value-based filter (e.g., `dbhyst`,
  `dbldap`): they should only suppress value bits, not alarm/property bits.

## C Locations
- `modules/database/src/std/filters/dbnd.c:filter` — send init preserves alarm/property bits
- `modules/database/test/std/filters/dbndTest.c` — test now sets `pfl->mask = DBE_VALUE`
