---
sha: 56f05d722dee4b8ca2968b8bface2737a3a9b185
short_sha: 56f05d7
date: 2021-01-14
author: Ben Franksen
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
tags: [db_field_log, dbfl_has_copy, scan-lock, data-ownership, race]
---
# dbGet: wrong condition for using db_field_log vs. live record data

## Root Cause
In `dbGet`, the two paths for scalar and array conversion used `!pfl` as the
condition to decide whether to read from the live record (`paddr->pfield`) or
from the `db_field_log` snapshot. However, a non-NULL `pfl` is not sufficient
to guarantee the field log owns a copy of the data. A `db_field_log` with
`type==dbfl_type_ref` and `u.r.dtor == NULL` is a reference to the live
record data (not a copy), meaning the scan lock must be held to access it
safely.

When `dbGet` was called from a CA subscription callback with such a
`db_field_log`, it would use the "pfl path" (bypassing `paddr->pfield`) but
read from `pfl->u.r.field` without the scan lock, accessing record data
concurrently with record processing — a data race.

## Symptoms
Data race / torn reads on CA subscription callbacks for array fields when the
`db_field_log` has `type==dbfl_type_ref` with `dtor==NULL` (i.e., the filter
pipeline passed through the uncopied live record pointer). Values seen by CA
clients could be partially updated or garbage, especially for `DBF_DOUBLE`
arrays or waveform records.

## Fix
Changed both conversion-path conditions from `!pfl` to `!dbfl_has_copy(pfl)`:

```c
if (!dbfl_has_copy(pfl)) {
    /* read from live record via paddr->pfield — scan lock required */
} else {
    /* read from pfl's own copy — no lock needed */
}
```

`dbfl_has_copy` (defined in `85822f3`) returns true only if the field log
owns its data (val type, OR ref type with non-NULL dtor, OR zero elements).

## Rust Applicability
Applies. In base-rs, any CA subscription delivery path that receives a
`DbFieldLog`-equivalent must correctly distinguish "we own the data" from
"this is a borrow of the live record". If field logs can carry either owned
or borrowed data, the borrow case must be resolved under the scan lock before
passing data to the CA serializer. An incorrect `is_some()` check on an
`Option<DbFieldLog>` when the log may carry a non-owning reference would
reproduce this bug.

## Audit Recommendation
Audit `base-rs/src/server/database/db_access.rs::db_get`: find the condition
that selects between reading from the live record and reading from the field
log. Ensure it uses the equivalent of `dbfl_has_copy` — checking both type
AND `dtor` presence AND `no_elements` — not simply `Option::is_some()`.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — two conditions changed from !pfl to !dbfl_has_copy(pfl)
