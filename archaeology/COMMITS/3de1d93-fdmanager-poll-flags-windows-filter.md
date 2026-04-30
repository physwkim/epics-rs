---
sha: 3de1d930598eaada44e4b0f25587fcda2663573e
short_sha: 3de1d93
date: 2025-01-30
author: Dirk Zimoch
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [poll, windows, socket-readiness, wsa-poll, fdmanager]
---

# fdManager: Filter poll() event flags rejected by WSAPoll on Windows

## Root Cause
`WSAPoll()` on Windows does not accept certain `events` flags that are valid
for POSIX `poll()`. Specifically, flags such as `POLLRDNORM`, `POLLRDBAND`,
`POLLWRBAND`, and `POLLHUP` are only valid in `revents` (returned by the
kernel) but must not be set in `events` (requested by the caller) under
Windows. Passing them as input causes `WSAPoll` to return `WSAEINVAL` or
silently ignore the sockets, breaking all CA channel readiness detection.

## Symptoms
- No CA connections established on Windows builds after the switch to `poll()`.
- `WSAPoll` returning error or zero-ready sockets even when data is available.

## Fix
Introduced `WIN_POLLEVENT_FILTER(ev)` macro that masks the `events` field to
`POLLIN | POLLOUT` on Windows and is a no-op on Linux/macOS/RTEMS. Applied
the macro at every `pollfd.events` assignment site in `fdManager::process()`.
Also corrected the equality check in the scan loop to use the filtered value
when verifying which `pollfd` entry corresponds to a given file descriptor.

## Rust Applicability
Eliminated. `ca-rs` and `pva-rs` use `tokio` for all I/O readiness, which
internally selects the correct OS primitive (epoll/kqueue/IOCP) per platform.
The `fdManager` poll-flag filtering problem does not arise in tokio-based code.

## Audit Recommendation
No action required. Ensure no hand-rolled `libc::poll` / `WSAPoll` calls exist
in the codebase; if cross-platform poll is ever needed, use tokio's polling
abstraction.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdManager::process` — added WIN_POLLEVENT_FILTER macro at pollfd.events assignment
