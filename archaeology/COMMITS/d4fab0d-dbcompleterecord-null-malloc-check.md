---
sha: d4fab0d20e9b45c94859c2085cbd761446d4bd79
short_sha: d4fab0d
date: 2023-03-12
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [null-check, malloc, oom, tab-completion, iocsh]
---
# dbCompleteRecord Missing NULL Check After malloc for Completion Array

## Root Cause
`dbCompleteRecord.cpp` builds an array of tab-completion suggestions for the
iocsh. After `malloc(sizeof(*ret) * (2u + suggestions.size()))`, the code
immediately wrote to `ret[0]`, `ret[n]`, etc. without checking whether
`malloc` returned `NULL`. On memory exhaustion, this is a null-pointer
dereference and crash.

## Symptoms
- `dbCompleteRecord()` crashes with a segfault on OOM when the completion
  array allocation fails.
- Only triggered under memory pressure; tab-completion in iocsh crashes
  instead of returning no suggestions.

## Fix
Wrap all array writes in `if(ret) { ... }`. If `malloc` returns `NULL`,
the function returns `NULL` to the readline completion handler, which
treats it as "no completions" — a safe degradation.

## Rust Applicability
`eliminated` — Rust allocation panics on OOM (by default) or returns
`Err` via `try_reserve()`. Either way, writing to a null pointer is
impossible. If base-rs implements iocsh tab-completion, allocation failure
is handled by the standard allocator path.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/database/src/ioc/dbStatic/dbCompleteRecord.cpp:dbCompleteRecord` — unguarded write after `malloc`
