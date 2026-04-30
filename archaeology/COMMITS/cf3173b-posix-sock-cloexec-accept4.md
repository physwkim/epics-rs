---
sha: cf3173b6f424121f4239e8cdfcbc944749425c87
short_sha: cf3173b
date: 2021-01-28
author: Michael Davidsaver
category: wire-protocol
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [socket, SOCK_CLOEXEC, accept4, O_CLOEXEC, fork-safety]
---
# POSIX sockets: O_CLOEXEC not set atomically on new sockets and accepted connections

## Root Cause
`epicsSocketCreate` called `socket(domain, type, protocol)` without
`SOCK_CLOEXEC`. After `fork()`, child processes would inherit all open EPICS
sockets, causing the child's close-on-exec semantics to be wrong: CA sockets
would linger in children (e.g., spawned by `system()` calls in IOC shell
scripts), consuming port resources and potentially interfering with CA
reconnection logic.

Similarly, `epicsSocketAccept` called plain `accept()`, which also does not
set `O_CLOEXEC` on the returned socket.

Between `socket()`/`accept()` and a subsequent `fcntl(fd, F_SETFD,
FD_CLOEXEC)` call there is a TOCTOU window where a concurrent `fork()` can
inherit the socket before the flag is set.

## Symptoms
After `fork()` (e.g., from IOC shell `system()` call), child processes inherit
CA and server sockets. If the child doesn't close them, the listening socket's
reference count prevents `SO_REUSEADDR` from taking effect on restart,
causing `bind()` to fail with `EADDRINUSE`.

## Fix
If `SOCK_CLOEXEC` is defined (Linux, FreeBSD, RTEMS-libbsd):
- `epicsSocketCreate`: passes `type | SOCK_CLOEXEC` to `socket()`.
- `epicsSocketAccept`: uses `accept4(sock, pAddr, addrlen, SOCK_CLOEXEC)`
  instead of `accept()`.

Falls back to `socket()`/`accept()` on platforms without `SOCK_CLOEXEC`.
The `F_SETFD` call is retained as paranoia even when `SOCK_CLOEXEC` is used.

Note: subsequently corrected in `43bd5ee` to exclude RTEMS/vxWorks which
define `SOCK_CLOEXEC` but cannot `execv()`.

## Rust Applicability
Rust `std::net` and `tokio::net` set `O_CLOEXEC` atomically on all newly
created sockets on Linux (via `SOCK_CLOEXEC` in socket2 crate). Eliminated
as a concern in Rust.

## Audit Recommendation
No audit needed. Rust socket creation handles `O_CLOEXEC` transparently.

## C Locations
- `modules/libcom/src/osi/os/posix/osdSock.c:epicsSocketCreate` — adds SOCK_CLOEXEC to socket() call
- `modules/libcom/src/osi/os/posix/osdSock.c:epicsSocketAccept` — uses accept4() with SOCK_CLOEXEC
