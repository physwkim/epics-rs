---
sha: 4a0f488657e208ab2ed6aed17473d42d19fc9d2d
short_sha: 4a0f488
date: 2021-02-25
author: Krisztián Löki
category: bounds
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/record_support.rs
    function: monitor
tags: [db-post-events, bptr, array-record, histogram, event-callback]
---
# histogramRecord wdog callback uses bptr instead of VAL field for db_post_events

## Root Cause
`histogramRecord`'s `wdogCallback` (watchdog/rate-limit callback) called:
```c
db_post_events(prec, prec->bptr, DBE_VALUE | DBE_LOG);
```
`db_post_events` takes a pointer to the *field* that changed, not a pointer to
the array data itself.  Passing `prec->bptr` (the array pointer) instead of
`&prec->val` (the VAL field address) causes the event subsystem to treat the
array data pointer value as the field address, computing a wrong field offset
and potentially reading garbage or causing access violations in the callback.

This is the watchdog path, distinct from the main `monitor()` function (which
had the same bug, fixed in `2340c6e6`).

## Symptoms
Monitor callbacks for the histogram VAL field (triggered by the watchdog timer)
delivered garbled or incorrect data to subscribers; potential crash if the
pointer arithmetic produced an address outside the record.

## Fix
Changed `prec->bptr` → `(void*)&prec->val` in `wdogCallback`.  `prec->val`
is the DBF_NOACCESS placeholder field whose address is the canonical
identifier for the array-value event.  The actual data is retrieved via
`get_array_info`.

## Rust Applicability
In `base-rs` record monitor dispatch, always post events referencing the
field descriptor (equivalent to `&prec->val` / the field's offset), not the
raw heap buffer pointer.  If events are dispatched by field index rather than
pointer arithmetic, this bug does not apply — but verify the implementation.

## Audit Recommendation
In `base-rs/src/server/database/record_support.rs`, any `post_event`
equivalent for array records must reference the field identity token, not the
raw buffer address.  Check histogram, compress, aai, and subArray record
equivalents.

## C Locations
- `modules/database/src/std/rec/histogramRecord.c:wdogCallback` — bptr → &prec->val
