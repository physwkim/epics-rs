---
sha: d691acc001315b8c2022920da24de66a94287f65
short_sha: d691acc
date: 2023-05-26
author: Ralph Lange
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [uninitialized, local-variable, catime, benchmark, undefined-behavior]
---
# Uninitialized Local Variable inlineIter in CA Timer Benchmark

## Root Cause
`catime.c:timeIt()` declared `unsigned inlineIter` without initializing it.
The variable is passed by pointer to `(*pfunc)(pItems, iterations, &inlineIter)`
where the called function is expected to set it. However, if the function
does not set it (or if the compiler does not initialize the stack slot to
zero), the subsequent use of `inlineIter` reads an indeterminate value —
undefined behavior per the C standard.

Found by static analysis (cppcheck / SonarQube).

## Symptoms
- Reading `inlineIter` before the callback assigns it yields garbage values
  that could affect timing statistics printed by the benchmark.
- Undefined behavior: the compiler is permitted to assume the uninitialized
  read does not happen, which can lead to surprising optimizations.

## Fix
Initialize `inlineIter = 0` at the point of declaration.

## Rust Applicability
`eliminated` — Rust requires all local variables to be initialized before
use; the compiler rejects `let x: u32; use(x);` with a compile-time error.
The `catime.c` benchmark has no direct Rust equivalent in ca-rs.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/ca/src/client/catime.c:timeIt` — `unsigned inlineIter` declared without initializer
