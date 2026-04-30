---
sha: e2d3b9a246d43495b5acaf3276d32de66b0db9b1
short_sha: e2d3b9a
date: 2021-06-28
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [socket, EMSGSIZE, cross-platform, UDP, error-handling]
---

# osiSockTest: Missing SOCK_EMSGSIZE Portability Definition

## Root Cause
The EPICS socket abstraction layer defines platform-specific error code aliases
(`SOCK_SHUTDOWN`, `SOCK_ENOTSOCK`, etc.) in per-OS `osdSock.h` headers.
`SOCK_EMSGSIZE` — used to detect "message too large" UDP errors — was not
defined in any of the OSI headers. The `osiSockTest` receiver thread used
`SOCKERRNO == EMSGSIZE` directly, which is non-portable (Windows uses
`WSAEMSGSIZE`; some embedded targets may differ).

The test was also missing a `SOCK_EINTR` continuation for interrupted `recvfrom`
calls, causing spurious test failures on signal-heavy platforms.

## Symptoms
On Windows, `osiSockTest` UDP receive loop does not compile (undefined
`WSAEMSGSIZE` via `EMSGSIZE`). On any platform, receipt of an oversized UDP
packet causes the receive loop to abort instead of discarding and continuing.

## Fix
Add `#define SOCK_EMSGSIZE <platform-value>` to all 10 `osdSock.h` headers.
In `osiSockTest.c`, replace the bare `EMSGSIZE` check with `SOCK_EMSGSIZE` and
add `SOCK_EINTR` continuation.

## Rust Applicability
Eliminated. In Rust, `tokio::net::UdpSocket::recv_from()` returns
`Err(io::Error)` whose `kind()` maps platform error codes to
`io::ErrorKind::WouldBlock`, `io::ErrorKind::Interrupted`, etc. There is no
need to define platform-specific error code aliases; `ErrorKind` is portable
across platforms. The UDP receive loop should match on `ErrorKind::Interrupted`
to continue and handle truncated/oversized datagrams via `recv_buf_size`.

## Audit Recommendation
No direct audit needed. In any Rust UDP receive loop (ca-rs udp.rs, pva-rs
udp.rs), confirm that `Err` variants with `ErrorKind::Interrupted` continue
rather than break the loop.

## C Locations
- `modules/libcom/src/osi/os/*/osdSock.h` (10 platforms) — added `#define SOCK_EMSGSIZE`
- `modules/libcom/test/osiSockTest.c:udpSockFanoutTestRx` — added `SOCK_EMSGSIZE` / `SOCK_EINTR` checks
