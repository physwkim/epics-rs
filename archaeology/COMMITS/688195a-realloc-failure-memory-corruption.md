---
sha: 688195a2734f9990641707bd122a19c7f9dfe697
short_sha: 688195a
date: 2023-05-26
author: Ralph Lange
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [realloc, memory-corruption, oom, null-pointer, macEnv]
---
# realloc() Failure in macDefExpand Silently Loses Original Buffer

## Root Cause
`macEnv.c:macDefExpand()` calls `realloc(dest, n)` to shrink the expansion
buffer when more than 20 bytes are unused. The return value of `realloc`
was assigned directly back to `dest`:

```c
dest = realloc(dest, n);
```

When `realloc()` fails, it returns `NULL` while the original allocation
remains valid at the old address. Assigning `NULL` to `dest` both:
1. Loses the pointer to the original buffer (memory leak).
2. Causes a subsequent `free(dest)` at `done:` to call `free(NULL)` — a
   no-op in C — so the original buffer is never freed.

Additionally, any code path that uses `dest` after the `NULL` assignment
would dereference a null pointer.

## Symptoms
- OOM during the shrink realloc causes the function to return a `NULL` result
  pointer, making the caller treat expansion as failed.
- The original allocation is leaked (never freed).
- If the caller tries to use the returned buffer without checking for NULL,
  the process crashes.

## Fix
Introduce a `newdest` temporary; only assign to `dest` if `realloc`
succeeds. If `realloc` fails, the original `dest` is preserved and freed
normally. This is the standard safe-realloc pattern.

## Rust Applicability
`eliminated` — Rust allocation APIs (`Vec::shrink_to_fit`, `String`,
`Box`) panic or return `Err` on allocation failure; there is no way to
silently lose a pointer on realloc failure. epics-rs uses `String`/`Vec`
for macro expansion.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/macLib/macEnv.c:macDefExpand` — `dest = realloc(dest, n)` loses pointer on OOM
