---
sha: 4ecc0daa793f8b64869eb8e8e922b1f60475b85c
short_sha: 4ecc0da
date: 2023-06-13
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [const, type-map, dbf-to-dbr, immutable, compile-time]
---
# mapDBFToDBR[] Table Missing const Qualifier

## Root Cause
`dbAccess.c` declares `mapDBFToDBR[]` as a module-level static array
mapping `DBF_*` field types to their corresponding `DBR_*` request types.
This table is never modified after initialization; it is a pure read-only
lookup table. Without the `const` qualifier, the C compiler may place it in
writable data segments and cannot catch accidental writes. A stray pointer
write to the table could corrupt the DBF→DBR type mapping globally, causing
all subsequent database field access to return wrong types.

## Symptoms
- Without `const`, no compiler error if code accidentally writes into
  `mapDBFToDBR[]`.
- Potential for stray pointer writes to silently corrupt the type map.
- Missed compiler optimization opportunities (read-only data in `.rodata`).

## Fix
Change declaration from `static short mapDBFToDBR[DBF_NTYPES]` to
`static const short mapDBFToDBR[DBF_NTYPES]`.

## Rust Applicability
`eliminated` — Rust module-level data is either a `const` array (compiled
into `.rodata`, literally unwritable) or a `static mut` (requires `unsafe`
to write). The DBF→DBR type map in base-rs would be `const` or `static
[DbfType; DBF_NTYPES]`, and accidental mutation is a compile error.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:mapDBFToDBR` — missing `const` on static type-map array
