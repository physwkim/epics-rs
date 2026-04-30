---
short_sha: cae597d
title: CA client suppresses repeated UDP send error messages per destination
status: applied
severity: medium
crate: ca-rs
files_changed:
  - crates/epics-ca-rs/src/client/search.rs
---

# Review: cae597d — Per-destination UDP search send-error dedup

## Verdict
applied — `client/search.rs::send_with_fanout` was logging every
`send_to` failure at `tracing::debug!`, which (a) hides the failure
from default-level operators and (b) at debug level still produces one
log entry per send when a destination is persistently unreachable
(active search runs ~every 30 ms during reconnection storms).

## Fix
`crates/epics-ca-rs/src/client/search.rs`:
- New `send_errors: HashMap<SocketAddr, std::io::ErrorKind>` field on
  `SearchEngineState`. Mirrors libca `udpiiu::SearchDestUDP::_lastError`.
- `send_with_fanout` now takes `&mut send_errors`. On error: log at
  `tracing::warn!` only when the error kind differs from the last
  recorded kind (first occurrence + change). On success: if a previous
  error was recorded, emit `tracing::info!("search send_to: recovered")`
  and clear the entry.
- Updated the three call sites in `send_due_searches` to thread
  `&mut state.send_errors`.

## C reference
`modules/ca/src/client/udpiiu.cpp:udpiiu::SearchDestUDP::searchRequest`
— added the `_lastError` member and the same dedup-on-change + recovery
log pattern; `udpiiu.h` adds `int _lastError = 0` field.

## Build
`cargo check -p epics-ca-rs` — clean.
