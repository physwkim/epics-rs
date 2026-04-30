---
sha: 14476391c0c497830c54c87241be015ad456a414
short_sha: 1447639
date: 2020-10-28
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [freelist, NULL-guard, dbEvent, lifecycle, event-task]
---

# NULL guard missing in event_task free-list return path

## Root Cause
In `event_task()`, after the event user context is torn down, the code
called `freeListFree(dbevEventUserFreeList, evUser)` unconditionally.
If the free-list was not initialized (NULL) — which can happen when the
event subsystem tears down in an unusual order or when `db_init_events`
was never called — this is a null-pointer dereference inside the free
list library.

## Symptoms
Crash (null dereference) or silent memory corruption during IOC shutdown
in the event task exit path. Most commonly triggered when shutdown is
initiated before the event subsystem is fully initialized, or when the
free-list is inadvertently cleared during teardown.

## Fix
Added a null check before `freeListFree`: if `dbevEventUserFreeList` is
NULL, print a diagnostic to stderr and skip the free. This prevents the
crash and provides a diagnostic trace.

## Rust Applicability
In Rust the allocator is always available and free-list pools are
replaced by standard heap allocation. The null-free-list hazard does not
exist. If epics-base-rs uses a `slab` or custom pool allocator for event
user objects, verify that pool teardown is ordered after all users have
returned their allocations. Likely eliminated in practice.

## Audit Recommendation
No direct audit needed. If base-rs uses an explicit object pool for
event contexts, verify the pool is destroyed last in the shutdown
sequence. Otherwise this is fully eliminated.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:event_task` — added null guard before `freeListFree(dbevEventUserFreeList, evUser)`
