---
sha: cd0e6a4f9a1d0e847282cbbba3486386f0dc3302
short_sha: cd0e6a4
date: 2021-02-05
author: Brendan Chandler
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/proto.rs
    function: null
tags: [CA-protocol, IPPORT_USERRESERVED, RTEMS, port-constant, include-order]
---
# caProto.h uses IPPORT_USERRESERVED without including its definition

## Root Cause
`caProto.h` defines `CA_SERVER_PORT` as `IPPORT_USERRESERVED + 56`, but
did not include `<osdSock.h>` (which defines `IPPORT_USERRESERVED`). This
created a header-inclusion order dependency: any translation unit that
included `caProto.h` before including socket headers would fail to compile
or use an undefined value for `IPPORT_USERRESERVED`. On RTEMS-kernel, the
`osdSock.h` for that platform did not define `IPPORT_USERRESERVED` at all.

## Symptoms
Compile error or silent use of `IPPORT_USERRESERVED` = 0 (implicit int)
on RTEMS-kernel targets, resulting in `CA_SERVER_PORT = 56` instead of
the correct `5056`. CA clients on RTEMS would connect to wrong port, failing
to discover or connect to CA servers.

On non-RTEMS platforms, the bug was latent but could surface if `caProto.h`
was included before socket headers in an unexpected compilation unit order.

## Fix
1. Added `#include <osdSock.h>` to `caProto.h` to make the dependency
   explicit and order-independent.
2. Added `#ifndef IPPORT_USERRESERVED / #define IPPORT_USERRESERVED 5000 /
   #endif` to `modules/libcom/src/osi/os/RTEMS-kernel/osdSock.h` to provide
   the constant on RTEMS-kernel where the BSD stack does not define it.

## Rust Applicability
Applies. In ca-rs, `CA_SERVER_PORT` (5064) and `CA_REPEATER_PORT` (5065)
must be defined as named constants without relying on `IPPORT_USERRESERVED`.
If any port constant is derived from `IPPORT_USERRESERVED` at compile time,
verify it resolves to 5000 (the IANA-assigned value). The correct CA ports
are `5056 + IPPORT_USERRESERVED` relative values; ensure they are defined
as literal `5064`/`5065` rather than computed from platform headers.

## Audit Recommendation
Audit `ca-rs/src/client/proto.rs` or constants file: confirm that
`CA_SERVER_PORT = 5064` and `CA_REPEATER_PORT = 5065` are hardcoded
constants, not derived from a platform-specific `IPPORT_USERRESERVED`.

## C Locations
- `modules/ca/src/client/caProto.h` — adds #include <osdSock.h> to fix dependency
- `modules/libcom/src/osi/os/RTEMS-kernel/osdSock.h` — adds IPPORT_USERRESERVED = 5000
