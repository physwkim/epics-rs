---
sha: db2f7d8b92c97ab2b359028d491373ee0a8804ae
short_sha: db2f7d8
date: 2020-10-26
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [smart-pointer, unique-ptr, auto-ptr, deprecated, c++11]
---
# CA client: migrate std::auto_ptr to std::unique_ptr for C++17 compat

## Root Cause
`std::auto_ptr` was deprecated in C++11 and removed in C++17. Several CA
client headers (`oldAccess.h`, `udpiiu.h`, `dbCAC.h`) and implementation files
used `std::auto_ptr<CallbackGuard>`, `std::auto_ptr<cacContext>`, and
`std::auto_ptr<searchTimer>` for RAII ownership. With C++17 compilers these
would fail to compile. Beyond compilation, `auto_ptr` has unsafe copy
semantics (copy == move) that can silently transfer ownership during copy
operations, leading to use-after-free or double-free.

## Symptoms
Compilation failures with C++17 compilers. Pre-C++17: latent use-after-free
risk from `auto_ptr` copy semantics when these smart pointers were passed by
value.

## Fix
Introduce a compatibility alias `ca::auto_ptr<T>` that maps to
`std::unique_ptr<T>` on C++11+ and `std::auto_ptr<T>` on older compilers.
Add `PTRMOVE(p)` macro for `std::move(p)` / identity respectively. Replace all
`std::auto_ptr` uses in CA client headers with `ca::auto_ptr`.

## Rust Applicability
Rust uses ownership by default — `Box<T>`, `Arc<T>`, and ownership transfer
via move are the compiler-enforced norm. The `auto_ptr` copy-semantics hazard
cannot arise. No audit needed.

## Audit Recommendation
None — Rust's ownership model eliminates this class of bug entirely.

## C Locations
- `modules/ca/src/client/oldAccess.h:ca_client_context` — `pCallbackGuard`, `pServiceContext` changed to `ca::auto_ptr`
- `modules/ca/src/client/udpiiu.h:SearchArray` — `value_type` changed to `ca::auto_ptr<searchTimer>`
- `modules/database/src/ioc/db/dbCAC.h:dbContext` — `pNetContext` changed to `ca::auto_ptr<cacContext>`
- `modules/ca/src/client/ca_client_context.cpp:ca_client_context::ca_client_context` — use `PTRMOVE` for transfer
