---
sha: b1f445925d8de11bca94e7d0210a40af8916bcd9
short_sha: b1f4459
date: 2020-02-11
author: Dirk Zimoch
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_elements
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_dbf_type
tags: [dbChannel, DBADDR, filter, link-storage, field-type]
---

# DB links stored DBADDR instead of dbChannel, bypassing filter metadata

## Root Cause
DB link internals stored a raw `DBADDR*` (a flat address struct) rather
than a `dbChannel*` (which carries the channel's installed filter list and
final element count after filters). As a result, `dbDbGetDBFtype` and
`dbDbGetElements` queried the raw field type and element count from
`DBADDR`, ignoring any filter plugin transformations that change the
effective type or count on the channel. Reads via `dbDbGetValue` also
fetched from `paddr` rather than through the channel abstraction, meaning
filters were never consulted during link initialization or type queries.

## Symptoms
A DB link pointing to a filtered channel (e.g., an array-slice filter
that changes `no_elements`) would report the pre-filter element count and
field type to record support, causing incorrect buffer allocation and
conversion decisions. Filter effects were silently ignored for all
link-metadata queries.

## Fix
Stored `dbChannel*` (opened via `dbChannelCreate` + `dbChannelOpen`) in
`plink->value.pv_link.pvt` instead of a malloc'd `DBADDR`. All link
methods (`getDBFtype`, `getElements`, `getValue`, `removeLink`) were
updated to work through `dbChannel` accessors. The channel is freed via
`dbChannelDelete` on link removal.

## Rust Applicability
In base-rs `links.rs`, the internal link representation must hold the
channel object (with its filter state) rather than a raw address/field
descriptor. If `db_db_get_elements` or `db_db_get_dbf_type` returns
values from a cached raw address rather than querying the live channel,
filter-modified counts/types will be invisible to record support, causing
the same buffer mismatch bugs.

## Audit Recommendation
In `src/server/database/links.rs`: find `get_elements` / `get_dbf_type`
for DB link types. Verify they call `channel.final_elements()` /
`channel.final_field_type()` (post-filter) rather than reading from a
cached raw field descriptor. Also verify the link cleanup path calls the
channel destructor / close equivalent.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbInitLink` — stores `dbChannel*` instead of `DBADDR*`
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetDBFtype` — uses `dbChannelFinalFieldType` instead of `paddr->field_type`
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetElements` — uses `dbChannelFinalElements` instead of `paddr->no_elements`
- `modules/database/src/ioc/db/dbDbLink.c:dbDbRemoveLink` — calls `dbChannelDelete` instead of `free`
- `modules/database/src/ioc/db/dbAccess.c:dbPutFieldLink` — creates channel via `dbChannelCreate/Open`, transfers ownership to link
