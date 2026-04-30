---
sha: 9e7cd241e07048f06212013494233feebb80e7eb
short_sha: 9e7cd24
date: 2024-09-02
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec/mbbi_record.rs
    function: special
  - crate: base-rs
    file: src/server/database/rec/mbbo_record.rs
    function: special
tags: [DBE_PROPERTY, mbbi, mbbo, enum-strings, CA-monitor]
---
# DBE_PROPERTY events missing for mbbi/mbbo when val != changed string index

## Root Cause
In `mbbiRecord` and `mbboRecord`, the `special` function handles writes
to the enum-string fields `ZRSTn` through `FFSTn`.  The old code posted
`DBE_PROPERTY` only when the currently selected value index matched the
changed string field index (`val == fieldIndex - ZRST`); otherwise it
skipped the `DBE_PROPERTY` event entirely and only posted `DBE_VALUE |
DBE_LOG` when the match held.

The correct behavior is: a change to any enum string is always a property
change and should always post `DBE_PROPERTY` (handled by the parent
`dbAccess.c:dbPut` path after this fix).  The record `special` only
needs to post `DBE_VALUE | DBE_LOG` when the currently displayed string
changed (i.e., `val` matches the changed index).

## Symptoms
Clients subscribed to `DBE_PROPERTY` on mbbi/mbbo records would not
receive the event when an enum string other than the currently selected
one was changed.  Their cached enum-string list would go stale.

## Fix
Removed the `DBE_PROPERTY` post from the record `special` handler.
`DBE_PROPERTY` is now posted centrally by `dbPut` (see b7cc33c) when any
property field is written.  The `special` handler retains only the
`DBE_VALUE | DBE_LOG` post, conditioned on `val == changed-string-index`.

## Rust Applicability
Applies.  In base-rs mbbi/mbbo record `special` handlers must not post
`DBE_PROPERTY` themselves; that responsibility belongs to the central
write path.  Audit that the record's `special` function only posts
`DBE_VALUE | DBE_LOG` when the active enum string changed.

## Audit Recommendation
In `mbbi_record.rs` and `mbbo_record.rs`, verify that the `special`
handler for ZRST–FFST field writes does NOT call `db_post_events` with
`DBE_PROPERTY`.  That event must be posted by `db_access.rs::dbPut`.

## C Locations
- `modules/database/src/std/rec/mbbiRecord.c:special` — removed DBE_PROPERTY post; kept DBE_VALUE|DBE_LOG only when val matches
- `modules/database/src/std/rec/mbboRecord.c:special` — same fix
