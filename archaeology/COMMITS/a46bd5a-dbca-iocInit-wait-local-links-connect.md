---
sha: a46bd5ae88087bbadab0d547141146ebf4d46dc8
short_sha: a46bd5a
date: 2025-09-20
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/ca_link.rs
    function: dbCaAddLinkCallbackOpt
tags: [dbCa, iocInit, CA-link, lifecycle, connect-ordering]
---
# dbCa: iocInit wait for local CA links to connect (later reverted)

## Root Cause
Before PR-713, `iocInit` signaled completion (via `startStopEvent`) as soon
as the `dbCaTask` initialized its CA client context, without waiting for
local CA links to actually connect. Database records using CA input links to
other local records could process before those links were connected, causing
the first scan cycle (particularly `PINI=YES` records) to read stale or
zero values rather than the actual PV values.

## Symptoms
On IOCs with local CA links and `PINI=YES` records:
- First process reads disconnected-link default (zero/empty) instead of
  the live value from the target record.
- Ordering-sensitive startup sequences (e.g., setpoint records reading
  feedback) produce incorrect initial states.

## Fix (later reverted by 3f382f6)
Add `initOutstanding` atomic counter. In `dbInitLink`, for local CA links
(those whose target PV exists in the local database), set
`DBCA_CALLBACK_INIT_WAIT` flag and increment `initOutstanding`.
In `eventCallback`, decrement `initOutstanding` on first connection event
for flagged links. In `dbCaTask`, only signal `startStopEvent` once
`initOutstanding` reaches zero. This ensured all local links were connected
before `iocInit` returned.

**Note:** This commit was subsequently reverted (see 3f382f6) due to
deadlock risk with circular CA links and slow-connecting targets.

## Rust Applicability
`applies` — The design problem (PINI records firing before CA links are
ready) applies equally to base-rs's CA link implementation. The correct
approach (not the reverted one) is to add a time-bounded startup fence or
to make PINI processing link-aware. Audit base-rs for the same ordering gap.

## Audit Recommendation
Audit `base-rs/src/server/database/ca_link.rs` and the IOC init sequence
for whether local CA links are guaranteed to be connected before the first
PINI scan. If not, document this as a known limitation or implement a
time-bounded readiness check (not a blocking wait that can deadlock).

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaLinkInitImpl` — added wait on initOutstanding==0
- `modules/database/src/ioc/db/dbCa.c:eventCallback` — added CA_INIT_WAIT decrement
- `modules/database/src/ioc/db/dbLink.c:dbInitLink` — detects local link, sets DBCA_CALLBACK_INIT_WAIT
