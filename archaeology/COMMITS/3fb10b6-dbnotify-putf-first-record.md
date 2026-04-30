---
sha: 3fb10b6d59f3434b86858bbc9d829aaf1fbfbfb7
short_sha: 3fb10b6
date: 2018-12-29
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_notify.rs
    function: process_notify_common
tags: [putf, dbNotify, callback-lifecycle, record-processing, flag-propagation]
---
# dbNotify must set PUTF on the first-record call only

## Root Cause
`processNotifyCommon()` was called both for the initial record invocation
(from `dbProcessNotify`) and for restart callbacks (from `notifyCallback`).
The `PUTF` (Put Field) flag signals that the record processing was triggered
by a CA/PVA put operation.  This flag was never set on the record, so
downstream records in a notify chain did not propagate the put-cause correctly.

The secondary issue was `epicsEventWait` (which can silently swallow errors)
was used in the test helper thread instead of `epicsEventMustWait`.

## Symptoms
Records linked via `dbNotify` (processNotify) did not have `PUTF` set,
causing `RPRO` (reprocess) logic to be skipped and monitor events to omit
the DBE_VALUE flag that depends on put-cause tracking.  The companion commit
`e9189947` added a regression test that marked this as `testTodo("Bug")`.

## Fix
Added a `first` parameter to `processNotifyCommon()`.
- When `first=1` (called from `dbProcessNotify`, i.e. the initial put) the
  function sets `precord->putf = TRUE` before scheduling processing.
- When `first=0` (called from `notifyCallback` for restart) PUTF is not
  re-set, preserving the original intent.
- Changed `epicsEventWait` → `epicsEventMustWait` in `tpnThread` to surface
  wait failures instead of silently continuing.

## Rust Applicability
`base-rs` will need equivalent logic when implementing `dbProcessNotify` /
async record processing chains.  The Rust analog is a future or task that
tracks whether the chain was initiated by a put-request; that boolean must
be threaded through from the initiating call only, not on restarts.

## Audit Recommendation
When implementing `db_notify.rs` `process_notify_common`, ensure the
"initiated by put" flag (equivalent to `putf`) is only set on the initial
invocation.  Do not re-set it on recursive/restart re-entries.

## C Locations
- `modules/database/src/ioc/db/dbNotify.c:processNotifyCommon` — added `first` param; sets `putf` when `first`
- `modules/database/src/ioc/db/dbNotify.c:dbProcessNotify` — passes `first=1`
- `modules/database/src/ioc/db/dbNotify.c:notifyCallback` — passes `first=0` for restarts
- `modules/database/src/ioc/db/dbNotify.c:tpnThread` — changed to `epicsEventMustWait`
