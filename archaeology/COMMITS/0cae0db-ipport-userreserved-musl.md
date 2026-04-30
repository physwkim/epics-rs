---
sha: 0cae0db98b1763b55974c92931a86fde6796edb0
short_sha: 0cae0db
date: 2020-07-03
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [musl, IPPORT_USERRESERVED, portability, Linux, CA-port]
---

# musl libc missing IPPORT_USERRESERVED breaks CA port binding

## Root Cause
`IPPORT_USERRESERVED` (value 5000) is the POSIX/BSD constant used to
determine the start of the user-assignable port range. musl libc omits
this constant, so any code that uses `IPPORT_USERRESERVED` for CA
ephemeral port selection fails to compile on musl-based Linux systems
(Alpine Linux, statically linked deployments).

## Symptoms
Compilation failure on musl-based Linux targets with
`undefined identifier IPPORT_USERRESERVED`. CA cannot bind its default
repeater/client port range on these platforms.

## Fix
Added a `#ifndef IPPORT_USERRESERVED / #define IPPORT_USERRESERVED 5000`
compatibility shim in the Linux `osdSock.h` header.

## Rust Applicability
Rust's `std::net` / tokio does not use `IPPORT_USERRESERVED`. Port
selection in epics-ca-rs uses hard-coded constants or config values
rather than OS headers. This is eliminated in Rust.

## Audit Recommendation
No audit needed — pure C portability fix with no Rust analog.

## C Locations
- `modules/libcom/src/osi/os/Linux/osdSock.h` — added `#ifndef IPPORT_USERRESERVED` compat define
