---
short_sha: f57acd2
status: not-applicable
files_changed: []
---
The C change added `testdbCaWaitForConnect()` to `dbCa.c` to give tests a synchronisation point for the CA-link two-phase connection (`connectionCallback` → `getAttribEventCallback`), separate from the existing data-update-count waiter. In `epics-base-rs` the `dbCa` integration layer is structurally absent: `Grep "wait_for_connect|wait_for_update_count|CaLink|testdb_ca"` over the whole crate returns no hits. The audit target `src/server/database/db_ca.rs` does not exist; CA-link reads/writes are routed through `PvDatabase::resolve_external_pv` (`src/server/database/mod.rs`) and the link helpers in `src/server/database/links.rs` and `link_set.rs`, with the actual CA client logic living in the separate `epics-ca-rs` crate (and `pva://` links in `epics-pva-rs`). There is no in-process CA channel state machine with separate `connect`/`monitor` callbacks to expose, and therefore no test-helper to add here. The corrective patch belongs in the CA client crate, not `base-rs`.
