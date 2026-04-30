---
short_sha: 6b5abf7
status: not-applicable
files_changed: []
---
The C bug was an explicit `if (*pnRequest <= 0) return S_db_badField;` guard in `dbDbGetValue` after filter execution that converted legitimate empty-array filter results into errors. base-rs does not implement the C dbDbLink/dbDbGetValue API, channel filters (`arr`/`ts`/etc.) or per-`nRequest` slot semantics — DB-link reads in `crates/epics-base-rs/src/server/database/links.rs::read_link_value` go through `get_pv` which returns the full `EpicsValue` (including legitimate empty-array variants such as `EpicsValue::DoubleArray(vec![])`) without any "len <= 0 → Err" gating. There is no equivalent guard to remove and no filter pipeline that could spuriously produce an error from a zero-element result. Closing as not-applicable.
