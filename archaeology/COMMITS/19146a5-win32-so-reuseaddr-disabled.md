---
sha: 19146a597b42bc5f03aed1d97ccef56b4c4d0fac
short_sha: 19146a5
date: 2020-06-19
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: bind_udp_socket
  - crate: ca-rs
    file: src/server/listener.rs
    function: bind_tcp_listener
tags: [so-reuseaddr, windows, socket, port-reuse, security]
---

# WIN32: Disable SO_REUSEADDR for Windows sockets

## Root Cause
On POSIX systems `SO_REUSEADDR` allows binding to a port that is in `TIME_WAIT`
state — the intended use. On Windows, `SO_REUSEADDR` has a fundamentally
different (and dangerous) meaning: it allows *any* process to bind to the same
port simultaneously, enabling socket hijacking. Additionally, Windows servers
always permit immediate port reuse after exit regardless of `SO_REUSEADDR`, so
setting the option on Windows buys nothing useful while opening a security hole.

## Symptoms
- On Windows, setting `SO_REUSEADDR` allows a rogue process to bind to the CA
  server's TCP/UDP port and intercept or inject channel access traffic.
- No legitimate benefit: Windows already releases ports immediately at close.

## Fix
Wrapped the `setsockopt(SO_REUSEADDR)` call in
`epicsSocketEnableAddressReuseDuringTimeWaitState()` with `#ifndef _WIN32`,
making it a no-op on Windows. The semantics comment explaining the difference
was added inline.

## Rust Applicability
Applies. Any socket bind in ca-rs or pva-rs that calls
`socket2::Socket::set_reuse_address(true)` (or tokio's equivalent) on Windows
will have the same dangerous semantics. The Rust `socket2` crate passes the
flag through to `setsockopt` without OS-specific filtering.

## Audit Recommendation
In ca-rs and pva-rs, audit every `set_reuse_address(true)` call site. On
Windows builds, either skip the call entirely (use `#[cfg(not(windows))]`) or
use `SO_EXCLUSIVEADDRUSE` instead, which is the Windows-idiomatic way to
prevent port sharing. Check both TCP listener bind and UDP multicast bind paths.

## C Locations
- `modules/libcom/src/osi/os/default/osdSockAddrReuse.cpp:epicsSocketEnableAddressReuseDuringTimeWaitState` — SO_REUSEADDR disabled on Windows
