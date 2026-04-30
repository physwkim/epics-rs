---
sha: b34aa594c87605b7846f6f06275ca0dba34ec7be
short_sha: b34aa59
date: 2020-06-10
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_lex_routines.rs
    function: db_menu_body
tags: [null-deref, parser, popFirstTemp, dbStatic, crash]
---

# Null guard cascade for popFirstTemp() return in DB parser

## Root Cause
`popFirstTemp()` was fixed in commit `3124d972` to return NULL when the temp list is empty (instead of crashing immediately). However, all callers (`dbMenuBody`, `dbRecordtypeBody`, `dbBreakBody`) did not check the returned pointer before immediately using it — dereferencing a NULL returned from a now-safe `popFirstTemp()` would still crash. This commit adds the necessary NULL guards at every call site.

## Symptoms
Parsing a malformed `.db` or `.dbd` file that triggers an earlier parse error (which clears `tempList`) would cause `popFirstTemp()` to return NULL, and the callers would immediately dereference it, crashing the IOC on startup.

## Fix
Added `if(!ptr) return;` checks after every `popFirstTemp()` call in `dbMenuBody`, `dbRecordtypeBody`, and `dbBreakBody`. The inner per-element `popFirstTemp()` calls in loops also now guard and return early. Error is already printed by the earlier failure; returning silently is correct.

## Rust Applicability
In base-rs DB parser, `pop_first_temp()` should return `Option<T>`. All call sites must handle `None` with an early return or `?`. If the parser uses a stack/arena, ensure the equivalent of "empty stack" is handled as a graceful error, not a panic. This pattern is a direct match to Rust's `Option` propagation.

## Audit Recommendation
In `base-rs/src/server/database/db_lex_routines.rs` (or the DB parser equivalent), audit every call to `pop_first_temp()` / equivalent and confirm all return `None` cases are handled without panic.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbMenuBody` — null guards after popFirstTemp for menu, names, values
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbRecordtypeBody` — null guards for recordtype and fields
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbBreakBody` — null guards for breaktable and raw/eng strings
