---
sha: b6626e4f60697d577097e335ce79e1ecbce3fee6
short_sha: b6626e4
date: 2023-01-22
author: Michael Davidsaver
category: flow-control
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/event.rs
    function: event_read
tags: [stall-detection, flow-control, event-queue, callback, eventsRemaining]
---
# dbEvent: detect possible queue stall when eventsRemaining is set

## Root Cause
In `dbEvent.c`, `event_read()` drains the event queue and calls user
callbacks with an `eventsRemaining` flag indicating whether more events follow.
Consumers are expected to continue flushing when `eventsRemaining != 0`.

If a consumer (e.g., CA server) receives `eventsRemaining=1` but does NOT
flush further (bug in the consumer, or flow-control stall), the queue
effectively stalls: the producer keeps posting events but the queue is full
and the consumer is not draining. Before this fix, there was no diagnostic
for this condition.

## Symptoms
- Silent CA subscription stall: updates stop flowing to clients but no error
  is reported.
- The queue fills up, new events get dropped/coalesced, and clients see
  missing updates without any log message to guide debugging.

## Fix
Added a `possibleStall` flag to `struct event_que`. After `event_read()`
finishes its drain loop, if the last callback was notified with
`eventsRemaining != 0` (stored in `notifiedRemaining`), it means the function
returned even though the consumer was told there were more events — this is a
stall condition. On first detection, it logs:
`ERL_WARNING " dbEvent possible queue stall"` and sets `possibleStall = 1`
(so the message is printed only once per queue).

## Rust Applicability
A Rust event queue implementation using `tokio::sync::mpsc` or a bounded
channel would surface backpressure as `send().await` blocking or
`try_send()` returning `Err(Full)`. The stall detection concept applies: if a
consumer task is not polling the channel, the channel fills and the producer
stalls. Rust's async backpressure is explicit, but a similar diagnostic
warning should be logged when the channel is consistently full.

## Audit Recommendation
In `base-rs` event/subscription dispatch: verify that when the event channel
is full (backpressure), a warning is logged and the situation is surfaced
(e.g., via a metric or a one-shot log message), rather than silently dropping
events or blocking indefinitely.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:event_read` — possibleStall detection added
- `modules/database/src/ioc/db/dbEvent.c:event_que` — possibleStall field added
