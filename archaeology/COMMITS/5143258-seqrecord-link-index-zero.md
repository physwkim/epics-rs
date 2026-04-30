---
sha: 5143258011e8180ba51d97f8915618576cc9048e
short_sha: 5143258
date: 2024-11-29
author: Dirk Zimoch
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [seqRecord, link-index, off-by-one, DLY0, field-offset]
---
# seqRecord link index 0 (DLY0/DO0) not handled by metadata getters

## Root Cause
`get_units`, `get_precision`, `get_graphic_double`, `get_control_double`,
and `get_alarm_double` all computed their field-group offset relative to
`indexof(DLY1)` — the first link group starting at index 1 — rather than
`indexof(DLY0)`.  This off-by-one origin meant that `DLY0` and `DO0`
yielded a negative `fieldOffset` and fell through the `if (fieldOffset >= 0)`
guard, silently returning default (uninitialised or zero) metadata for
link group 0.

## Symptoms
`DLY0` (group 0 delay) always appeared with zero units and zero precision.
`DO0` (group 0 output) returned no alarm limits.  Only links 1–15 were
handled correctly.

## Fix
Changed all five offset-computation baselines from `indexof(DLY1)` to
`indexof(DLY0)`.

## Rust Applicability
Eliminated.  In Rust, seq-record field groups would be stored as a `Vec`
or array indexed from 0; metadata dispatch would iterate over a slice and
never miscompute the base index.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/std/rec/seqRecord.c:get_units` — base changed to indexof(DLY0)
- `modules/database/src/std/rec/seqRecord.c:get_precision` — base changed to indexof(DLY0)
- `modules/database/src/std/rec/seqRecord.c:get_graphic_double` — base changed to indexof(DLY0)
- `modules/database/src/std/rec/seqRecord.c:get_control_double` — base changed to indexof(DLY0)
- `modules/database/src/std/rec/seqRecord.c:get_alarm_double` — base changed to indexof(DLY0)
