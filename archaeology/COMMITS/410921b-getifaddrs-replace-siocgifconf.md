---
sha: 410921b5efafcd9014a3da7dbfd5f47d220d5288
short_sha: 410921b
date: 2021-01-07
author: Michael Ritzert
category: network-routing
severity: high
rust_verdict: partial
audit_targets:
  - crate: ca-rs
    file: src/client/net_intf.rs
    function: discover_broadcast_addresses
tags: [network-interface, getifaddrs, SIOCGIFCONF, broadcast, multicast]
---
# Network interface enumeration: replace SIOCGIFCONF with getifaddrs

## Root Cause
The old `osdNetIntf.c` (default platform) used `SIOCGIFCONF` ioctl to
enumerate network interfaces for broadcast address discovery. `SIOCGIFCONF`
has several well-known limitations:
- On Linux, it does not report IPv6 addresses.
- It requires a fixed-size buffer guessed by the caller; if the buffer is
  too small, some interfaces are silently omitted.
- The ioctl is not thread-safe on some platforms.
- It does not enumerate point-to-point interface peers correctly.

On Linux, macOS, FreeBSD, and cygwin, `getifaddrs(3)` is available and
correctly handles all these cases, using a linked list of `ifaddrs` structs
dynamically allocated by the kernel.

## Symptoms
On hosts with many network interfaces or non-standard interface configurations
(e.g., VPN tunnels, virtual bridges, containers), `SIOCGIFCONF` could silently
miss interfaces. CA broadcast search packets would not be sent on the missed
interfaces. PVs on isolated network segments would not be found.

On macOS/Darwin and FreeBSD, multicast group membership also depends on
correctly enumerating interfaces; missed interfaces meant multicast CA
search also failed on those NICs.

## Fix
New `osdNetIfAddrs.c` implements `osiSockDiscoverBroadcastAddresses` and
`osiLocalAddress` using `getifaddrs`. Platforms that support it (Darwin,
Linux, FreeBSD, cygwin, iOS) now include `osdNetIfAddrs.c` instead of
`osdNetIntf.c`. The old `SIOCGIFCONF`-based code is preserved in
`osdNetIfConf.c` for platforms that need it (default/fallback).

## Rust Applicability
Partial. ca-rs uses the `if-addrs` or `nix` crate for interface enumeration,
which already calls `getifaddrs` on supported platforms. However, the
specific logic for filtering broadcast addresses (`IFF_BROADCAST`, matching
against `pMatchAddr`) and handling the `IFF_LOOPBACK` / `INADDR_LOOPBACK`
special cases must be verified. Also verify that the `osiLocalAddress` cached
once-lookup is correctly implemented (ca-rs should not re-enumerate
interfaces on every search unless the cache is invalidated).

## Audit Recommendation
Audit `ca-rs/src/client/net_intf.rs` or equivalent: (1) verify `getifaddrs`
or `if-addrs` is used (not `SIOCGIFCONF`), (2) check that IFF_BROADCAST
filtering preserves the match-address logic from `osiSockDiscoverBroadcastAddresses`,
(3) verify loopback exclusion is correct, (4) check that the local-address
cache uses a `once_cell` or `OnceLock`.

## C Locations
- `modules/libcom/src/osi/osdNetIfAddrs.c:osiSockDiscoverBroadcastAddresses` — new getifaddrs-based implementation
- `modules/libcom/src/osi/osdNetIfAddrs.c:osiLocalAddrOnce` — getifaddrs-based local addr cache
- `modules/libcom/src/osi/osdNetIfConf.c` — old SIOCGIFCONF code preserved for default/fallback platforms
- `modules/libcom/src/osi/os/Darwin/osdNetIntf.c` — redirects to osdNetIfAddrs.c
- `modules/libcom/src/osi/os/Linux/osdNetIntf.c` — redirects to osdNetIfAddrs.c
