---
sha: 27fe3e4468ec62d4d17f775c4aced939a666ba36
short_sha: 27fe3e4
date: 2020-03-30
author: Ben Franksen
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_field_log.rs
    function: null
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_get
  - crate: base-rs
    file: src/server/database/db_channel.rs
    function: run_post_chain
tags: [db_field_log, dbfl_type_rec, scan-lock, data-ownership, filter-pipeline]
---
# db_field_log: eliminate dbfl_type_rec, unify live-record reference into dbfl_type_ref

## Root Cause
`db_field_log` had three types: `dbfl_type_rec` (no data, depends on the
live record being locked), `dbfl_type_ref` (external pointer), and
`dbfl_type_val` (inline scalar). The `dbfl_type_rec` type was a
special "virtual" log that held no data of its own — to read it, the caller
had to hold the scan lock and go through the record's `get_array_info` rset.

This three-way dispatch was duplicated in every filter (`arr.c`, `ts.c`) and
in `dbGet`. Each filter had to explicitly handle the `dbfl_type_rec` case by:
1. Locking the record.
2. Calling `get_array_info`.
3. Copying data into a new buffer.
4. Switching the type to `dbfl_type_ref`.

This was error-prone: filters that forgot to handle `dbfl_type_rec` (or that
incorrectly handled it) would produce data races. The `ts.c` filter, for
example, had to call `dbChannelMakeArrayCopy` just to make the data coherent
before replacing the timestamp — meaning two copies were needed for timestamped
array data.

Additionally, `dbExtractArrayFromRec` and `dbExtractArrayFromBuf` were separate
functions with different calling conventions, and `dbChannelMakeArrayCopy` was
a leaky abstraction.

## Symptoms
Any filter that forgot to handle `dbfl_type_rec` would operate on a zero/
uninitialised data pointer from the log's `u.r.field` (since `dbfl_type_rec`
stored no data there), causing NULL pointer dereference or reading garbage.
The `arr.c` filter explicitly checked for `dbfl_type_rec` and locked the
record, but the logic was complex and error-prone. Adding new filters required
understanding all three cases.

## Fix
Eliminated `dbfl_type_rec`. Freshly created field logs from `db_create_read_log`
now use `dbfl_type_ref` with `u.r.dtor == NULL` to signal "live record
reference" (data not yet copied). The ownership check is now:
- `dtor == NULL && no_elements > 0`: live record reference, lock needed.
- `dtor != NULL`: owned copy, no lock needed.
- `type == dbfl_type_val`: inline scalar, no lock needed.

The new `dbfl_has_copy` macro (added in subsequent commit `85822f3`) encodes
this exactly. The `arr.c` and `ts.c` filters now check `!pfl->u.r.dtor` to
decide whether to lock.

`dbExtractArrayFromRec` and `dbExtractArrayFromBuf` were merged into a single
`dbExtractArray(src, dst, field_size, nTarget, nSource, offset, incr)`
function. `dbChannelMakeArrayCopy` was removed. `dbChannelGetArrayInfo` was
added as a helper for the common boilerplate around `get_array_info`.

Also fixed `arrTest.cpp`: string index used `MAX_STRING_SIZE` instead of
`pfl->field_size`, which would produce wrong offsets for non-`MAX_STRING_SIZE`
strings.

## Rust Applicability
Applies directly. If base-rs implements a filter pipeline with `DbFieldLog`,
the two-variant design (inline-val / ref-with-dtor-or-not) is the correct
model. Key invariants to encode:
1. A `DbFieldLog` carrying a live-record borrow (`dtor==None`, `no_elements>0`)
   must never be dereferenced without the scan lock.
2. Filter implementations receive a `DbFieldLog` that may be a live reference;
   they must check this and copy before modifying.
3. `dbExtractArray` logic (offset wrap-around modulo original `no_elements`)
   must be correct.

The `arrTest.cpp` bug (using `MAX_STRING_SIZE` instead of `field_size` for
string stride) has a Rust analog: if string fields are indexed with a hardcoded
stride of 40 bytes instead of the actual `field_size`, the array test would
pass for normal strings but fail for LSI/LSO fields with longer strings.

## Audit Recommendation
If base-rs implements channel filters:
1. Verify the field log type has only two variants (val and ref), with ref
   carrying an `Option<Dtor>` or `bool` for copy ownership.
2. Verify that `arr` filter equivalent calls `get_array_info` under scan lock
   only when the field log does not own its data.
3. Verify string array indexing uses `field_size` (not `MAX_STRING_SIZE`).
4. Verify `dbExtractArray` equivalent handles offset wrap-around correctly.

## C Locations
- `modules/database/src/ioc/db/db_field_log.h` — removes dbfl_type_rec, simplifies to 2 types
- `modules/database/src/ioc/db/dbAccess.c:dbGet` — removes dbfl_type_rec branches
- `modules/database/src/ioc/db/dbChannel.c` — removes dbChannelMakeArrayCopy, adds dbChannelGetArrayInfo
- `modules/database/src/ioc/db/dbExtractArray.c:dbExtractArray` — unified (replaces FromRec/FromBuf)
- `modules/database/src/std/filters/arr.c:filter` — simplified, checks !dtor for lock decision
- `modules/database/src/std/filters/ts.c:filter` — simplified using dbExtractArray
- `modules/database/test/std/filters/arrTest.cpp` — fixes string stride: field_size not MAX_STRING_SIZE
