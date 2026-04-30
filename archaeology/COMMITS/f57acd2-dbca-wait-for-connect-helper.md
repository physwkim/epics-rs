---
sha: f57acd2c106940533ec665ba26e06fa118f08808
short_sha: f57acd2
date: 2021-11-05
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: testdb_ca_wait_for_connect
tags: [CA-link, connection-wait, test-sync, dbCa, lifecycle]
---

# Add testdbCaWaitForConnect() for CA link connection synchronization

## Root Cause
The existing test helper `testdbCaWaitForUpdateCount()` waited for a data
update counter to reach a target, but had no way to wait for the initial CA
connection. CA link connection follows a two-phase path:

1. `connectionCallback()` fires when the channel connects.
2. For non-string types, `CA_GET_ATTRIBUTES` is queued and the attributes
   callback fires later.

The original `testdbCaWaitForUpdateCount()` only hooked `pca->monitor`
(data update callback), not `pca->connect` (connection callback). As a result,
tests waiting for a CA link to be ready had no reliable synchronization point
for the connection event itself — they could proceed before the link was
connected, leading to intermittent test failures.

Additionally, `testdbCaWaitForUpdateCount()` did not check `pca->isConnected`,
so it would loop trying to count updates even if the channel was not yet
connected, causing hangs.

## Symptoms
- Test race: test proceeds before CA link is connected, reads uninitialized data.
- `testdbCaWaitForUpdateCount()` hanging on first call if channel not yet connected.
- No way to wait specifically for connection (as opposed to first data update).

## Fix
- Refactored shared logic into internal `testdbCaWaitForEvent(plink, cnt, event)`.
- Added `testEventConnect` enum value alongside `testEventCount`.
- In the wait loop, check `!pca->isConnected` as the stop condition for connect
  events, and `nUpdate < cnt` for count events.
- Hook both `pca->connect` and `pca->monitor` with the same callback so either
  event wakes the waiter.
- In `connectionCallback()`: for DBR_STRING (no attributes phase), immediately
  invoke `pca->connect` callback after connection; for other types, defer until
  after `getAttribEventCallback`.

## Rust Applicability
In `base-rs`, CA link tests should have analogous async synchronization:
- An `mpsc` or `watch` channel that fires on CA link connection.
- A separate channel/counter for data updates.
- Test helpers should await `connect_rx.recv()` before asserting data.

## Audit Recommendation
- In `base-rs/src/server/database/db_ca.rs`: verify that connection and first
  data update notifications are separate signals, allowing tests to await each
  independently.
- Check that `CaLink` exposes a `connected()` async method or notifier that
  fires when the CA channel first connects (not when data arrives).

## C Locations
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForConnect` — new public API
- `modules/database/src/ioc/db/dbCa.c:testdbCaWaitForEvent` — internal refactor
- `modules/database/src/ioc/db/dbCa.c:connectionCallback` — fires connect callback for DBR_STRING
- `modules/database/src/ioc/db/dbCa.h:testdbCaWaitForConnect` — declaration
