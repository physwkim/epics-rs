# chunk_03 — 9 APPLIES candidates / 22 N/A

(Agent flagged many IOC-related items as APPLIES that are actually N/A for pva-rs core, but should re-verify each one.)

## 57f79ce7 — APPLIES (UNVERIFIED) — dbChannel clobbering guard via RSET callback
## 382dd294 — APPLIES (UNVERIFIED) — Precision=0 filtering for integer types
## ff3e293a — APPLIES (UNVERIFIED) — Empty event rejection in group source (likely IOC, N/A)
## 88b67527 — APPLIES (UNVERIFIED) — Boolean variant support for record._options.process
## a9eea922 — APPLIES (UNVERIFIED) — Struct/union array relaxed assignment
## 7211143b — APPLIES (UNVERIFIED) — Idempotent re-finish() in MonitorControl
## 9b099be0 — APPLIES (UNVERIFIED) — Callback release in ServerOp cleanup (use-after-free)
## d8f7de8c — APPLIES (UNVERIFIED) — GET response prototype cache synchronization
## c06d4bb6 — APPLIES (UNVERIFIED) — Putorder enforcement for write authorization (IOC)
## ec0b21d2 — APPLIES (UNVERIFIED) — Dtor ordering: db_cancel_event before ~MonitorControlOp (IOC, use-after-free)

22 others: N/A (docs, refactors, IOC-only, C++-specific, libevent).
