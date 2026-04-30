---
sha: c6247329ab562b66680c96e672909b8cae685ef8
short_sha: c624732
date: 2021-06-04
author: Andrew Johnson
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [socket, CLOEXEC, preprocessor, warning, portability]
---

# osdSock.c: SOCK_CLOEXEC Redefinition Warning When Not Supported

## Root Cause
`osdSock.c` conditionally defined `SOCK_CLOEXEC` as `(0)` for platforms that
lack the `SOCK_CLOEXEC` socket flag:

```c
#else
#  define SOCK_CLOEXEC (0)
```

On some platforms or compiler configurations, `SOCK_CLOEXEC` was already
defined by a system header (even when `HAVE_SOCK_CLOEXEC` was not set),
causing a macro redefinition warning treated as error.

## Symptoms
Compilation warning `-Wmacro-redefined` on some Linux/glibc configurations
where `SOCK_CLOEXEC` is defined in system headers but the detection logic
did not set `HAVE_SOCK_CLOEXEC`.

## Fix
Add `#undef SOCK_CLOEXEC` immediately before the fallback `#define SOCK_CLOEXEC (0)`.

## Rust Applicability
Eliminated. Rust does not use C preprocessor macros. `O_CLOEXEC`/`SOCK_CLOEXEC`
are handled transparently by tokio/mio on all platforms; file descriptors are
automatically marked `FD_CLOEXEC` when created by Rust standard library or
tokio sockets.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c` — added `#undef SOCK_CLOEXEC` before fallback define
