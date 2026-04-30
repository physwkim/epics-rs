---
sha: 8ff7658291f9ad37d6578a41cca255181247a158
short_sha: 8ff7658
date: 2019-09-25
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [windows, winsock, initialization, aslib, lifecycle]
---

# asLib missing osiSockAttach before aToIPAddr on Windows

## Root Cause
On Windows, Winsock must be initialized via `WSAStartup` before any socket API
call. EPICS wraps this in `osiSockAttach()`. The Access Security library
(`asLib`) calls `aToIPAddr()` (which uses `inet_addr` or `getaddrinfo`) during
ACF file parsing inside `asInitialize`. `asInitializeOnce` (the `epicsCallOnce`
callback) did not call `osiSockAttach()` before acquiring the mutex and
proceeding to parse, so on Windows the socket subsystem was uninitialized when
`aToIPAddr` ran, causing `WSANOTINITIALISED` errors or crashes.

## Symptoms
On Windows IOCs using access security (ACF files with HOST rules that require
IP address resolution), `asInitialize` could fail or crash because Winsock
was not yet initialized at the point `aToIPAddr` was called during parsing.

## Fix
Add `osiSockAttach()` as the first call inside `asInitializeOnce`, before the
mutex creation, ensuring Winsock is initialized before any IP address parsing
occurs.

## Rust Applicability
Rust on Windows uses the `socket2` / `tokio` crates which handle Winsock
initialization internally (typically via the `winapi` or `windows-sys` crates
that call `WSAStartup` in their initialization path). No explicit
`osiSockAttach` equivalent is needed; Rust runtimes handle this automatically.
Eliminated.

## Audit Recommendation
No action needed. Verify that access-security IP-resolution in `base-rs`
(if implemented) happens after the tokio runtime is started, which guarantees
Winsock is up.

## C Locations
- `modules/libcom/src/as/asLibRoutines.c:asInitializeOnce` — osiSockAttach missing before aToIPAddr
