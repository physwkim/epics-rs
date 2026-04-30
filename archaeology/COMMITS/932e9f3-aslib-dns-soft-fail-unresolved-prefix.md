---
sha: 932e9f3b21375e03641f9c1316b123e041ee2fe7
short_sha: 932e9f3
date: 2019-06-04
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/access_security.rs
    function: hag_add_host
tags: [dns, access-security, soft-fail, hostname, network-routing]
---
# asLib: soft-fail DNS lookup, store "unresolved:<host>" instead of aborting

## Root Cause
When `asCheckClientIP=1` (IP-based access security) and a hostname in the ACF
file could not be resolved by DNS, `asHagAddHost()` returned `S_asLib_noHag`
(a hard error), which caused ACF loading to abort. This meant a transient DNS
failure at IOC startup time could prevent the IOC from loading its access
security configuration entirely.

## Symptoms
- IOC fails to start access security if any HAG hostname is temporarily
  unresolvable (DNS server down, DHCP race, etc.).

## Fix
Instead of returning an error, store the tag `"unresolved:<hostname>"` in the
HAGNAME list. At runtime, client IP strings will never match this prefix, so
the host effectively gets no access (conservative safe-fail). An `errlogPrintf`
warning is emitted so operators know a hostname failed to resolve.

Also renames `asUseIP` → `asCheckClientIP` for clarity and adds proper
`tolower` normalization for the non-IP hostname path.

## Rust Applicability
Partial. If base-rs implements IP-based ACF access security with DNS resolution,
it should also soft-fail unresolvable hostnames rather than aborting ACF load.
The "store a sentinel that never matches" pattern should be replicated.

## Audit Recommendation
In `base-rs/src/server/access_security.rs` (or equivalent), if DNS resolution
of HAG entries is implemented: verify that resolution failure stores a
never-matching sentinel and logs a warning, rather than propagating an error
that aborts ACF loading.

## C Locations
- `modules/libcom/src/as/asLibRoutines.c:asHagAddHost` — soft-fail: store `"unresolved:<host>"` on DNS failure
- `modules/database/src/ioc/rsrv/camessage.c:host_name_action` — `asUseIP` → `asCheckClientIP`
- `modules/database/src/ioc/rsrv/caservertask.c:create_tcp_client` — `asUseIP` → `asCheckClientIP`
