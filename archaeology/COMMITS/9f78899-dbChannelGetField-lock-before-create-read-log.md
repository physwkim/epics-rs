---
sha: 9f788996dcb8eb4eea6d36831a79e8d2edf29638
short_sha: 9f78899
date: 2023-02-23
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/channel.rs
    function: db_channel_get_field
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: dbChannel_get_count
tags: [db_create_read_log, dbScanLock, race, filter, field-log]
---
# db: acquire record lock before db_create_read_log and dbChannelGetField

## Root Cause
Since commit `27fe3e4` `db_create_read_log()` began accessing record fields
(specifically array size/type metadata) to initialize the `db_field_log`
structure. However, callers in `dbChannelGetField` and `dbChannel_get_count`
(and their callers in `read_reply`/`read_action` in `camessage.c`) called
`db_create_read_log` *before* acquiring the record's `dbScanLock`. This created
a race: the record could be concurrently modified by a scan thread while the
field log was being initialized, corrupting the log's `type`, `no_elements`, or
array pointer.

Additionally, callers in `testdbVGetFieldEqual` and `testdbGetArrFieldEqual`
duplicated the filter setup boilerplate (create log, run pre/post chains) that
`dbChannelGetField` should handle internally.

## Symptoms
Under concurrent scan load: corrupted `db_field_log` leading to incorrect array
element count or type in the filter chain, producing wrong values or crashes in
the arr/sync filters. Silent data corruption in CA GET responses with filters.

## Fix
Move `db_create_read_log` + `dbChannelRunPreChain/PostChain` into
`dbChannelGetField` and `dbChannel_get_count` themselves, *inside* the
`dbScanLock/dbScanUnlock` region. Remove the duplicate call sequences from
`read_reply`, `read_action` (camessage.c), and the test helpers. Also update
`dbChArrTest.cpp` to call `dbChannelGet` (locked externally) instead of
`dbChannelGetField` (which now handles locking internally).

## Rust Applicability
In `base-rs`, the channel-get path must hold the record's scan lock for the
entire duration of `db_create_read_log` + filter chain execution + field read.
If `db_channel_get_field` or `dbChannel_get_count` is implemented as an async
fn that yields between lock acquisition and field-log creation, a race window
exists. Ensure the lock is held continuously across: filter log creation, pre
chain, post chain, and the actual `dbChannelGet`.

## Audit Recommendation
In `base-rs/src/server/database/channel.rs::db_channel_get_field` and
`base-rs/src/server/database/db_access.rs::dbChannel_get_count`, verify that:
1. `db_create_read_log` is called with the record scan lock already held.
2. No `await` point exists between lock acquisition and `db_delete_field_log`.
3. `read_reply`/`read_action` in `ca-rs/src/server/` do NOT duplicate the
   filter setup — they should pass `None` for `pfl` and let the inner function
   handle it.

## C Locations
- `modules/database/src/ioc/db/dbChannel.c:dbChannelGetField` — moved filter setup inside `dbScanLock`
- `modules/database/src/ioc/db/db_access.c:dbChannel_get_count` — moved filter setup inside `dbScanLock`
- `modules/database/src/ioc/rsrv/camessage.c:read_reply` — removed duplicate filter setup
- `modules/database/src/ioc/rsrv/camessage.c:read_action` — removed duplicate filter setup
- `modules/database/test/ioc/db/dbChArrTest.cpp` — wrap `db_create_read_log` calls in `dbScanLock`
