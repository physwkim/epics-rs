---
short_sha: dac620a
status: not-applicable
files_changed: []
---
The targeted file `src/server/database/db_link.rs` does not exist in base-rs, and there are no `db_db_get_control_limits` / `_graphic_limits` / `_alarm_limits` / `_precision` / `_units` analogs. Metadata for records is read directly from the local `CommonFields` (`record_instance.rs::alarm_limits`) without traversing INP links, so the `DBLINK_FLAG_VISITED` self-loop guard the C fix added has no current target. If a metadata-via-link feature is later added, this guard will need to be reintroduced.
