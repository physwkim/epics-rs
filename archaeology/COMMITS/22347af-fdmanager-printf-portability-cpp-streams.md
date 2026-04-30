---
sha: 22347af1708c1a118744e506b432cb1e985750bb
short_sha: 22347af
date: 2025-12-04
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [portability, printf, windows, fdmanager, diagnostics]
---
# fdManager::show uses C++ streams to avoid printf format portability issues

## Root Cause
`fdReg::show()` and `fdRegId::show()` used `printf` with a platform-guarded
`%"I"d` / `%"ll"d` format string to print the file descriptor value.
The Windows `"I"` size modifier for `SOCKET`/`INT_PTR` types is not
recognized by gcc/g++ `gnu_printf` format checkers on MinGW even when the
actual runtime is MSVC-compatible, producing spurious format-string warnings
that could mask real issues.

## Symptoms
Compiler format-string warnings on MinGW builds. No runtime breakage, but
the diagnostic noise could cause `-Werror` builds to fail.

## Fix
Replace all `printf` calls in `fdReg::show()` and `fdRegId::show()` with
`std::cout <<` streaming, which uses overloaded `operator<<` and is
type-safe regardless of platform integer width.

## Rust Applicability
`eliminated` — Rust's `println!`/`eprintln!` macros are type-checked at
compile time via format string parsing; there is no equivalent of the
`%I` vs `%ll` portability gap. The entire fdManager is replaced by tokio's
async I/O reactor.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdReg::show` — printf → std::cout
- `modules/libcom/src/fdmgr/fdManager.cpp:fdRegId::show` — printf → std::cout
