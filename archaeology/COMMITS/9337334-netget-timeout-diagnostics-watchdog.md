---
sha: 933733465e0b5008d907a9699f4a06f37b1809a1
short_sha: 9337334
date: 2019-02-13
author: Andrew Johnson
category: timeout
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, timeout, diagnostics, netget, watchdog]
---
# netget.plt: per-operation watchdog timers with diagnostic messages on timeout

## Root Cause
The `netget.plt` integration test used a single global 30-second alarm for the
entire test. If any step timed out, the alarm handler called `$ioc->kill` and
`BAIL_OUT` with no context about which step had failed. On Windows, `chomp` was
also broken for CR+LF line endings, causing line-matching to fail silently.

## Symptoms
- Flaky CI: timeout bail-out with no information about which CA/PVA step hung.
- Windows: `dbl` output matching failed because of trailing `\r`.

## Fix
Replace the global alarm with per-operation `watchdog { ... } $timeout, kill_bail("doing X")` 
helper. Each operation (start softIoc, dbl, dbgf, casr, caget, pvasr, pvget)
gets its own 10-second window. On timeout, `BAIL_OUT("Timeout doing X")` gives
actionable diagnostics. Fix `chomp` → regex strip for Windows CR+LF.
Also add `EPICS_CAS_BEACON_PORT` to prevent port collision with other tests.

## Rust Applicability
Eliminated. Test infrastructure only; no Rust production code affected.

## Audit Recommendation
None required.

## C Locations
- `modules/database/test/std/rec/netget.plt` — per-op watchdog, CR+LF fix, beacon port
- `modules/database/src/tools/EPICS/IOC.pm:_getline` — chomp → regex strip for Windows
