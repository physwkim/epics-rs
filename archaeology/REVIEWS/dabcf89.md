---
short_sha: dabcf89
status: not-applicable
files_changed: []
---
The C bug was in `mbboDirectRecord.c::init_record`: it unconditionally called `bitsFromVAL()` so explicit `field(B0..B1F, ...)` settings were overwritten by a zero `VAL` when `UDF` was set. In `epics-base-rs` the `mbboDirect` record type is not implemented — `src/server/records/` contains `mbbo.rs` but no `mbbodirect.rs`. The audit target `src/server/database/rec/mbbodirect.rs` does not exist (nor does the parent `database/rec/` directory). The base `mbbo` record in base-rs has no `B0..B1F` bit fields and therefore no `bits ↔ VAL` synchronisation at init: `Grep "B0|B1F|bitsFromVAL|bits_from_val"` over the records tree returns no hits. The bit-field/VAL init priority bug is structurally absent.
