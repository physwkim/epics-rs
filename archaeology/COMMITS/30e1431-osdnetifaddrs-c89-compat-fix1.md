---
sha: 30e1431fb458659baef752bcd4fa84484ed057f0
short_sha: 30e1431
date: 2021-02-08
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [c89, compatibility, network-interface, getifaddrs, declaration]
---
# osdNetIfAddrs: C99 for-loop variable declaration breaks C89 compilers

## Root Cause
`osiSockDiscoverBroadcastAddresses` in `osdNetIfAddrs.c` used a C99-style
for-loop variable declaration (`for (struct ifaddrs *ifa = ifaddr; ...)`).
This file was introduced in commit `410921b` to replace `SIOCGIFCONF` with
`getifaddrs`. The C99 idiom is not accepted by C89-strict compilers.

## Symptoms
Compile failure on embedded targets (e.g., RTEMS with a C89-strict GCC) that
use the new `osdNetIfAddrs.c` network interface enumeration code. Broadcast
address discovery would be unavailable.

## Fix
Moved the `ifa` declaration before the loop body:

```c
struct ifaddrs *ifa;
...
for (ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
```

## Rust Applicability
Not applicable. Rust has no C89 constraints. Eliminated.

## Audit Recommendation
No audit needed. Pure compiler-compatibility fix.

## C Locations
- `modules/libcom/src/osi/osdNetIfAddrs.c:osiSockDiscoverBroadcastAddresses` — C99 for-loop declaration
