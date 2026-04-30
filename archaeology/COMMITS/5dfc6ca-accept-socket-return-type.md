---
sha: 5dfc6caf3c898b213c8458f4766152a1b5a3e477
short_sha: 5dfc6ca
date: 2024-02-01
author: Freddie Akeroyd
category: network-routing
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [socket, accept, type-mismatch, SOCKET, int]
---

# epicsSocketAccept returns SOCKET not int

## Root Cause
`epicsSocketAccept()` was declared and implemented to return `int` and take an `int` sock parameter, but the Windows `SOCKET` type is `UINT_PTR` (unsigned 64-bit on x64). Returning a SOCKET through `int` silently truncates the handle on 64-bit Windows, producing a corrupted or zero socket value. The `iocLogClient::insock` field was also `int` instead of `SOCKET`.

## Symptoms
On 64-bit Windows, accepted client sockets could be corrupted to invalid values, causing the iocLogServer to fail to communicate with newly accepted clients. The bug manifests only when the allocator returns a handle whose upper 32 bits are non-zero — relatively rare but non-deterministic.

## Fix
Changed `epicsSocketAccept()` signature on WIN32, POSIX, and vxWorks to accept `SOCKET` and return `SOCKET`. Updated `osiSock.h` declaration and `iocLogClient::insock` field to `SOCKET`.

## Rust Applicability
Eliminated. Rust's `std::net::TcpListener::accept()` and `tokio::net::TcpListener::accept()` return a typed `TcpStream`; there is no raw socket handle integer to truncate. The Rust type system prevents this class of error by construction.

## Audit Recommendation
No action needed in Rust code. If any `ca-rs` or `pva-rs` code uses `libc::accept()` directly (e.g., via unsafe FFI), confirm the return value is stored in a `libc::SOCKET`-equivalent type (`RawFd` on POSIX is `i32` which is correct there; Windows-specific code should use `SOCKET`).

## C Locations
- `modules/libcom/src/osi/osiSock.h:epicsSocketAccept` — declaration changed from `int` to `SOCKET`
- `modules/libcom/src/osi/os/WIN32/osdSock.c:epicsSocketAccept` — implementation fixed
- `modules/libcom/src/osi/os/posix/osdSock.c:epicsSocketAccept` — implementation fixed
- `modules/libcom/src/osi/os/vxWorks/osdSock.c:epicsSocketAccept` — implementation fixed
- `modules/libcom/src/log/iocLogServer.c:iocLogClient::insock` — field type fixed
