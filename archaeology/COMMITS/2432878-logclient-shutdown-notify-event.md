---
sha: 243287877311748057baa61937ea8d8a530f8d27
short_sha: 2432878
date: 2019-08-27
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [log-client, shutdown, thread, sleep, cancellation]
---

# logRestart Thread Uses Event Wait Instead of Sleep for Fast Exit

## Root Cause
The `logClientRestart` background thread called `epicsThreadSleep(LOG_RESTART_DELAY)`
unconditionally in its loop. On `logClientDestroy()`, the destroy function set
`shutdown = 1` and interrupted the blocking socket call, but the thread could
then re-enter the sleep and block for the full `LOG_RESTART_DELAY` before
checking `shutdown` again. This caused slow shutdown of IOCs, particularly
when the log server was unreachable and the thread was sleeping.

## Symptoms
IOC shutdown stalls for up to `LOG_RESTART_DELAY` seconds waiting for the
logRestart thread to wake from its fixed sleep.

## Fix
Add a dedicated `shutdownNotify` `epicsEvent`. `logClientDestroy()` signals it
immediately after setting `shutdown = 1`. The restart loop replaces
`epicsThreadSleep(LOG_RESTART_DELAY)` with
`epicsEventWaitWithTimeout(shutdownNotify, LOG_RESTART_DELAY)` so it wakes
immediately when signaled.

## Rust Applicability
Eliminated. In Rust, the reconnect loop is an `async` task. Shutdown is driven
by dropping a `CancellationToken` or by closing the task's `JoinHandle` with
`abort()`. `tokio::time::sleep(delay).await` is already cancellable at await
points — there is no analog of the C fixed-sleep problem.

## Audit Recommendation
No direct audit needed. Confirm that the Rust log-client reconnect loop's
sleep is `tokio::time::sleep(...).await` inside a `tokio::select!` arm that
also watches a cancellation token, rather than a blocking `thread::sleep`.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientRestart` — replaced `epicsThreadSleep` with `epicsEventWaitWithTimeout`
- `modules/libcom/src/log/logClient.c:logClientDestroy` — signals `shutdownNotify` before thread join
