---
sha: a9731b90f6c5469449dd33085f76aae1bae3fc78
short_sha: a9731b9
date: 2020-07-17
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [oom, calloc, dbpf, shell, deadlock]
---

# OOM guard in dbpf prevents shell freeze on calloc failure

## Root Cause
`dbpf()` used `dbCalloc()` (which calls `cantProceed()` on failure — logging + infinite spin) to allocate the string array buffer for array puts. If the system was out of memory, this caused the IOC shell thread to freeze indefinitely rather than returning an error.

## Symptoms
Running `dbpf` on an array PV under memory pressure would hang the IOC shell (and by extension, any CLI session) forever, since `cantProceed()` loops on failure instead of returning.

## Fix
Switched to plain `calloc()` and added an explicit NULL check, returning `-1` with an error message on failure. This commit's change was subsequently superseded by `d1491e0` which replaced the whole path with JSON-based parsing.

## Rust Applicability
Rust's allocator panics on OOM by default (or returns `Err` with the `try_reserve` API). There is no `cantProceed()` equivalent. This bug class is structurally eliminated — a Rust `Vec::with_capacity()` or `Box::new()` will either succeed or unwind. No audit needed.

## Audit Recommendation
None. Rust allocation failure semantics are handled by the runtime.

## C Locations
- `modules/database/src/ioc/db/dbTest.c:dbpf` — `dbCalloc` replaced with `calloc` + null check
