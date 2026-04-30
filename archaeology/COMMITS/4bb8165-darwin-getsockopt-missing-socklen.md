---
sha: 4bb81654d60a5d343aeee1aa1ac8276c9ab43f75
short_sha: 4bb8165
date: 2019-10-04
author: Dirk Zimoch
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [darwin, getsockopt, socklen_t, abi, socket]
---

# Darwin/iOS getsockopt SO_NWRITE missing socklen_t argument

## Root Cause
On POSIX systems `getsockopt(2)` takes five arguments:
`getsockopt(fd, level, optname, optval, optlen*)`. The Darwin/iOS implementation
of `epicsSocketUnsentCount` called:

```c
getsockopt(sock, SOL_SOCKET, SO_NWRITE, &unsent)
```

omitting the mandatory fifth argument `socklen_t *optlen`. On Darwin, `optlen`
is an in/out parameter; without it the call either fails with `EFAULT` (if the
stack value happens to be a bad pointer) or silently reads garbage into `unsent`.
This is undefined behavior — the call signature mismatch is a C API violation.

The same bug was present in the iOS port.

## Symptoms
`epicsSocketUnsentCount()` on macOS/iOS always returned -1 (getsockopt failed)
or returned an incorrect unsent byte count. The log client's backlog tracking
was therefore always wrong on Apple platforms, potentially causing messages to
be dropped or the buffer memmove to shift by the wrong amount.

## Fix
Add `socklen_t len = sizeof(unsent);` and pass `&len` as the fifth argument:

```c
socklen_t len = sizeof(unsent);
if (getsockopt(sock, SOL_SOCKET, SO_NWRITE, &unsent, &len) == 0)
    return unsent;
```

## Rust Applicability
Rust's `nix` and `socket2` crates always pass the correct signature for
`getsockopt`. When reading `SO_NWRITE` (TCP unsent bytes) via `socket2::Socket`,
the `socklen_t` argument is handled internally by the safe wrapper. Eliminated.

## Audit Recommendation
No action needed. If `ca-rs` or `base-rs` ever call raw libc `getsockopt`
directly (via `libc::getsockopt`), audit those call sites for the 5-argument
form. Currently tokio/socket2 abstractions handle this correctly.

## C Locations
- `modules/libcom/src/osi/os/Darwin/osdSockUnsentCount.c:epicsSocketUnsentCount` — missing 5th arg
- `modules/libcom/src/osi/os/iOS/osdSockUnsentCount.c:epicsSocketUnsentCount` — same bug, iOS port
