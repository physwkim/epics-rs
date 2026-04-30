---
short_sha: 88bfd6f
status: applied
files_changed:
  - crates/epics-base-rs/src/types/value.rs
---
Genuine bug. The Rust string-to-numeric path used `s.parse::<f64>()` in `EpicsValue::to_f64()`, which is decimal-float-only — `"0x1A"` returned `None` and `"017"` returned `17.0`, mirroring the original C `epicsParseInt*(..., base=10, ...)` defect. Since `EpicsValue::convert_to` for `Short`/`Long`/`Char`/`Enum`/`Float` runs `self.to_f64().unwrap_or(0.0) as <int>`, a CA put of `"0x1A"` to a `DBF_LONG` field would silently land as `0` and `"017"` would land as `17` decimal, identical to the C behavior the upstream commit fixed.

The existing `EpicsValue::parse(dbr_type, s)` helper (used by db_loader / iocsh) already calls `parse_int` which auto-detects `0x`/`0` prefixes correctly — so db file loading was fine. The CA-put / type-coercion path through `convert_to → to_f64` was the gap.

Fix: added a private `parse_string_to_f64` helper that auto-detects `0x`/`0X` (hex) and leading-`0`-with-digits (octal, but only when no `.` / `e` is present so `"0.5"` and `"0e6"` still parse as floats), with optional leading `+`/`-` sign, then falls back to `s.parse::<f64>()`. `EpicsValue::String(s) => s.parse().ok()` was replaced with `parse_string_to_f64(s)`. C ref cited inline: `dbConvert.c` / `dbConvertBase=0` (88bfd6f, 2025-11-05).

`cargo check -p epics-base-rs`: clean.
