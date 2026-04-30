---
sha: 9d393c4437d36f935715229f18b2d7fbd6eec5f3
short_sha: 9d393c4
date: 2024-08-10
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [pointer-comparison, uintptr_t, UB, ordering, registerAllRecordDeviceDrivers]
---

# UB: Comparing Pointer Casts via char* Instead of uintptr_t

## Root Cause
`compareLoc::operator()` in `registerAllRecordDeviceDrivers.cpp` compared two
`sizeOffset` function pointers by casting them to `char *` and using `<`.
Comparing pointers to different objects via relational operators (`<`, `>`) is
undefined behavior in C++ when they do not point into the same array or object.
The correct approach is to cast to `uintptr_t` (an integer type) first, which
makes the comparison well-defined (implementation-defined ordering, but no UB).

## Symptoms
On compilers that perform UB-based optimizations, the sort of record/device
driver table could produce incorrect ordering, leading to mis-registration of
record or device support entries. In practice rarely triggered but strictly UB.

## Fix
Changed `reinterpret_cast<char *>(lhs.sizeOffset) < reinterpret_cast<char *>(rhs.sizeOffset)`
to `reinterpret_cast<uintptr_t>(lhs.sizeOffset) < reinterpret_cast<uintptr_t>(rhs.sizeOffset)`.

## Rust Applicability
Eliminated. Rust prohibits direct pointer relational comparisons on arbitrary
pointers; the `std::ptr` API makes ordering explicit. Function pointers are
not comparable at all without explicit casting to `usize`. The epics-rs driver
registry uses a Rust `HashMap` keyed by string name, not pointer ordering.

## Audit Recommendation
None required — Rust eliminates this category of bug.

## C Locations
- `modules/database/src/ioc/misc/registerAllRecordDeviceDrivers.cpp:compareLoc::operator()` — pointer relational comparison UB
