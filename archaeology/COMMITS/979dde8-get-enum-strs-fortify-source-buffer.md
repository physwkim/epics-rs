---
sha: 979dde8376405a77761e51366cbb42cc788d5199
short_sha: 979dde8
date: 2024-06-20
author: Michael Davidsaver
category: bounds
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: get_enum_strs
tags: [FORTIFY_SOURCES, enum-strs, buffer, dbAccess, strncpy]
---
# get_enum_strs uses pointer arithmetic that trips _FORTIFY_SOURCES=3

## Root Cause
`get_enum_strs` in `dbAccess.c` iterated over enum string slots using a
`ptemp` pointer arithmetic pattern (`ptemp += sizeof(strs[0])`).
`_FORTIFY_SOURCE=3` uses `__builtin_object_size` to validate `strncpy`
destination sizes at compile time; the pointer-arithmetic approach hides
the per-slot size from the compiler, causing spurious fortify warnings
and potentially incorrect size checks.  The function also mismanaged the
`*ppbuffer` advance: the output-buffer pointer was incremented at the end
of the function body rather than at entry, leading to double-advance on
early-exit paths.

## Symptoms
Compile-time fortify warnings with `-D_FORTIFY_SOURCE=3`.  Potential
incorrect buffer-advance on error exits.

## Fix
Rewrote `get_enum_strs` to:
1. Advance `*ppbuffer` at function entry (unconditionally), so all
   exit paths (including error falls-through) leave the buffer pointer
   correct for the next option.
2. Use direct indexed access `penum->strs[i]` in the `strncpy` calls
   so `__builtin_object_size` can see the per-slot size.
3. Restructured the DBF_ENUM / DBF_MENU / DBF_DEVICE branches to use
   early `return` on success and fall-through to a `nostrs` label on
   failure, removing the `goto choice_common` pattern.

## Rust Applicability
Applies.  In base-rs the enum-string encoding function for CA responses
must write into a fixed-size slot array.  Use indexed slice access
rather than raw pointer arithmetic to keep slice-bounds-checking active.
Also verify that the output buffer pointer is always advanced regardless
of whether the field type is DBF_ENUM/MENU/DEVICE or falls through to
the "no strings" case.

## Audit Recommendation
In `db_access.rs::get_enum_strs` (or its equivalent CA DBR_ENUM_STRS
encoder), verify: (1) the buffer cursor advances at entry for this
option, not only on success; (2) string slots are written via indexed
access, not raw pointer arithmetic.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:get_enum_strs` — full rewrite for _FORTIFY_SOURCE=3 compatibility; buffer advance moved to entry; indexed strncpy
