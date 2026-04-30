---
short_sha: f2fe9d1
status: not-applicable
files_changed: []
---
The C bug is in `devBiSoftRaw.c::readLocked` — the "Raw Soft Channel" device support for `bi` reads INP into RVAL via `dbGetLink` and (before the fix) skipped `if (prec->mask) prec->rval &= prec->mask`.

`epics-base-rs` does not implement a "Raw Soft Channel" device support module. There is no `crates/epics-base-rs/src/server/database/dev/bi_soft_raw.rs` (the audit_targets path), no `dev/` directory, and no separate device-support shim that loads INP into `bi.rval`. `is_soft_dtyp("Raw Soft Channel")` (`server/device_support.rs:7`) is recognized as a soft DTYP only so the framework skips the registered-driver dispatch — but the Rust soft-channel path applies INP directly to `VAL` via `Record::set_val` (`server/database/processing.rs:326`), not into RVAL through a masked-write helper. `BiRecord::process` (`server/records/bi.rs:156`) drives `val = (rval == 0) ? 0 : 1` and only runs that conversion when `skip_convert` is false (i.e. when device support set RVAL — never via soft channel here).

Because there is no RVAL-loading raw-soft path, there is no place where a missing `rval &= mask` could silently swallow the MASK field. If a future device-support driver chooses to populate RVAL on a `bi`, applying MASK there is the driver's responsibility (same as in C). Structurally absent in the rewrite.
