---
sha: d7635413410761a6866c6b39b5bbe8642a6f5643
short_sha: d763541
date: 2025-10-08
author: bsbevins
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/channel.rs
    function: host_minor_protocol
tags: [CA-protocol, minor-version, tcpiiu, channel-access, wire-protocol]
---
# CA client: expose server protocol minor version via ca_host_minor_protocol()

## Root Cause
The CA client tracked the server's protocol minor version in `tcpiiu::minorProtocolVersion`
(read from the server's `VERSION` reply during TCP handshake) but provided no
public API to retrieve it. The ca-nameserver and gateway tools needed to
report the connected server's CA protocol version to clients, but had no way
to read it from the client library.

Additionally, the base class `cacChannel::getHostMinorProtocol` and
`netiiu::getHostMinorProtocol` had no implementation, meaning calls on
disconnected or UDP channels would return garbage or crash.

## Symptoms
CA nameserver/gateway cannot report accurate server protocol versions.
Calling any `getHostMinorProtocol` method on a disconnected channel before
this fix returns an uninitialized value.

## Fix
Add `virtual unsigned getHostMinorProtocol(epicsGuard<epicsMutex>&) const`
to the `cacChannel` → `netiiu` → `nciu` / `tcpiiu` / `udpiiu` virtual
dispatch chain:
- `cacChannel`: returns `0u` (safe default).
- `netiiu`: returns `CA_UKN_MINOR_VERSION`.
- `tcpiiu::getHostMinorProtocol`: returns `this->minorProtocolVersion` (already
  populated from the wire handshake).
- `nciu::getHostMinorProtocol`: delegates to `this->piiu->getHostMinorProtocol`.
- `udpiiu::getHostMinorProtocol`: delegates to `netiiu::getHostMinorProtocol`.

New public C API: `LIBCA_API unsigned ca_host_minor_protocol(chid pChan)`.
Returns `CA_UKN_MINOR_VERSION` for disconnected channels.

## Rust Applicability
`applies` — ca-rs's `Channel` struct should expose the server's CA protocol
minor version, read from the `VERSION` message during TCP connect. If
ca-rs exposes a `host_name()` API, it should also expose
`host_minor_protocol()` returning `Option<u16>` (None = disconnected).
This is needed for any gateway or nameserver built on ca-rs.

## Audit Recommendation
Audit `ca-rs/src/client/channel.rs` (or `tcpiiu.rs`):
1. Verify the minor protocol version is stored after parsing the server's
   `VERSION` reply.
2. Add a public `host_minor_protocol() -> Option<u16>` method that returns
   `None` for disconnected channels and `Some(version)` once connected.

## C Locations
- `modules/ca/src/client/tcpiiu.cpp:tcpiiu::getHostMinorProtocol` — reads `minorProtocolVersion` under mutex
- `modules/ca/src/client/nciu.cpp:nciu::getHostMinorProtocol` — delegates to piiu
- `modules/ca/src/client/netiiu.cpp:netiiu::getHostMinorProtocol` — returns CA_UKN_MINOR_VERSION
- `modules/ca/src/client/oldChannelNotify.cpp:ca_host_minor_protocol` — new public API entry point
- `modules/ca/src/client/cadef.h` — new `ca_host_minor_protocol` declaration + `HAS_CA_HOST_MINOR_PROTOCOL` macro
