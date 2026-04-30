---
sha: e860617389c14eb297742e1fad6e4f0dcf659194
short_sha: e860617
date: 2019-01-27
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dbDbLink.rs
    function: process_target
tags: [RPRO, PUTF, procThread, self-link, async-record, lifecycle]
---
# dbDbLink processTarget: add procThread ownership to fix RPRO/PUTF regression

## Root Cause
The `RPRO` (re-process) flag suppression relied on checking whether the
destination record was "being processed recursively by us" using `pdst->pact`.
But when a source record (psrc) with `putf=1` linked to a destination record
(pdst) that was already active (`pact=1`) and had been claimed by the current
thread (i.e., processing was recursive), the old code still set `rpro=TRUE`
because it only checked `psrc->putf` without verifying whether *we* were the
ones doing the processing.

The original check `else if (psrc->putf)` (without `procThread!=self`) would
set `rpro` even for recursive calls from the same thread — causing the record
to queue another async completion when one was already in progress, leading
to double-processing or infinite loops on async records.

## Symptoms
- Chain 4 test (`chain4_pos`, `chain4_rel`, `chain4_lim`): async record
  re-processed an extra time after the initial async completion, resulting in
  counts of 2 instead of 1. The test was previously marked `testTodoBegin("Bug")`.
- Potential infinite reprocessing on records with self-referential or circular
  async link chains with `putf=1`.

## Fix
Add `procThread` field to `dbCommonPvt` (per-record private data). In
`processTarget`:
1. Before processing: record whether each of src/dst had no claimed thread
   (`srcset`/`dstset`). Set `procThread = epicsThreadGetIdSelf()` for newly
   claimed records.
2. Change the RPRO condition to `else if (psrc->putf && dbRec2Pvt(pdst)->procThread!=self)`:
   only set `rpro` if the destination was NOT already being processed by us.
3. After `dbProcess(pdst)`: assert both procThread fields still equal self,
   then clear the newly-claimed ones back to NULL.

## Rust Applicability
Applies. In base-rs `process_target`, the RPRO/PUTF suppression logic requires
a per-record "current processing thread" marker. Verify:
1. That a Rust equivalent (e.g., an `Option<ThreadId>` in record private state)
   exists or is planned.
2. That the RPRO flag is not set when the destination is already owned by the
   current task/thread.
3. That the marker is cleared after processing completes, not left set.

## Audit Recommendation
In `base-rs/src/server/database/dbDbLink.rs::process_target`:
- Check that the RPRO equivalent has a `current_processor != self` guard.
- If the record processing model uses tokio tasks (not OS threads), the
  "current processor" comparison needs to use a task-local ID or a per-record
  `Arc<Mutex<Option<TaskId>>>` rather than `epicsThreadGetIdSelf()`.

## C Locations
- `modules/database/src/ioc/db/dbCommonPvt.h:dbCommonPvt` — added `procThread` field
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — procThread claim/release + RPRO guard
- `modules/database/test/std/rec/asyncproctest.c` — removed `testTodoBegin("Bug")` for chain4
