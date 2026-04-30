---
sha: dec4fc30d948d4c717430ada3f1366d6dfa204b2
short_sha: dec4fc3
date: 2020-06-22
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbpf, declaration, shadow, c-cleanup, dead-variable]
---

# Remove stray epicsOldString array declaration shadowing outer scope in dbpf

## Root Cause
`dbpf()` had a stray `epicsOldString *array;` declaration inside the whitespace-parsing block that shadowed or conflicted with a pointer used across the block boundary. This caused the subsequent `dbCalloc` result to be assigned to the inner scoped pointer rather than the outer one, so `pvalue = (void*)array` at the end of the block would use an uninitialized or mismatched pointer value.

## Symptoms
Array string puts via `dbpf` could use an incorrect buffer pointer, potentially reading from uninitialized memory or the wrong allocation when passing the array to `dbPutField`.

## Fix
Removed the stray `epicsOldString *array;` local declaration from inside the block. The outer `array` variable (allocated by `dbCalloc`) is now used correctly throughout.

## Rust Applicability
Rust's scoping rules and borrow checker prevent this class of bug — a variable declared in an inner scope cannot silently shadow and misroute an outer pointer. Eliminated.

## Audit Recommendation
None. This is a C-specific declaration-shadowing bug eliminated by Rust's ownership and scoping model.

## C Locations
- `modules/database/src/ioc/db/dbTest.c:dbpf` — removed stray `epicsOldString *array` inner declaration
