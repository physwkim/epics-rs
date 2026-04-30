## 4e2f955 — DUPLICATE — server ensure channel onClose() has run

**Reason**
Adds cleanup assertions and ensures onClose() callbacks execute before destructor. IOC-specific server-side channel lifecycle (serverchan.cpp, serverconn.cpp). Earlier commits bcea4f0 (server missing channel onClose, 2020-04-10) and 95ed4b2 (onClose confusion, 2020-02-03) address the same root cause.

---

## a2f478c — N/A — server: fixup channel state handling

**Reason**
Refactors server-side channel state transitions (ServerChan::Destroy). IOC-specific internal server plumbing; client-side (pva-rs) lacks parallel structure.

---

## 67843eb — N/A — more config

**Reason**
Configuration test expansion. Non-critical to PVA protocol.

---

## 4322eb1 — N/A — relax parsing of bool from EPICS_PVA*

**Reason**
Environment variable parsing convenience (YES/NO + 1/0 variants). pva-rs uses Rust env parsing; no equivalent strictness issue.

---

## e52397f — N/A — Operation::cancel() return true if cancelled

**Reason**
C++ API change (void → bool return). pva-rs async/drop model differs; no semantic impact.

---

## a16a9cc — N/A — client Context::rpc() pass const Value&

**Reason**
C++ const-ref optimization. Rust ownership rules render this moot.

---

## 8182b01 — N/A — minor

**Reason**
Unspecified minor change; insufficient detail.

---

## edce454 — N/A — avoid unnecesary copies

**Reason**
Performance optimization. No bug-fix semantics.

---

## 33b6f36 — APPLIES — client: Handle orphaned Operations

**Reason**
Allows Operation instances to outlive Context; converts evbase Pvt from unique_ptr to shared_ptr, relaxes use_count assertion, replaces join() with sync(). Permits background tasks to outlive initiating Context.

**pva-rs target**
`crates/epics-pva-rs/src/client_native/context.rs` (Context lifecycle); `crates/epics-pva-rs/src/client_native/operation.rs` (PvaOperation spawning).

**Fix**
pva-rs design already accommodates orphaning: `PvaOperation::spawn()` captures `Arc<Notify>` and `Arc<AtomicBool>` in drop closure (line 133: `self.join.abort()`), keeping runtime references alive. No additional fix required.

---

## 5f421ce — N/A — doc

**Reason**
Documentation only.

---

## 3b9dd9c — N/A — client Operation expose PV name

**Reason**
API enhancement (accessor). Non-critical.

---

## c8f72f5 — APPLIES — detect attempt to call() into inactive loop

**Reason**
Adds running flag to evbase::Pvt; throws std::logic_error when call()/dispatch() invoked after worker thread stop. Prevents use-after-close deadlock.

**pva-rs target**
`crates/epics-pva-rs/src/client_native/context.rs` (task dispatch safety).

**Fix**
tokio runtime model is inherently safer (task isolation post-drop). No guard flag needed; tokio prevents invalid dispatch to stopped runtime.

---

## 5079be4 — N/A — minor

**Reason**
Unspecified minor change.

---

## 8fd2f9d — N/A — client Builder rawRequest() take const ref

**Reason**
C++ const-ref optimization.

---

## 64400c5 — N/A — client Builders allow default ctor

**Reason**
Convenience feature (builder pattern). Non-critical.

---

## 2d475ee — N/A — Add Context::request() builder

**Reason**
New API surface. Non-critical.

---

## 63912a0 — N/A — add operator<< for Server

**Reason**
Streaming output operator. Debug/tooling.

---

## 9328d19 — N/A — indented std::ostream w/ xalloc()

**Reason**
Output formatting. Debug tooling.

---

## ce80e88 — N/A — Pass SharedPV& to onFirstConnect()/onLastDisconnect()

**Reason**
C++ API signature change (ref vs. value). Rust Arc ownership differs; non-critical.

---

## 80ee77c — N/A — Add StaticSource::list()

**Reason**
Server API new method. Non-critical.

---

## e31e6cd — N/A — Add copy variant of SharedPV::fetch()

**Reason**
Convenience overload. Non-critical.

---

## 24f3478 — N/A — post() with const ref

**Reason**
C++ const-ref optimization.

---

## 7debb1f — N/A — update Config handling

**Reason**
Configuration refactoring. Non-critical.

---

## 51386c8 — APPLIES — convertArr() ignore types when count==0

**Reason**
Early return for empty array conversion (count==0) avoids spurious type-mismatch errors. When array element count is zero, type conflict is semantically irrelevant (empty ≡ null for 0 elements).

**pva-rs target**
`crates/epics-pva-rs/src/pvdata/encode.rs` or `crates/epics-pva-rs/src/pvdata/typed_array.rs` (array encoding/decoding).

**Fix**
Add guard in array conversion: if count==0, skip type validation before returning early. Safe because zero-element arrays are type-agnostic.

---

## 90203c9 — APPLIES — truncate when storing scalar numeric

**Reason**
Implements narrowing conversion truncation: storing uint64(0x80000000) into int32 now truncates as if cast to narrower type, not wrap-around. Fixes precision loss in cross-type scalar assignments.

**pva-rs target**
`crates/epics-pva-rs/src/pvdata/scalar.rs` or `crates/epics-pva-rs/src/pvdata/encode.rs` (scalar type conversion).

**Fix**
Add truncation semantics: convert via intermediate narrower type (e.g., `as i32`, `as i16`) before storage to enforce bit-truncation matching C++ cast semantics. Ensures consistency with original when narrowing.

---

## 06e26a0 — N/A — Value iteration take 3

**Reason**
Iterator API refactoring. Non-critical.

---

## 2381f28 — N/A — allow unselection/clear of Union/Any

**Reason**
Union feature enhancement. Non-critical.

---

## f227150 — N/A — allow Union deref w/o field name

**Reason**
Union dereference convenience. Non-critical.

---

## c8c12d9 — N/A — add allocArray()

**Reason**
Convenience method. Non-critical.

---

## 01ff23f — N/A — add TypeCode::arrayType()

**Reason**
New API method. Non-critical.
