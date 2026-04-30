---
sha: a4bc0db6e61162d929b9f4c9f6137c95171ca8ae
short_sha: a4bc0db
date: 2024-12-27
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: connection_callback
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: event_callback
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: access_rights_callback
tags: [dbCa, CP-link, PUTF, RPRO, scan-once, race]
---
# dbCa: CP link updates must set PUTF/RPRO via dbCaTask, not scanOnce callback

## Root Cause
CA input links with the `CP` or `CPP` attribute trigger record processing when
the upstream CA value changes. The old code used `scanOnceCallback(prec,
scanComplete, pca)` — a "scan once" mechanism that queued the record for
processing via a separate completion callback. This `scanComplete` callback ran
in a callback thread and incremented/decremented a `scanningOnce` counter to
coalesce rapid updates.

The bug: `scanOnceCallback` did NOT set `prec->putf = TRUE` before calling
`dbProcess`, so the record processed without the `PUTF` flag set. `PUTF`
indicates "this process was triggered by a CA PUT (or CP link)"; records that
check `pact` + `putf` for correct `RPRO` handling (reprocess-on-active) could
behave incorrectly. Additionally, the `scanningOnce` counter and its
`scanComplete` callback created a race: the callback held a reference to `pca`
(incrementing its refcount) but the `scanningOnce` drain logic was complex and
could leave the counter stuck if `dbCaRemoveLink` raced with a queued callback.

## Symptoms
Records linked via `CP`/`CPP` did not have `PUTF` set during processing, which
could cause:
- Out-of-phase `RPRO` (reprocess) behavior on asynchronous records.
- Incorrect "already-active" handling for records that check `prec->putf`.
Reference-count imbalance if `dbCaRemoveLink` raced with `scanComplete`.

## Fix
Remove `scanLinkOnce`/`scanComplete` entirely. Instead, set a new action flag
`CA_DBPROCESS` in the `link_action` bitmask from within the `eventCallback`,
`connectionCallback`, and `accessRightsCallback` (all hold `pca->lock`). The
`dbCaTask` main loop processes `CA_DBPROCESS` after unlocking by calling the
new `db_process(prec)` helper, which correctly sets `prec->putf = TRUE` (or
`prec->rpro = TRUE` if already active) before calling `dbProcess`.

## Rust Applicability
In `base-rs`, CA input CP links feed record processing via an async task that
calls the Rust equivalent of `dbProcess`. The `putf`/`rpro` field semantics
must be preserved: the CP-link processing path must set `putf=true` before
calling `db_process`, and if `pact` is set, it must set `rpro=true` instead.
The scan-once coalescing pattern should be replaced by tokio channel coalescing
(e.g., `watch::channel` or a bounded mpsc with try_send discard), not a
separate callback with refcount management.

## Audit Recommendation
In `base-rs/src/server/database/db_ca.rs`:
1. Verify that `connection_callback`, `event_callback`, and
   `access_rights_callback` for CP links do NOT directly call `db_process` from
   the callback context — they must post an action to the `dbCaTask` equivalent.
2. Verify `db_process` (or its Rust equivalent) sets `putf=true` before
   `db_process(prec)` and sets `rpro=true` if `pact` is already set.
3. Verify no separate "scan once" queue with external refcount tracking exists.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:connectionCallback` — replace `scanLinkOnce` with `link_action |= CA_DBPROCESS`
- `modules/database/src/ioc/db/dbCa.c:eventCallback` — replace `scanLinkOnce` with `addAction(pca, CA_DBPROCESS)`
- `modules/database/src/ioc/db/dbCa.c:accessRightsCallback` — replace `scanLinkOnce` with `addAction(pca, CA_DBPROCESS)`
- `modules/database/src/ioc/db/dbCa.c:dbCaTask` — add `CA_DBPROCESS` handling, call `db_process(prec)`
- `modules/database/src/ioc/db/db_access.c:db_process` — new helper: set `putf`/`rpro` then call `dbProcess`
- `modules/database/src/ioc/db/dbCaPvt.h` — add `CA_DBPROCESS 0x2000` flag constant
