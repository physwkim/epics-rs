---
sha: c19605232af60ff493c4cdea63640b168f5ce299
short_sha: c196052
date: 2021-01-18
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [atomic, typo, preprocessor, C++, linkage]
---
# epicsAtomicDefault.h: __cpluplus typo silently disables extern "C" block

## Root Cause
`epicsAtomicDefault.h` used `#ifdef __cpluplus` (missing an 's') in two
places — at the opening `extern "C" {` guard and at its closing `}`. The
correct macro is `__cplusplus`. As a result, when this header was included
from C++ translation units, the `extern "C"` block was never opened or
closed. All atomic operation inline functions defined in the header were
compiled with C++ name mangling instead of C linkage.

## Symptoms
On C++ builds using the fallback atomic implementation (platforms where
hardware atomic primitives are not available and `epicsAtomicDefault.h` is
used instead of `epicsAtomicOSD.h`): link errors referencing mangled names
for `epicsAtomicIncrIntT`, `epicsAtomicDecrIntT`, etc. The bug is latent on
platforms where `epicsAtomicOSD.h` provides the correct implementation, but
silently breaks the fallback path.

## Fix
Corrected both occurrences from `__cpluplus` to `__cplusplus`.

## Rust Applicability
Rust uses `std::sync::atomic` (wraps LLVM atomics), which has no C linkage
issues. Eliminated.

## Audit Recommendation
No audit needed. C++ preprocessor typo with no Rust analog.

## C Locations
- `modules/libcom/src/osi/epicsAtomicDefault.h` — both extern "C" guards used wrong macro name
