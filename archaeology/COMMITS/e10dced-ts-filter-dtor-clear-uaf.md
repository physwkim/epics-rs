---
sha: e10dcede7d4b1d2f4c3b6aa45610de0c07a6b538
short_sha: e10dced
date: 2022-05-19
author: Jure Varlec
category: lifecycle
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [use-after-free, double-free, dtor, field-log, stale-pointer]
---
# ts filter: clear dtor field after destruction to prevent double-free

## Root Cause
In `replace_fl_value()` and `channelRegisterPost()` in `ts.c`, after calling
`pfl->u.r.dtor(pfl)` to destroy the old field log value, the `dtor` function
pointer was NOT cleared. This left a stale (dangling or non-null) pointer in
`pfl->u.r.dtor`.

If the same `pfl` was subsequently destroyed through a different code path
that checked `dtor != NULL` and called it again, the destructor would be
invoked twice on already-freed memory — a classic double-free / use-after-free
bug.

## Symptoms
- Double-free heap corruption when a `db_field_log` with type `dbfl_type_ref`
  is processed by the ts filter and later destroyed via the normal
  `db_delete_field_log()` path.
- Memory corruption may be silent or cause heap allocator crashes.

## Fix
After `pfl->u.r.dtor(pfl)` is called, immediately set `pfl->u.r.dtor = NULL`.
This is done in both:
1. `replace_fl_value()` — after destroying the old reference value.
2. `channelRegisterPost()` — after probing and freeing the channel's probe
   field log.

## Rust Applicability
In Rust, destructors run exactly once via `Drop`. The pattern of a manually-
managed `dtor` function pointer on a union field does not exist. The
double-free is statically prevented by ownership. No audit needed.

## Audit Recommendation
None — eliminated by Rust's ownership and Drop semantics.

## C Locations
- `modules/database/src/std/filters/ts.c:replace_fl_value` — dtor cleared after call
- `modules/database/src/std/filters/ts.c:channelRegisterPost` — dtor cleared after probe destruction
