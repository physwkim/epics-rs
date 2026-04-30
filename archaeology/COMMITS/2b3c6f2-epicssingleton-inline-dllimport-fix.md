---
sha: 2b3c6f2e262a7dfc506cce66547ad2dc74d8a80b
short_sha: 2b3c6f2
date: 2021-10-18
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [singleton, dllimport, Windows, mingw, template-inline]
---

# epicsSingleton: inline template methods to fix mingw dllimport/export issues

## Root Cause
`epicsSingleton<T>` had its template methods defined out-of-line in separate
`.cpp` files. On Windows with MinGW, template instantiations in DLLs can
cause linker errors when the `LIBCOM_API` (dllimport/dllexport) annotation
is on the class itself rather than only on the out-of-line methods. The class-
level `LIBCOM_API` on `SingletonUntyped` caused all methods to be marked for
import/export, creating symbol conflicts when templates were instantiated
across DLL boundaries.

Additionally, the `epicsSingletonMutex.cpp` file had `pEPICSSigletonMutex`
and `SingletonMutexOnce` at file scope without being in an anonymous namespace,
polluting the global symbol table.

## Symptoms
- Linker errors on Windows/MinGW when building EPICS applications as DLLs.
- Symbol visibility conflicts with `epicsSingleton` template instantiations.
- (Minor) global symbol pollution from `epicsSigletonOnceFlag` / `pEPICSSigletonMutex`.

## Fix
- Removed `LIBCOM_API` from the `SingletonUntyped` class declaration.
- Added `LIBCOM_API` only to the two out-of-line methods: `incrRefCount` and
  `decrRefCount`.
- Moved all `epicsSingleton<TYPE>` template method bodies inline into the class
  definition in the header (no separate `.cpp` instantiation needed).
- Wrapped `epicsSingletonMutex.cpp` file-scope variables in an anonymous
  `namespace { }` block.

## Rust Applicability
Rust has no DLL export/import annotation system at the template instantiation
level (monomorphization is handled by the compiler). The `once_cell::sync::Lazy`
or `std::sync::OnceLock` pattern in Rust replaces `epicsSingleton` entirely.
This C++ linkage-visibility issue is fully eliminated in Rust.

## Audit Recommendation
None — eliminated by Rust's monomorphization and `OnceLock`/`Lazy` primitives.

## C Locations
- `modules/libcom/src/cxxTemplates/epicsSingleton.h:SingletonUntyped` — LIBCOM_API moved to methods
- `modules/libcom/src/cxxTemplates/epicsSingleton.h:epicsSingleton` — template methods inlined
- `modules/libcom/src/cxxTemplates/epicsSingletonMutex.cpp` — anonymous namespace added
