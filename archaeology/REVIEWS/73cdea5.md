---
short_sha: 73cdea5
title: rsrv/asLib — rename asUseIP→asCheckClientIP, ignore client hostname when set
status: not-applicable
crate: base-rs
---

# Review

## Audit Targets
- `src/server/access_security.rs` (function: null) — file exists
  (`crates/epics-base-rs/src/server/access_security.rs`) but contains
  only the ACF parser + `check_access` evaluator. It has no
  `asCheckClientIP` / `asUseIP` setting and no protocol code that
  consumes a client-supplied hostname.

## Verification
- Grep across `epics-base-rs/src/`:
  - `asCheckClientIP`, `asUseIP`, `use_ip`, `check_client_ip` — 0 hits.
  - `host_name` / `hostname`: `pv.rs:62` (struct field), `env.rs`,
    `access_security.rs` (rule input arg only).
- The hostname/IP decision belongs to the protocol-server crates
  (ca-rs, pva-rs) that originate `host_name` from either a CA
  `HOST_NAME` command or the peer IP. base-rs only consumes whatever
  string the caller passes into `check_access`.

## Decision
**not-applicable** — the rename and policy switch live in the
CA-server message-handling layer (`camessage.c`,
`caservertask.c`), which is not part of `base-rs`. base-rs has no
local equivalent of `asCheckClientIP` to introduce or rename.

The corresponding audit item belongs to the ca-rs server.

## C Reference
- `modules/database/src/ioc/rsrv/camessage.c:host_name_action`
- `modules/database/src/ioc/rsrv/caservertask.c:create_tcp_client`
- `modules/libcom/src/as/asLib.h`,
  `modules/libcom/src/as/asLibRoutines.c`
</content>
</invoke>