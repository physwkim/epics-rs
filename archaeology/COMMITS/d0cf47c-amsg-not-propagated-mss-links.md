---
sha: d0cf47cd6f38a0fc8995510ea849111e4253615c
short_sha: d0cf47c
date: 2024-11-19
author: Jure Varlec
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_value
  - crate: base-rs
    file: src/server/database/rec_gbl.rs
    function: recGblInheritSevr
tags: [alarm, AMSG, MSS-link, propagation, severity]
---
# AMSG alarm message not propagated through MSS links

## Root Cause
`recGblInheritSevr` handled the `pvlOptMSS` link-option case by calling
`recGblSetSevr(precord, stat, sevr)` which carries status + severity but
drops the alarm message string (`amsg`/`namsg`).  MSS (Maximum Severity
with Status) links are supposed to inherit the full alarm state including
message, but the message was silently discarded.

## Symptoms
A record inheriting alarm status via an MS/MSS link would show the
correct STAT/SEVR values but the AMSG field would remain blank or stale.
Operators relying on the alarm message for diagnostics would see no
message on the downstream record.

## Fix
Renamed the internal function to `recGblInheritSevrMsg`, added a `msg`
parameter, and routed the `pvlOptMSS` case through
`recGblSetSevrMsg(precord, stat, sevr, "%s", msg)`.  The old
`recGblInheritSevr` wrapper now calls `recGblInheritSevrMsg(..., NULL)`.
Both `dbDbGetValue` (read path) and `dbDbPutValue` (write path) now pass
`amsg`/`namsg` to the new function.

## Rust Applicability
Applies.  In base-rs the equivalent of `recGblInheritSevr` needs to
propagate `amsg` when link mode is MSS.  Audit the alarm inheritance
helper and both the get-value and put-value db-link paths.

## Audit Recommendation
In `db_link.rs` check both the get-value and put-value link paths:
wherever alarm severity is inherited from the linked record, verify that
the alarm message (`amsg`) is also copied when link mode is MSS.
In `rec_gbl.rs` confirm that `inherit_sevr` or its equivalent accepts
and propagates an optional message string for the MSS case.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — called recGblInheritSevr, now calls recGblInheritSevrMsg with amsg
- `modules/database/src/ioc/db/dbDbLink.c:dbDbPutValue` — same fix for write path, passes namsg
- `modules/database/src/ioc/db/recGbl.c:recGblInheritSevr` — refactored to delegate to recGblInheritSevrMsg
- `modules/database/src/ioc/db/recGbl.h` — new recGblInheritSevrMsg declaration
