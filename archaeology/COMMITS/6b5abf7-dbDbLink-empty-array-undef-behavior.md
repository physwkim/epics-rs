---
sha: 6b5abf76c854937c5442cf22f4453306094a66d1
short_sha: 6b5abf7
date: 2020-06-01
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_db_link.rs
    function: db_db_get_value
tags: [empty-array, undefined-behavior, dbDbLink, filter, bounds]
---
# dbDbLink: remove early error return that blocked empty array reads

## Root Cause
`dbDbGetValue` in `dbDbLink.c` had a guard `if (pnRequest && *pnRequest <= 0) return S_db_badField;`
that fired after running channel filters. When an `arr` filter legitimately
reduced an array to zero elements (e.g., a backward range or an out-of-bounds
start index), `*pnRequest` became 0 and the function returned `S_db_badField`
as if it were an error. This caused forward-link processing to fail for waveform
records whose filtered read legitimately produced an empty array.

The underlying issue is that an empty-array result (0 elements) from a filter
is valid and defined; the guard was treating it as undefined behavior, but the
real undefined behavior was in how callers of `dbDbGetValue` handled a `*pnRequest == 0`
result *without* the guard — which was already handled safely elsewhere.

## Symptoms
Waveform (`wf`) records receiving data via a db-link with an `arr` filter failed
with `S_db_badField` when the filter produced zero elements (backward range,
start beyond NORD, etc.) instead of succeeding with an empty array. The `ai`
scalar record on the same link correctly returned a failure (as documented),
but the waveform should succeed with an empty result.

## Fix
Remove the `if (pnRequest && *pnRequest <= 0) return S_db_badField;` check
after filter execution. Empty array (zero elements) is now passed through as a
valid result. Updated tests to verify `wf` succeeds (not fails) for the empty
cases and `ai` continues to fail them.

## Rust Applicability
In `base-rs` the equivalent of `dbDbGetValue` is the db-link read path. Any
Rust implementation that maps C `nRequest <= 0` to an error must be audited:
returning `Err` for zero-element reads blocks legitimate empty-array monitor
updates through filtered links.

## Audit Recommendation
In `base-rs/src/server/database/db_db_link.rs::db_db_get_value` (or the
equivalent Rust link-read function), verify that a zero-element count after
filter evaluation is returned as `Ok(0)`, not as an error. Add a unit test for
the backward-range filter path that asserts `Ok` with empty slice.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — remove `if (*pnRequest <= 0) return S_db_badField` guard after filter run
