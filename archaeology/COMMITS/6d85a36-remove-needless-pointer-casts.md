---
sha: 6d85a36397de0666f12dca2054c47eb0b3742849
short_sha: 6d85a36
date: 2025-11-24
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [cast, pointer, c-cleanup, portability, compiler-warning]
---
# Remove needless C-style pointer casts across CA and database files

## Root Cause
Widespread use of unnecessary explicit pointer casts (e.g., `(void*)`,
`(char*)`, `(const char*)`) scattered across CA client files
(`access.cpp`, `repeater.cpp`, `casw.cpp`, `convert.cpp`, `tcpiiu.cpp`,
`udpiiu.cpp`) and numerous database source files. In C++ these casts are
either redundant or mask legitimate type mismatches. The compiler already
performs implicit conversions; explicit casts suppress useful warnings.

## Symptoms
No runtime symptoms. The casts were purely cosmetic noise that could
suppress legitimate `-Wcast-qual` or similar warnings, hiding actual type
errors in future refactors.

## Fix
Delete the redundant casts throughout the codebase. Over 50 files touched.
No logic changes — the resulting code behaves identically.

## Rust Applicability
`eliminated` — Rust's type system does not permit implicit pointer casts;
raw pointer conversions require explicit `as` or `From`/`Into` and are
typically confined to `unsafe` blocks. The pattern of silent no-op casts
masking type issues simply does not exist.

## Audit Recommendation
No audit required. Rust's borrowing rules prevent this category of issue
entirely.

## C Locations
- `modules/ca/src/client/access.cpp` — redundant casts removed
- `modules/ca/src/client/repeater.cpp` — redundant casts removed
- `modules/database/src/ioc/db/dbEvent.c` — 18 cast removals
- `modules/database/src/ioc/db/dbScan.c` — 30 cast removals
- `modules/database/src/ioc/rsrv/camessage.c` — redundant casts removed
