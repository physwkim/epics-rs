---
sha: 931054d4fd96d0896bb74c2765c731bbbcef7295
short_sha: 931054d
date: 2019-09-17
author: Dirk Zimoch
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [windows, socket, printf, format-string, portability]
---

# logClient uses %d to print SOCKET which is UINT_PTR on Windows

## Root Cause
On Windows, `SOCKET` is defined as `UINT_PTR` (an unsigned pointer-sized
integer, 64 bits on x64). Passing it to `fprintf` with `%d` (which expects
`int`, 32 bits) is undefined behavior — the format string reads only 32 bits of
a 64-bit value, potentially printing a garbage or truncated socket "number".

The specific instance was in `logClientConnect`:

```c
fprintf(stderr, "%s:%d shutdown(%d,SHUT_RD) error ...", __FILE__, __LINE__,
        pClient->sock, sockErrBuf);
```

## Symptoms
On Windows 64-bit builds, the socket value printed in the error message would be
truncated or garbage. In theory, depending on calling convention and stack
layout, undefined behavior could corrupt the format string parsing state.

## Fix
Remove the socket value from the format string entirely (since it is not
meaningful for diagnostics on Windows anyway):

```c
fprintf(stderr, "%s:%d shutdown(sock,SHUT_RD) error was \"%s\"\n",
        __FILE__, __LINE__, sockErrBuf);
```

## Rust Applicability
Rust's `format!` / `println!` macros are type-checked at compile time; passing
a `RawSocket` (which is `u64` on Windows via `std::os::windows::io`) with `{}`
or `{:?}` prints correctly. Format string/type mismatches are compile errors.
Eliminated.

## Audit Recommendation
No action needed. Rust format macros enforce type safety.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientConnect` — `%d` used for SOCKET on Windows
