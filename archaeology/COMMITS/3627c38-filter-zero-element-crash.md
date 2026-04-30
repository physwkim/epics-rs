---
sha: 3627c38a575dc7f08a93ae7879da2b4c5def0e29
short_sha: 3627c38
date: 2020-02-12
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [filter-chain, empty-array, crash, bounds, dbDbLink]
---

# Crash when filter result reduces array to 0 elements in dbDbGetValue

## Root Cause
`dbDbGetValue` ran the filter pre/post chain on array channels without
first checking whether the channel had any elements. When
`dbChannelFinalElements(chan) < 1`, the code entered the filter block,
allocated a read log, and ran chains that operated on a zero-element
array — leading to undefined behavior or crash. The filter block was
also incorrectly placed inside the `else` branch of the fast scalar
conversion check, meaning it was never reached for array channels.

## Symptoms
IOC crash (segfault or assertion) when a DB link reads from a channel
that has filter plugins installed but zero final elements after filter
evaluation.

## Fix
Moved the filter-chain block to execute first (before the scalar fast
path), added an element-count guard returning `S_db_badField` and
setting LINK alarm when `dbChannelFinalElements(chan) < 1`. Also added
warning logging in `dbDbInitLink` / `dbDbAddLink` when the target channel
has zero elements at link initialization time.

## Rust Applicability
In base-rs `links.rs`, the filter-chain invocation for DB links must be
guarded by a `final_elements > 0` check before any buffer or log
allocation. If filter processing is attempted on a zero-element channel,
a panic or empty-slice UB can occur. The Rust fix pattern is: early
return `Err(DbError::EmptyArray)` and set LINK alarm before running
filter chains.

## Audit Recommendation
In `src/server/database/links.rs`: locate the filter-chain invocation in
the read path. Verify it is guarded with `channel.final_elements() > 0`
before `run_pre_chain` / `run_post_chain`. Also check that when the guard
fires, a LINK alarm is set on the record.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — moved filter block before scalar fast path, added element-count guard
- `modules/database/src/ioc/db/dbDbLink.c:dbDbInitLink` — added warning for zero-element target at init
- `modules/database/src/ioc/db/dbDbLink.c:dbDbAddLink` — added warning for zero-element target at init
