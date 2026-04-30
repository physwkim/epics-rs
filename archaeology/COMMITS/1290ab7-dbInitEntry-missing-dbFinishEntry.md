---
sha: 1290ab7c6c30392f04058537cabee6a95d39f18c
short_sha: 1290ab7
date: 2019-11-24
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [lifecycle, dbEntry, leak, registry, RAII]
---

# registerRecordTypes: dbInitEntry called without matching dbFinishEntry — resource leak

## Root Cause
`registryCommon.c::registerRecordTypes` called `dbInitEntry(&dbEntry, pbase)`
inside a loop over record types and then called `sizeOffset(dbEntry.precordType)`.
However, `dbFinishEntry` was never called after each iteration, leaving the
`dbEntry` state (which may hold allocated cursors, pointers into the database
structure, or lock state) unreleased until the next `dbInitEntry` call
overwrites it. This is a lifecycle violation: `dbInitEntry`/`dbFinishEntry`
are an acquire/release pair, analogous to a mutex lock/unlock.

## Symptoms
- Resource leak (of whatever `dbInitEntry` acquires internally) for every
  record type registered except the last.
- Potential double-init corruption if `dbInitEntry` has internal state that
  assumes it starts clean.

## Fix
Add `dbFinishEntry(&dbEntry)` at the end of the loop body, after `sizeOffset`.

## Rust Applicability
In Rust, the `dbInitEntry`/`dbFinishEntry` pattern would be expressed as a
guard struct with `impl Drop { fn drop(&mut self) { dbFinishEntry(...) } }`,
or as a scoped block. Rust's RAII model makes it impossible to forget to call
the finish function because the compiler enforces Drop. Eliminated.

## Audit Recommendation
None — eliminated by Rust's RAII / Drop trait.

## C Locations
- `modules/database/src/ioc/registry/registryCommon.c:registerRecordTypes` — missing dbFinishEntry in loop
