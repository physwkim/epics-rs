---
sha: acd1aef2a02f9934ca091ffb7e54ebfe9388a36a
short_sha: acd1aef
date: 2025-10-08
author: Evan Daykin
category: lifecycle
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/link.rs
    function: parse_link
tags: [outlink, cp-cpp-modifier, link-parsing, warning, modifier-discard]
---
# Silent CP/CPP Modifier Discard on Output Links

## Root Cause
`dbParseLink()` silently stripped the `CP`/`CPP` modifiers from CA output
links (these modifiers only make sense on input links, since they request
"process on change" and "process on change with pvar" semantics). The strip
was correct behavior, but it produced no diagnostic. Operators configuring
output links with these flags received no feedback that the modifier had
been ignored, making misconfiguration silent.

Additionally, only `CPP` was stripped — `CP` was overlooked — so `CP` on an
output link was silently preserved in the parsed modifier set even though it
has no effect on output links.

## Symptoms
- Database record with `OUT field(…) CP` or `CPP` appeared to load without
  error but the modifier was silently dropped (or in the CP case, retained
  incorrectly).
- Operators could not easily debug why a monitor-on-write was not occurring.

## Fix
`dbParseLink()` signature extended with `recname`/`fieldname` parameters so
callers pass context.  Before stripping `CP|CPP` on `DBF_OUTLINK`, the code
now emits an `errlogPrintf` warning naming the source record and field.
Both `pvlOptCPP` and `pvlOptCP` are stripped together.

## Rust Applicability
`partial` — epics-rs does not yet have a full link-parsing layer, but the
equivalent will be needed in `base-rs` when parsing IOC DB files.  The Rust
implementation should:
- Reject or warn on `CP`/`CPP` modifiers on output links during link parsing.
- Return a diagnostic to the caller rather than silently dropping the flag.

## Audit Recommendation
When `base-rs` implements DB link parsing, audit the output-link modifier
filtering path to ensure unsupported modifiers produce a log warning rather
than silent discard. The function signature should carry record/field
context for diagnostics.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbParseLink` — strip + warn on CP/CPP for DBF_OUTLINK
- `modules/database/src/ioc/db/dbAccess.c:dbPutFieldLink` — caller updated to pass recname/fieldname
