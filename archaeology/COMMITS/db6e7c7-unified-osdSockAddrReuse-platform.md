---
sha: db6e7c7a22b73f70a8b93e2aa4b6fa505e0218a6
short_sha: db6e7c7
date: 2020-02-05
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [socket, SO_REUSEADDR, WIN32, platform, datagram-fanout]
---

# WIN32/Cygwin osdSockAddrReuse only set SO_REUSEADDR — platform divergence

## Root Cause
WIN32 and Cygwin had their own `osdSockAddrReuse.cpp` that called only
`SO_REUSEADDR` for `epicsSocketEnableAddressUseForDatagramFanout`. The POSIX
version had been updated to prefer `SO_REUSEPORT` (which is needed for BSD
and macOS for true datagram fanout). The diverged WIN32/Cygwin versions were
never updated, meaning datagram fanout on those platforms only set the weaker
`SO_REUSEADDR`. This was also misleading because the WIN32 version of
`epicsSocketEnableAddressReuseDuringTimeWaitState` was a no-op (correct on
WIN32 since the OS always allows immediate rebind), but the fanout function
was not a no-op.

## Symptoms
- On WIN32 with a multicast or fanout UDP setup, only `SO_REUSEADDR` is set,
  not `SO_REUSEPORT` (which may behave differently on some WIN32 versions).
- Platform inconsistency: different behaviour of
  `epicsSocketEnableAddressUseForDatagramFanout` across POSIX/WIN32/Cygwin.

## Fix
Delete the WIN32 and Cygwin specialisations. Move the `posix/` version to
`default/` so it applies to all platforms (the default implementation now
sets both `SO_REUSEPORT` where available and `SO_REUSEADDR`).

## Rust Applicability
In Rust (using `socket2` or `tokio`), socket option calls are made directly
via cross-platform APIs. `socket2::Socket::set_reuse_address` and
`set_reuse_port` are available. The Rust code itself ensures consistent
options are applied on all platforms by calling both where available. No
platform-specific divergence of the C kind exists. Eliminated.

## Audit Recommendation
None — eliminated by the unified Rust socket API.

## C Locations
- `modules/libcom/src/osi/os/WIN32/osdSockAddrReuse.cpp` — deleted (only SO_REUSEADDR, no-op TimeWait)
- `modules/libcom/src/osi/os/cygwin32/osdSockAddrReuse.cpp` — deleted (same)
- `modules/libcom/src/osi/os/posix/osdSockAddrReuse.cpp` → `default/` — unified impl
