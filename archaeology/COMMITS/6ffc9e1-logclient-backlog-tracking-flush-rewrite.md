---
sha: 6ffc9e17dacf7af450cdee2e3915cb73f317ef96
short_sha: 6ffc9e1
date: 2019-09-17
author: Dirk Zimoch
category: flow-control
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/log_client.rs
    function: flush
tags: [log-client, backlog, flow-control, send-queue, message-loss]
---

# logClient flush discards messages already in OS send queue

## Root Cause
The original `logClientFlush` tracked only bytes successfully handed to
`send()` and assumed all of them had left the host immediately. In reality,
TCP send is buffered — bytes passed to `send()` sit in the OS send queue until
ACKed. When a connection broke (or was detected broken), the code called
`logClientClose` and then reset `nextMsgIndex`, effectively discarding messages
that were in the OS send queue but had not yet been acknowledged.

Additionally, on reconnect, `nSent` started from 0 instead of accounting for
bytes still in flight from before the connection drop.

The rewrite added:
1. A `backlog` field tracking how many bytes `epicsSocketCountUnsentBytes`
   reports as still in the OS send queue.
2. `nSent` initialized to `pClient->backlog` at the start of flush (so
   already-queued bytes are not double-counted).
3. Error handling refactored to separate the send loop from the error-reporting
   block, making the control flow clearer.

```c
// nSent now starts at the OS-reported backlog, not at 0:
nSent = pClient->backlog;
while (nSent < pClient->nextMsgIndex && pClient->connected) {
    status = send(...);
    if (status < 0) break;
    nSent += status;
}
```

## Symptoms
On a log server restart or network interruption, messages buffered in the OS
send queue were silently discarded. After reconnect, only messages that arrived
*after* reconnection appeared in the log, creating gaps during transient
outages.

## Fix
Initialize the flush loop's `nSent` from the current OS send-queue backlog, so
the remaining buffer math correctly accounts for already-queued bytes. Separate
error handling into a post-loop check. Track `backlog` persistently in the
client struct.

## Rust Applicability
A Rust async log client using tokio's `BufWriter`/`AsyncWriteExt` has a similar
issue: bytes handed to `write()` or `flush()` enter the tokio send buffer and
the kernel send queue. If the connection is reset, those bytes are lost. The
Rust equivalent should use `SO_SNDBUF` / `TIOCOUTQ` awareness or keep an
application-level retry buffer. Partial applicability — the bug class (silent
message loss on connection drop) can arise in Rust log clients that rely solely
on tokio TCP write success as confirmation of delivery.

## Audit Recommendation
In `base-rs/src/log/log_client.rs`: verify that on connection error, pending
messages are retained in the application-side buffer (not only in the tokio
write buffer) so they can be retried on reconnect. Check that `flush()` is not
treating `write()` success as delivery confirmation.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientFlush` — major rewrite of send loop + backlog tracking
- `modules/libcom/src/log/logClient.c` — new `backlog` field in `logClient` struct
