---
sha: 3f382f6b68521c686b078aad6782fb0c693fbc09
short_sha: 3f382f6
date: 2025-10-17
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/ca_link.rs
    function: ioc_init_wait
tags: [dbCa, iocInit, CA-link, connect-sync, revert]
---
# Revert: dbCa iocInit wait for local CA links to connect

## Root Cause
Commit `a46bd5ae` (PR-713) added a mechanism for `iocInit` to block in
`dbCaLinkInitImpl` until all local CA links connected, using an atomic
counter `initOutstanding` decremented by a `CA_INIT_WAIT` action in the
dbCaTask event loop. This revert (`3f382f6`) removes that mechanism because
it caused deadlocks or hangs in certain IOC configurations where local CA
links did not connect quickly (e.g., when the server side was not yet
running at `iocInit` time, or circular CA links existed).

## Symptoms
The original PR-713 feature caused `iocInit` to hang indefinitely when:
- A local CA link target record was not yet in the database at link-init time.
- Circular CA link dependencies existed.
- The CA server took longer to accept connections than expected.

The revert restores the pre-PR-713 behavior: `dbCaLinkInitImpl` signals
`startStopEvent` as soon as the dbCaTask initializes the CA context, without
waiting for link connections.

## Fix
Remove:
- `initOutstanding` atomic counter and all its inc/dec paths.
- `CA_INIT_WAIT` link_action bit and the `CA_INIT_WAIT` event handling in
  `dbCaTask`.
- `DBCA_CALLBACK_INIT_WAIT` flag and `dbCaAddLinkCallbackOpt` extended API.
- The `dbInitLink` logic that set the flag for local CA links.

Restore `dbCaAddLinkCallback` to the simpler API and `dbCaAddLink` to call
it directly.

## Rust Applicability
`applies` — If base-rs or ca-rs implements an equivalent iocInit
synchronization for CA link readiness, this revert documents that a
blocking-wait approach is dangerous. The safe pattern is to signal
readiness after the CA context is initialized, not after all links connect.
Any `await`-ing on all CA links to connect during startup should be
time-bounded or non-blocking.

## Audit Recommendation
Audit `base-rs/src/server/database/ca_link.rs` for any future attempt to
block `iocInit` until CA links connect. If such logic exists, ensure it has
a timeout and does not create deadlock potential with circular links.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaLinkInitImpl` — changed from waiting on initOutstanding to signaling immediately after CA context init
- `modules/database/src/ioc/db/dbCa.c:eventCallback` — removed CA_INIT_WAIT decrement path
- `modules/database/src/ioc/db/dbLink.c:dbInitLink` — removed DBCA_CALLBACK_INIT_WAIT flag for local links
