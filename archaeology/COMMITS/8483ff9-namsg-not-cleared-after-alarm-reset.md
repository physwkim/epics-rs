---
sha: 8483ff95608ca81a0cd84547e913924ba9b23b34
short_sha: 8483ff9
date: 2024-11-14
author: Jure Varlec
category: lifecycle
severity: low
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec_gbl.rs
    function: recGblResetAlarms
tags: [alarm, NAMSG, NSTAT, NSEV, stale-message, clear]
---
# NAMSG not cleared after alarm promoted to AMSG, leaving stale message

## Root Cause
`recGblResetAlarms` promotes the pending alarm state (NSTA/NSEV/NAMSG)
to the live alarm state (STAT/SEVR/AMSG) and resets NSTA and NSEV to
zero.  However, NAMSG was not zeroed after being copied to AMSG.  When
the next record processing cycle produces no alarm, NSTAT/NSEV are set to
zero but NAMSG retained the stale message from the previous alarm.
Because NAMSG was non-empty, the code comparing `namsg != amsg` could
still trigger another `DBE_ALARM` event even when no alarm was active.

## Symptoms
Records that previously had an alarm message would show a stale error
string in AMSG even after clearing alarm status.  Clients subscribed to
`DBE_ALARM` could receive spurious alarm events.

## Fix
Added `pdbc->namsg[0] = '\0';` immediately after copying NAMSG to AMSG
in `recGblResetAlarms`, matching the clear-after-promote pattern already
used for NSTA and NSEV.

## Rust Applicability
Applies.  In base-rs the alarm-reset routine must clear the pending alarm
message field (`namsg`/`pending_amsg`) to an empty string after promoting
it to the live field, mirroring the clear-after-promote pattern for
`nstat` and `nsev`.

## Audit Recommendation
In `rec_gbl.rs` audit `recGblResetAlarms` (or its Rust equivalent):
after `amsg = namsg`, verify `namsg` is reset to an empty string.  Also
check that the `DBE_ALARM` event-trigger comparison accounts for this
clear.

## C Locations
- `modules/database/src/ioc/db/recGbl.c:recGblResetAlarms` — added namsg[0] = '\0' after copying to amsg
