---
sha: c51c83b1d507bccc2ca30daecb7022feb64c2765
short_sha: c51c83b
date: 2020-02-25
author: Dirk Zimoch
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/links.rs
    function: db_db_get_value
tags: [PINI, field-log, heap-alloc, lifetime, revert]
---

# Revert stack-allocated field-log fix: heap alloc required for PINI safety

## Root Cause
A prior fix (commit a590151) replaced `db_create_read_log()` (heap
allocation) with a stack-local `db_field_log fl = {}` to avoid a
use-after-free during PINI callback processing. However the revert reveals
that switching to a stack-local log broke the `db_delete_field_log` / filter
post-chain lifecycle: `dbChannelRunPreChain` and `dbChannelRunPostChain`
may reallocate or swap out the pointer they receive, meaning a stack-local
value cannot be safely returned through these calls. The revert restores
heap allocation (`db_create_read_log`) and explicit `db_delete_field_log`.

## Symptoms
Stack-allocated `db_field_log` passed by pointer through pre/post filter
chains could be freed or replaced by the chain internally, leaving a
dangling stack pointer. The original PINI crash was a use-after-free of
a heap-allocated log that was not deleted before callbacks fired.

## Fix
Reverts to heap-allocated field log with explicit `db_delete_field_log`,
adds null-check on allocation failure returning `S_db_noMemory`, and
properly gates the empty-array error on the post-filter element count.

## Rust Applicability
In base-rs `links.rs` (analogous to `dbDbLink.c::dbDbGetValue`), any
filter-chain invocation that receives a mutable reference to a field-log
structure must not pass a local stack reference. If the chain takes
`&mut FieldLog` and can replace it (analogous to the C pointer-swap
pattern), the log must be owned on the heap (e.g., `Box<FieldLog>`) for
the call duration. Check whether `run_pre_chain` / `run_post_chain` in
base-rs can replace or move the log.

## Audit Recommendation
In `src/server/database/links.rs`, find `run_pre_chain` / `run_post_chain`
call sites. Verify the field-log value is heap-owned (Box or Arc) rather
than a local variable when passed to filter chains. Also check null/OOM
handling after field-log allocation.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:dbDbGetValue` — reverted to heap-alloc field log with null-check
