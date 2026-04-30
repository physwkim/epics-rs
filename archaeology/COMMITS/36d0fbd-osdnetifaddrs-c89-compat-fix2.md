---
sha: 36d0fbd7be85a2ab44b64733d69435ec8663effc
short_sha: 36d0fbd
date: 2021-02-08
author: Andrew Johnson
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [c89, compatibility, network-interface, getifaddrs, loop-variable]
---
# osdNetIfAddrs: C89 incompatible declaration in for-loop in osiLocalAddrOnce

## Root Cause
A second instance of a C99-style for-loop variable declaration
(`for (struct ifaddrs *ifa = ifaddr; ...)`) was left in `osiLocalAddrOnce`
after the first instance in `osiSockDiscoverBroadcastAddresses` was fixed in
commit `30e1431`. C89 requires all variable declarations to precede
statements in a block.

## Symptoms
Compile failure on C89-strict compilers (e.g., older GCC with `-std=c89` or
certain embedded toolchains). This would break network interface enumeration
in `osiLocalAddrOnce`, which populates the cached local-address list used by
broadcast discovery.

## Fix
Moved the `ifa` declaration to the top of the function alongside `ifaddr`:

```c
struct ifaddrs *ifaddr, *ifa;
```

and changed the for-loop to `for (ifa = ifaddr; ...)`.

## Rust Applicability
Rust does not have C89 constraints. Network interface enumeration in ca-rs
uses the `nix` or `if-addrs` crate, which has no such issue. Eliminated.

## Audit Recommendation
No audit needed. Compiler-compatibility fix with no logic change.

## C Locations
- `modules/libcom/src/osi/osdNetIfAddrs.c:osiLocalAddrOnce` — C99 for-loop variable declaration
