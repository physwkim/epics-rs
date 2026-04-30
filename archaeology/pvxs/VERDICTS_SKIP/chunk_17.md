# SKIP Classification Verdicts — chunk_17

**Summary**: 30 commits analyzed. 28 N/A (94%), 0 APPLIES, 1 BUG-FIX (not propagated), 1 DUPLICATE.

---

## dff2db60 — N/A — add pvxcall
**Reason**: Tool creation (pvxcall CLI). No core logic change.

---

## 876a6150 — N/A — NTURI helper
**Reason**: NT type helper addition (nt.h/nt.cpp). Documentation/API convenience, not a bug fix.

---

## 6d717ad2 — N/A — testput
**Reason**: Test suite addition (test/testput.cpp). Testing infrastructure only.

---

## 3405e3c5 — N/A — client CMD_DESTROY_CHANNEL
**Reason**: Protocol state machine refinement (client channel cleanup on destructor). State-specific C++ PVA detail; pva-rs manages channel lifecycle differently via Rust ownership.

---

## 3c34887c — N/A — add pvxput
**Reason**: Tool creation (pvxput CLI).

---

## d8811703 — N/A — pvxget
**Reason**: Tool creation (pvxget CLI).

---

## a0b49437 — N/A — client pvRequest building
**Reason**: PVRequest syntax/builder refactor (src/clientreq.cpp). API surface change, not a hidden bug.

---

## 1ef1d0e1 — N/A — add Member::addChild
**Reason**: Data structure helper (pvdata/type.cpp). Minor API extension.

---

## b7544998 — N/A — shuffle client *Builder
**Reason**: API reorganization (client.h header layout). Refactoring only.

---

## b52065cc — N/A — client get/put/rpc
**Reason**: Feature implementation (core client operations added). New capability, not a fix.

---

## df837b5f — N/A — minor info
**Reason**: Single-line comment fix (clientintrospect.cpp:1 change).

---

## f76f1c8c — BUG-FIX — client info cancel
**Reason**: Fixes leak/hang when canceling info operation. Converts `assert(state==Connecting)` → guard + early return, and adds proper channel reset on cancel. **Not propagated to pva-rs** (no direct ops_v2 info operation cancel handler with equivalent state-race protection).
**pva-rs target**: crates/epics-pva-rs/src/client_native/ops_v2.rs — info operation cancel path.
**Fix**: Add guard + context cleanup on info cancel to prevent state-race between user cancel() call and background connection task.

---

## cfa18525 — N/A — update Config
**Reason**: Configuration structure refactoring + expand() semantics (config.cpp). API design, not a bug fix.

---

## bd1cbe1b — BUG-FIX — client Context safety
**Reason**: Adds null-pointer checks to Context::config() and Context::hurryUp(). Prevents crash when Context is used after pvt deletion.  **Not critical for pva-rs** — Rust ownership model prevents null pointers; equivalent safety is compile-time guaranteed.

---

## 1a4e6e8d — N/A — hurryUp()
**Reason**: Search acceleration feature (introduces hurryUp() with rate limiting). Feature implementation, not a bug fix.

---

## baa851c4 — N/A — client doc
**Reason**: Documentation additions (client.rst + doxygen in client.h).

---

## 1edeab8a — N/A — start client
**Reason**: Major feature: entire client implementation (clientget/clientconn/clientreq/config). Feature introduction, not a fix.

---

## 1aa1b56a — N/A — add from_wire_type()
**Reason**: Data wire decoding helper (dataimpl.h). API addition.

---

## 6504466b — N/A — separate basic TCP RX handling
**Reason**: Code extraction (serverconn → conn + serverconn, TCP parsing refactor). Refactoring/modularity, not a bug fix.

---

## 8c646cd6 — N/A — more log
**Reason**: Logging additions (evhelper.cpp + server.cpp). Observability only.

---

## 65e3af5e — N/A — minor
**Reason**: Add consumed() helper method to VectorOutBuf (pvaproto.h:3 lines). API convenience.

---

## 99ce43c0 — N/A — prefer log_*_printf()
**Reason**: Logging macro migration across 16 files. Code style/convention, not a bug fix.

---

## d6a4865a — N/A — server config expand
**Reason**: Config expansion semantics refactoring (config.cpp:88 lines). Configuration logic reorganization.

---

## e3e9655f — N/A — separate server Config
**Reason**: Config separation (server vs. client config extraction). Module refactoring.

---

## 11185667 — N/A — server no need to track chanByCID
**Reason**: Data structure optimization (remove serverchan.cpp map). Memory efficiency, no functional change.

---

## 09bd75b2 — N/A — misc
**Reason**: Comment removal (dataimpl.h:1 line, "duplicates associated FieldDesc::code"). Trivial cleanup.

---

## 4b5439a4 — N/A — print server Config
**Reason**: Diagnostic print helper for server Config (server.cpp:35 lines). Observability.

---

## 9cf21933 — BUG-FIX — minor (SharedPV buildReadonly)
**Reason**: Adds read-only rejection handler to SharedPV::buildReadonly() — ensures PUT to readonly PV returns error message instead of silently failing. **Not in pva-rs** — no public buildReadonly() equivalent; SharedPV::on_put() handler pattern exists but readonly semantic not implemented.
**pva-rs target**: crates/epics-pva-rs/src/server_native/shared_pv.rs — SharedPV API.
**Fix**: Provide buildReadonly() factory or on_put-based decorator that rejects all puts.

---

## ab58b58d — N/A — Server add builtin Source
**Reason**: Source abstraction + builtin-source trait. Architecture/feature, not a bug fix.

---

## 7c8e72f5 — BUG-FIX — minor (SharedPV buildMailbox timestamp check)
**Reason**: Adds null guard before calling isMarked() on timeStamp field: `if(ts && !ts.isMarked(...))`. Prevents crash when timeStamp not present in mailbox put. **Not protected in pva-rs** — no equivalent null guard in mailbox-put codepath.
**pva-rs target**: crates/epics-pva-rs/src/server_native/shared_pv.rs — put dispatch handling timeStamp.
**Fix**: Guard access to optional timeStamp field before calling methods on it; skip auto-timestamp only when field exists and is unmarked.

