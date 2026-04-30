---
short_sha: eeb198d
title: arrRecord — pfield assignment must move from cvt_dbaddr to get_array_info
crate: base-rs
status: not-applicable
---

# Review

## Verdict
**not-applicable** — base-rs does not expose array-buffer pointers at
address-resolution time, so the cached-stale-pointer footgun cannot occur.

## Evidence
- `crates/epics-base-rs/src/server/database/db_access.rs` is a high-level
  Rust API (`DbChannel`, `DbSubscription`, `DbMultiMonitor`). It contains no
  `cvt_dbaddr`, no `paddr`, no `pfield`, and no `bptr`-equivalent: a grep for
  `cvt_dbaddr|cvt_db_addr|pfield|paddr` returns zero hits in the crate.
- Reads go through `db.get_pv(name).await` which returns an owned
  `EpicsValue` snapshot taken under the record's tokio `RwLock` at access time
  — there is no "address" object that caches a raw pointer between
  resolution and dbGet.
- The waveform record (`src/server/records/waveform.rs`) stores its data in
  owned Rust collections and has no `bptr` re-allocation API to invalidate.

## Decision
The C bug pattern (cache `prec->bptr` in `paddr->pfield` at name-to-addr
resolution, then dereference much later when the buffer may have been
reallocated) cannot occur in a model that snapshots values per-access under
the scan lock. No fix to apply. If a low-level `dbAddr`-style cached handle
is ever added, defer pointer capture until the read happens inside the
record's lock.
