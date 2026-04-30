---
sha: 511bf1ffcae641a9994c0f37c8bb66ed81caf44e
short_sha: 511bf1f
date: 2023-11-25
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [const, dbLink, type-system, mutable-aliasing, correctness]
---

# Const-ify dbLink type mapping arrays

## Root Cause
`pamaplinkType[]`, `ppstring[]`, and `msstring[]` were declared as `char *` arrays (non-const), meaning external code could legally (per C type rules) write through them, potentially corrupting the global link-type mapping table. The `pamaplinkType` global was also missing a size in its declaration (`maplinkType pamaplinkType[]`) which prevented compile-time size checking.

## Symptoms
No crash was observed, but the non-const declaration was an invitation for accidental mutation. Any external code that obtained a pointer to these arrays through the non-const interface could overwrite link type strings, corrupting all subsequent link-type lookups globally.

## Fix
Added `const` to `pamaplinkType`, `ppstring`, `msstring`, and `maplinkType::strvalue`. Updated the `pamaplinkType` extern declaration to `const maplinkType pamaplinkType[LINK_NTYPES]` with explicit size. Changed `msstring` in `dbLock.c` to `const char *`.

## Rust Applicability
Eliminated. In Rust, `static` data is immutable by default (`static FOO: [&str; N] = [...]`). Mutation requires `static mut` which is `unsafe`. The equivalent lookup tables in `base-rs` would naturally be `const` or `static` slices, enforced by the type system.

## Audit Recommendation
No action needed. If `base-rs` exposes a C-compatible `pamaplinkType` symbol for FFI, verify the C-side declaration matches `const`.

## C Locations
- `modules/database/src/ioc/dbStatic/link.h:maplinkType::strvalue` — `char *` → `const char *`
- `modules/database/src/ioc/dbStatic/link.h:pamaplinkType` — added `const`, explicit size `[LINK_NTYPES]`
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:ppstring,msstring,pamaplinkType` — all `const`-ified
- `modules/database/src/ioc/db/dbLock.c:msstring` — `char *` → `const char *`
