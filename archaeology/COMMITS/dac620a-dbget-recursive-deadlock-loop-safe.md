---
sha: dac620a70800e9288e98a8e9a46d036655d2ebcc
short_sha: dac620a
date: 2024-11-29
author: Dirk Zimoch
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_control_limits
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_graphic_limits
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_alarm_limits
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_precision
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_units
tags: [deadlock, recursion, dbGet, link, loop-detection]
---
# dbGet infinite recursion when input link points back to same field

## Root Cause
Records that retrieve metadata (precision, units, control limits, graphic
limits, alarm limits) from an input link call `dbGet` on the linked
address.  If that link points back to the same record field (a self-loop
or mutual loop), `dbGet` triggers `get_precision` / `get_units` etc.
again on the same record, which calls `dbGet` again, causing unbounded
recursion that eventually overflows the stack.  No cycle detection existed
before this fix.

## Symptoms
Stack overflow (SIGSEGV or crash) when a field's input link is configured
to point back to itself or to form a cycle through metadata queries.
Difficult to reproduce accidentally but can be triggered by misconfigured
databases.

## Fix
Introduced `dbDbGetOptionLoopSafe`, a wrapper that sets a per-link
`DBLINK_FLAG_VISITED` flag before calling `dbGet` and clears it
afterward.  If the flag is already set when the wrapper is re-entered, it
returns `S_dbLib_badLink` immediately, breaking the cycle.  All five
metadata getters (`dbDbGetControlLimits`, `dbDbGetGraphicLimits`,
`dbDbGetAlarmLimits`, `dbDbGetPrecision`, `dbDbGetUnits`) were refactored
to use this wrapper.  A new `DBLINK_FLAG_VISITED = 4` bit was added to
`link.h`.

## Rust Applicability
Applies.  In base-rs the equivalent metadata query functions on db-link
must guard against self-referential links.  Rust's ownership prevents
stack overflows less trivially here because the recursion goes through a
function pointer dispatch table (rset), not through Rust borrow rules.
A visited-flag or a `RecursionGuard` RAII type should be used.

## Audit Recommendation
In `db_link.rs` audit each metadata getter (get_control_limits,
get_graphic_limits, get_alarm_limits, get_precision, get_units): verify
that a per-link "visited" flag or equivalent is set before recursing into
the linked record and cleared on return.  Without this guard a
self-referential link will overflow the async stack in a Tokio task.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetOptionLoopSafe` — new loop-safe wrapper with DBLINK_FLAG_VISITED
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetControlLimits` — refactored to use loop-safe wrapper
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetGraphicLimits` — refactored to use loop-safe wrapper
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetAlarmLimits` — refactored to use loop-safe wrapper
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetPrecision` — refactored to use loop-safe wrapper
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetUnits` — refactored to use loop-safe wrapper
- `modules/database/src/ioc/dbStatic/link.h` — added DBLINK_FLAG_VISITED = 4
