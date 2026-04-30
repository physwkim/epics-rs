---
sha: cd0e6a4f9a1d0e847282cbbba3486386f0dc3302
short_sha: cd0e6a4
date: 2021-02-05
title: "caProto.h uses IPPORT_USERRESERVED without including its definition"
crate: epics-ca-rs
audit_targets:
  - file: crates/epics-ca-rs/src/protocol.rs
status: not-applicable
---

# Review

## Audit
The C bug is `caProto.h` defining `CA_SERVER_PORT = IPPORT_USERRESERVED + 56`
without including `<osdSock.h>`, plus RTEMS-kernel not defining
`IPPORT_USERRESERVED` at all — yielding `CA_SERVER_PORT = 56` instead of
`5064` on RTEMS.

`epics-ca-rs/src/protocol.rs:30-31`:
```
pub const CA_SERVER_PORT: u16 = 5064;
pub const CA_REPEATER_PORT: u16 = 5065;
```

These are literal constants. There is no `IPPORT_USERRESERVED` derivation
anywhere in the crate (or in the workspace — confirmed by grep). The
`EPICS_CA_SERVER_PORT` env-var override in `client/mod.rs:2037` falls back
to the literal `CA_SERVER_PORT` constant, not to a platform-derived value.

## Verdict
not-applicable — the include-order / platform-header dependency cannot
exist in Rust constants.

## Files Changed
None.
