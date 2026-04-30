---
sha: f6e8a750266fe0d2b64dac5bbcc0e1ec5228d25a
short_sha: f6e8a75
date: 2021-08-12
author: Michael Davidsaver
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: db_db_get_value
tags: [DBF_MENU, DBF_ENUM, type-promotion, db-link, channel-type]
---
# DB link reads DBF_MENU field as DBF_ENUM due to wrong type query

## Root Cause
In the fast-path for simple scalar DB links without filters,
`dbDbGetValue()` called `dbChannelFinalFieldType()` to determine the field
type. For `DBF_MENU` fields, `dbChannelOpen()` promotes the reported type to
`DBF_ENUM` (for CA compatibility), so `dbChannelFinalFieldType()` returns
`DBF_ENUM`. The fast-path comparison `dbfType > DBF_DEVICE` then incorrectly
excluded MENU fields from the scalar shortcut, or applied the wrong conversion
routine. Additionally, using the "final" (promoted) type caused the get
dispatch to treat a MENU as a ENUM string-pair, losing direct integer access.

## Symptoms
- Reads of `DBF_MENU` fields via DB links return wrong values or error
  `S_db_badDbrtype` when accessed as a native integer type (e.g., from a
  `calc` record linking to a menu field).
- The symptom only appeared on the shortcut path (no filters, simple scalar)
  because the slow path correctly used the original field type.

## Fix
Replace `dbChannelFinalFieldType(chan)` with `dbChannelFieldType(chan)` in the
fast-path type check. The comment explains: for a simple scalar without filters,
the "final" type carries no additional information, but for `DBF_MENU` it
incorrectly returns `DBF_ENUM` due to the probe in `dbChannelOpen()`. The
original field type (`DBF_MENU`) must be used for the conversion dispatch.

## Rust Applicability
In `base-rs`, when implementing the DB-link fast path for scalar fields, the
channel's native field type (pre-promotion) must be used for the conversion
dispatch, NOT the CA-compatible exported type. If a Rust `DbChannel` struct
stores both `field_type` and `ca_type` (or `final_type`), the link resolver
must use `field_type` when selecting the `get_value` conversion routine. Using
the CA-exported type will silently mishandle MENU fields, treating them as ENUM
(string index pair) instead of a raw integer.

## Audit Recommendation
In `base-rs/src/server/database/db_link.rs::db_db_get_value` (or equivalent):
verify the scalar fast-path uses `channel.field_type()` (pre-promotion) rather
than `channel.ca_type()` or `channel.exported_type()`. Add a unit test:
create a MENU field, link to it via a DB link, and verify the integer value
round-trips correctly without going through string conversion.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — `dbChannelFinalFieldType` → `dbChannelFieldType` in fast-path type check
