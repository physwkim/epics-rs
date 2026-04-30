---
sha: 76b9461ee82a65d7cdd28b487febe20df018cae2
short_sha: 76b9461
date: 2019-11-12
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [windows, socket, errno, error-codes, portability]
---

# logClient Windows SOCK_ECONNRESET vs ECONNRESET mismatch

## Root Cause
On Windows, socket error codes are not standard POSIX `errno` values. The code
used `errno == ECONNRESET || errno == EPIPE` to check whether a 0-byte `send()`
detected a broken connection, but on Windows the correct constants are
`SOCK_ECONNRESET` and `SOCK_EPIPE` (which EPICS maps to the WSA equivalents).
Using bare `ECONNRESET` / `EPIPE` on Windows always evaluates false, so the
broken-connection detection via 0-byte send never fired on Windows.

## Symptoms
On Windows, `logClientFlush` never detected a stale TCP connection via the
0-byte send probe. The backlog counter would remain non-zero but no reconnect
was triggered; log messages silently accumulated until the buffer was full.

## Fix
Replace `errno == ECONNRESET || errno == EPIPE` with
`errno == SOCK_ECONNRESET || errno == SOCK_EPIPE` in `logClientFlush`.
`SOCK_E*` macros are EPICS portability wrappers that expand to the correct
WSAGetLastError values on Windows and to standard `errno` names on POSIX.

## Rust Applicability
Rust's `std::io::Error` (and tokio) always use OS-native error kinds translated
into `ErrorKind::ConnectionReset` / `ErrorKind::BrokenPipe` etc., so there is
no platform-specific error-code mismatch in the Rust implementation. The
log-client equivalent in epics-rs uses tokio TCP streams which surface these as
typed `io::ErrorKind` variants. Eliminated.

## Audit Recommendation
No action needed; Rust's error abstraction prevents this class of bug entirely.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientFlush` — errno vs SOCK_ERRNO comparison
