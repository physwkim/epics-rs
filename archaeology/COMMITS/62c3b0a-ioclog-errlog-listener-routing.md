---
sha: 62c3b0a585a4abcaae7794e487627bea29712be8
short_sha: 62c3b0a
date: 2019-08-27
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/log/ioc_log.rs
    function: ioc_log_client_init
tags: [errlog, log-client, subscriber, routing, lifecycle]
---

# iocLog: errlog Listener Registered on Wrong Object (All Clients)

## Root Cause
`logClientCreate()` unconditionally called `errlogAddListener(logClientSendMessage, pClient)`
and `logClientDestroy()` called `errlogRemoveListeners(logClientSendMessage, pClient)`.
This meant the low-level `logClient` factory itself managed the errlog subscription —
which is incorrect because `logClientCreate` is a generic primitive used by multiple
callers. When `iocLogClientInit` created one client and later another caller created
a second client, both would receive all errlog messages (fan-out to all clients),
which was not the intended routing (only the IOC-level log client should forward
errlog output to the log server).

## Symptoms
Multiple log clients receive duplicate errlog output. If a secondary log client
is created for another purpose, it incorrectly forwards all IOC log messages.

## Fix
Moved `errlogAddListener` / `errlogRemoveListeners` out of the generic `logClientCreate`/
`logClientDestroy` and into the IOC-specific `iocLogClientInit()` wrapper, with
a corresponding `iocLogClientDestroy()` registered via `epicsAtExit`. The generic
`logClient` layer is now listener-agnostic.

## Rust Applicability
In Rust, the log-client layer and the errlog subscriber bridge are likely separate
structs. The audit question is: does the log-client constructor subscribe itself
to an errlog broadcast, or is that done by a higher-level `IocLog` wrapper?
If the `LogClient::new()` constructor subscribes to a global errlog channel,
it may cause duplicate delivery if multiple `LogClient` instances are created.

## Audit Recommendation
In `base-rs/src/log/ioc_log.rs`, verify that `errlog` listener registration is
done by the IOC-level wrapper only, not by the generic `LogClient` constructor.
Check whether there is a `Drop` impl that correctly unregisters the listener.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientCreate` — removed `errlogAddListener` call
- `modules/libcom/src/log/logClient.c:logClientDestroy` — removed `errlogRemoveListeners` call
- `modules/libcom/src/log/iocLog.c:iocLogClientInit` — now calls `errlogAddListener` + registers `epicsAtExit(iocLogClientDestroy)`
- `modules/libcom/src/log/iocLog.c:iocLogClientDestroy` — new function, calls `errlogRemoveListeners`
