---
sha: e0dfb6cff8db1f192a5eee0bd8dc56095f51d290
short_sha: e0dfb6c
date: 2020-02-13
author: Dirk Zimoch
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [PINI, field-log, use-after-free, stack-alloc, lifetime]
---

# PINI crash: use stack-local field-log to avoid heap UAF in filter chain

## Root Cause
The original `dbDbGetValue` allocated a `db_field_log` on the heap via
`db_create_read_log()` and passed it through `dbChannelRunPreChain` /
`dbChannelRunPostChain`. During PINI (process-on-init), these callbacks
could trigger re-entrant access to the same field-log or free it before
the caller's `db_delete_field_log` ran, causing a use-after-free.

## Symptoms
Crash (segfault) in `dbDbGetValue` during PINI processing when a filter
chain is installed on a DB link. The heap-allocated field log was freed
or reused before `db_delete_field_log` completed.

## Fix
Replaced heap `db_create_read_log` with a stack-local `db_field_log fl`
(zero-initialized), initialized `fl.ctx` and `fl.type` explicitly, and
passed `&fl` directly to `dbChannelRunPreChain` / `dbChannelRunPostChain`.
Because the stack variable lives for the duration of the call, no
use-after-free is possible. The `db_delete_field_log` call is eliminated.

Note: this fix was later reverted by c51c83b because the pre/post chain
can reallocate the pointer; see that commit for the full resolution.

## Rust Applicability
In base-rs `links.rs`, if a `FieldLog` struct is passed by reference
through filter-chain calls that might move or replace it, passing a
local reference is unsound. The Rust borrow checker enforces this: if
`run_pre_chain(&mut fl)` can replace `fl`, the function must take
`FieldLog` by value and return a new one. Verify the chain API signature.

## Audit Recommendation
In `src/server/database/links.rs`: find `run_pre_chain` / `run_post_chain`
signatures. Check whether they take `&mut FieldLog` (which cannot be
replaced) or `FieldLog -> FieldLog` (value semantics, safe). If the C
pattern of pointer replacement is preserved, the Rust equivalent must
use `Box<FieldLog>` passed by value through the chain.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — switched to stack-local `db_field_log fl`
