---
sha: 4ee766b6b1ef87ca8ea9152137718d254bad0fae
short_sha: 4ee766b
date: 2024-11-29
author: Dirk Zimoch
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [seqRecord, bitmask, field-index, DLY, DOL]
---
# seqRecord wrong bitmask `& 2` should be `& 3` for DLY/DO field type

## Root Cause
`seqRecord` stores pairs of fields: `DLYn` (delay, even offset) and `DOn`
(output, odd offset).  The `get_units`, `get_precision`,
`get_graphic_double`, `get_control_double`, and `get_alarm_double`
functions select the field type using a bitmask on the field-group offset.
The original code used `fieldOffset & 2` (mask for bit 1 only) instead
of `fieldOffset & 3` (mask for bits 0 and 1).  With a 2-bit group stride
`(DLYn, DOn)`, the correct modulus is `& 3` to distinguish index 0 (DLY),
1 (DO), 2 (DLY next pair), 3 (DO next pair).  The mask `& 2` would
misidentify offset 3 as a DLY field.

## Symptoms
`get_units`, `get_precision`, `get_graphic_double`, and `get_control_double`
would return incorrect metadata for `DOn` fields at odd positions that map
to fieldOffset ≡ 3 (mod 4).  In practice this was masked because these
functions are never called for link fields (`bit0 = 1`).

## Fix
Changed all five switch/condition expressions from `fieldOffset & 2` to
`fieldOffset & 3`.

## Rust Applicability
Eliminated.  seq-record field metadata dispatch in Rust would use an
enum or a pattern-match over field kind, not a raw bitmask arithmetic.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/std/rec/seqRecord.c:get_units` — `& 2` → `& 3`
- `modules/database/src/std/rec/seqRecord.c:get_precision` — `& 2` → `& 3`
- `modules/database/src/std/rec/seqRecord.c:get_graphic_double` — `& 2` → `& 3`
- `modules/database/src/std/rec/seqRecord.c:get_control_double` — `& 2` → `& 3`
- `modules/database/src/std/rec/seqRecord.c:get_alarm_double` — `& 2` → `& 3`
