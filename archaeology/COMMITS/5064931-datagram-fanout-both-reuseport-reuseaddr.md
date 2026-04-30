---
sha: 5064931aa6e54481832951b4f27a982c5003233d
short_sha: 5064931
date: 2020-02-05
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: bind_repeater_socket
  - crate: ca-rs
    file: src/server/udp.rs
    function: bind_server_socket
tags: [socket, SO_REUSEPORT, SO_REUSEADDR, datagram-fanout, Linux]
---

# Datagram fanout socket: must set both SO_REUSEPORT and SO_REUSEADDR on Linux

## Root Cause
On Linux, `SO_REUSEPORT` and `SO_REUSEADDR` are not equivalent for UDP
datagram fanout. `SO_REUSEPORT` allows multiple sockets bound to the same
address to coexist only with other `SO_REUSEPORT` sockets; `SO_REUSEADDR`
similarly only shares with `SO_REUSEADDR` sockets. A socket that sets only
one option cannot rebind a port held by a socket that set the other. Setting
**both** options allows full sharing with any combination of existing
listeners. The previous code used only one (`SO_REUSEPORT` if available,
otherwise `SO_REUSEADDR`), which could fail to rebind in mixed-option
scenarios (e.g., the caRepeater and a CA server both binding the same UDP
broadcast port).

## Symptoms
- `bind()` failure on Linux when one socket has `SO_REUSEPORT` and the
  existing socket has only `SO_REUSEADDR`, or vice versa.
- caRepeater or CA server unable to start because another process already
  holds the CA UDP port with a different reuse option.

## Fix
Set both `SO_REUSEPORT` (if defined) **and** `SO_REUSEADDR` unconditionally.
Refactored into a `setfanout()` helper to reduce duplication.

## Rust Applicability
In ca-rs, any UDP socket that needs datagram fanout (the repeater listen
socket, the CA server UDP socket) must call both `socket.set_reuse_port(true)`
**and** `socket.set_reuse_address(true)` via `socket2::Socket`. Calling only
one may silently fail to bind on Linux when another process holds the port
with the other option.

## Audit Recommendation
Audit all UDP socket creation paths in ca-rs that call
`set_reuse_port` or `set_reuse_address`:
1. Confirm **both** are called, not just one.
2. Check the order: set options before `bind()`.
3. Verify on macOS/BSD (where `SO_REUSEPORT` semantics differ from Linux)
   the behaviour is still correct.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSockAddrReuse.cpp:epicsSocketEnableAddressUseForDatagramFanout` — set both SO_REUSEPORT and SO_REUSEADDR
