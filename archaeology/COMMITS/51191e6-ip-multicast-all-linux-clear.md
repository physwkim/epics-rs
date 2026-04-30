---
sha: 51191e6155e16f88fabdf7eac860cff84a63c4e0
short_sha: 51191e6
date: 2021-08-04
author: Michael Davidsaver
category: network-routing
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: create_udp_socket
  - crate: pva-rs
    file: src/net/udp.rs
    function: bind_multicast_socket
tags: [multicast, linux, socket-option, network-routing, multi-homed]
---

# Linux IP_MULTICAST_ALL Default Causes Unintended Multicast Reception

## Root Cause
Linux 2.6.31+ introduced `IP_MULTICAST_ALL` (socket option 49), which defaults
to **1** (enabled). With this default, any UDP socket bound to `0.0.0.0` or a
multicast address receives **all** multicast packets delivered to any group
joined on any interface — regardless of which specific groups that socket has
joined. This is non-compliant with RFC behavior and causes spurious multicast
packet delivery on multi-homed hosts.

For EPICS, a CA or PVA socket that has only joined the beacon multicast group
on one NIC will also receive multicast packets meant for other groups on other
NICs, leading to false search replies, spurious beacon processing, or
unexpected traffic.

## Symptoms
On multi-homed Linux hosts, CA/PVA receive spurious multicast packets from
groups they never joined. This can manifest as phantom search responses,
duplicate beacons, or unexpected traffic visible in `tcpdump`.

## Fix
Call `setsockopt(sock, IPPROTO_IP, IP_MULTICAST_ALL, &val=0, sizeof(val))`
immediately after creating each `AF_INET`/`SOCK_DGRAM` socket on Linux. The
option is defined as `49` inline if the header is too old to include it.

## Rust Applicability
Directly applies. Rust `tokio::net::UdpSocket` and `std::net::UdpSocket` do
not set `IP_MULTICAST_ALL=0` automatically. The socket must be configured via
`setsockopt` (using the `nix` crate or a raw libc call) after binding on Linux.

## Audit Recommendation
In `ca-rs/src/client/udp.rs` and `pva-rs/src/net/udp.rs`, after `UdpSocket::bind()`,
add a Linux-only `setsockopt(IPPROTO_IP, IP_MULTICAST_ALL, 0)`. Use
`#[cfg(target_os = "linux")]`. Check whether the `socket2` crate exposes this;
if not, use `libc::setsockopt` directly. This matches the existing `pva-rs`
`IP_MULTICAST_ALL` audit finding already in kodex.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c:epicsSocketCreate` — added Linux-only `setsockopt(IP_MULTICAST_ALL, 0)` after socket creation
