---
sha: 73cdea5517d625243ba149abf4a1368fbae8fe81
short_sha: 73cdea5
date: 2019-05-08
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/access_security.rs
    function: null
tags: [access-security, hostname, ip-address, rename, network-routing]
---
# rsrv/asLib: rename asUseIP→asCheckClientIP, ignore client hostname when set

## Root Cause
The variable `asUseIP` was not clearly named: it controlled whether the server
trusted the client-provided hostname string or used the actual IP address for
access control. The rename to `asCheckClientIP` makes the semantics obvious.

Additionally, `host_name_action()` in the CA server was previously only
conditionally skipping the client-provided hostname; the review confirmed the
logic is correct: when `asCheckClientIP=1`, the server ignores the `HOST_NAME`
command entirely and uses the IP it knows from the TCP connection.

## Symptoms
- No runtime bug. The soft concern is: if future CA server code checked
  `asCheckClientIP` inconsistently, a client could spoof its hostname to
  bypass access-security HAG checks.

## Fix
Rename `asUseIP` → `asCheckClientIP` in the header, implementation, and iocsh
variable registration. Confirm `host_name_action()` and `create_tcp_client()`
use the new name consistently.

## Rust Applicability
Partial. ca-rs server-side (`ca-rs/src/server/`) should implement
`asCheckClientIP` semantics: when IP-mode is active, derive the client hostname
string from the peer IP address (as `"%u.%u.%u.%u"`) rather than accepting
the `HOST_NAME` command payload.

## Audit Recommendation
In ca-rs server host_name command handler: if `asCheckClientIP` is enabled,
verify the handler ignores the client-provided name and the peer IP is used for
all AS checks.

## C Locations
- `modules/database/src/ioc/rsrv/camessage.c:host_name_action` — `asUseIP` → `asCheckClientIP`
- `modules/database/src/ioc/rsrv/caservertask.c:create_tcp_client` — `asUseIP` → `asCheckClientIP`
- `modules/libcom/src/as/asLib.h` — extern declaration renamed
- `modules/libcom/src/as/asLibRoutines.c` — definition renamed
