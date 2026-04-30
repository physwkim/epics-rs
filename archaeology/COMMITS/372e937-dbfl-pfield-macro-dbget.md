---
sha: 372e937717af65b903d7b9885b7c34e151c9bd86
short_sha: 372e937
date: 2021-01-14
author: Ben Franksen
category: lifecycle
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
tags: [db_field_log, field-pointer, macro, abstraction, refactor]
---
# dbGet: duplicated dbfl_type_val/ref dispatch replaced with dbfl_pfield macro

## Root Cause
In `dbGet`, the expression to obtain a data pointer from a `db_field_log`
was duplicated in two places (fast-convert path and general convert path):

```c
if (pfl->type == dbfl_type_val)
    localAddr.pfield = (char *) &pfl->u.v.field;
else
    localAddr.pfield = (char *)  pfl->u.r.field;
```

This duplication meant that if `db_field_log` union layout changed, all
copies would need to be updated. The `dbfl_has_copy` macro (from `85822f3`)
already abstracted the ownership check; there was no equivalent for field
pointer extraction.

## Fix
Added `dbfl_pfield(p)` macro to `db_field_log.h`:

```c
#define dbfl_pfield(p) \
  ((p)->type==dbfl_type_val ? &p->u.v.field : p->u.r.field)
```

Both duplicated expressions in `dbGet` replaced with `localAddr.pfield =
dbfl_pfield(pfl)`.

## Rust Applicability
Partial. In base-rs, the `DbFieldLog` Rust type's equivalent of "get a
pointer to the field data" should be encapsulated in a method (e.g.,
`fn field_ptr(&self) -> *const u8`) rather than matched inline at each call
site. Duplication of match arms is a maintainability issue, not a correctness
bug. No immediate audit priority.

## Audit Recommendation
If base-rs has a `DbFieldLog` or equivalent struct with a val/ref union
pattern, ensure field-data access is centralized in a method rather than
duplicated at call sites.

## C Locations
- `modules/database/src/ioc/db/db_field_log.h:dbfl_pfield` — new macro
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — two call sites replaced
