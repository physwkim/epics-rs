---
sha: 951b6acbbc102a5865592f671d2b410e96c5a4d3
short_sha: 951b6ac
date: 2020-08-03
author: Andrew Johnson
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/server/tcp.rs
    function: null
  - crate: ca-rs
    file: src/client/transport.rs
    function: null
  - crate: pva-rs
    file: src/server_native/tcp.rs
    function: null
tags: [TCP_NODELAY, Cygwin, latency, CA, portability]
---

# Cygwin missing TCP_NODELAY declaration causes CA build failure

## Root Cause
On Cygwin, `TCP_NODELAY` is defined in `<netinet/tcp.h>` but not pulled
in by `<sys/socket.h>` alone (unlike Linux/macOS). The CA server and
client code that calls `setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, ...)` 
references this constant without explicitly including `<netinet/tcp.h>`,
causing a compilation error on recent Cygwin toolchains.

## Symptoms
Build failure on Cygwin: `TCP_NODELAY undeclared`. CA runtime behavior
is correct on platforms where the include is transitively present; the
issue is purely a missing include causing compile-time failure.

## Fix
Added `#ifndef TCP_NODELAY / #include <netinet/tcp.h> / #endif` in
`cygwin32/osdSock.h`.

## Rust Applicability
In epics-ca-rs and epics-pva-rs, TCP socket creation uses tokio's
`TcpStream` which calls `set_nodelay(true)` via Rust's `std::net`
abstraction — the `TCP_NODELAY` constant is handled internally by the
Rust standard library and tokio. However, verify that `set_nodelay(true)`
is actually called on all accepted and connected sockets. A missing call
increases CA latency by batching small packets.

## Audit Recommendation
In `src/server/tcp.rs` (ca-rs): search for `set_nodelay`. Verify it is
called on every accepted socket (line 432 was found). In
`src/client/transport.rs` (ca-rs): verify `set_nodelay` is called after
`TcpStream::connect`. In `src/server_native/tcp.rs` (pva-rs): same check.
If any accept/connect path omits `set_nodelay(true)`, add it to prevent
Nagle-algorithm batching latency on small CA/PVA control messages.

## C Locations
- `modules/libcom/src/osi/os/cygwin32/osdSock.h` — conditional include of `<netinet/tcp.h>` for `TCP_NODELAY`
