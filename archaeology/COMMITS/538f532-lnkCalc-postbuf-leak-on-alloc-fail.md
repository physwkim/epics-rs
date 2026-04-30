---
sha: 538f5321841276704b657f206a0f97a79178e722
short_sha: 538f532
date: 2020-02-12
author: Karl Vestin
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [memory-leak, lnkCalc, alloc-failure, free, json-link]
---

# lnkCalc: postbuf leaked when inbuf malloc fails

## Root Cause
In `lnkCalc_string` (the JSON link calculator string-value handler),
`postbuf` is allocated first, then `inbuf` is allocated. If the `inbuf`
`malloc` fails, the function returned `jlif_stop` immediately without
freeing `postbuf`, leaking that allocation. Additionally, in
`lnkCalc_integer` and `lnkCalc_double`, `errlogPrintf` was called after
`return jlif_stop`, so the log message was dead code and never printed.

## Symptoms
- Memory leak of `postbuf` every time a calculator link fails to allocate
  `inbuf` (OOM path).
- Silent failure: no log message printed for unexpected integer/double
  values in the link JSON.

## Fix
1. Add `free(postbuf)` before returning from the `!inbuf` failure path.
2. Move `errlogPrintf` before `return jlif_stop` in `lnkCalc_integer` and
   `lnkCalc_double` so the message is actually emitted.

## Rust Applicability
In Rust, allocations are RAII-managed (`Vec`, `String`, `Box`). When a Rust
allocation fails (OOM), the allocator panics by default; there is no fallible
path to leak an earlier allocation. Partial-allocation leaks of this form are
not possible in safe Rust. Eliminated.

## Audit Recommendation
None — eliminated by Rust's ownership model.

## C Locations
- `modules/database/src/std/link/lnkCalc.c:lnkCalc_string` — missing `free(postbuf)` on `inbuf` alloc failure
- `modules/database/src/std/link/lnkCalc.c:lnkCalc_integer` — `errlogPrintf` after `return` (dead code)
- `modules/database/src/std/link/lnkCalc.c:lnkCalc_double` — same dead log
