# SKIP Commits Analysis — chunk_13

## 290a268 — N/A — add Value::lookup()
**Reason**: Value API addition; no protocol change.

## bb7ac1e — N/A — rework iteration, extend to Union
**Reason**: Value iteration API enhancement; pva-rs uses different patterns.

## 2d799c2 — N/A — add Value::nmembers()
**Reason**: Field count utility API; no protocol change.

## 3a74785 — N/A — fixup Value::Iterable
**Reason**: C++ API const-correctness fix; pva-rs has no equivalent.

## b9e025a — N/A — Value::compareType() -> Value::equalType()
**Reason**: API rename; internal logic only.

## 6dbd7f5 — N/A — NTScalar explicit ctor
**Reason**: C++ constructor safety; no behavior change.

## 5e05249 — N/A — Allow ID for StructA/UnionA
**Reason**: C++ template minor change; no behavior change.

## 783c53b — N/A — minor
**Reason**: Whitespace/comment cleanup.

## 7a9d5cb — N/A — WIN32: SetErrorMode(0)
**Reason**: Windows test infrastructure only; pva-rs uses Rust abstractions.

## b6ee231 — N/A — ensure osiSockAttach()
**Reason**: Socket init via EPICS platform layer; pva-rs uses tokio.

## d615689 — N/A — print array of quoted strings
**Reason**: Debug output formatting; no protocol change.

## 69efc4b — N/A — minor
**Reason**: Code cleanup in evhelper.

## 609768a — N/A — add client channel cache
**Reason**: Feature: 20s channel TTL cache; not a bug fix.

## 47790e5 — N/A — ResultWaiter::complete() only once
**Reason**: C++ async race fix (early-return pattern); pva-rs uses tokio.

## 6c98614 — N/A — minor
**Reason**: util.cpp cleanup.

## b2826a5 — N/A — shared_array(begin, end) use std::distance()
**Reason**: Refactor; pva-rs uses Vec.

## add9906 — N/A — thread_local -> ThreadEvent
**Reason**: Thread-local storage TLS-destructor workaround; pva-rs uses tokio.

## 549fd92 — N/A — change Server::listSource()
**Reason**: API signature change; different architecture.

## 1a46261 — N/A — minor
**Reason**: Whitespace cleanup.

## 88d23a0 — N/A — shared_array add iterator range ctor
**Reason**: API addition; pva-rs uses standard Vec.

## e741e41 — N/A — conn limit sequential messages
**Reason**: Performance fairness (max 4 msgs/conn); pva-rs uses tokio scheduler.

## 2a0dd0e — N/A — minor
**Reason**: Comment addition.

## 02f24a7 — N/A — Made it build on EPICS 3.14.12.6, all unit tests pass.
**Reason**: EPICS compat (trigger→signal); pva-rs doesn't link EPICS.

## 46bcf87 — N/A — replace epicsParse*() with std::sto*()
**Reason**: Refactor EPICS→C++ standard library parsing; not a bug fix.

## 6bb0a36 — N/A — MSVC is weird
**Reason**: MSVC compiler workaround; pva-rs doesn't target C++ MSVC.

## 6ffcf60 — N/A — 3.14 compat
**Reason**: EPICS 3.14 compatibility; pva-rs doesn't depend on EPICS base.

## c158bbd — N/A — doc
**Reason**: Documentation only.

## bc30f4c — N/A — add Value::ifMarked()
**Reason**: Marked-field helper API; no protocol change.

## 5d7de7 — N/A — workaround MSVC weirdness
**Reason**: MSVC compiler workaround; pva-rs doesn't target C++ MSVC.

## f1cc5a2 — N/A — minor thread_local
**Reason**: Code cleanup in evhelper/log.
