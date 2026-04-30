---
sha: 3b484f58d3f12fa0fb9d15fc09f0f654569f97c8
short_sha: 3b484f5
date: 2023-03-06
author: Michael Davidsaver
category: other
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/const_link.rs
    function: load_scalar
  - crate: base-rs
    file: src/server/database/const_link.rs
    function: load_array
tags: [link, empty-string, semantic, const-link, validation]
---
# dbConstLink: treat empty string same as unset link

## Root Cause
In `dbConstLink.c`, the functions `dbConstLoadScalar()`, `dbConstLoadLS()`,
and `dbConstLoadArray()` checked `if (!pstr)` to detect an unset constant
link string. However, a link value of `""` (empty string) was syntactically
valid but semantically meaningless — JSON parsing of `""` would fail or return
no data, and scalar conversion of `""` also fails. The caller would receive
`S_db_badField` for NULL but would silently proceed (or get a parse error)
for `""`, producing inconsistent behavior.

## Symptoms
- A record with `field(INPA, "")` behaves differently from one with no INPA
  field at all, even though both should mean "no constant link".
- Subtle misconfiguration in .db files can silently produce zero/default values
  rather than a clear error.

## Fix
Changed the guard from `if (!pstr)` to `if (!pstr || !pstr[0])` in all three
functions, so that an empty-string constant link is treated identically to an
unset (NULL) link and returns `S_db_badField` immediately.

## Rust Applicability
A Rust const-link implementation would parse the link string during
initialization. The equivalent validation is checking `str.is_empty()` before
attempting JSON/scalar parse. The Rust implementation should explicitly reject
empty strings at link-parse time and return an `Err` rather than allowing the
parse to silently produce a default value.

## Audit Recommendation
In `base-rs` const-link parsing: confirm that `""` as a link value is
rejected early with an appropriate error, not silently treated as a zero or
default. Look for `str.parse::<f64>()` or `serde_json::from_str` called
without an upfront `is_empty()` guard.

## C Locations
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadScalar` — `!pstr || !pstr[0]` guard
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadLS` — same guard
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadArray` — same guard
