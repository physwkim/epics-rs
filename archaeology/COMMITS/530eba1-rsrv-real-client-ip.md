---
sha: 530eba133fd1fc3af91457bad73220cfcfbbe2cc
short_sha: 530eba1
date: 2018-06-16
author: Michael Davidsaver
category: network-routing
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/server/client.rs
    function: create_tcp_client
  - crate: base-rs
    file: src/server/database/access_security.rs
    function: hag_add_host
tags: [access-security, asUseIP, client-ip, hostname-spoofing, network-security]
---
# rsrv: use verified client IP address instead of client-supplied hostname

## Root Cause
The CA server's access security relied on the hostname string sent by the
client in the `CA_PROTO_HOST_NAME` message (`host_name_action`).  This
hostname was under the client's control — any client could claim to be
`"trusted-host.example.com"` regardless of its actual IP address.  The
access security library (`asLib`) stored and compared HAG (Host Access Group)
entries as case-folded hostname strings, so a spoofed hostname would pass HAG
checks.

## Symptoms
A malicious or misconfigured client could bypass HAG-based access restrictions
by sending a forged hostname in the CA handshake.  Symptom was typically
silent — no error logged — the incorrect host name simply matched (or
mismatched) the ACF rules.

## Fix
Introduced global flag `asUseIP` (default 0, backward-compatible).
When `asUseIP = 1`:
1. `asHagAddHost()` — resolves HAG hostnames to dotted-decimal IPv4 strings
   at ACF load time using `aToIPAddr()`.
2. `create_tcp_client()` — formats `client->pHostName` as
   `"%u.%u.%u.%u"` from the TCP peer address (from `accept(2)`), bypassing
   the client-provided name entirely.
3. `host_name_action()` — returns `RSRV_OK` immediately (ignores the
   client-supplied name) when `asUseIP` is set.
4. Registered `asUseIP` as an iocsh variable for runtime configuration.

## Rust Applicability
The CA server in `ca-rs` should support an equivalent `as_use_ip` mode.
`create_tcp_client` maps to `src/server/client.rs` — when `asUseIP`-mode
is active, derive the client's identity from `TcpStream::peer_addr()` rather
than the hostname in the CA handshake.  `hag_add_host` in access security
should resolve names to `IpAddr` at configuration load time when the flag is
set.

## Audit Recommendation
In `ca-rs/src/server/client.rs`, verify that the client's hostname used for
access-security evaluation is derived from the verified socket peer address
(not the CA `HOST_NAME` message) when IP-mode is active.  In
`base-rs/src/server/database/access_security.rs`, ensure HAG host entries
are resolved to `IpAddr` at ACF parse time when IP-mode is configured.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:create_tcp_client` — sets pHostName from peer IP when asUseIP
- `modules/database/src/ioc/rsrv/camessage.c:host_name_action` — ignores client-provided name when asUseIP
- `modules/libcom/src/as/asLibRoutines.c:asHagAddHost` — resolves hostnames to IPs at ACF load
- `modules/libcom/src/as/asLib.h` — declares asUseIP extern
- `modules/libcom/src/iocsh/libComRegister.c:libComRegister` — registers asUseIP iocsh variable
