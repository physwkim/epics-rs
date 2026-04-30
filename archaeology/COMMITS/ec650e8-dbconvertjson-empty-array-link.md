---
sha: ec650e8c3faaa78cdf6077249eb5cad48dbfa6a8
short_sha: ec650e8
date: 2022-07-26
author: Dirk Zimoch
category: bounds
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_convert_json.rs
    function: db_put_convert_json
tags: [dbConvertJSON, empty-array, constant-link, JSON-parse, zero-length]
---
# dbPutConvertJSON: empty JSON string not handled, passed to yajl causing parse error

## Root Cause
`dbPutConvertJSON()` did not check for an empty input JSON string (`jlen == 0`) before calling `yajl_alloc` and `yajl_parse`. Passing an empty buffer to yajl causes it to return a parse error (expecting a valid JSON value), which propagated as `S_db_badField`. An empty string is a valid representation of an empty array input link (0 elements) and should silently succeed with `*pnRequest = 0`.

## Symptoms
Array input links configured with an empty JSON string (e.g., `field(INPA, "")` via dbConstLink) returned `S_db_badField` during IOC startup, failing link initialization for records expecting zero-element input.

## Fix
Add an early return at the top of `dbPutConvertJSON`: `if (!jlen) { *pnRequest = 0; return 0; }`. Commit `ec650e8`.

## Rust Applicability
In base-rs, if a JSON-to-DBR converter is implemented, it must handle the empty string case explicitly: return `Ok(0)` elements immediately without passing the empty buffer to the parser. Rust JSON parsers (serde_json, etc.) also error on empty input.

## Audit Recommendation
In `base-rs/src/server/database/db_convert_json.rs` (or equivalent), verify that `db_put_convert_json("")` returns `Ok(0)` and does not attempt to parse an empty buffer.

## C Locations
- `modules/database/src/ioc/db/dbConvertJSON.c:dbPutConvertJSON` — missing `jlen == 0` early return
