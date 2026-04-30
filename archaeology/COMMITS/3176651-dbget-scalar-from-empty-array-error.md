---
sha: 3176651c7116b2bbc8aedc0ec28a562c900cfbc3
short_sha: 3176651
date: 2020-06-09
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
tags: [bounds, empty-array, dbget, scalar-read, out-of-bounds]
---

# dbGet: Return error when reading scalar from empty array

## Root Cause
`dbGet()` has a fast path for scalar reads: when `offset == 0` and
`nRequest` is NULL or `no_elements == 1`, it calls `dbFastGetConvertRoutine`
directly without checking whether the array has any elements. If `no_elements
== 0` (an empty array field), this dereferences `pfield` at offset 0, reading
uninitialized or garbage memory and returning it as the field value.

The condition checked `COUNT <= 0` to mean "empty", but the fast-path branch
only guarded against `nRequest == NULL` — it did not verify `no_elements >= 1`
before the dereference.

## Symptoms
- `caget` on a PV whose waveform has zero elements returns a non-zero garbage
  value in the first element instead of signaling an error.
- Downstream monitors see a spurious value change when the array transitions
  to empty.
- Potential read past array bounds if `pfield` is a heap-allocated buffer.

## Fix
Added a guard in the `offset == 0` fast path:
```c
else if (no_elements < 1) {
    status = S_db_onlyOne;
    goto done;
}
```
This ensures that when `nRequest` is NULL (scalar-only request) and the array
is empty, `dbGet` returns `S_db_onlyOne` instead of dereferencing element 0.

## Rust Applicability
Applies. In `base-rs::db_access::db_get`, any fast-path scalar read must
check that the underlying slice is non-empty before indexing element 0.
A `slice.first()` / `slice.get(0)` pattern naturally handles this, but an
explicit bounds check is needed if raw pointer arithmetic is used.

## Audit Recommendation
In `base-rs/src/server/database/db_access.rs:db_get`, verify the scalar fast
path does not access `fields[0]` when `no_elements == 0`. Prefer
`fields.get(0).ok_or(DbError::OutOfBounds)?` pattern.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — added no_elements < 1 guard before fast-path scalar dereference
