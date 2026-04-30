---
sha: 3d25756065fb0bb311321fcf226a1fad0a3e537c
short_sha: 3d25756
date: 2023-09-14
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [encapsulation, private-api, evSubscrip, opaque-type, ABI]
---

# Privatize evSubscrip struct behind EPICS_PRIVATE_API guard

## Root Cause
The `evSubscrip` struct was fully public in `dbChannel.h`, exposing internal
event subscription fields (`npend`, `callBackInProgress`, `useValque`, etc.)
to external consumers. External code could read or write these fields, making
it impossible to refactor the internals (as done in the companion double-cancel
fix) without breaking ABI/API compatibility.

## Symptoms
- No runtime bug directly — a build-time / ABI encapsulation gap.
- External code relying on `evSubscrip` field layout would silently break or
  behave incorrectly after the double-cancel refactor changed field semantics.

## Fix
Moved the `struct evSubscrip` body behind `#ifdef EPICS_PRIVATE_API`, leaving
only a forward declaration `struct evSubscrip;` and `typedef struct evSubscrip
evSubscrip;` visible publicly. Internal files (`dbEvent.c`, test) define
`EPICS_PRIVATE_API` before including the header.

## Rust Applicability
In Rust, struct fields are private by default unless `pub` is used. There is no
analog C-style opaque-typedef problem. The Rust `base-rs` equivalent of
`evSubscrip` (subscription state) should have all fields private to the module,
exposed only through a handle type. This is naturally enforced by the language.

## Audit Recommendation
None — eliminated by Rust's module privacy system.

## C Locations
- `modules/database/src/ioc/db/dbChannel.h:evSubscrip` — struct body gated on EPICS_PRIVATE_API
- `modules/database/src/ioc/db/dbEvent.c` — defines EPICS_PRIVATE_API
