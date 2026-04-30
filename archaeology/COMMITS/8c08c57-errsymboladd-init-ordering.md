---
sha: 8c08c5724725ca8012093672fa6fa16984c05875
short_sha: 8c08c57
date: 2023-03-08
author: Emilio Perez
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/log/err_sym.rs
    function: err_symbol_add
tags: [initialization, errSymbol, race, hash-table, lifecycle]
---

# errSymbolAdd fails if called before errSymBld (init ordering bug)

## Root Cause
The error symbol hash table (`hashtable`) was heap-allocated by `errSymBld()` and guarded by an `initialized` flag. `errSymbolAdd()` appended to an `ELLLIST errnumlist` (not the hash table), and the hash table was only built from that list during `errSymBld()`. Calling `errSymbolAdd()` after `errSymBld()` (or before it) did not insert into the hash table, so the symbol was never findable via `errSymMsg()`. Additionally, `errSymBld()` was not thread-safe, and there was no mutex protecting concurrent additions.

## Symptoms
Calling `errSymbolAdd()` from module initialization code (before `iocInit`/`errlogInit`) would appear to succeed but the symbol would never be retrievable. Concurrent `errSymbolAdd()` calls could corrupt the list. Issue #268 in the EPICS tracker.

## Fix
Completely redesigned the error symbol storage: statically allocated `errHashTable_t` (fixed array + mutex, zero-initialized at program start), initialized lazily via `epicsThreadOnce`. `errSymbolAdd()` now inserts directly into the hash table under a mutex, working correctly at any time. Duplicate codes with different messages return `S_err_codeExists`; identical duplicates return 0 (success). Added comprehensive unit tests.

## Rust Applicability
Applies. `base-rs` needs an error symbol registry equivalent (`err_sym_add`, `err_sym_msg`). If this is implemented with a `Mutex<HashMap<u32, &'static str>>` initialized via `std::sync::OnceLock` or `once_cell::sync::Lazy`, it must:
1. Allow insertions at any time (pre- and post-IOC-init).
2. Handle duplicate codes gracefully (return error, not panic).
3. Be thread-safe for concurrent insertions.
4. Validate module numbers (≥ 501, i.e., ≥ `MIN_MODULE_NUM`).

## Audit Recommendation
In `base-rs/src/log/err_sym.rs`, verify:
1. The symbol table is initialized via `OnceLock`/`Lazy` (not a manual `bool initialized` flag).
2. `err_symbol_add()` acquires a lock before insertion.
3. Duplicate code with different message returns `Err(CodeExists)`.
4. Module number < 501 returns `Err(InvalidCode)`.

## C Locations
- `modules/libcom/src/error/errSymLib.c:errSymBld` — simplified to just populate the now-always-valid hash table
- `modules/libcom/src/error/errSymLib.c:errSymbolAdd` — now inserts directly into locked hash table
- `modules/libcom/src/error/errSymLib.c:initErrorHashTable` — new, called via `epicsThreadOnce`
- `modules/libcom/src/error/errSymLib.c:errSymLookupInternal` — uses mutex for lookup
