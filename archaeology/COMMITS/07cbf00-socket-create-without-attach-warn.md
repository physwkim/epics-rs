---
sha: 07cbf00187bca98d74ad1d13094b82acb5912c0e
short_sha: 07cbf00
date: 2022-10-27
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [socket, osiSockAttach, portability, diagnostic, initialization]
---

# Warn when epicsSocketCreate called without osiSockAttach

## Root Cause
`osiSockAttach()` was a no-op on POSIX (it only does real work on Windows where WSAStartup must be called). This meant that code calling `epicsSocketCreate()` without first calling `osiSockAttach()` would succeed silently on POSIX but fail on Windows. There was no diagnostic to catch this portability hazard.

## Symptoms
EPICS code that works on Linux/macOS would fail on Windows with socket errors because the Winsock library was never initialized. The bug only surfaced during Windows port testing, making it hard to catch in POSIX-only CI.

## Fix
Added an atomic `nAttached` counter to the POSIX `osiSockAttach`/`osiSockRelease` pair. `epicsSocketCreate()` now checks `nAttached == 0` and emits a one-shot warning if so, flagging the portability problem at the usage site.

## Rust Applicability
Eliminated. Tokio and `std::net` handle platform socket initialization internally (including WSAStartup on Windows). There is no concept of `osiSockAttach` in Rust — socket creation is always safe to call. The portability hazard does not exist.

## Audit Recommendation
No action needed. Confirm that `ca-rs` and `pva-rs` do not call any raw Winsock APIs before Tokio runtime initialization.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c:osiSockAttach` — added `nAttached` atomic increment
- `modules/libcom/src/osi/os/posix/osdSock.c:osiSockRelease` — added `nAttached` atomic decrement
- `modules/libcom/src/osi/os/posix/osdSock.c:epicsSocketCreate` — added `!nAttached` warning
