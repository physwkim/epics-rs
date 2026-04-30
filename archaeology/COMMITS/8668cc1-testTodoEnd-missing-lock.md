---
sha: 8668cc12671f1144ba8f957b6647efbfb5827bb3
short_sha: 8668cc1
date: 2020-02-14
author: Michael Davidsaver
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [race, test-infra, lock, epicsUnitTest, todo]
---

# testTodoEnd() nulls the todo pointer without holding the lock

## Root Cause
`epicsUnitTest.c::testTodoEnd()` originally set the module-global `todo`
pointer to `NULL` directly without calling `testTodoBegin(NULL)`.
`testTodoBegin` acquires the unit-test mutex before writing `todo`. The
unprotected write in `testTodoEnd` is therefore a data race relative to any
concurrent test assertion that reads `todo` under the lock.

## Symptoms
- Rare spurious test result misclassification (a failing assertion might be
  classified as a TODO-expected failure or vice versa) when multiple test
  threads issue assertions concurrently while `testTodoEnd` fires.

## Fix
Replace the bare `todo = NULL` assignment with a call to
`testTodoBegin(NULL)`, which acquires the lock before writing.

## Rust Applicability
This is EPICS C test infrastructure. The epics-rs project uses Rust's built-in
`#[test]` framework (or custom harnesses). Rust's type system enforces shared
state access through `Mutex`/`RwLock`, so a bare unprotected write to a shared
pointer is not expressible in safe Rust. Eliminated.

## Audit Recommendation
None — eliminated by Rust's type system.

## C Locations
- `modules/libcom/src/misc/epicsUnitTest.c:testTodoEnd` — bare write to `todo` without lock
