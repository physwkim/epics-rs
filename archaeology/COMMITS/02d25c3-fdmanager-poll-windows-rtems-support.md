---
sha: 02d25c3c48731b19955bbb530f48191e61afec1f
short_sha: 02d25c3
date: 2025-01-23
author: Dirk Zimoch
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [poll, windows, rtems, fdmanager, fd-limit]
---

# fdManager: Enable poll() on Windows Vista+ and RTEMS new-stack

## Root Cause
The `fdManager` used `select()` on Windows and RTEMS, limiting monitored
file descriptors to `FD_SETSIZE` (typically 64 on Windows, 1024 on RTEMS).
For CA gateway deployments this meant hard channel count limits that would
silently drop connections beyond the cap. Windows Vista+ provides `WSAPoll()`
and RTEMS with the BSD new network stack supports POSIX `poll()`, both of
which remove this limit.

## Symptoms
- CA gateway on Windows stops monitoring connections after 64 channels.
- RTEMS (new-stack) gateway silently drops channels beyond 1024.
- No error reported at the OS level; `select()` ignores fds beyond FD_SETSIZE.

## Fix
Extended the `FDMGR_USE_POLL` preprocessor guard to include
`_WIN32_WINNT >= 0x600` (Vista+) and `defined(__rtems__) && !defined(RTEMS_LEGACY_STACK)`.
On Windows, `poll` is `#define`d to `WSAPoll`. Cygwin is explicitly excluded
because its `poll()` emulates using `select()`. Also moved the `#include`
ordering so `poll.h` is not included on Windows (it uses the define instead).

## Rust Applicability
Eliminated. tokio selects IOCP on Windows and the appropriate kqueue/epoll
variant on other platforms, all without fd count limits. No `fdManager`
equivalent exists in the Rust codebase.

## Audit Recommendation
No action required.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp` — FDMGR_USE_POLL extended to Windows/RTEMS
- `modules/libcom/src/fdmgr/fdManager.h` — poll.h include guarded to exclude Windows
