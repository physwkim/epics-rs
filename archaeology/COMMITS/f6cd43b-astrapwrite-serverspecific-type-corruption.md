---
sha: f6cd43b7cc88fd6f26b266dc0290458614c43b9a
short_sha: f6cd43b
date: 2024-06-11
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [asTrapWrite, serverSpecific, dbChannel, pfield, type-safety]
---
# asTrapWrite serverSpecific typed as void* allows callback to corrupt pfield

## Root Cause
`asTrapWriteMessage.serverSpecific` was typed as `void *`, while in
practice it always pointed to a `dbChannel`.  Listener callbacks called
via `asTrapWriteBeforeWithData` and `asTrapWriteAfterWrite` received the
opaque pointer and could (by accident or design) modify `dbAddr::pfield`
through it.  Because `pfield` is set to the specific record field being
written before the callback and must be restored afterward, an errant
callback that forgot to restore it would leave the field pointer corrupted
for subsequent database operations.

## Symptoms
Rare corruption of `dbAddr::pfield` after a trap-write listener callback
that cast `serverSpecific` to `dbAddr*` and modified `pfield`.  No crash
immediately, but subsequent `dbPut`/`dbGet` would write/read the wrong
field.

## Fix
Changed `serverSpecific` from `void *` to `struct dbChannel *`.
Added explicit save/restore of `chan->addr.pfield` around each listener
callback invocation in both `asTrapWriteBeforeWithData` and
`asTrapWriteAfterWrite`, so the field pointer is guaranteed to be correct
even if a callback forgets.

## Rust Applicability
Eliminated.  In Rust, a CA server trap-write callback mechanism would use
a typed trait object or a typed channel, making it impossible to pass the
wrong pointer.  The save/restore pattern is also unnecessary because Rust
closures capture by reference with lifetime guarantees.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/as/asTrapWrite.c:asTrapWriteBeforeWithData` — changed addr→chan type, added pfield save/restore around each listener call
- `modules/libcom/src/as/asTrapWrite.c:asTrapWriteAfterWrite` — added pfield save/restore around each listener call
- `modules/libcom/src/as/asTrapWrite.h:asTrapWriteMessage` — serverSpecific typed from void* to struct dbChannel*
