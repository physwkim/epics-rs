---
sha: 0f75e0aa7fc8d5219c95855c0cd52a24be1d2c35
short_sha: 0f75e0a
date: 2019-03-13
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dbDbLink.rs
    function: process_target
tags: [assertion, dbDbLink, RPRO, PUTF, proc-thread, lifecycle]
---
# dbDbLink processTarget: replace assert() with errlogPrintf for procThread mismatches

## Root Cause
`processTarget()` used `assert()` to verify that `procThread` was set to the
current thread before and after calling `dbProcess(pdst)`. If the logic was
wrong (e.g., a self-link or re-entrant processing case), the assert would abort
the IOC process rather than logging a diagnostic and continuing.

Also: the pre-assert for `!claim_src` checked `psrc->procThread == self`, but
the pre-assert for `!claim_dst` also checked `psrc->procThread` (copy-paste
error — should check `pdst`). This means the dst check was silently wrong.

## Symptoms
- IOC process abort via `assert()` when a record self-link or unusual
  re-entrant processing path hits the procThread invariant check.
- The copy-paste error in the dst pre-condition meant the dst invariant was
  never actually verified.

## Fix
Replace all asserts with `errlogPrintf("Logic Error: processTarget ...")`.
This degrades gracefully: log the problem and continue processing, rather than
crashing the IOC. The copy-paste error in the dst pre-assert is also fixed by
changing to a compound check: `if(psrc->procThread!=self || pdst->procThread!=self)`.

## Rust Applicability
Applies. In base-rs `dbDbLink.rs::process_target`, if there is a `procThread`
equivalent (a per-record "currently processing by" marker used for RPRO/PUTF
suppression of recursive links), verify that invariant violations are logged
(not panicked) and do not abort the server.

## Audit Recommendation
In `base-rs/src/server/database/dbDbLink.rs`: if `process_target` has a
recursive-processing guard, use `tracing::error!` on invariant violation rather
than `panic!` or `unwrap()`.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — assert → errlogPrintf, fix copy-paste dst check
