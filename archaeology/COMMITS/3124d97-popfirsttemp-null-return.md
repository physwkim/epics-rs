---
sha: 3124d972bf7d70c58e46bf5faee2828af779555e
short_sha: 3124d97
date: 2020-06-10
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_lex_routines.rs
    function: pop_first_temp
tags: [null-deref, parser, crash, dbStatic, bad-record-name]
---

# Fix crash in popFirstTemp() when temp list is empty on bad record name

## Root Cause
`popFirstTemp()` unconditionally dereferenced `ellFirst(&tempList)` without checking for NULL. When the temp list was empty (caused by an earlier parse error, e.g., a bad record name producing a grammar error that left `tempList` in an inconsistent state), `ptempListNode` was NULL and the immediate dereference of `ptempListNode->item` caused a SIGSEGV.

## Symptoms
Parsing a `.db` or `.dbd` file containing a bad record name (triggering a lex/grammar error that leaves the temp list empty) would crash the IOC process with a null pointer dereference inside the parser cleanup path.

## Fix
Initialized `ptemp = NULL` and guarded the dereference with `if(ptempListNode)`. Returns NULL when the list is empty. Callers are fixed in the follow-up commit (`b34aa594`) to handle the NULL return.

## Rust Applicability
In base-rs, `pop_first_temp()` must return `Option<T>`. The original C crash maps directly to a `unwrap()` on `None` in Rust, which would panic. The fix maps to returning `None` and letting callers use `?` propagation.

## Audit Recommendation
In `base-rs/src/server/database/db_lex_routines.rs`, confirm `pop_first_temp()` returns `Option<T>` and that the implementation does not `unwrap()` the linked-list head.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:popFirstTemp` — initialized `ptemp=NULL`; guarded dereference inside `if(ptempListNode)`
