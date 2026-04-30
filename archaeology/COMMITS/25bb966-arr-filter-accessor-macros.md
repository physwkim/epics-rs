---
sha: 25bb966cbc4be1f2a47116a8b9ec940bc2270b12
short_sha: 25bb966
date: 2020-03-14
author: Andrew Johnson
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [array-filter, accessor-macro, encapsulation, dbChannel, bounds]
---

# Array filter arr.c: use dbChannel accessor macros, not raw struct fields

## Root Cause
The array filter (`modules/database/src/std/filters/arr.c`) accessed
`dbChannel` internal fields directly (`chan->addr.no_elements`,
`chan->addr.special`, `chan->addr.pfield`, `chan->addr.field_type`,
`chan->addr.field_size`) instead of going through the provided accessor
macros (`dbChannelElements`, `dbChannelSpecial`, `dbChannelField`,
`dbChannelFieldType`, `dbChannelFieldSize`). If the internal layout of
`dbChannel` changes, or if those fields are later overridden by the channel
layer (e.g., a plugin filter), the raw field access silently returns stale
or incorrect values, leading to wrong element counts or out-of-bounds array
slicing.

## Symptoms
- Array slice results with incorrect element count or offset when the channel
  has a special override (SPC_DBADDR) and `get_array_info` adjusts the live
  element count at runtime.
- Potential silent data corruption if field layout changes between EPICS
  releases.

## Fix
Replace all six direct struct-field accesses with their corresponding
accessor macros throughout the `filter()` function. The fix is purely a
refactor from the API surface; no algorithm changes.

## Rust Applicability
In Rust the database layer (`base-rs`) owns the channel abstraction. Channel
fields are accessed via typed Rust structs/methods, so there is no equivalent
of raw C struct pointer access bypassing an abstraction layer. The Rust type
system enforces this at compile time. No audit needed.

## Audit Recommendation
None — eliminated by Rust's type system.

## C Locations
- `modules/database/src/std/filters/arr.c:filter` — six direct struct-field
  accesses replaced by accessor macros
