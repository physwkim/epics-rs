---
sha: 12c56ffc954e4cde72c43174ebd11ccd0d523c1e
short_sha: 12c56ff
date: 2025-11-26
author: Dirk Zimoch
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [integer-overflow, malloc, size_t, flex, build-tool]
---
# flex allocate_array: int*int overflow before malloc size arg

## Root Cause
`allocate_array()` and `reallocate_array()` in the bundled flex lexer generator
pass `element_size * size` as the size argument to `malloc`/`realloc` using two
`int` operands. On 64-bit platforms, the intermediate product is computed as a
signed 32-bit multiplication. If either factor is large enough, the product
wraps to a small positive or negative value before being widened to `size_t`,
leading to an under-allocated buffer.

## Symptoms
Silent heap under-allocation when flex processes very large grammar files.
Subsequent writes off the end of the buffer cause heap corruption, usually
manifesting as a crash or non-deterministic parse errors in the build tool, not
in the runtime IOC.

## Fix
Cast one operand to `size_t` before the multiplication:
```c
mem = malloc( (size_t) element_size * size );
new_array = realloc( array, (size_t) size * element_size );
```
This forces unsigned 64-bit arithmetic on 64-bit hosts, preventing signed
overflow.

## Rust Applicability
`eliminated` — This code lives in the bundled flex build tool
(`modules/libcom/src/flex/misc.c`), which has no Rust analog. In Rust,
`Vec::with_capacity(n)` and slice indexing handle allocation sizing with
built-in overflow checks. No audit required.

## Audit Recommendation
No audit needed. Rust's allocator API (`Layout::from_size_align`,
`GlobalAlloc::alloc`) requires `Layout` which enforces size-alignment at
construction time, making this class of bug structurally impossible.

## C Locations
- `modules/libcom/src/flex/misc.c:allocate_array` — `malloc(element_size * size)` without upcast
- `modules/libcom/src/flex/misc.c:reallocate_array` — `realloc(..., size * element_size)` without upcast
