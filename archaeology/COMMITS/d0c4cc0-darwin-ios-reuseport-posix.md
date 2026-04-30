---
sha: d0c4cc0cec8d31b88ee480d2d7f007f17cf0cdbf
short_sha: d0c4cc0
date: 2020-01-12
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [Darwin, iOS, SO_REUSEPORT, platform, socket]
---

# Darwin/iOS osdSockAddrReuse set only SO_REUSEPORT — not SO_REUSEADDR

## Root Cause
Darwin and iOS had separate `osdSockAddrReuse.cpp` files that called only
`SO_REUSEPORT` for `epicsSocketEnableAddressUseForDatagramFanout`. After the
POSIX default implementation was updated to prefer `SO_REUSEPORT` where
defined, the Darwin/iOS specialisations became redundant — and since the
POSIX version now handles `SO_REUSEPORT` correctly, the platform-specific
files were only adding maintenance burden without adding value.

## Symptoms
- Minor: Darwin/iOS used only `SO_REUSEPORT`, the updated POSIX default uses
  both; previously no practical issue since Darwin defines `SO_REUSEPORT`.
- The specialisation prevented picking up future fixes in the default impl.

## Fix
Delete the Darwin and iOS specialisations. They fall through to the updated
POSIX `default/osdSockAddrReuse.cpp` which sets both `SO_REUSEPORT` and
`SO_REUSEADDR`.

## Rust Applicability
In Rust, `socket2` already abstracts platform differences; the same
`set_reuse_port()` / `set_reuse_address()` calls work on Darwin and Linux.
No platform-specific code is needed. Eliminated.

## Audit Recommendation
None — eliminated by the unified Rust socket API.

## C Locations
- `modules/libcom/src/osi/os/Darwin/osdSockAddrReuse.cpp` — deleted (only SO_REUSEPORT)
- `modules/libcom/src/osi/os/iOS/osdSockAddrReuse.cpp` — deleted (only SO_REUSEPORT)
