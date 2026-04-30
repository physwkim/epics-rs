---
sha: 4d63e65b9d48a3cb000264544cc97d7211510fb2
short_sha: 4d63e65
date: 2025-01-20
author: Dirk Zimoch
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [poll, select, fd-limit, fdmanager, ca-gateway]
---

# fdManager: Replace select() with poll() to remove FD_SETSIZE limit on Linux

## Root Cause
The original `fdManager::process()` implementation used `select()`, which on
Linux is limited to monitoring at most `FD_SETSIZE` (1024) file descriptors.
The CA gateway regularly exceeds this limit in production deployments, causing
all channels beyond fd 1023 to be silently ignored — no readiness notification,
no data delivery.

## Symptoms
- CA gateway stops delivering data on channels after ~1024 active connections.
- No error or warning from the OS; `select()` simply ignores out-of-range fds.
- Applications calling `FD_SET` with fd >= FD_SETSIZE trigger undefined behavior
  (buffer overflow into adjacent stack memory).

## Fix
Introduced `FDMGR_USE_POLL` / `FDMGR_USE_SELECT` compile-time switches.
On Linux, `FDMGR_USE_POLL` is defined, replacing the `fd_set`-based
`select()` loop with a `std::vector<pollfd>` grown on demand. The poll path
builds the `pollfds` vector by iterating registered `fdReg` objects, calls
`poll()`, then scans `revents` to dispatch callbacks. The `select()` path
remains for platforms without `poll()` support.

## Rust Applicability
Eliminated. tokio uses `epoll` on Linux (no fd limit), `kqueue` on macOS, and
IOCP on Windows. The FD_SETSIZE constraint does not exist.

## Audit Recommendation
No action required.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdManager::process` — replaced select() with poll() under FDMGR_USE_POLL
- `modules/libcom/src/fdmgr/fdManager.h` — added FDMGR_USE_POLL/SELECT guards and pollfd vector
