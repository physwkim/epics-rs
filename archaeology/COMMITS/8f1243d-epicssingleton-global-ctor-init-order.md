---
sha: 8f1243da406aebf5a8cebda65ea84e5dbeeff286
short_sha: 8f1243d
date: 2022-09-29
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [global-constructor, singleton, init-order, constexpr, cpp11]
---
# epicsSingleton Global Constructor Causes Static Initialization Order Fiasco

## Root Cause
`epicsSingleton.h` defines `SingletonUntyped` and `epicsSingleton<T>` classes
with non-trivial constructors (initializing `_pInstance` and `_refCount` to
zero). When instances of these types are used as `static` or global variables,
the C++ static initialization order fiasco applies: if another translation
unit's global/static variable is constructed before `SingletonUntyped`'s
constructor runs, it sees uninitialized data.

Prior to C++11, there is no standard way to guarantee zero-initialization
before the constructor runs for non-trivial types. With C++11 `constexpr`
constructors, the compiler is required to evaluate the constructor at
compile time, placing the result in `.rodata`/`.data` with static storage
duration — effectively zero-initialized before any dynamic initialization
occurs.

## Symptoms
- Potential use of uninitialized singleton state during startup if multiple
  translation units initialize globals in unexpected order.
- Race during early startup where `_pInstance == 0` check is not reliable.

## Fix
Add `constexpr` to `SingletonUntyped()` and `epicsSingleton()` constructors
when compiling with C++11 or later (`__cplusplus >= 201103L`). This promotes
them to constant-initialized objects, eliminating the static initialization
order fiasco.

## Rust Applicability
`eliminated` — Rust `static` variables are either `const`-initialized at
compile time or use `std::sync::OnceLock`/`once_cell::sync::OnceCell` for
lazy initialization. The singleton pattern in epics-rs uses `OnceLock` or
module-level `static` with `Arc<Mutex<T>>`, both of which have defined
initialization order. The static init order fiasco does not exist in Rust.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/cxxTemplates/epicsSingleton.h:SingletonUntyped` — non-constexpr constructor risks init-order fiasco
- `modules/libcom/src/cxxTemplates/epicsSingleton.h:epicsSingleton` — same
