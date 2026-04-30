---
sha: 97bf9171c6c08b4a02141c05dca33c2022a4b301
short_sha: 97bf917
date: 2020-02-12
author: hanlet
category: network-routing
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/repeater.rs
    function: ca_repeater
tags: [multicast, caRepeater, UDP, IP_ADD_MEMBERSHIP, beacon]
---

# caRepeater does not join multicast groups — misses multicast CA beacons

## Root Cause
The CA Repeater process binds a UDP socket to receive beacons and fan them
out to registered clients. Before this fix the repeater never called
`IP_ADD_MEMBERSHIP`, so it only received unicast and broadcast datagrams.
Sites using multicast CA beacon addresses (addresses in 224.0.0.0/4,
configured via `EPICS_CAS_BEACON_ADDR_LIST` or `EPICS_CA_ADDR_LIST`) would
silently drop all multicast beacons at the OS socket layer, causing CA
clients to miss beacon anomalies and fail to detect server restarts quickly.

## Symptoms
- Slow reconnection after IOC restart when using multicast beacon addresses.
- `ca_repeater` process never forwards multicast beacons to subscribed clients.
- No error message — silent delivery failure.

## Fix
After the repeater socket is created and bound, parse the beacon address list
(`EPICS_CAS_BEACON_ADDR_LIST` first, falling back to `EPICS_CA_ADDR_LIST`),
identify any multicast addresses (first octet 224–239), and call
`setsockopt(sock, IPPROTO_IP, IP_ADD_MEMBERSHIP, ...)` for each. Errors are
logged but do not abort the repeater.

## Rust Applicability
`ca-rs` implements the repeater functionality. If the Rust repeater joins a
UDP socket and processes beacon addresses, it must similarly call
`IP_ADD_MEMBERSHIP` (via `socket2::Socket::join_multicast_v4` or equivalent)
for each multicast address in the beacon list. Without this, Rust's ca-rs
repeater would silently miss multicast beacons on multicast-configured sites.

## Audit Recommendation
Audit `ca-rs/src/client/repeater.rs` (or equivalent beacon-listen path):
1. Confirm the repeater socket calls `join_multicast_v4` for every multicast
   address in the beacon address list.
2. Verify the address-range check (224.0.0.0–239.255.255.255) is correct.
3. Confirm errors from `join_multicast_v4` are logged rather than silently
   ignored.

## C Locations
- `modules/ca/src/client/repeater.cpp:ca_repeater` — IP_ADD_MEMBERSHIP loop added after socket bind
