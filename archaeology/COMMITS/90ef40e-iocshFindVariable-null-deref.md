---
sha: 90ef40e62b3aedff41a3886845b5045027bff903
short_sha: 90ef40e
date: 2019-11-24
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [null-deref, iocsh, registry, lifecycle, crash]
---

# iocshFindVariable() dereferences null when name not registered — crash

## Root Cause
`iocshFindVariable` called `registryFind(iocshVarID, name)` which returns
`NULL` when the name is not found, then unconditionally dereferenced the
result (`return temp->pVarDef`). Any call with an unregistered variable name
causes a null pointer dereference.

## Symptoms
- Crash (segfault) in the IOC shell when `iocshFindVariable` is called with
  an unknown variable name — for example from a script or interactive shell
  before a variable is registered.

## Fix
Add null check: `return temp ? temp->pVarDef : 0`.

## Rust Applicability
In Rust, `HashMap::get` returns `Option<&V>`, which cannot be dereferenced
without handling `None`. There is no equivalent of an unconditional C pointer
dereference. Eliminated by Rust's type system.

## Audit Recommendation
None — eliminated by `Option` in Rust.

## C Locations
- `modules/libcom/src/iocsh/iocsh.cpp:iocshFindVariable` — null check added after registryFind
