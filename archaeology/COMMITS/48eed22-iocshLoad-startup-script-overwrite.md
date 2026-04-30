---
sha: 48eed22f3b84d7f3fb14a36cf8bdc4b2d60d3e6d
short_sha: 48eed22
date: 2024-09-20
author: DW
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [iocsh, env-var, IOCSH_STARTUP_SCRIPT, overwrite, startup]
---
# iocshLoad: do not overwrite IOCSH_STARTUP_SCRIPT if already set

## Root Cause
`iocshLoad` unconditionally called `epicsEnvSet("IOCSH_STARTUP_SCRIPT", pathname)`
whenever `pathname` was non-NULL, even if the environment variable was already
set. When a startup script (which set `IOCSH_STARTUP_SCRIPT` itself) called
`iocshLoad` on a secondary script, the variable was overwritten to point to the
secondary script, losing the original value. Tools or hooks that read
`IOCSH_STARTUP_SCRIPT` to find the primary startup file would see the wrong path.

## Symptoms
`IOCSH_STARTUP_SCRIPT` reflected the last `iocshLoad` call rather than the
primary startup script when nested scripts used `iocshLoad`. Monitoring tools or
autosave configurations that relied on this variable saw incorrect paths.

## Fix
Add `!getenv("IOCSH_STARTUP_SCRIPT")` guard: only set the variable if it is not
already defined.

## Rust Applicability
If `base-rs` or a Rust IOC shell implementation exposes an equivalent
`IOCSH_STARTUP_SCRIPT` environment variable, apply the same guard. In practice,
the Rust IOC shell does not rely on this C environment variable pattern.
Eliminated.

## Audit Recommendation
None — eliminated by Rust's explicit argument passing over implicit env-var
communication.

## C Locations
- `modules/libcom/src/iocsh/iocsh.cpp:iocshLoad` — add `!getenv("IOCSH_STARTUP_SCRIPT")` guard before `epicsEnvSet`
