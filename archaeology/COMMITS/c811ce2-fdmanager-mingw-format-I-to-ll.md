---
sha: c811ce218e56695c24440d973fc7d1a2f4f6c1d8
short_sha: c811ce2
date: 2025-11-11
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [portability, mingw, printf-format, windows, fdmanager]
---
# fdManager: replace Windows %I printf modifier with portable %ll on MinGW

## Root Cause
`fdRegId::show()` used `printf("\tfd = %"` with `"I"` for Windows builds to
handle `SOCKET`/`INT_PTR` width. After the project switched to `gnu_printf`
format attribute on MinGW, the MSVC-specific `"I"` modifier is no longer
recognized by the format checker. `"ll"` is the portable 64-bit modifier
accepted by both MSVC and gcc/MinGW.

## Symptoms
Compiler format-string warning `-Wformat` on MinGW/Windows builds. No
runtime impact.

## Fix
Replace `"I"` with `"ll"` in the conditional `printf` format string:
```c
printf("\tfd = %"
#if defined(_WIN32)
    "ll"
#endif
    "d, type = %d\n", fd, type);
```

## Rust Applicability
`eliminated` — Rust format macros are type-checked at compile time with no
platform-specific format specifiers. tokio replaces fdManager entirely.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdRegId::show` — `%I` → `%ll` on Windows
