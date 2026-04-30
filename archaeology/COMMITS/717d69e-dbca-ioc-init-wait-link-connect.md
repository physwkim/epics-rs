---
sha: 717d69e1fcf3cb8dc4533ba53bdae838a396d274
short_sha: 717d69e
date: 2025-09-20
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/db_ca.rs
    function: db_ca_run
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: db_ca_add_link_callback_opt
tags: [iocInit, CA-link, connect-ordering, PINI, atomic-counter]
---
# dbCa: iocInit must wait for local CA links to connect before PINI

## Root Cause
`dbCaRun()` was called during `iocInit` to start the CA link worker but
immediately returned without waiting for local CA link channels to connect.
Consequently, `initHookAfterIocRunning` fired and `PINI` records processed
before their CA input links had completed the connection + subscribe handshake.
Records reading from CA-linked PVs got undefined/UDF values at startup.

## Symptoms
- Records with `field(PINI, RUNNING)` and a CA input link read stale or UDF
  values on first process during `iocInit`.
- The race was timing-dependent: on fast hosts or with few local links, the
  links might connect before `PINI` processing by coincidence.

## Fix
Added an atomic counter `initOutstanding` (one count per link with the new
`DBCA_CALLBACK_INIT_WAIT` flag). `dbCaAddLinkCallbackOpt()` increments the
counter when the flag is set. In the CA worker's event loop, after a link
completes its full initialization handshake (connection + optional monitor
subscription + optional attribute fetch), the worker decrements the counter;
when it reaches zero it signals `startStopEvent`. `dbCaRun()` then loops
waiting on `startStopEvent` until `initOutstanding == 0` before returning to
the `iocInit` sequence.

Three deferred `CA_INIT_READY` paths ensure "ready" is declared only after all
needed data arrives:
- If native/string monitors are requested: deferred to `eventCallback`.
- If attribute fetch is needed: deferred to `getAttribEventCallback`.
- Otherwise: declared in `connectionCallback`.

## Rust Applicability
In `ca-rs` / `base-rs`, the equivalent of `dbCaRun()` (the step that moves the
IOC from PAUSE to RUNNING state) must await all local CA link connections before
returning. The idiomatic Rust approach is an `AtomicUsize` counter + a
`tokio::sync::Notify` (or `broadcast`). Increment on link creation, decrement
when the link completes the full subscribe/attribute cycle, and `notify.await`
in the `iocInit` future. This is directly analogous to the C fix.

## Audit Recommendation
In `base-rs/src/server/database/db_ca.rs::db_ca_run` (or the async
`ioc_init` state machine): confirm there is a barrier that awaits all local CA
link connections before advancing to the `AfterIocRunning` hook. The barrier
must be deferred until after subscribe AND attribute fetch complete, not just
after the TCP connection is established. Check that `PINI` record processing
is triggered only after this barrier clears.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaRun` — added `while(initOutstanding) epicsEventMustWait(startStopEvent)` loop
- `modules/database/src/ioc/db/dbCa.c:dbCaAddLinkCallbackOpt` — increments `initOutstanding` when `DBCA_CALLBACK_INIT_WAIT` flag set
- `modules/database/src/ioc/db/dbCa.c:connectionCallback` — sets `CA_INIT_READY` or defers it
- `modules/database/src/ioc/db/dbCa.c:eventCallback` — sets `CA_INIT_READY` after first monitor event
- `modules/database/src/ioc/db/dbCa.c:getAttribEventCallback` — sets `CA_INIT_READY` after attribute fetch
- `modules/database/src/ioc/db/dbCaPvt.h` — added `DBCA_CALLBACK_INIT_WAIT` flag and `CA_INIT_READY` action bit
