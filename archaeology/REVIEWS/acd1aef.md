---
short_sha: acd1aef
title: Silent CP/CPP modifier discard on output links
status: fixed
crate: base-rs
---

# Review

## Audit Targets
- `src/server/database/link.rs::parse_link` — actual location is
  `src/server/record/link.rs::parse_link_v2`. The parser is generic
  (no `field_kind` / `dbf_outlink` parameter), so the warning context
  must come from the caller that knows the target field.

## Verification
`parse_link_v2` (record/link.rs:74) accepts any link string and strips
trailing modifiers (`CP`, `CPP`, etc.) without distinguishing input vs
output usage. The OUT-link callsite is
`src/server/record/record_instance.rs:742-747` — the only place that
unambiguously owns an output link.

Before the fix, writing `record.OUT = "REC.FIELD CP"` silently parsed
as `ChannelProcess` policy on an output link, mirroring the pre-fix C
behavior (silent strip with no diagnostic).

## Fix
In `record_instance.rs::put_common_field`, the `OUT` arm now emits
`eprintln!("Warning: <rec>.OUT: CP/CPP modifier ignored on output link",
...)` when the trimmed link string ends with ` CP` or ` CPP` before
delegating to `parse_link_v2`. This mirrors `dbParseLink` from
`acd1aef`, which strips `pvlOptCP|pvlOptCPP` and emits an
`errlogPrintf` naming the source record/field.

The parser itself is unchanged: it still strips `CP`/`CPP` (correct,
since that is also what C does for output links). Only the diagnostic
is added at the OUT-aware callsite.

## Validation
`cargo check -p epics-base-rs` — clean build.

## C Reference
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbParseLink` —
  strip + warn on CP/CPP for `DBF_OUTLINK`
- `modules/database/src/ioc/db/dbAccess.c:dbPutFieldLink` —
  caller updated to pass recname/fieldname

## Files Changed
- `crates/epics-base-rs/src/server/record/record_instance.rs`
</content>
</invoke>