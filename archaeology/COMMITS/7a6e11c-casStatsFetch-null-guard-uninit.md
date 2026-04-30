---
sha: 7a6e11cae0f46cdb4da148c278aed3a6daeadb08
short_sha: 7a6e11c
date: 2025-02-06
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/server/stats.rs
    function: cas_stats_fetch
  - crate: ca-rs
    file: src/server/camessage.rs
    function: cas_client_initiating_current_thread
tags: [rsrv, initialization, null-deref, stats, lifecycle]
---
# RSRV: guard casStatsFetch and casClientInitiatingCurrentThread against uninitialized state

## Root Cause
Two RSRV (CA record server) functions accessed global state that is only
initialized when the CA server starts:

1. `casStatsFetch` called `LOCK_CLIENTQ` (which dereferences `clientQlock`)
   without checking if `clientQlock` was non-NULL. If called before
   `caServerInit` (e.g., from another IOC module's startup hook or from a
   test that disables the DB server via `dbServer`), it dereferenced a NULL
   mutex pointer → crash.

2. `casClientInitiatingCurrentThread` called
   `epicsThreadPrivateGet(rsrvCurrentClient)` without checking if
   `rsrvCurrentClient` was non-NULL. Same condition → crash.

## Symptoms
Crash (NULL dereference) in `casStatsFetch` or `casClientInitiatingCurrentThread`
when called before RSRV was initialized, or when RSRV was disabled via
`dbServer` configuration. This could be triggered by third-party modules that
called `casStatsFetch` at iocInit hook time before `rsrv_init` had run.

## Fix
- `casStatsFetch`: add `if(!clientQlock)` early-return that zeroes the output
  counters and returns immediately.
- `casClientInitiatingCurrentThread`: add `if(!rsrvCurrentClient)` early-return
  that returns `RSRV_ERROR`.

## Rust Applicability
In `ca-rs`, the CA server stats and per-client thread-local are managed via
`Arc<ServerState>` that is `None`/`Some` depending on whether the server has
started. Any stats query path must check for `None` state. In Rust this maps
naturally to `Option<Arc<ServerState>>` — but a `stats_fetch` that panics on
`unwrap()` when the server hasn't started is the equivalent bug.

## Audit Recommendation
In `ca-rs/src/server/stats.rs::cas_stats_fetch` (or the equivalent Rust stats
function), verify that calling it before server initialization returns zero
counts, not panics or undefined behavior. In
`ca-rs/src/server/camessage.rs::cas_client_initiating_current_thread`, verify
the thread-local is checked for `None` before use.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:casStatsFetch` — add `if(!clientQlock)` null guard
- `modules/database/src/ioc/rsrv/camsgtask.c:casClientInitiatingCurrentThread` — add `if(!rsrvCurrentClient)` null guard
