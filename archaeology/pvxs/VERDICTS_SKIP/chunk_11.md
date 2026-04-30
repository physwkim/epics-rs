## 26cf7f00 — N/A — doc

**Reason**: Documentation update (COPYRIGHT, conf.py, source.rst, client.h).

---

## ae3fc6a3 — N/A — server: separate logger for mailbox put handler

**Reason**: Logging enhancement (adds logmailbox, improves debug output detail). pva-rs does not yet expose mailbox-specific logging instrumentation.

---

## 17bdc2c2 — N/A — doc

**Reason**: Documentation update (value.rst, sharedArray.h header comments).

---

## 48ae394a — APPLIES — server: correct ExecOp::op()

**Reason**: Critical bug: ExecOp constructor hardcoded `_op = Info` instead of reading the command type (GET/PUT/RPC). This causes all execute operations to report wrong type.

**pva-rs target**: `crates/epics-pva-rs/src/server_native/tcp.rs` handle_op() + OpState.kind field

**Fix**: pva-rs correctly captures OpKind in the closure passed to handle_op (Get/Put/Monitor/Rpc per command). OpState.kind is set at construction from the parameter. No bug present—pva-rs avoids the hardcoded default.

---

## b9b1a609 — APPLIES — server: correct ConnectOp::op()

**Reason**: Critical bug: ConnectOp constructor hardcoded `_op = Info` instead of reading command type. This causes all channel-create operations to report wrong type.

**pva-rs target**: `crates/epics-pva-rs/src/server_native/tcp.rs` handle_op() + OpState.kind field

**Fix**: pva-rs correctly passes OpKind::* as a parameter to handle_op and stores in OpState.kind. No hardcoded default—the kind is set from the incoming command (Get/Put/Monitor/Rpc).

---

## 6aa1c76c — N/A — Server: prefix special/automatic Sources with __

**Reason**: API/naming convention (renames "builtin"→"__builtin", "server"→"__server" source names). pva-rs has no IOC-style Source registry (uses trait DynSource).

---

## 178fbebb — N/A — evhelper: loop self-join is crit

**Reason**: Log-level bump (log_err→log_crit for self-join race). pva-rs uses tokio runtime; no evhelper equivalent.

---

## a051c27877 — N/A — evbase constify

**Reason**: Constify methods (join/sync/dispatch/call/assertInLoop/inLoop). pva-rs uses tokio runtime; no evbase equivalent.

---

## 9481dcd8 — N/A — avoid including event2 headers from our public headers

**Reason**: Header hygiene (move event2/util.h to private header). pva-rs has no libevent dependency (uses tokio).

---

## 2ad25b5e — N/A — include channel names in Server::operator<<

**Reason**: Logging/debug output format (add channel name to Server display). pva-rs does not expose operator<< Debug printing for Server.

---

## c7b915b6 — N/A — typo

**Reason**: Typo fix (copyIn→tryCopyIn call in Value::tryFrom template). pva-rs uses different field assignment API.

---

## 45ce8e96 — N/A — add PVXS_ABI_VERSION

**Reason**: Feature addition (version macro + docs). pva-rs cargo versioning handles ABI.

---

## 6dbfd87a — N/A — minor

**Reason**: Comment/formatting in src/pvxs/source.h. No functional change.

---

**Summary**: 2 bugs found (ExecOp::op, ConnectOp::op), both properly avoided in pva-rs by design (OpKind passed as parameter, no hardcoded default). Remaining 10 commits are docs, features, refactors, or infrastructure (evbase/event2).
