---
sha: 85822f3051d2236144bb46dc2c24b7e38143e531
short_sha: 85822f3
date: 2020-04-01
author: Ben Franksen
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
tags: [db_field_log, data-ownership, scan-lock, race, subscription]
---
# db_field_log: missing abstraction for data-ownership check enables scan-lock races

## Root Cause
`db_field_log` can represent data in three states:
1. `dbfl_type_val`: scalar value stored inline in the struct (owned).
2. `dbfl_type_ref` with `u.r.dtor != NULL`: array copy owned by a filter,
   must be freed via dtor (owned).
3. `dbfl_type_ref` with `u.r.dtor == NULL` and `no_elements > 0`: pointer
   into live record data (not owned — scan lock required to access).
4. `dbfl_type_ref` with `no_elements == 0`: empty array, conceptually owned
   even though no dtor is set (no data to access).

The "does this log own its data?" question was answered with the expression:

```c
(!pfl || (pfl->type==dbfl_type_ref && !pfl->u.r.dtor))
```

This expression was duplicated in `dbAccess.c` (used for the `get_array_info`
call gate and for the convert path selection) and referenced in comments in
`dbChannel.h`. The `no_elements==0` special case was documented but not
encoded in any predicate, making it easy to get wrong. The immediately
following commit `56f05d7` demonstrated that the inlined version was indeed
wrong in two places.

## Fix
Added `dbfl_has_copy(p)` macro to `db_field_log.h`:

```c
#define dbfl_has_copy(p) \
  ((p) && ((p)->type==dbfl_type_val || (p)->u.r.dtor || (p)->no_elements==0))
```

This centralizes the three-case ownership logic in one tested expression.
Used in `dbAccess.c` to gate the `get_array_info` call:

```c
if (!dbfl_has_copy(pfl) && paddr->pfldDes->special == SPC_DBADDR && ...)
    prset->get_array_info(paddr, &no_elements, &offset);
```

The `dbChannel.h` comment updated to reference `dbfl_has_copy`.

## Rust Applicability
Applies. In base-rs, the Rust equivalent of `db_field_log` must encode
ownership semantics clearly. Options:
- Use `Cow<[u8]>` for borrowed-vs-owned field data.
- Use an enum with `Owned(Vec<u8>)` and `Borrowed(*const u8)` variants.
- The "borrowed" variant must always be accessed under the record scan lock.

If the design uses a plain `Option<Arc<FieldLog>>`, the borrowed/owned
distinction is lost and the scan-lock requirement cannot be enforced by
the type system.

## Audit Recommendation
Audit `base-rs/src/server/database/db_access.rs` and the field-log type:
verify that "borrowed from record" field logs are distinguishable from
"owned copy" field logs at the type level. Check whether the `get_array_info`
equivalent is only called for non-owned logs. Check that subscription
dispatch holds the scan lock when delivering a borrowed-reference field log.

## C Locations
- `modules/database/src/ioc/db/db_field_log.h:dbfl_has_copy` — new macro encoding 3-case ownership
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — uses dbfl_has_copy to gate get_array_info
- `modules/database/src/ioc/db/dbChannel.h` — comment updated to reference dbfl_has_copy
