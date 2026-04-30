---
sha: 65ef6e9d5946bff8dee1d140245b12f9073a9252
short_sha: 65ef6e9
date: 2020-01-12
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: bind_repeater_socket
tags: [socket, SO_REUSEPORT, BSD, datagram-fanout, multicast]
---

# POSIX datagram fanout: SO_REUSEADDR insufficient on BSD — need SO_REUSEPORT

## Root Cause
BSD derivatives (macOS, FreeBSD) require `SO_REUSEPORT` for multiple sockets
to bind the same UDP address. `SO_REUSEADDR` alone does not enable UDP
datagram fanout on BSD; only `SO_REUSEPORT` does. The previous implementation
always used `SO_REUSEADDR` on POSIX platforms, which silently failed to allow
fanout on BSD targets, preventing the caRepeater from co-existing with other
CA processes on macOS.

Additionally, RTEMS needed `__BSD_VISIBLE 1` defined before system headers to
expose `SO_REUSEPORT`.

## Symptoms
- `bind()` failure on macOS/FreeBSD for the second socket binding a CA UDP
  port when only `SO_REUSEADDR` is set.
- Multiple CA clients or the repeater fail to start on BSD platforms.

## Fix
Add `#ifdef SO_REUSEPORT` guard: if the platform defines `SO_REUSEPORT`,
use it; otherwise fall back to `SO_REUSEADDR`. Add `__BSD_VISIBLE 1` for RTEMS.
(A subsequent commit also sets both options simultaneously on Linux.)

## Rust Applicability
`socket2::Socket::set_reuse_port(true)` is the Rust equivalent. On BSD/macOS,
datagram fanout sockets must call `set_reuse_port(true)`. Note that
`set_reuse_port` is only available on Unix targets (not WIN32); the ca-rs
socket setup must conditionally call it with `#[cfg(unix)]`.

## Audit Recommendation
Audit UDP socket creation in ca-rs:
1. Confirm `set_reuse_port(true)` is called on all POSIX targets for fanout sockets.
2. Verify the `#[cfg(unix)]` guard is in place to avoid WIN32 compilation errors.
3. Cross-reference with 5064931 — both `set_reuse_port` and `set_reuse_address`
   should be set on Linux.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSockAddrReuse.cpp:epicsSocketEnableAddressUseForDatagramFanout` — prefer SO_REUSEPORT via X_REUSEUDP macro
