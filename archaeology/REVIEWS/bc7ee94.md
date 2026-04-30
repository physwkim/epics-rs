---
short_sha: bc7ee94
title: Remove spurious warning when PUTF is set on target with PACT false
status: not-applicable
crate: base-rs
---

# Review

## Audit Targets
- `src/server/database/db_link.rs::process_target` — file does not exist;
  link processing lives in `src/server/database/processing.rs` and
  `src/server/database/links.rs` (high-level rewrite, not a port of
  `dbDbLink.c`).

## Verification
Searched for `putf` / `PUTF` / `pact` references in
`crates/epics-base-rs/src/server/database/`:
- `processing.rs:823-857` — CP-link target dispatch (8) sets
  `tg.common.putf = true` before calling `process_record_with_links`.
- `processing.rs:1322-1357` — equivalent CP path inside
  `execute_process_actions`.
- `processing.rs:415,432,1100` — PACT lifecycle handling (set/clear).

No diagnostic prints `Warning: '<rec>.PUTF' found true with PACT false`
or any equivalent message in the link-processing path. The bug being
fixed in `bc7ee94` (a stale `errlogPrintf` block left over after
`3fb10b6`) has no corresponding code in `base-rs` to remove.

## Decision
**not-applicable** — base-rs does not emit the spurious warning the C
patch removes. No structural equivalent of the offending diagnostic
exists.

## C Reference
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — removed warning block
</content>
</invoke>