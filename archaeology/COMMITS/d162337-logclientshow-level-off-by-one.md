---
sha: d162337b9a52a860e6c4e079f2f147163d3f7226
short_sha: d162337
date: 2019-09-17
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [log-client, diagnostics, show, level, off-by-one]
---

# logClientShow displays socket info at wrong verbosity level

## Root Cause
`logClientShow` used `if (level > 1)` to gate the display of socket status and
connect cycle count, but `level > 1` means the information only appears at level
2 or above. Standard EPICS diagnostic convention is that `level > 0` (i.e.
level >= 1) shows basic status. The socket info was effectively hidden at the
most common diagnostic level (1).

Additionally the prefix display and the socket/count display were in the wrong
order relative to each other.

## Symptoms
Calling `logClientShow(id, 1)` would show only the connection state ("connected
to ..." or "disconnected") but not the socket status or connect cycle count,
making single-level diagnostics less informative than intended.

## Fix
- Change `if (level > 1)` to `if (level > 0)` for socket/connect-count display.
- Move the prefix display before the level-gated blocks.
- Add `if (level > 1)` block showing unsent byte count and buffer contents.

## Rust Applicability
No equivalent API exists in the Rust log module (tokio-based log clients use
`tracing` or structured logging without an explicit `show` function). Eliminated.

## Audit Recommendation
No action needed.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientShow` — level > 1 should be level > 0 for basic status
