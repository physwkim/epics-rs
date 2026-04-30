---
sha: 552b2d17668add44f59f721ee6c1dc328c6067da
short_sha: 552b2d1
date: 2021-02-19
author: Michael Davidsaver
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/const_link.rs
    function: load_scalar
tags: [bounds-check, dbrtype, array-oob, link, const-link]
---
# dbConstAddLink: missing bounds check on dbrType before table lookup

## Root Cause
In `dbConstLoadScalar`, after handling the JSON path, the code falls through to:

```c
return convert[dbrType](pstr, pbuffer, NULL);
```

The `convert[]` table is indexed by `dbrType` (a `short`). No bounds check
was performed before indexing, so a caller with an invalid or out-of-range
`dbrType` value would access memory beyond the end of the `convert` array.
The companion fix in `6c914d19` addressed the same issue in a related path;
this commit adds the same guard to the scalar path in `dbConstLink.c`.

## Symptoms
Out-of-bounds read/function-pointer call when a DB record field is assigned
a constant link with an invalid `dbrType`. The consequence is a likely
segfault or UB (corrupted function pointer dereference), potentially
triggerable via a malformed `.db` file loaded into the IOC.

## Fix
Added a bounds check before the table lookup:

```c
if(dbrType >= NELEMENTS(convert))
    return S_db_badDbrtype;
```

This mirrors the guard added to `dbGet`/`dbPut` paths in the companion fix.

## Rust Applicability
Applies. In base-rs, the equivalent scalar conversion dispatches by field type.
Any match/dispatch on a field type value received from external input (parsed
DB file or CA wire) must ensure the type value is within the valid enum range
before using it as an index. Rust enums provide this safety by default if
field types are decoded via a `TryFrom` conversion, but if raw integer values
are used as indices into a slice, the same OOB bug is possible.

## Audit Recommendation
Audit `base-rs/src/server/database/const_link.rs` (or equivalent) — find all
places where a field-type integer is used to index a conversion table. Ensure
either (a) the type is validated as a valid enum variant before use, or (b) a
bounds check precedes the index. Also audit any CA server code in ca-rs that
accepts a `dbrType` from the network and dispatches on it.

## C Locations
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadScalar` — missing bounds check on dbrType before convert[] lookup
