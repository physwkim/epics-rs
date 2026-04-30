---
sha: ec5badc737d06fba9cbbadcceebb7a4dbdab741f
short_sha: ec5badc
date: 2025-01-29
author: Dirk Zimoch
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [poll, darwin, macos, fdmanager, socket-readiness]
---

# fdManager: Enable poll() on Darwin (macOS) to remove FD_SETSIZE limit

## Root Cause
The `FDMGR_USE_POLL` compile-time switch was only activated for Linux, Windows
Vista+, and RTEMS. macOS (Darwin) was left using the `select()`-based path,
which is limited to `FD_SETSIZE` file descriptors (typically 1024). On macOS,
applications like the CA gateway that open thousands of sockets would silently
stop monitoring channels beyond FD 1023.

## Symptoms
- CA gateway on macOS stops monitoring channels after ~1024 active connections.
- No error reported; `select()` silently ignores fds beyond FD_SETSIZE.

## Fix
Added `defined(darwin)` to the preprocessor condition that selects
`FDMGR_USE_POLL`, enabling `poll()` on macOS builds. macOS `poll()` is fully
POSIX-compliant and has no file descriptor count limit.

## Rust Applicability
Eliminated. tokio uses `kqueue` on macOS and `epoll` on Linux, both of which
have no practical fd limit. The `select()`/FD_SETSIZE constraint does not exist
in tokio-based code.

## Audit Recommendation
No action required.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp` — FDMGR_USE_POLL macro condition extended to include Darwin
