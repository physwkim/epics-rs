---
sha: 78d2f20fa8cbc6a357c6340f890f8d3f84e0be84
short_sha: 78d2f20
date: 2021-06-24
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [atomics, gcc, memory-barrier, conditional, build-system]
---

# GCC Atomic Intrinsic Conditionals Incorrect for i386 and Modern GCC

## Root Cause
`epicsAtomicCD.h` (GCC compiler layer) used `GCC_ATOMIC_INTRINSICS_GCC4_OR_BETTER`
to gate `__sync_synchronize()` availability. The old condition was:
`(__GNUC__ * 100 + __GNUC_MINOR__) >= 401` — i.e., any GCC >= 4.1.
This was historically correct but missed two cases:

1. **i386**: `__GCC_HAVE_SYNC_COMPARE_AND_SWAP_*` is not defined for i386 even
   in modern GCC (CAS is not lock-free on i386 in all GCC versions), but
   `__sync_synchronize()` *is* inlined for i386 as of GCC 8. The old code
   would incorrectly skip the barrier on i386.

2. **Stale `GCC_ATOMIC_INTRINSICS_MIN_X86`**: The old heuristic checked for
   `__i486 || __pentium || __pentiumpro || __MMX__` to enable CAS on x86 sub-
   architectures. Modern GCC uses `__GCC_HAVE_SYNC_COMPARE_AND_SWAP_N` macros
   which are authoritative; the handwritten x86 sub-arch list was redundant and
   potentially wrong (could enable CAS ops that the compiler won't actually
   emit correctly).

## Symptoms
On i386 targets, memory barrier intrinsics were incorrectly disabled, leaving
the EPICS atomic operations without a proper barrier. This could manifest as
subtle memory-ordering bugs on SMP i386 systems (rare in modern deployments
but still tested by EPICS CI).

## Fix
Replace `GCC_ATOMIC_INTRINSICS_GCC4_OR_BETTER` with
`GCC_ATOMIC_INTRINSICS_AVAIL_SYNC`, defined as
`defined(GCC_ATOMIC_INTRINSICS_AVAIL_INT_T) || defined(GCC_ATOMIC_INTRINSICS_AVAIL_SIZE_T) || defined(__i386)`.
Remove the old `GCC_ATOMIC_INTRINSICS_MIN_X86` / `GCC_ATOMIC_INTRINSICS_EARLIER`
heuristics. Gate CAS ops on `INT_T` and `SIZE_T` macros only (no legacy fallback).

## Rust Applicability
Eliminated. Rust uses `std::sync::atomic` which is backed by LLVM atomics.
LLVM correctly selects barrier and CAS instructions per target; there is no
manual compiler-version or target-feature gating required.

## Audit Recommendation
No audit needed. Rust's atomic primitives are correct by construction on all
supported targets.

## C Locations
- `modules/libcom/src/osi/compiler/gcc/epicsAtomicCD.h` — replaced legacy `GCC4_OR_BETTER` and `MIN_X86` conditionals with authoritative `GCC_HAVE_SYNC_COMPARE_AND_SWAP_*` macros
