---
sha: 2b0161d9bfd8fc604942996d1812d245bf4a00f7
short_sha: 2b0161d
date: 2019-08-28
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [log-client, startup, blocking, connect, availability]
---

# logClient creation blocks 5 seconds when log server unavailable

## Root Cause
`logClientCreate` contained a synchronous spin-wait loop after spawning the
background connection thread:

```c
epicsTimeGetCurrent(&begin);
epicsMutexMustLock(pClient->mutex);
do {
    epicsMutexUnlock(pClient->mutex);
    epicsEventWaitWithTimeout(pClient->stateChangeNotify,
        LOG_SERVER_CREATE_CONNECT_SYNC_TIMEOUT / 10.0);
    epicsTimeGetCurrent(&current);
    diff = epicsTimeDiffInSeconds(&current, &begin);
    epicsMutexMustLock(pClient->mutex);
} while (!pClient->connected && diff < LOG_SERVER_CREATE_CONNECT_SYNC_TIMEOUT);
epicsMutexUnlock(pClient->mutex);
```

`LOG_SERVER_CREATE_CONNECT_SYNC_TIMEOUT` was 5.0 seconds. When the log server
was not running at IOC startup, `logClientCreate` would block the calling thread
for the full 5 seconds before returning, delaying IOC initialization.

The design intention was to synchronize with the first connection, but the log
client is supposed to be resilient to log server unavailability — it should
buffer and retry. Blocking startup for a non-critical service was undesirable.

## Symptoms
IOC startup was delayed by up to 5 seconds whenever the log server was
unavailable, even transiently. This was especially visible in test environments
or during boot races.

## Fix
Remove the synchronization loop entirely. `logClientCreate` returns immediately
after spawning the background thread. The background thread handles connection
and retry independently. IOC initialization is not blocked.

## Rust Applicability
A Rust log client using tokio would `tokio::spawn` the connection task and
return a handle immediately — there is no synchronous connect-wait in a properly
async design. Eliminated.

## Audit Recommendation
No action needed. Verify that `base-rs` log client initialization does not
contain any `.await` on the initial connection before returning from the
constructor.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientCreate` — synchronous 5s connect-wait removed
