---
sha: 9048aa34386bc3d7c9b4e42acfdf5b5ace951782
short_sha: 9048aa3
date: 2022-06-12
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [union, dtor, field-log, C-union-alias, type-punning]
---
# db_field_log dtor accessed via wrong union member, causing UB

## Root Cause
`db_field_log` held the destructor pointer `dtor` inside the `dbfl_ref`
sub-struct, which is a member of a C union (`u.r.dtor`). However, `dtor`
semantically belongs to the whole `db_field_log` entry regardless of whether
the log is type `dbfl_type_val` or `dbfl_type_ref`. Accessing `u.r.dtor`
when the active union member was `dbfl_val` is undefined behavior (C
type-punning via inactive union member). Additionally, `dbfl_has_copy` checked
`u.r.dtor` to decide ownership, but if the type was `dbfl_type_val` the field
was not defined in that branch.

## Symptoms
Filters (arr, ts) and the dbEvent engine read or write `pfl->u.r.dtor` in
code paths that may be active for either union variant. If a `dbfl_type_val`
log was live and `u.r.dtor` happened to be read, the function pointer value
was garbage, potentially calling into random memory.

## Fix
Move `dtor` out of `dbfl_ref` and promote it to a top-level field of
`db_field_log`, placed before the union. All access sites
(`dbEvent.c`, `arr.c`, `ts.c`, test files) updated to use `pfl->dtor`
directly. The `dbfl_has_copy` macro likewise updated.

## Rust Applicability
Rust enums replace C unions and their members are strictly typed — there is no
union-member aliasing. A Rust `FieldLog` would be an `enum { Val(...), Ref{pvt, field, dtor: Option<Box<dyn FnOnce>>} }` where `dtor` can only be
accessed on the `Ref` variant. This C bug is structurally eliminated by the
type system.

If base-rs mirrors `db_field_log` as a struct with a variant enum, ensure
the ownership/destructor callback is placed outside the variant (e.g., as a
top-level `Option<Box<dyn FnOnce(&mut FieldLog)>>` field), matching the
corrected C layout.

## Audit Recommendation
No direct Rust bug to audit, but review base-rs `FieldLog` or equivalent
struct to confirm the drop/dtor logic is on the outer struct, not inside an
inner enum variant where it might be inaccessible.

## C Locations
- `modules/database/src/ioc/db/db_field_log.h:db_field_log` — dtor promoted from `dbfl_ref.dtor` to top-level field
- `modules/database/src/ioc/db/dbEvent.c:db_create_field_log` — updated to `pLog->dtor`
- `modules/database/src/ioc/db/dbEvent.c:db_delete_field_log` — updated to `pfl->dtor`
- `modules/database/src/std/filters/arr.c:filter` — updated all dtor accesses
- `modules/database/src/std/filters/ts.c:filter` — updated dtor access
