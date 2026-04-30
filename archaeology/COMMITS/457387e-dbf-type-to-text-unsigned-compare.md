---
sha: 457387ed3857f8f72f64c0589cbc4acb3d8e2cd8
short_sha: 457387e
date: 2024-08-12
author: Dirk Zimoch
category: type-system
severity: low
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/db_access.rs
    function: dbf_type_to_text
tags: [signed-unsigned, comparison, dbf-type, macro, ca-protocol]
---

# dbf_type_to_text macro signed comparison warning with unsigned type argument

## Root Cause

The `dbf_type_to_text(type)` macro in `db_access.h` bounds-checked the type
argument with:

```c
((type) >= -1 && (type) < dbf_text_dim-2)
```

When `type` is an unsigned integer (`unsigned int`, `epicsUInt16`, etc.), the
compiler warns that the `>= -1` comparison is always true for unsigned values.
Worse, if an unsigned `type` wraps around (e.g., `(unsigned int)-1` == `UINT_MAX`),
the lower bound check silently passes and the macro indexes into `dbf_text` with
`UINT_MAX + 1` → undefined behavior.

The fix rewrites the lower bound check as `(type+1) >= 0`. For signed arguments
this is equivalent (type+1 is negative only if type < -1). For unsigned arguments,
the addition wraps only if `type == UINT_MAX`, in which case `type+1 == 0` and the
check correctly fails.

## Symptoms

- Compiler warning `-Wtype-limits` on any call site that passes an unsigned type
  to `dbf_type_to_text`.
- If an unsigned value wrapping to `UINT_MAX` is passed, the macro would index
  `dbf_text[UINT_MAX]` → buffer overread / UB.

## Fix

Changed:
```c
((type) >= -1 && (type) < dbf_text_dim-2)
```
to:
```c
((type+1) >= 0 && (type) < dbf_text_dim-2)
```

## Rust Applicability

In ca-rs, any function that maps a DBF type code to a text string must correctly
bound-check the index before indexing into the lookup table. Rust's array/slice
indexing panics on out-of-bounds, so a bounds check is still required at the
conversion site. If the DBF type is represented as `u16` or similar, the
equivalent of `type >= -1` must not be written as a signed comparison.

Use `dbf_text.get(type as usize + 1)` with `Option` handling rather than manual
index arithmetic.

## Audit Recommendation

1. Find ca-rs `dbf_type_to_text` equivalent — verify no unchecked array index on
   type code.
2. Ensure the function handles type codes outside the valid range without panic
   in production (return `None` or a sentinel string).

## C Locations
- `modules/ca/src/client/db_access.h:dbf_type_to_text` — macro bounds check fix
