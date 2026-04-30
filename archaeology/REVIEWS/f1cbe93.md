---
short_sha: f1cbe93
status: already-fixed
files_changed: []
---
The C revert restored `epicsTime::getCurrent()` (wall-clock) over a
broken `getMonotonic()` in CA timer/timeout paths. The Rust risk is the
inverse: timeout deadlines must use a monotonic source. Audit of
`crates/epics-ca-rs/src/client/` found timeout sites all built on
`std::time::Instant` (monotonic) — `client/search.rs` uses
`Instant`-keyed `BTreeSet<(Instant, u32)>` for `deadline_set`, scheduling
through `epics_base_rs::runtime::task::sleep_until`; the only
`SystemTime::now()` calls are (a) `search.rs:539` extracting `subsec_nanos`
purely as an entropy source for backoff jitter (never compared as a
deadline) and (b) `subscription.rs:85` stamping a `Snapshot.timestamp`
field for downstream consumers (wall-clock-by-design). The C bug class
is structurally absent. No `search_timer.rs`, `tcp_iiu.rs`, or `cac.rs`
exist (the CA client is structured around tokio tasks, not a timer
queue).
