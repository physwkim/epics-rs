---
short_sha: d47fa4c
title: aSub record — fetch_values dbGetLink on constant input links
status: not-applicable
severity: medium
crate: base-rs
files_changed: []
---

# Review: d47fa4c — aSub constant input link skip

## Verdict
not-applicable — the C bug is `aSubRecord.c::fetch_values` calling
`dbGetLink()` unconditionally on every INPA..INPZ link, with `dbGetLink()`
returning an error for constant links and aborting the whole loop.

In base-rs the equivalent flow lives in
`crates/epics-base-rs/src/server/database/processing.rs:236-273`. Each
multi-input link is parsed by `parse_link_v2()`
(`src/server/record/link.rs:74`) which classifies a literal numeric or
quoted string into `ParsedLink::Constant`. The reader
`PvDatabase::read_link_with_alarm()`
(`src/server/database/links.rs:53-80`) handles every variant explicitly,
including a `ParsedLink::Constant(_) => (link.constant_value(), None)`
arm — constants always yield a value with no error. The loop never
short-circuits on a single link result.

The `multi_input_links()` for `ASubRecord`
(`src/server/records/asub_record.rs:830`) only enumerates INPA..INPL,
mirroring the same shape but going through the safe per-link path.
There is no error-on-constant code path that could cause the upstream
bug to occur.

## C reference
`modules/database/src/std/rec/aSubRecord.c:fetch_values` — added
`if (dbLinkIsConstant(plink)) continue;` before `dbGetLink()`.

## Build
No code changes; `cargo check -p epics-base-rs` already clean.
