---
sha: f8df3473ab080bedb917640e12608064a4489c9f
short_sha: f8df347
date: 2020-08-25
author: Ralph Lange
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [strncat, off-by-one, WIN32, buffer-overflow, osdSock]
---

# Windows osdSock: strncat/strncpy missing -1 on buffer size

## Root Cause
`osiSockAttach()` on Windows called `strncat(title, ..., sizeof(title))`
and `strncpy(title, ..., sizeof(title))` without subtracting 1 from the
size. `strncat` appends up to N bytes plus a null terminator: passing
`sizeof(title)` as N means the null byte can be written one byte past
the end of the buffer. `strncpy` similarly can write exactly `sizeof`
bytes without ensuring null termination.

## Symptoms
One-byte stack buffer overflow on the Windows console title buffer during
`osiSockAttach()`. Most likely benign in practice (the byte past the
buffer is often another stack variable), but technically undefined
behavior and can cause a crash with strict stack guards.

## Fix
Changed both calls to use `sizeof(title)-1` and kept the explicit
`title[sizeof(title)-1] = '\0'` null-terminator assignment that was
already present.

## Rust Applicability
Rust string handling does not use null-terminated buffers or `strncat`.
This pattern is fully eliminated in any Rust socket initialization code.
No audit needed.

## Audit Recommendation
No audit needed — this is a C-specific null-terminated buffer hazard
with no Rust analog.

## C Locations
- `modules/libcom/src/osi/os/WIN32/osdSock.c:osiSockAttach` — `strncat`/`strncpy` size arguments corrected to `sizeof-1`
