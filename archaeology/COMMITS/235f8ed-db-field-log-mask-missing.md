---
sha: 235f8ed2fb85270a1b9edddbff6a1c5b10f484b9
short_sha: 235f8ed
date: 2020-04-20
author: Michael Davidsaver
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_create_event_log
  - crate: base-rs
    file: src/server/database/db_field_log.rs
    function: null
tags: [db_field_log, event-mask, DBE_PROPERTY, filter, monitor]
---

# db_field_log missing DBE_* mask prevents filter from distinguishing DBE_PROPERTY

## Root Cause

`db_field_log` did not carry the originating subscription event mask (`DBE_VALUE`,
`DBE_ALARM`, `DBE_PROPERTY`, etc.). Channel-access and PVA filters that run on
event logs need to distinguish property-change events from value-change events
to avoid, e.g., dropping property notifications that happen to match a
value-change filter condition.

Without the mask field, filters received an event log with `ctx == dbfl_context_event`
but had no way to know which `DBE_*` bits triggered the event. A filter checking
for `DBE_PROPERTY` would always see `mask == 0` and either drop valid property
events or apply incorrect logic.

## Symptoms

- Filters (e.g., `utag` filter) cannot guard against `DBE_PROPERTY` events;
  in utag.c the fix `if(pfl->ctx!=dbfl_context_event || pfl->mask&DBE_PROPERTY)`
  was already present but the mask was always 0 before this commit.
- Channel access monitors with compound filters may process or drop property
  events incorrectly.

## Fix

Added `unsigned char mask` field to `db_field_log` struct (between the bitfield
flags and the timestamp). In `db_create_event_log()` (dbEvent.c), set
`pLog->mask = pevent->select` so the triggering subscription's `select` bits
are propagated into every event log.

## Rust Applicability

The base-rs equivalent of `db_field_log` must carry a `mask: u8` (or `EventMask`)
field. When creating event logs from subscription callbacks, the originating
subscription's event mask must be copied into `mask`. Filters that check
`DBE_PROPERTY` behavior in Rust must read this field, not assume a default.

## Audit Recommendation

1. Locate the Rust `DbFieldLog` / `FieldLog` struct — verify it contains an
   event-mask field.
2. Locate `db_create_event_log` equivalent — verify the subscription's select
   mask is stored into the log.
3. Any filter implementation that guards on `DBE_PROPERTY` must read the field,
   not hard-code behavior.

## C Locations
- `modules/database/src/ioc/db/db_field_log.h:db_field_log` — added `mask` field
- `modules/database/src/ioc/db/dbEvent.c:db_create_event_log` — sets `pLog->mask = pevent->select`
