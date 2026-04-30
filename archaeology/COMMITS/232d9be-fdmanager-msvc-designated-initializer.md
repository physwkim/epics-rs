---
sha: 232d9bec10d60b88e4f1f6b5d1d68a5aaa9ccbd0
short_sha: 232d9be
date: 2025-09-23
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [MSVC, C++20, designated-initializer, fdManager, portability]
---
# fdManager: remove C++20 designated initializer incompatible with MSVC before C++20

## Root Cause
A previous commit added `priv->pollfds.emplace_back(pollfd{.fd = ..., .events = ...})`
using C++20 designated initializers guarded by `#if __cplusplus >= 201100L`
(which should have been `202002L` for C++20). MSVC does not support designated
initializers in C++ mode before C++20, and the version check was wrong (201100L
corresponds to C++11, not C++20), so MSVC builds attempted to use the
designated-initializer form and failed.

## Symptoms
MSVC compilation error in `fdManager.cpp` when building with C++ standards
below C++20 (the typical MSVC default for EPICS builds).

## Fix
Remove the `#if __cplusplus >= 201100L` branch entirely and always use the
portable struct-initialization form:
```cpp
struct pollfd pollfd;
pollfd.fd = iter->getFD();
pollfd.events = WIN_POLLEVENT_FILTER(PollEvents[iter->getType()]);
pollfd.revents = 0;
priv->pollfds.push_back(pollfd);
```

## Rust Applicability
`eliminated` — Rust's struct initialization is always by-name (`Foo { field: val }`),
which is both safe and works in all editions. tokio replaces fdManager. No audit needed.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdManager::process` — C++20 designated initializer with wrong version guard
