---
sha: 43bd5ee1c29806274a0ae8be852a944d9babdda1
short_sha: 43bd5ee
date: 2021-02-05
author: Michael Davidsaver
category: wire-protocol
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [socket, SOCK_CLOEXEC, accept4, RTEMS, vxWorks, platform]
---
# SOCK_CLOEXEC/accept4 incorrectly enabled on RTEMS 5.1 and vxWorks

## Root Cause
Commit `cf3173b` added `SOCK_CLOEXEC`/`accept4()` support conditioned on
`#ifdef SOCK_CLOEXEC`. RTEMS 5.1 defines `SOCK_CLOEXEC` (via libbsd) but
does not implement `accept4()` — the call would fail or call the wrong
function. Furthermore, neither RTEMS nor vxWorks can call `execv()`, so
`O_CLOEXEC` serves no purpose: there are no child processes to inherit file
descriptors. The `HAVE_SOCK_CLOEXEC` macro gate was insufficient.

## Symptoms
On RTEMS 5.1 builds, `epicsSocketAccept` would call `accept4()` which either
doesn't exist (link error) or fails at runtime, breaking all TCP server
connections (CA server, CA repeater).

## Fix
Changed the conditional from `#ifdef SOCK_CLOEXEC` to:

```c
#if defined(SOCK_CLOEXEC) && !defined(__rtems__) && !defined(vxWorks)
```

Added comment: "neither RTEMS nor vxWorks can execv(), no point."
This ensures `HAVE_SOCK_CLOEXEC` is not defined on these platforms even if
their headers expose the constant.

## Rust Applicability
Rust's `std::net::TcpListener::accept()` and `tokio::net::TcpListener::accept()`
handle `O_CLOEXEC` transparently on Linux/macOS (via `SOCK_CLOEXEC` or
`F_SETFD`). On embedded Rust targets there is no `execv()` analog. This is
eliminated as a concern in Rust.

## Audit Recommendation
No audit needed. Platform-specific socket flag guard with no Rust analog.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c` — SOCK_CLOEXEC guard excludes RTEMS and vxWorks
