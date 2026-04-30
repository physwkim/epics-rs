---
short_sha: 832abbd
title: subRecord — propagate error from bad INP links instead of silently succeeding
crate: base-rs
status: not-applicable
---

# Review

## Verdict
**not-applicable** — the C bug's structural shape (a `process()` that fetches
its own INPx links and accumulates a status it then drops) has no Rust
counterpart. The Rust pipeline factors link reads out of `process()` and into
shared infrastructure, so the literal "`return 0` instead of `return status`"
mistake cannot be made.

## Evidence
- `crates/epics-base-rs/src/server/records/sub_record.rs:208`
  `fn process(&mut self) -> CaResult<ProcessOutcome>` already returns
  `Result`; its body just `Ok(ProcessOutcome::complete())` — it never reads
  links itself, so it has no per-link status to drop.
- The INPA..INPL fetch happens once for all multi-input record types in
  `crates/epics-base-rs/src/server/database/processing.rs:228-274`
  (the "1.5 Multi-input link fetch" block) using
  `LinkOps::read_link_with_alarm`
  (`crates/epics-base-rs/src/server/database/links.rs:53`). Successful values
  are applied to A..L (`processing.rs:330-335`), and source-record alarm
  state is forwarded into the local record via `link_alarms` and
  `rec_gbl_set_sevr` according to MS / MSI / MSS / NMS
  (`processing.rs:490-505`).
- The architectural divergence from C `subRecord.c:process` means a fix here
  is not a one-line `return status` — it would require either (a) per-record
  knowledge of "this record's link list is required" plumbed back into the
  centralized fetcher, or (b) a cross-cutting policy applied to every
  multi-input record (calc, calcout, sub, sel, asub, scalcout, transform).
  Either is well above the >50-line scope ceiling for this round.

## Decision
No fix applied. The fact that `process()` returns `CaResult<_>` in Rust
already enforces the audit's "propagate Err" recommendation at the type
level for any error sub_record itself produces. The remaining gap — turn
"non-empty INP link with no resolvable target" into a record-level
LINK/INVALID alarm — is an architectural enhancement (covers all multi-input
record types, not just sub) and should be tracked separately rather than
sneaked into a sub-record-only patch.

C ref: `modules/database/src/std/rec/subRecord.c:process` — `return 0` →
`return status`.
