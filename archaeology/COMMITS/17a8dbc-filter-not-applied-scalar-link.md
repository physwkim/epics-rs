---
sha: 17a8dbc2d7a8fd8673ed7cc3bc044cfeef25951e
short_sha: 17a8dbc
date: 2020-02-12
author: Dirk Zimoch
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [filter, dbDbLink, data-flow, link-read, filter-bypass]
---

# Filters not applied when reading via DB link (dbDbGetValue)

## Root Cause
`dbDbGetValue` applied filters only for scalar fast-path reads, but the
filter block was absent from the general array-read path. When a channel
had filters installed, the code fell through to the unfiltered scalar or
array conversion without running `dbChannelRunPreChain` /
`dbChannelRunPostChain`. This meant channel-level filters (e.g., array
slicing, deadband, timestamp filters) were silently ignored for DB link
reads.

## Symptoms
DB link `INP`/`OUT` fields that referenced channels with filter
specifications would return unfiltered data. For example, a link with
`{arr: {s:5, e:10}}` (array slice filter) would return the full array
instead of the requested slice.

## Fix
Added a filter-chain execution block inside `dbDbGetValue` that runs
before the scalar fast-path check: when `ellCount(&chan->filters) > 0`,
allocate a read log, run pre/post chain, call `dbChannelGet` with the
log, then free it. The scalar fast-path and direct-conversion paths are
reached only when no filters are installed.

## Rust Applicability
In base-rs `links.rs`, the DB link read path (`db_db_get_value`) must
check for filters before choosing between fast-path scalar conversion and
full channel get. If filter chain execution is omitted when filters are
installed, the Rust implementation silently bypasses user-configured
channel-level filters. Check the read path branching logic.

## Audit Recommendation
In `src/server/database/links.rs`: find the `db_db_get_value` or
equivalent function. Verify that when `channel.filters().is_empty()` is
false, the code invokes `run_pre_chain` / `run_post_chain` before the
final value conversion, and does NOT fall through to the scalar fast path.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — added filter-chain execution block before scalar fast path
