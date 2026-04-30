---
sha: 5ae9cc6536a41337cc8272f3ea8a9bd6123a2742
short_sha: 5ae9cc6
date: 2025-07-10
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [dns, getaddrinfo, thread-safety, reentrant, hostname]
---

# Replace gethostbyname with reentrant getaddrinfo

## Root Cause
`gethostbyname()` and `gethostbyaddr()` are not thread-safe: they return a
pointer into a shared static buffer. POSIX marks them obsolete in favor of
`getaddrinfo()` / `getnameinfo()`, which are fully reentrant. In a
multi-threaded EPICS IOC or CA client, concurrent hostname lookups would race
on this buffer, producing silently corrupted `in_addr` values or stale host
names.

## Symptoms
- Intermittent connection failures to CA servers when multiple threads resolve
  host names simultaneously (e.g., during beacon storm processing).
- Host name cache returning the wrong IP on 64-bit multi-core systems where
  the lookup mutex was not applied consistently.

## Fix
The old `ipAddrToHostName()` / `hostToIPAddr()` implementations guarded by
`infoMutex` were replaced with new implementations using `getnameinfo()` and
`getaddrinfo()` respectively. The mutex and the old locked implementations are
compiled out under `USE_INFO` / `USE_BY` preprocessor guards. `getaddrinfo`
accepts `AF_INET`-only hints to match the old IPv4 behavior.

## Rust Applicability
Eliminated. Rust's standard-library DNS resolution (`ToSocketAddrs` /
`tokio::net::lookup_host`) uses `getaddrinfo` under the hood and is thread-safe
by design. No `gethostbyname` analog exists in the Rust codebase.

## Audit Recommendation
No action required. Confirm that any hand-rolled hostname lookups in ca-rs or
pva-rs go through `tokio::net::lookup_host` (or the synchronous `std::net`
equivalents) and never call libc `gethostbyname` via FFI.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c:hostToIPAddr` — replaced with getaddrinfo
- `modules/libcom/src/osi/os/posix/osdSock.c:ipAddrToHostName` — replaced with getnameinfo
