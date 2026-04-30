---
short_sha: d0cf47c
status: not-applicable
files_changed: []
---
The C bug was that `recGblInheritSevr` (called from `dbDbLink.c::dbDbGetValue` and `dbDbPutValue`) propagated only `stat`/`sevr` through MSS links and dropped the alarm message string (`amsg`/`namsg`); the fix introduced `recGblInheritSevrMsg` to also carry the message. In `epics-base-rs` the AMSG concept is structurally absent: `CommonFields` (`src/server/record/common_fields.rs:8-63`) carries `sevr`, `stat`, `nsev`, `nsta`, `acks`, `ackt`, `udf`, `udfs` and friends — there is no `amsg` or `namsg` field. The audit targets `src/server/database/db_link.rs` and `src/server/database/rec_gbl.rs` do not exist; the link-alarm-inheritance code path (`src/server/database/links.rs::read_link_with_alarm` and the MS/NMS/MSS/MSI propagation in `processing.rs:491-505` via `rec_gbl_set_sevr`) carries only `stat` and `sevr`. There is no message string that could be silently dropped, so the bug cannot occur. Adding AMSG support is a feature gap, not a corrective patch.
