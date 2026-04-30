---
sha: 752ec12261cf10155754534c3de8ea7bdfda5e54
short_sha: 752ec12
date: 2019-09-19
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [memmove, bounds, log-client, buffer, ordering]
---

# logClient memmove uses stale nextMsgIndex before decrement

## Root Cause
`logClientFlush` tracked how much of the message buffer had been successfully
sent via `nSent` and `pClient->nextMsgIndex`. After accounting for the OS-level
send backlog, the code needed to:
1. Subtract `nSent` from `nextMsgIndex` to track remaining bytes.
2. Call `memmove` to shift remaining bytes to the front of the buffer.

The original order was:

```c
// BEFORE (broken):
if (nSent > 0) {
    memmove(pClient->msgBuf, &pClient->msgBuf[nSent],
        pClient->nextMsgIndex);     // ← uses OLD nextMsgIndex (too large)
    pClient->nextMsgIndex -= nSent; // ← decrement happens AFTER
}
```

`memmove` was called with the old (un-decremented) `nextMsgIndex` as the length,
so it copied `nSent` extra bytes beyond the end of the valid data — either
copying garbage into the buffer or reading past its end (heap overread).

## Symptoms
After a partial send, the message buffer would contain garbage data appended
after the valid unset portion. Subsequent log messages could be corrupted or
truncated. On platforms where the buffer sits near a heap boundary, the overread
could corrupt allocator metadata.

## Fix
Decrement `nextMsgIndex` first, then gate `memmove` on the updated value:

```c
// AFTER (correct):
pClient->nextMsgIndex -= nSent;
if (nSent > 0 && pClient->nextMsgIndex > 0) {
    memmove(pClient->msgBuf, &pClient->msgBuf[nSent],
        pClient->nextMsgIndex);     // ← uses updated (smaller) length
}
```

## Rust Applicability
In Rust, a ring buffer or `VecDeque` would be used for the message buffer; the
drain-and-shift pattern is encapsulated in safe APIs that cannot memmove the
wrong length. Eliminated.

## Audit Recommendation
No action needed. If `base-rs` log module uses a manual ring buffer with raw
slice copies, audit the shift-after-send logic for correct index ordering.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientFlush` — memmove called before nextMsgIndex decrement
