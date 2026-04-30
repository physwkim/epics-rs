---
sha: b7cc33c3c9c1a6cc14290b9a558e97bd89171e80
short_sha: b7cc33c
date: 2024-09-02
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: dbPut
tags: [DBE_PROPERTY, DBE_VALUE, event-ordering, CA-monitor, property-update]
---
# DBE_PROPERTY event posted after DBE_VALUE instead of before

## Root Cause
In `dbPut`, after a write to a property (metadata) field, the code
posted `DBE_VALUE | DBE_LOG` first, then `DBE_PROPERTY` last.  Clients
that subscribe to `DBE_VALUE` receive the value change event, then
immediately try to format the new value using the (not-yet-updated)
property metadata.  Only after they have already rendered the old format
does the `DBE_PROPERTY` event arrive and trigger a property update.

## Symptoms
Channel Access monitors on enum, analog, or string records with
formatting metadata (EGU, PREC, enum strings) briefly display values in
the wrong format when a property field is written.  The correct format
only appears after the subsequent `DBE_PROPERTY` event is processed.

## Fix
Moved the `db_post_events(precord, NULL, DBE_PROPERTY)` call to before
the second `dbPutSpecial` pass (which may trigger `DBE_VALUE`/`DBE_LOG`).
Combined with the preceding commit that made property events conditional
on actual change, this ensures clients receive property metadata before
the value event they need to format.

## Rust Applicability
Applies.  In base-rs `dbPut` must post `DBE_PROPERTY` before
`DBE_VALUE | DBE_LOG` when a property field is written.  The ordering
matters for CA clients that render values using property metadata.

## Audit Recommendation
In `db_access.rs` (or equivalent write path), verify the event-posting
order: `DBE_PROPERTY` must be dispatched before `DBE_VALUE | DBE_LOG`
when `propertyUpdate` is true.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPut` — moved DBE_PROPERTY post before second dbPutSpecial pass
