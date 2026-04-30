## 91dd4d4 — N/A — pvxput: verbose flag show marked fields
**Reason**: Feature addition (verbose display option for marked fields during put).

## 42ec160 — N/A — pvxput: do not mark all fields
**Reason**: Pvxs field-marking logic has no equivalent in pva-rs; pvput-rs uses direct string→PV parsing with no selective field transport.

## 6f5b511 — N/A — pvxvct use endpoint
**Reason**: API refactor (SockAddr vs SockEndpoint); no semantic bug fix.

## dfd568e — N/A — pvxvct IP range parsing
**Reason**: Added nbit>32 validation to CIDR parsing; pva-rs filters only by exact IpAddr, not CIDR ranges.

## f0e76c0 — N/A — fix pvxlist help message
**Reason**: Documentation correction (usage example syntax).

## fa4294a — N/A — pvxmonitor option to show queueSize
**Reason**: Feature addition (new --queue-size flag).

## 9c233ea — N/A — typo
**Reason**: Comment typo in pvxlist.

## c141478 — N/A — pvxvct print beacons as info (shown by default)
**Reason**: Log-level change (Info vs Debug); display policy, not a bug.

## 6d8ec57 — N/A — Add pvxmshim
**Reason**: New tool addition; no applicability to pva-rs.

## 0e2a5ff — N/A — avoid pvxmonitor hang on interrupt
**Reason**: MPMCFIFO queue sizing workaround for libevent callback threading; pva-rs uses tokio async tasks with different concurrency model.

## d50c63c — N/A — pvxget
**Reason**: Initial implementation; not a bug fix.

## 6f39d9a — N/A — CLI tools print libevent version
**Reason**: Feature addition (version reporting).

## 3896c27 — N/A — CLI tools print version
**Reason**: Feature addition (version reporting).

## 9fc9457 — N/A — pvxget/monitor add formatting options
**Reason**: Feature addition (formatting flags).

## 52eb0d3 — N/A — pvxput minor
**Reason**: Trivial change; no semantic impact.

## bb53bb8 — APPLIES — fix pvxvct
**Reason**: Added PV name filtering to SEARCH callback; pva-rs accepts -P flag but does not filter Search output by PV names.
**pva-rs target**: crates/epics-pva-rs/src/bin/pvxvct-rs.rs:154-186
**Fix**: Add pvnames set to Args struct, populate from repeated -P args, and filter names Vec at line 179 before printing; also verify Beacon path (line 124+) filters by pvnames if provided.

## 024185e — N/A — pvxinfo usage
**Reason**: Documentation correction (usage example).

## 9691728 — N/A — fix Beacon decode
**Reason**: Changed skip offset M+=1 to M+=2 for u16 "change" field; pva-rs correctly uses get_u16(order) at line 131.

## 4b65a2f — N/A — cleanup debug
**Reason**: Removed debug log statement; no functional impact.
