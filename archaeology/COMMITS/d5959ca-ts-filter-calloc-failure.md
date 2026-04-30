---
sha: d5959ca20ae956c2f9c68ef356f446c604f245ce
short_sha: d5959ca
date: 2022-05-19
author: Jure Varlec
category: leak
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [allocation-failure, null-deref, field-log, filter, calloc]
---
# ts filter: handle calloc failures gracefully

## Root Cause
In `ts_array()` and `ts_string()` within `ts.c`, the code called
`allocTsArray()` / `allocString()` (both backed by `freeListCalloc`) but did
not check the returned pointer for `NULL`. If allocation failed:

- `ts_array()` immediately stored `no_elements = 2` and called
  `ts_to_array(... pfl->u.r.field)` — dereferencing the NULL pointer.
- `ts_string()` set `pfl->u.r.dtor = freeString` before checking for NULL,
  meaning the destructor would be called on a NULL field pointer.

Both lead to NULL pointer dereferences or heap corruption on OOM.

## Symptoms
- Segfault or heap corruption in the ts filter when the allocator returns NULL
  (e.g., under heavy load or memory exhaustion).
- Potentially silent corruption: a NULL `u.r.field` with a non-NULL `dtor`
  causes double-free behaviour when `db_delete_field_log` later calls the dtor.

## Fix
In `ts_array()`: wrapped the post-alloc code in `if (pfl->u.r.field)`;
on failure sets `no_elements = 0` and `dtor = NULL` (safe empty state).

In `ts_string()`: added an early return after the NULL check, setting
`no_elements = 0` and `dtor = NULL` before the `dtor` assignment, then
returning, so the dtor is never set on a NULL pointer.

## Rust Applicability
In Rust, allocation failures either panic (global allocator) or return
`Err(AllocError)` via `try_reserve`. There is no silent NULL return from
`Box::new` or `Vec`. This specific null-check pattern is eliminated by the
type system. No audit needed.

## Audit Recommendation
None — eliminated by Rust's allocation model.

## C Locations
- `modules/database/src/std/filters/ts.c:ts_array` — NULL check added after allocTsArray()
- `modules/database/src/std/filters/ts.c:ts_string` — early-return NULL guard added before freeString assignment
