---
short_sha: a46bd5a
status: not-applicable
files_changed: []
---
This commit (PR-713 in C-EPICS) adds an `initOutstanding` atomic counter so `iocInit` blocks until all local CA links connect; it was later reverted (3f382f6) because of deadlock potential. base-rs has no dbCa task, no `dbCaLinkInitImpl`, and no `iocInit`-time blocking on link readiness — CA links resolve lazily via `PvDatabase::resolve_external_pv` (database/mod.rs:287). The "PINI fires before local CA links connect" ordering gap that PR-713 tried to close also does not directly apply: in base-rs, local-target CA links normally short-circuit through `read_link_value`'s direct DB lookup (links.rs:19) when the target is in the same `PvDatabase`, so PINI processing finds the live value without needing a CA round-trip. External-target CA links return `None` if the upstream isn't ready, matching pre-PR-713 C semantics (and avoiding the deadlock the revert restored). No fix needed; the safe pattern (lazy resolution, no blocking-wait fence) is already in place.
