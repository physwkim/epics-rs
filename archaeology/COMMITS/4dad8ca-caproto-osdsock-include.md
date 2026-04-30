---
sha: 4dad8ca50351cec1a6addcc8edb89f4ba46ee818
short_sha: 4dad8ca
date: 2021-06-25
author: Freddie Akeroyd
category: wire-protocol
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [caProto, include, portability, LIBCOM_API, build]
---

# caProto.h Uses osdSock.h Instead of Public osiSock.h

## Root Cause
`caProto.h` included `<osdSock.h>` (the internal, OS-specific header) instead
of `<osiSock.h>` (the public portability wrapper). External modules (e.g.,
PCAS) that include only `<caProto.h>` — without going through the full EPICS
build system — would not have `osdSock.h` in their include path, causing
compilation failure with `LIBCOM_API` or `IPPORT_USERRESERVED` undefined.

## Symptoms
Compilation failure in the PCAS (Portable Channel Access Server) module:
`LIBCOM_API` undefined when building against a standalone EPICS base install
that exposes only public headers.

## Fix
Change `#include <osdSock.h>` to `#include <osiSock.h>` in `caProto.h`.

## Rust Applicability
Eliminated. In Rust, there are no platform-specific header split issues. The
CA wire protocol constants (`IPPORT_USERRESERVED`, port numbers) are defined
as Rust constants in `ca-rs/src/wire/protocol.rs` or similar, with no
include-path dependency.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/ca/src/client/caProto.h` — changed `#include <osdSock.h>` to `#include <osiSock.h>`
