# Chunk 06 SKIP Classification Verdicts

## d692d7da — N/A — loc_bad_alloc

**Reason:** Exception-handling test infrastructure & debug output improvements. Touches util/unittest internals and error allocation paths.

---

## 798a1548 — N/A — 1.1.3a1

**Reason:** Version bump & release notes only.

---

## 36e537e — N/A — appease -Wnoexcept

**Reason:** C++-specific compiler warning (noexcept specifier). pvxs/Rust divergence.

---

## 6f770d0 — N/A — final-ize some derived types

**Reason:** C++-specific virtual destructor `final` qualifiers. No Rust equivalent.

---

## fedbec6 — N/A — server: rework cleanup of connection, channel, and operation

**Reason:** Refactoring connection/channel lifecycle management into dedicated `cleanup()` methods. pva-rs uses async task cancellation (tokio AbortHandle) instead of explicit cleanup() calls; lifecycle is implicit in Arc/RwLock drop semantics.

---

## 4ced2d8 — N/A — print char types as integers

**Reason:** Test/debug output formatting for unittest assertions. Rust has different debug trait implementations.

---

## 1aad37c — N/A — minor

**Reason:** Code comment/whitespace only.

---

## da2737f — N/A — minor

**Reason:** Header comment removal. No functional impact.

---

## 6a56017 — N/A — 1.1.2a1

**Reason:** Version bump & release notes.

---

## f75bcc5 — N/A — constify OpBase and friends

**Reason:** C++-specific const qualifier propagation. Rust borrowing rules enforce immutability at compile time; not a functional issue.

---

## a9699be — N/A — minor

**Reason:** Test coverage script & single-line instrumentation.

---

## 66441a4 — N/A — doc

**Reason:** Documentation only.

---

## a7ce56a — N/A — add shared_array::thaw()

**Reason:** New C++-API for shared array mutation. pva-rs uses Value type system; equivalent functionality is orthogonal.

---

## e7d7f18 — N/A — Value::lookup throw NoField

**Reason:** Exception behavior change. pva-rs returns Result<T> for lookups; Rust semantics differ.

---

## db6b7ba — N/A — minor

**Reason:** Minor config/logging adjustments.

---

## 2ea141a — N/A — doc

**Reason:** Documentation only.

---

## af20a88 — N/A — 1.1.0

**Reason:** Version bump & release notes.

---

## ac0f794 — N/A — Add MonitorStat::maxQueue

**Reason:** Monitoring statistic API. pva-rs server::Source trait does not expose queue depth metrics.

---

## 53f83b6 — N/A — Add Value::clear()

**Reason:** New C++-API for Value type clearing. pva-rs constructor/Default handles initialization differently.

---

## febc823 — N/A — Client subscription add batch pop() and stats()

**Reason:** New C++-API for client subscription batch operations. pva-rs async client uses tokio channels; batch semantics not applicable.

---

## a4c6540 — N/A — Add TypeDef::as() overload to change Struct -> StructA

**Reason:** C++-type conversion API. pva-rs FieldDesc is recursive enum; no structural conversion needed.

---

## e38197 — N/A — Make FieldDesc partly const

**Reason:** C++-const correctness refactor. Not relevant to Rust.

---

## d6fe9c7 — N/A — minor

**Reason:** Single-line typo/comment fix.

---

## 7610575 — APPLIES — reduce "non-existent IOID" noise

**Reason:** Adds `rxRegistryDirty` flag to track decode-sync issues when server sends stale IOIDs. Prevents confusing error logs with context about potential registry corruption.

**pva-rs target:** `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/proto/message.rs` + client decoder loop

**Fix:** pva-rs client decode handlers (search/monitor get) should set a dirty flag when receiving unknown IOID, add hint to decode-error messages like "registry may be dirty".

---

## 89c02b4 — N/A — allow TypeDef to append StructA and UnionA

**Reason:** C++-type system API extension. pva-rs FieldDesc construction is different.

---

## 4164d1b — N/A — client: explicit ctor for builders

**Reason:** C++-specific: fixes C++17 implicit-to-explicit constructor ambiguity. No Rust parallel.

---

## 1591a78 — N/A — ConnectBuilder add default ctor

**Reason:** C++-API convenience feature. pva-rs builders use builder pattern; defaults are constructor args.

---

## de7e8c3 — N/A — Add ConnectBuilder::server()

**Reason:** New C++-API for explicit server override. pva-rs ConnectBuilder has `address()` + search integration; behavior is different.

---

## 3f3e394 — APPLIES — Limit beaconTrack by size as well as time

**Reason:** Prevents unbounded beacon-tracking map growth during server restart storms. pvxs: 20k GUID×proto×address entries max; drops oldest on overflow. pva-rs beacon_throttle.rs does NOT have size limits.

**pva-rs target:** `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/client_native/beacon_throttle.rs`

**Fix:** Add max-size cap (e.g., 20k entries) to BeaconTracker::observe(); prune oldest-seen entries when at limit, log debug message on drop.

---

## 298689a — N/A — minor

**Reason:** Local variable whitespace/style only.
