---
sha: 4ad98d5b4f0c7a07c2dd0a555d8cfe9827ec0e92
short_sha: 4ad98d5
date: 2020-05-07
author: Dirk Zimoch
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [freelist, private-api, dbEvent, init-ordering, encapsulation]
---
# db_init_event_freelists exposed in public header; must be private internal API

## Root Cause
`db_init_event_freelists()` was declared in the public `dbEvent.h` header
without any guard. This function initializes the internal `db_field_log`
freelist used exclusively by `dbChannel` and the event subsystem. Exposing it
publicly allowed external callers to inadvertently re-initialize or double-init
the freelist, potentially corrupting in-flight `db_field_log` allocations.

## Symptoms
If any external code (or test) called `db_init_event_freelists()` after the
subsystem was already running, the freelist would be reset while live
allocations existed, leading to double-free or use-after-free of field log
entries in the event pipeline.

## Fix
Move `db_init_event_freelists` declaration to the `#ifdef EPICS_PRIVATE_API`
block in `dbEvent.h`, matching `db_cleanup_events`. In `dbChannel.c`, add
`#define EPICS_PRIVATE_API` before the include so that internal callers retain
access.

## Rust Applicability
Rust module visibility (`pub(crate)` / `pub(super)`) eliminates the
public-header exposure problem entirely. The freelist init would be a
`pub(crate)` function or simply called within the same module. No direct Rust
analog to audit.

## Audit Recommendation
No Rust code change required. Verify that any freelist / pool initialization
in base-rs `db_event.rs` is module-private and not re-exported.

## C Locations
- `modules/database/src/ioc/db/dbEvent.h:db_init_event_freelists` — moved from public to `EPICS_PRIVATE_API` guard
- `modules/database/src/ioc/db/dbChannel.c` — added `#define EPICS_PRIVATE_API` to retain internal access
