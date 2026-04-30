---
sha: 395015aac4d24fdc81e757b4db7e162a8b52a9fc
short_sha: 395015a
date: 2023-07-07
author: JJL772
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [macro, STATIC_ASSERT, typedef, collision, preprocessor]
---

# STATIC_ASSERT macro typedef name collision on same line

## Root Cause
The pre-C++11 fallback implementation of `STATIC_ASSERT(expr)` generated a typedef name using `STATIC_JOIN(static_assert_failed_at_line_, __LINE__)`. If the macro was used twice on the same line (e.g., in a macro expansion that emits multiple `STATIC_ASSERT` calls on the same logical line), both would expand to the same typedef name (same `__LINE__`), causing a "redefinition of typedef" compile error or silent collision. The `STATIC_JOIN` macro also only took two arguments, limiting flexibility.

## Symptoms
Compile error "redefinition of typedef" or type collision when `STATIC_ASSERT` appeared multiple times at the same source line number, which happens when `STATIC_ASSERT` is used inside other macros. This would prevent building on pre-C++11 compilers (old GCC, MSVC) with certain header configurations.

## Fix
Added `__COUNTER__` (a non-standard but widely supported compiler extension) as a second uniquifier in addition to `__LINE__`. The new typedef name is `static_assert_<COUNTER>_failed_at_line_<LINE>`. For compilers that don't support `__COUNTER__` (noted: GCC 4.1), falls back to `0` as the counter, which still collides on same-line use but is the best available option.

## Rust Applicability
Eliminated. Rust has `assert!()` and compile-time assertions via `const _: () = assert!(...)`, which are native language constructs with no typedef-collision hazard. The `STATIC_ASSERT` macro pattern has no Rust equivalent issue.

## Audit Recommendation
No action needed.

## C Locations
- `modules/libcom/src/osi/epicsAssert.h:STATIC_ASSERT` — added `__COUNTER__` uniquifier to typedef name
