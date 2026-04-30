---
sha: 716f102dd6d017a1e207fb4465db8c7d6c7746b0
short_sha: 716f102
date: 2018-06-15
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [rsrv, accept, getpeername, peer-address, socket]
---
# rsrv: getpeername() called after socket operations instead of using accept() result

## Root Cause
`req_server()` called `epicsSocketAccept()` with a bare `struct sockaddr` buffer, then passed the new socket to `create_tcp_client()`, which called `getpeername()` to retrieve the peer address. This is redundant: `accept()` already returns the peer address in its second argument. More critically, `getpeername()` could fail (returning an error and logging "peer address fetch failed") and cause the client to be destroyed, even though the connection was valid — this can happen transiently on some OS implementations when the socket is briefly in an intermediate state.

Additionally, the code did not validate `sa_family` or `addLen` after `accept()`, so it could silently accept non-IPv4 connections and pass a truncated address structure.

## Symptoms
- Rare "CAS: peer address fetch failed" log followed by connection teardown, causing CA clients to reconnect unnecessarily.
- On some platforms, non-IPv4 sockets (e.g., IPv6) could be accepted and cause downstream address misinterpretation.

## Fix
- Change the `accept()` buffer from `struct sockaddr` to `osiSockAddr` (the union type).
- Pass the already-populated `sockAddr` directly to `create_tcp_client()` as a parameter.
- Validate `sa_family == AF_INET` and `addLen >= sizeof(sockAddr.ia)` immediately after `accept()`, rejecting invalid results early.
- Remove the redundant `getpeername()` call from `create_tcp_client()`.
Commit `716f102`.

## Rust Applicability
In Rust's async CA server (ca-rs), `TcpListener::accept()` returns a `TcpStream` + `SocketAddr` directly — the peer address is available immediately without a separate `getpeername()` call. This specific bug is eliminated by the Rust std/tokio API design.

## Audit Recommendation
No audit needed — tokio's `TcpListener::accept()` returns `(TcpStream, SocketAddr)` in a single call. Verify ca-rs server accept loop does not call `peer_addr()` separately when the accept result already contains the address.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:req_server` — accept() buffer type + validation
- `modules/database/src/ioc/rsrv/caservertask.c:create_tcp_client` — redundant getpeername() call
- `modules/database/src/ioc/rsrv/server.h` — function signature change
