# epics-base Archaeology — By Crate
Generated: 2026-04-30 | Applies + Partial entries

## ca-rs

### `2bcaa54` — CA UDP: memcpy with non-null extsize but null pExt pointer — null dereference
- **Date**: 2020-02-12 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `push_datagram_msg`
- **Audit Rec**: Audit `ca-rs/src/client/udp.rs::push_datagram_msg` (or equivalent UDP frame
builder):
1. Confirm extension data is typed as `Option<&[u8]>` or `&[u8]` (never a
   separate length + raw pointer).
2. Verify that the ext-length field in the wire header always matches the
   actual slice length.
3....

### `87acb98` — CA hostname length limit overflow when parsing EPICS_CA_ADDR_LIST
- **Date**: 2022-08-20 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/client/addr_list.rs` | **Function**: `add_addr_to_ca_address_list`
- **Audit Rec**: Audit `ca-rs/src/client/addr_list.rs` (or equivalent env-parsing code). Verify:
1. No fixed-length buffer is used for hostname tokens.
2. Each token from the space-split `EPICS_CA_ADDR_LIST` is passed in full to DNS resolution.
3. Very long tokens (>253 chars) are rejected with a proper error, not...

### `717d69e` — dbCa: iocInit must wait for local CA links to connect before PINI
- **Date**: 2025-09-20 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/client/db_ca.rs` | **Function**: `db_ca_run`
- **Audit Rec**: In `base-rs/src/server/database/db_ca.rs::db_ca_run` (or the async
`ioc_init` state machine): confirm there is a barrier that awaits all local CA
link connections before advancing to the `AfterIocRunning` hook. The barrier
must be deferred until after subscribe AND attribute fetch complete, not...

### `51191e6` — Linux IP_MULTICAST_ALL Default Causes Unintended Multicast Reception
- **Date**: 2021-08-04 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `create_udp_socket`
- **Audit Rec**: In `ca-rs/src/client/udp.rs` and `pva-rs/src/net/udp.rs`, after `UdpSocket::bind()`,
add a Linux-only `setsockopt(IPPROTO_IP, IP_MULTICAST_ALL, 0)`. Use
`#[cfg(target_os = "linux")]`. Check whether the `socket2` crate exposes this;
if not, use `libc::setsockopt` directly. This matches the existing...

### `530eba1` — rsrv: use verified client IP address instead of client-supplied hostname
- **Date**: 2018-06-16 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/server/client.rs` | **Function**: `create_tcp_client`
- **Audit Rec**: In `ca-rs/src/server/client.rs`, verify that the client's hostname used for
access-security evaluation is derived from the verified socket peer address
(not the CA `HOST_NAME` message) when IP-mode is active.  In
`base-rs/src/server/database/access_security.rs`, ensure HAG host entries
are resolved...

### `772c10d` — RSRV_SERVER_PORT Truncated for Port Numbers Above 9999
- **Date**: 2024-06-14 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/server/rsrv.rs` | **Function**: `rsrv_init`
- **Audit Rec**: In `ca-rs/src/server/rsrv.rs` (or equivalent init function): verify that the
port number formatted for `RSRV_SERVER_PORT` uses the full port value without
any manual buffer size reduction. Search for `env::set_var("RSRV_SERVER_PORT"`.

### `97bf917` — caRepeater does not join multicast groups — misses multicast CA beacons
- **Date**: 2020-02-12 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/client/repeater.rs` | **Function**: `ca_repeater`
- **Audit Rec**: Audit `ca-rs/src/client/repeater.rs` (or equivalent beacon-listen path):
1. Confirm the repeater socket calls `join_multicast_v4` for every multicast
   address in the beacon address list.
2. Verify the address-range check (224.0.0.0–239.255.255.255) is correct.
3. Confirm errors from...

### `410921b` — Network interface enumeration: replace SIOCGIFCONF with getifaddrs
- **Date**: 2021-01-07 | **Category**: network-routing | **Severity**: high | **Verdict**: partial
- **File**: `src/client/net_intf.rs` | **Function**: `discover_broadcast_addresses`
- **Audit Rec**: Audit `ca-rs/src/client/net_intf.rs` or equivalent: (1) verify `getifaddrs`
or `if-addrs` is used (not `SIOCGIFCONF`), (2) check that IFF_BROADCAST
filtering preserves the match-address logic from `osiSockDiscoverBroadcastAddresses`,
(3) verify loopback exclusion is correct, (4) check that the...

### `71e4635` — testdbCaWaitForEvent: race between event destroy and CA context flush
- **Date**: 2025-10-17 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/client/db_ca.rs` | **Function**: `db_ca_sync`
- **Audit Rec**: In `ca-rs/src/client/db_ca.rs::db_ca_sync` (or its async equivalent): verify
it flushes both the dbCa worker queue AND the CA client context's I/O event
queue before returning. In any test utilities that wait for CA callbacks,
ensure the sync call precedes any shared state destruction (equivalent...

### `7a6e11c` — RSRV: guard casStatsFetch and casClientInitiatingCurrentThread against uninitialized state
- **Date**: 2025-02-06 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/stats.rs` | **Function**: `cas_stats_fetch`
- **Audit Rec**: In `ca-rs/src/server/stats.rs::cas_stats_fetch` (or the equivalent Rust stats
function), verify that calling it before server initialization returns zero
counts, not panics or undefined behavior. In
`ca-rs/src/server/camessage.rs::cas_client_initiating_current_thread`, verify
the thread-local is...

### `a864f16` — dbCa Test Sync Race: Missing Refcount and Wrong Lock Release Order
- **Date**: 2024-06-11 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/client/link.rs` | **Function**: `testdb_ca_wait_for_event`
- **Audit Rec**: In `ca-rs/src/client/link.rs`: audit all functions that (a) read a `CaLink`
reference from a shared structure, (b) release the parent lock, and (c)
subsequently use the reference — ensure an Arc clone is held across the gap.
Also audit `db_ca_sync` equivalent for correct lock release ordering...

### `7b6e48f` — casw uses monotonic clock for beacon timestamps — wrong clock domain
- **Date**: 2020-02-11 | **Category**: timeout | **Severity**: high | **Verdict**: applies
- **File**: `src/bin/casw.rs` | **Function**: `main`
- **Audit Rec**: Audit `ca-rs/src/bin/casw.rs::main` and any beacon receive/process path:
1. Confirm beacon arrival timestamps use `SystemTime::now()` not `Instant::now()`.
2. Check beacon hash-table entries — verify stored times are wall-clock.
3. If inter-beacon interval is computed for anomaly detection, ensure...

### `f1cbe93` — Revert getMonotonic() → getCurrent() in CA timers and timer queue
- **Date**: 2020-04-23 | **Category**: timeout | **Severity**: high | **Verdict**: applies
- **File**: `src/client/search_timer.rs` | **Function**: `schedule_next`
- **Audit Rec**: - Search `ca-rs/src/client/` for any `SystemTime::now()` or `chrono::Utc::now()`
  used as a timeout deadline — replace with `tokio::time::Instant::now()`.
- Verify `search_timer.rs`, `tcp_iiu.rs`, and `cac.rs` all use
  `tokio::time::sleep_until` / `tokio::time::Instant` for their retry intervals.

### `c5012d9` — Make sure epicsInt8 is signed on all architectures
- **Date**: 2021-12-17 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/client/com_buf.rs` | **Function**: `push_int8`
- **Audit Rec**: - In `ca-rs/src/client/com_buf.rs`: verify `push_dbf_char` / analogous function
  uses `i8` not `u8` when writing CA type 4 data.
- In `ca-rs/src/client/com_que_recv.rs`: verify `copy_out_bytes` for DBF_CHAR
  reads back as `i8`.
- In `base-rs/src/server/database/db_types.rs`: verify `DBF_CHAR`...

### `8cc2039` — Fix dbr_size_n macro: COUNT==0 must yield base size, not zero
- **Date**: 2020-06-05 | **Category**: wire-protocol | **Severity**: high | **Verdict**: applies
- **File**: `src/client/codec.rs` | **Function**: `dbr_size_n`
- **Audit Rec**: In `ca-rs/src/client/codec.rs`, find the payload-size calculation for DBR
response decoding. Confirm that `count=0` results in reading exactly 0 value
bytes from the wire, not 1 element's worth of (potentially garbage) bytes.

### `08b741e` — CA Repeater: Fallback to In-Process Thread When exec Fails
- **Date**: 2021-04-19 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/repeater.rs` | **Function**: `start_repeater_if_not_installed`
- **Audit Rec**: In `ca-rs/src/client/repeater.rs`, verify that `start_repeater_if_not_installed()`
falls back to an in-process repeater task whenever `Command::spawn()` returns
`Err(...)`, including the common `NotFound` / `PermissionDenied` cases.
Check that the fallback task is actually spawned (not just a...

### `5b37663` — aToIPAddr crashes on NULL input string
- **Date**: 2020-08-06 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/addr_list.rs` | **Function**: `null`
- **Audit Rec**: In `src/client/addr_list.rs` (ca-rs) and `src/config/env.rs` (pva-rs):
search for environment variable reads for `ADDR_LIST` config. Verify
the result is `Option`-wrapped and that `None` (unset env var) is handled
gracefully (empty list, not panic or unwrap). Check for any `.unwrap()`
or direct...

### `6dba2ec` — caRepeater inherits parent stdin/out/err — causes problems when spawned by caget
- **Date**: 2020-02-13 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/bin/carepeater.rs` | **Function**: `main`
- **Audit Rec**: Audit `ca-rs/src/bin/carepeater.rs::main`:
1. Check whether the binary redirects stdio to `/dev/null` on POSIX.
2. If spawned via `Command::new().spawn()` from another binary, verify
   `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())` is set
   on the `Command` to avoid...

### `19146a5` — WIN32: Disable SO_REUSEADDR for Windows sockets
- **Date**: 2020-06-19 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `bind_udp_socket`
- **Audit Rec**: In ca-rs and pva-rs, audit every `set_reuse_address(true)` call site. On
Windows builds, either skip the call entirely (use `#[cfg(not(windows))]`) or
use `SO_EXCLUSIVEADDRUSE` instead, which is the Windows-idiomatic way to
prevent port sharing. Check both TCP listener bind and UDP multicast bind...

### `5064931` — Datagram fanout socket: must set both SO_REUSEPORT and SO_REUSEADDR on Linux
- **Date**: 2020-02-05 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `bind_repeater_socket`
- **Audit Rec**: Audit all UDP socket creation paths in ca-rs that call
`set_reuse_port` or `set_reuse_address`:
1. Confirm **both** are called, not just one.
2. Check the order: set options before `bind()`.
3. Verify on macOS/BSD (where `SO_REUSEPORT` semantics differ from Linux)
   the behaviour is still correct.

### `65ef6e9` — POSIX datagram fanout: SO_REUSEADDR insufficient on BSD — need SO_REUSEPORT
- **Date**: 2020-01-12 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `bind_repeater_socket`
- **Audit Rec**: Audit UDP socket creation in ca-rs:
1. Confirm `set_reuse_port(true)` is called on all POSIX targets for fanout sockets.
2. Verify the `#[cfg(unix)]` guard is in place to avoid WIN32 compilation errors.
3. Cross-reference with 5064931 — both `set_reuse_port` and `set_reuse_address`
   should be set...

### `951b6ac` — Cygwin missing TCP_NODELAY declaration causes CA build failure
- **Date**: 2020-08-03 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/tcp.rs` | **Function**: `null`
- **Audit Rec**: In `src/server/tcp.rs` (ca-rs): search for `set_nodelay`. Verify it is
called on every accepted socket (line 432 was found). In
`src/client/transport.rs` (ca-rs): verify `set_nodelay` is called after
`TcpStream::connect`. In `src/server_native/tcp.rs` (pva-rs): same check.
If any accept/connect...

### `c23012d` — CA server (rsrv) suppresses repeated beacon UDP send error messages
- **Date**: 2018-01-30 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/online_notify.rs` | **Function**: `rsrv_online_notify_task`
- **Audit Rec**: Audit the ca-rs beacon send loop. Confirm per-destination error deduplication is present. The recovery log ("ok after error") is optional but good practice.

### `cae597d` — CA client suppresses repeated UDP send error messages per destination
- **Date**: 2018-11-14 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/udp.rs` | **Function**: `search_request`
- **Audit Rec**: Audit `ca-rs/src/client/udp.rs` UDP send path. Confirm that repeated identical send errors are deduplicated (log only on first occurrence and on recovery). If the current implementation logs unconditionally, add a `last_error` field per destination.

### `a42197f` — CA client: Allow writing zero-element arrays via caput
- **Date**: 2020-06-08 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/channel.rs` | **Function**: `write`
- **Audit Rec**: In `ca-rs/src/client/channel.rs:write`, check for a `count == 0` early-return
or error guard that would block empty-array puts. Also verify that the CA
message encoder correctly encodes `count=0` (payload = just the DBR fixed
header, no value bytes).

### `cd0e6a4` — caProto.h uses IPPORT_USERRESERVED without including its definition
- **Date**: 2021-02-05 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/proto.rs` | **Function**: `null`
- **Audit Rec**: Audit `ca-rs/src/client/proto.rs` or constants file: confirm that
`CA_SERVER_PORT = 5064` and `CA_REPEATER_PORT = 5065` are hardcoded
constants, not derived from a platform-specific `IPPORT_USERRESERVED`.

### `d763541` — CA client: expose server protocol minor version via ca_host_minor_protocol()
- **Date**: 2025-10-08 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/client/channel.rs` | **Function**: `host_minor_protocol`
- **Audit Rec**: Audit `ca-rs/src/client/channel.rs` (or `tcpiiu.rs`):
1. Verify the minor protocol version is stored after parsing the server's
   `VERSION` reply.
2. Add a public `host_minor_protocol() -> Option<u16>` method that returns
   `None` for disconnected channels and `Some(version)` once connected.

### `a352865` — Print ANSI-colored error prefix to stderr in udpiiu and tools
- **Date**: 2023-10-27 | **Category**: other | **Severity**: low | **Verdict**: partial
- **File**: `src/client/udpiiu.rs` | **Function**: `exception_resp_action`
- **Audit Rec**: In `ca-rs/src/client/udpiiu.rs`, verify that exception response error messages use `tracing::error!()` or equivalent rather than `eprintln!()`. Similarly audit `caput` CLI equivalent in `ca-rs` tools if present.

### `e4a81bb` — Document zero and NaN timeout semantics for CA and epicsEvent APIs
- **Date**: 2022-01-04 | **Category**: timeout | **Severity**: low | **Verdict**: applies
- **File**: `src/client/mod.rs` | **Function**: `ca_pend_io`
- **Audit Rec**: - In `ca-rs/src/client/mod.rs`, verify that `ca_pend_io` / `ca_pend_event`
  with `timeout=Duration::ZERO` behave as non-blocking poll, not a sleep.
- Check that any conversion from CA's `f64` timeout to `Duration` handles NaN
  by mapping to `Duration::MAX` (infinite wait), not panicking.

### `1d056c6` — CA Command-Line Tools Ignore EPICS_CLI_TIMEOUT Environment Variable
- **Date**: 2022-12-06 | **Category**: timeout | **Severity**: low | **Verdict**: partial
- **File**: `src/tools/tool_lib.rs` | **Function**: `use_ca_timeout_env`
- **Audit Rec**: In `ca-rs` tool implementations, check for `EPICS_CLI_TIMEOUT` env var
before processing command-line arguments. Ensure that a parse failure for
`-w` preserves the env-var-derived timeout rather than reverting to a
hardcoded default.

### `457387e` — dbf_type_to_text macro signed comparison warning with unsigned type argument
- **Date**: 2024-08-12 | **Category**: type-system | **Severity**: low | **Verdict**: applies
- **File**: `src/client/db_access.rs` | **Function**: `dbf_type_to_text`
- **Audit Rec**: 1. Find ca-rs `dbf_type_to_text` equivalent — verify no unchecked array index on
   type code.
2. Ensure the function handles type codes outside the valid range without panic
   in production (return `None` or a sentinel string).

### `8c99340` — CA: clarify count=0 means variable-size array subscription
- **Date**: 2019-05-09 | **Category**: wire-protocol | **Severity**: low | **Verdict**: applies
- **File**: `src/client/subscription.rs` | **Function**: `null`
- **Audit Rec**: Audit `ca-rs/src/client/subscription.rs`: confirm that count=0 is passed
through to the wire without substitution, and that each monitor update uses the
element count from the response header rather than a stored initial count.

## pva-rs

### `51191e6` — Linux IP_MULTICAST_ALL Default Causes Unintended Multicast Reception
- **Date**: 2021-08-04 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/net/udp.rs` | **Function**: `bind_multicast_socket`
- **Audit Rec**: In `ca-rs/src/client/udp.rs` and `pva-rs/src/net/udp.rs`, after `UdpSocket::bind()`,
add a Linux-only `setsockopt(IPPROTO_IP, IP_MULTICAST_ALL, 0)`. Use
`#[cfg(target_os = "linux")]`. Check whether the `socket2` crate exposes this;
if not, use `libc::setsockopt` directly. This matches the existing...

### `5b37663` — aToIPAddr crashes on NULL input string
- **Date**: 2020-08-06 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/config/env.rs` | **Function**: `null`
- **Audit Rec**: In `src/client/addr_list.rs` (ca-rs) and `src/config/env.rs` (pva-rs):
search for environment variable reads for `ADDR_LIST` config. Verify
the result is `Option`-wrapped and that `None` (unset env var) is handled
gracefully (empty list, not panic or unwrap). Check for any `.unwrap()`
or direct...

### `951b6ac` — Cygwin missing TCP_NODELAY declaration causes CA build failure
- **Date**: 2020-08-03 | **Category**: network-routing | **Severity**: medium | **Verdict**: applies
- **File**: `src/server_native/tcp.rs` | **Function**: `null`
- **Audit Rec**: In `src/server/tcp.rs` (ca-rs): search for `set_nodelay`. Verify it is
called on every accepted socket (line 432 was found). In
`src/client/transport.rs` (ca-rs): verify `set_nodelay` is called after
`TcpStream::connect`. In `src/server_native/tcp.rs` (pva-rs): same check.
If any accept/connect...

## base-rs

### `0a1fb25` — dbCaGetLink fails with alarm when reading scalar from empty CA-linked array
- **Date**: 2020-06-29 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `dbCaGetLink`
- **Audit Rec**: Audit `base-rs/src/server/database/db_ca.rs::dbCaGetLink` (or the equivalent CA-link read path). Confirm the scalar fast-path branch checks `used_elements >= 1` before conversion and raises `LINK_ALARM / INVALID_ALARM` on failure.

### `12cfd41` — dbPut raises LINK/INVALID alarm when writing empty array into scalar field
- **Date**: 2020-07-06 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_put`
- **Audit Rec**: In `base-rs/src/server/database/db_access.rs::db_put` (or equivalent), verify that when `n_request == 0` and the target is a scalar field (no array support), the code sets a LINK/INVALID alarm and returns without calling the fast-put converter.

### `2340c6e` — Array records: move bptr assignment from cvt_dbaddr to get_array_info
- **Date**: 2021-02-25 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/record_support.rs` | **Function**: `cvt_dbaddr`
- **Audit Rec**: In `base-rs/src/server/database/record_support.rs`, for array-type records,
verify that the field buffer pointer is resolved at access time (equivalent to
`get_array_info`), not cached at descriptor-creation time (`cvt_dbaddr`).
Check compress, histogram, and subArray record equivalents.

### `3176651` — dbGet: Return error when reading scalar from empty array
- **Date**: 2020-06-09 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: In `base-rs/src/server/database/db_access.rs:db_get`, verify the scalar fast
path does not access `fields[0]` when `no_elements == 0`. Prefer
`fields.get(0).ok_or(DbError::OutOfBounds)?` pattern.

### `3627c38` — Crash when filter result reduces array to 0 elements in dbDbGetValue
- **Date**: 2020-02-12 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/links.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `src/server/database/links.rs`: locate the filter-chain invocation in
the read path. Verify it is guarded with `channel.final_elements() > 0`
before `run_pre_chain` / `run_post_chain`. Also check that when the guard
fires, a LINK alarm is set on the record.

### `39c8d56` — dbGet crashes on empty array: missing element-count guard before filter
- **Date**: 2020-02-13 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: In `src/server/database/db_access.rs`: search for `no_elements` or
equivalent and verify a `< 1` / `== 0` guard exists after filter
application, before any buffer slicing. In `src/server/database/links.rs`:
find `run_pre_chain`/`run_post_chain` call sites and verify they are
guarded by a...

### `552b2d1` — dbConstAddLink: missing bounds check on dbrType before table lookup
- **Date**: 2021-02-19 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/const_link.rs` | **Function**: `load_scalar`
- **Audit Rec**: Audit `base-rs/src/server/database/const_link.rs` (or equivalent) — find all
places where a field-type integer is used to index a conversion table. Ensure
either (a) the type is validated as a valid enum variant before use, or (b) a
bounds check precedes the index. Also audit any CA server code in...

### `60fa2d3` — Null pointer dereference in postfix() on empty operator stack
- **Date**: 2023-07-18 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/calc/postfix.rs` | **Function**: `postfix`
- **Audit Rec**: In `base-rs/src/calc/postfix.rs::postfix()`, verify:
1. The SEPARATOR (comma) handling checks for empty stack before the inner loop.
2. The CLOSE_PAREN handling checks for empty stack before the inner loop.
3. The appropriate error enum variants (`BadSeparator`, `ParenNotOpen`) are returned.
4....

### `6b5abf7` — dbDbLink: remove early error return that blocked empty array reads
- **Date**: 2020-06-01 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_db_link.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `base-rs/src/server/database/db_db_link.rs::db_db_get_value` (or the
equivalent Rust link-read function), verify that a zero-element count after
filter evaluation is returned as `Ok(0)`, not as an error. Add a unit test for
the backward-range filter path that asserts `Ok` with empty slice.

### `8c9e42d` — Numeric overflow in epicsStrnRawFromEscaped octal/hex escape parsing
- **Date**: 2020-07-28 | **Category**: bounds | **Severity**: high | **Verdict**: applies
- **File**: `src/server/iocsh/registry.rs` | **Function**: `null`
- **Audit Rec**: Search `src/server/db_loader/mod.rs` and `src/server/iocsh/registry.rs`
for escape sequence parsing (look for `\\x`, `isxdigit`, or `parse_escape`
patterns). If a custom parser is used, verify: (1) hex escapes are
limited to 2 digits, and (2) octal escapes check for overflow before
consuming the...

### `beec00b` — Compress Record N-to-M Array Compression Bounds Error with Partial Buffer
- **Date**: 2024-03-14 | **Category**: bounds | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/records/compress.rs` | **Function**: `compress_array`
- **Audit Rec**: In `base-rs/src/server/database/records/compress.rs:compress_array`:
- Verify that N-to-1 algorithm iterates over input in chunks of exactly `n`,
  using `chunks(n)` or equivalent, not computing total groups upfront.
- Verify `PBUF=NO` correctly discards incomplete final chunks.
- Verify the median...

### `446e0d4` — dbnd filter: pass through DBE_ALARM and DBE_PROPERTY events unconditionally
- **Date**: 2021-10-03 | **Category**: flow-control | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/filters/dbnd.rs` | **Function**: `filter`
- **Audit Rec**: - In `base-rs/src/server/database/filters/dbnd.rs:filter`: verify that
  `DBE_ALARM` and `DBE_PROPERTY` bits in `field_log.mask` are never cleared by
  the deadband check.
- Apply the same audit to any other value-based filter (e.g., `dbhyst`,
  `dbldap`): they should only suppress value bits, not...

### `6ffc9e1` — logClient flush discards messages already in OS send queue
- **Date**: 2019-09-17 | **Category**: flow-control | **Severity**: high | **Verdict**: partial
- **File**: `src/log/log_client.rs` | **Function**: `flush`
- **Audit Rec**: In `base-rs/src/log/log_client.rs`: verify that on connection error, pending
messages are retained in the application-side buffer (not only in the tokio
write buffer) so they can be retried on reconnect. Check that `flush()` is not
treating `write()` success as delivery confirmation.

### `39b0301` — Record deletion leaks all link field allocations (dbDeleteRecord)
- **Date**: 2024-06-18 | **Category**: leak | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_static_lib.rs` | **Function**: `dbDeleteRecord`
- **Audit Rec**: 1. Find the Rust `delete_record` / `remove_record` path in base-rs.
2. Verify the record's link fields are fully dropped (not just the record node
   itself) before the containing allocation is freed.
3. Add a test that creates and deletes records with each link type and runs under
   ASAN or...

### `0a6b9e4` — scanStop() Before scanStart() Causes Crash or Hang
- **Date**: 2024-06-14 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/scan.rs` | **Function**: `scan_stop`
- **Audit Rec**: In `base-rs/src/server/database/scan.rs:scan_stop` (or equivalent): verify
that calling `stop()` before `start()` is handled gracefully (returns early
rather than unwrapping a None JoinHandle or awaiting a channel that was never
opened). Check for `unwrap()` calls on `Option<JoinHandle>` fields.

### `16c3202` — waveform: PACT=TRUE Lost, Causes Double-Processing on Async Completion
- **Date**: 2021-07-21 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/waveform_record.rs` | **Function**: `process`
- **Audit Rec**: In `base-rs/src/server/database/waveform_record.rs`, verify that the `process()`
function marks the record as active (equivalent of `pact = TRUE`) BEFORE
performing any async device call. Check other record types for the same
pattern: the active-flag assignment must be the first mutation in...

### `27fe3e4` — db_field_log: eliminate dbfl_type_rec, unify live-record reference into dbfl_type_ref
- **Date**: 2020-03-30 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_field_log.rs` | **Function**: `null`
- **Audit Rec**: If base-rs implements channel filters:
1. Verify the field log type has only two variants (val and ref), with ref
   carrying an `Option<Dtor>` or `bool` for copy ownership.
2. Verify that `arr` filter equivalent calls `get_array_info` under scan lock
   only when the field log does not own its...

### `29fa062` — errlog: rewrite with double-buffering to avoid holding lock during print
- **Date**: 2021-02-19 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/log/errlog.rs` | **Function**: `errlog_thread`
- **Audit Rec**: In `base-rs/src/log/errlog.rs` (or equivalent): verify that the log message
consumer task does NOT hold any shared lock while writing to stderr/file. If
using `Mutex<Queue>`, drain the queue into a local `Vec` first, release the
lock, then call `write_all`. Confirm flush semantics use an atomic...

### `3124d97` — Fix crash in popFirstTemp() when temp list is empty on bad record name
- **Date**: 2020-06-10 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_lex_routines.rs` | **Function**: `pop_first_temp`
- **Audit Rec**: In `base-rs/src/server/database/db_lex_routines.rs`, confirm `pop_first_temp()` returns `Option<T>` and that the implementation does not `unwrap()` the linked-list head.

### `3f382f6` — Revert: dbCa iocInit wait for local CA links to connect
- **Date**: 2025-10-17 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/ca_link.rs` | **Function**: `ioc_init_wait`
- **Audit Rec**: Audit `base-rs/src/server/database/ca_link.rs` for any future attempt to
block `iocInit` until CA links connect. If such logic exists, ensure it has
a timeout and does not create deadlock potential with circular links.

### `56f05d7` — dbGet: wrong condition for using db_field_log vs. live record data
- **Date**: 2021-01-14 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: Audit `base-rs/src/server/database/db_access.rs::db_get`: find the condition
that selects between reading from the live record and reading from the field
log. Ensure it uses the equivalent of `dbfl_has_copy` — checking both type
AND `dtor` presence AND `no_elements` — not simply `Option::is_some()`.

### `62c11c2` — dbDbLink processTarget: self-link must not set RPRO (infinite reprocess loop)
- **Date**: 2019-02-02 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/dbDbLink.rs` | **Function**: `process_target`
- **Audit Rec**: In `base-rs/src/server/database/dbDbLink.rs::process_target`: verify the
self-link guard (`src_id != dst_id`) is present before setting any
"reprocess-after-async" flag. Add a regression test with a self-linking record.

### `717d69e` — dbCa: iocInit must wait for local CA links to connect before PINI
- **Date**: 2025-09-20 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `db_ca_add_link_callback_opt`
- **Audit Rec**: In `base-rs/src/server/database/db_ca.rs::db_ca_run` (or the async
`ioc_init` state machine): confirm there is a barrier that awaits all local CA
link connections before advancing to the `AfterIocRunning` hook. The barrier
must be deferred until after subscribe AND attribute fetch complete, not...

### `7709239` — Null guard for put_array_info function pointer before calling in dbPut
- **Date**: 2020-07-17 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_put`
- **Audit Rec**: Audit `base-rs/src/server/database/db_access.rs::db_put` (or equivalent) for all RSET optional-callback dispatch sites. Confirm each is wrapped in `if let Some(f) = prset.put_array_info { f(...) }` or equivalent.

### `85822f3` — db_field_log: missing abstraction for data-ownership check enables scan-lock races
- **Date**: 2020-04-01 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: Audit `base-rs/src/server/database/db_access.rs` and the field-log type:
verify that "borrowed from record" field logs are distinguishable from
"owned copy" field logs at the type level. Check whether the `get_array_info`
equivalent is only called for non-owned logs. Check that...

### `8a0fc03` — dbPutFieldLink: propagate dbChannelOpen() error status correctly
- **Date**: 2021-11-03 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_put_field_link`
- **Audit Rec**: - In `base-rs/src/server/database/db_access.rs`: verify `db_put_field_link`
  returns `Result` and uses `?` on `db_channel_open()`.
- Grep for `let _ =` on fallible calls in `db_access.rs`.

### `a46bd5a` — dbCa: iocInit wait for local CA links to connect (later reverted)
- **Date**: 2025-09-20 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/ca_link.rs` | **Function**: `dbCaAddLinkCallbackOpt`
- **Audit Rec**: Audit `base-rs/src/server/database/ca_link.rs` and the IOC init sequence
for whether local CA links are guaranteed to be connected before the first
PINI scan. If not, document this as a known limitation or implement a
time-bounded readiness check (not a blocking wait that can deadlock).

### `a74789d` — Decimate and Sync Filters Incorrectly Drop DBE_PROPERTY Monitor Events
- **Date**: 2023-05-03 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/filters/decimate.rs` | **Function**: `filter`
- **Audit Rec**: In `base-rs` filter implementations, verify that the event dispatch loop
checks `event.mask & DBE_PROPERTY` before applying any deadband, decimation,
or synchronisation logic. The pattern is: if the event carries property data,
forward it immediately regardless of filter state.

### `ac6eb5e` — callbackRequest: No Guard Against Uninitialized Callback Queue
- **Date**: 2021-06-20 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/callback.rs` | **Function**: `callback_request`
- **Audit Rec**: In `base-rs/src/server/database/callback.rs`, verify that `callback_request()`
checks that the queue/channel is initialized (not `None`/dropped) before
attempting to push. Confirm that `callback_cleanup()` sets the queue reference
to `None` so post-shutdown calls are detected rather than panicking.

### `b34aa59` — Null guard cascade for popFirstTemp() return in DB parser
- **Date**: 2020-06-10 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_lex_routines.rs` | **Function**: `db_menu_body`
- **Audit Rec**: In `base-rs/src/server/database/db_lex_routines.rs` (or the DB parser equivalent), audit every call to `pop_first_temp()` / equivalent and confirm all return `None` cases are handled without panic.

### `b35064d` — dbEvent: Revert join, Implement Safe Exit Semaphore Shutdown Protocol
- **Date**: 2019-06-23 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_close_events`
- **Audit Rec**: In `base-rs/src/server/database/db_event.rs`, verify the shutdown handshake:
- Worker task signals completion via a `oneshot::Sender` or `Notify`.
- Shutdown path awaits the signal before dropping shared `Arc<EventUser>` state.
- No `Mutex`/`Condvar`/channel is destroyed before the worker task...

### `bac8851` — Revert asCaStop() thread join to avoid deadlock on shutdown
- **Date**: 2020-03-23 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/as_ca.rs` | **Function**: `as_ca_stop`
- **Audit Rec**: In `base-rs/src/server/database/as_ca.rs:as_ca_stop`, verify that:
1. The access security CA task is not joined while the caller holds any mutex
   that the task may be waiting on.
2. Prefer `handle.abort()` for hard-stop, or a dedicated shutdown signal
   (e.g., `CancellationToken`) that allows...

### `c51c83b` — Revert stack-allocated field-log fix: heap alloc required for PINI safety
- **Date**: 2020-02-25 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/links.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `src/server/database/links.rs`, find `run_pre_chain` / `run_post_chain`
call sites. Verify the field-log value is heap-owned (Box or Arc) rather
than a local variable when passed to filter chains. Also check null/OOM
handling after field-log allocation.

### `ca2ea14` — dbEvent: Worker Thread Must Be Joined on Close
- **Date**: 2021-04-02 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_close_events`
- **Audit Rec**: In `base-rs/src/server/database/db_event.rs`, verify that `db_close_events()`
(or the equivalent `Drop` impl) awaits the worker JoinHandle. Confirm the
JoinHandle is not just dropped — tokio tasks are not automatically cancelled
on JoinHandle drop (unlike `abort()`).

### `e0dfb6c` — PINI crash: use stack-local field-log to avoid heap UAF in filter chain
- **Date**: 2020-02-13 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/links.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `src/server/database/links.rs`: find `run_pre_chain` / `run_post_chain`
signatures. Check whether they take `&mut FieldLog` (which cannot be
replaced) or `FieldLog -> FieldLog` (value semantics, safe). If the C
pattern of pointer replacement is preserved, the Rust equivalent must
use...

### `e860617` — dbDbLink processTarget: add procThread ownership to fix RPRO/PUTF regression
- **Date**: 2019-01-27 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/dbDbLink.rs` | **Function**: `process_target`
- **Audit Rec**: In `base-rs/src/server/database/dbDbLink.rs::process_target`:
- Check that the RPRO equivalent has a `current_processor != self` guard.
- If the record processing model uses tokio tasks (not OS threads), the
  "current processor" comparison needs to use a task-local ID or a per-record
 ...

### `f4be9da` — Null callback function pointer crash in callbackRequest
- **Date**: 2023-11-03 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/callback.rs` | **Function**: `callback_request`
- **Audit Rec**: In `base-rs/src/server/database/callback.rs`, verify that `callback_request()` validates the callback function is present before enqueueing. If `CALLBACK` structs are translated to a Rust struct with `callback: Option<fn(...)>`, ensure the dispatcher panics gracefully or returns an error rather...

### `fab8fd7` — dbEvent: handle multiple db_event_cancel() calls safely
- **Date**: 2023-09-14 | **Category**: lifecycle | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_cancel_event`
- **Audit Rec**: - Audit `base-rs/src/server/database/db_event.rs` for any subscription cancel
  path that frees state while the delivery task may still reference it.
- Check that `CancellationToken` or equivalent is used so that a second
  cancel is a no-op.
- Verify that the delivery task does not hold an `Arc`...

### `2ff44cb` — callback.c: join callback threads on callbackStop()
- **Date**: 2022-07-30 | **Category**: lifecycle | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/callback.rs` | **Function**: `callback_stop`
- **Audit Rec**: In `base-rs` callback dispatch: verify that callback worker task `JoinHandle`s
are stored per priority level and awaited (or aborted+awaited) in the shutdown
path. Ensure no `tokio::spawn` for callback workers is fire-and-forget.

### `49fddaa` — errlogRemoveListeners: self-removal during callback causes use-after-free
- **Date**: 2022-11-15 | **Category**: lifecycle | **Severity**: high | **Verdict**: partial
- **File**: `src/log/errlog.rs` | **Function**: `errlog_remove_listeners`
- **Audit Rec**: In `base-rs/src/log/errlog.rs`, check the errlog listener dispatch loop: ensure that if a listener removes itself during iteration, the removal is deferred until after the iteration completes. Using a `Vec` with `retain()` after the callback loop (not during) is the idiomatic Rust fix.

### `8a30200` — ts filter: replace cantProceed with non-fatal error handling
- **Date**: 2022-06-15 | **Category**: lifecycle | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/filters/ts.rs` | **Function**: `filter`
- **Audit Rec**: Audit `base-rs` filter infrastructure: verify that `filter()` trait methods
return `Result<Option<FieldLog>>` (not bare `FieldLog`) and that an invalid
filter configuration does not cause a panic. Ensure no `unwrap()`/`expect()`
on enum discriminants in timestamp-filter logic.

### `bded79f` — dbScan: join periodic and once-scan threads on scanStop()
- **Date**: 2022-07-30 | **Category**: lifecycle | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/scan.rs` | **Function**: `scan_stop`
- **Audit Rec**: In `base-rs` scan task management: verify that all periodic scan task
`JoinHandle`s are stored and awaited (or aborted+awaited) during shutdown.
Confirm no `tokio::spawn` call for scan tasks is made without storing the
handle (i.e., no fire-and-forget scan tasks).

### `f430389` — iocShutdown: always stop worker threads, not only in isolated mode
- **Date**: 2022-07-30 | **Category**: lifecycle | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/ioc_init.rs` | **Function**: `ioc_shutdown`
- **Audit Rec**: In `base-rs` IOC lifecycle (shutdown path): verify that all scan and callback
tasks are joined/aborted unconditionally on shutdown, not only in test/isolated
mode. Check for conditional `abort()` or `join()` calls that might be skipped
in the standard runtime configuration.

### `530eba1` — rsrv: use verified client IP address instead of client-supplied hostname
- **Date**: 2018-06-16 | **Category**: network-routing | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/access_security.rs` | **Function**: `hag_add_host`
- **Audit Rec**: In `ca-rs/src/server/client.rs`, verify that the client's hostname used for
access-security evaluation is derived from the verified socket peer address
(not the CA `HOST_NAME` message) when IP-mode is active.  In
`base-rs/src/server/database/access_security.rs`, ensure HAG host entries
are resolved...

### `271f20f` — dbEvent: expand synchronization — fix busy-wait and labor-pending race
- **Date**: 2025-08-27 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/event.rs` | **Function**: `db_flush_extra_labor_event`
- **Audit Rec**: Audit `base-rs/src/server/database/event.rs::db_flush_extra_labor_event` and
`db_cancel_event` (or their Rust equivalents) for sleep-based polling and for
the race where a pending-labor flag check exits before the labor has actually
completed.

### `8735a7b` — dbCa: Acquire dbScanLock around db_process() in CA link task
- **Date**: 2025-06-16 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `dbca_task`
- **Audit Rec**: Search `base-rs/src/server/database/db_ca.rs` for any `process()` call site
not wrapped in a scan-lock guard. This is a high-severity race — the omission
is trivially reproducible under any CA link workload.

### `89f0f13` — Callback subsystem uses non-atomic state flag causing data races on init/stop/cleanup
- **Date**: 2017-11-08 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/callback.rs` | **Function**: `callbackInit`
- **Audit Rec**: In `callback.rs`, audit:
1. The init/stop/cleanup state transitions — must be `AtomicU8::compare_exchange` or `Mutex`-guarded.
2. The per-queue shutdown flag — must be `AtomicBool::store(true, Ordering::Release)` with a corresponding `load(Ordering::Acquire)` in the worker.
3. Double-init guard...

### `9f78899` — db: acquire record lock before db_create_read_log and dbChannelGetField
- **Date**: 2023-02-23 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/channel.rs` | **Function**: `db_channel_get_field`
- **Audit Rec**: In `base-rs/src/server/database/channel.rs::db_channel_get_field` and
`base-rs/src/server/database/db_access.rs::dbChannel_get_count`, verify that:
1. `db_create_read_log` is called with the record scan lock already held.
2. No `await` point exists between lock acquisition and...

### `9f868a1` — Concurrent db_cancel_event causes hang via shared flush semaphore
- **Date**: 2023-10-23 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_cancel_event`
- **Audit Rec**: In `base-rs/src/server/database/db_event.rs`, audit the subscription cancellation path:
1. Verify that concurrent `db_cancel_event()` calls each get their own `tokio::sync::Notify` or `oneshot::channel`.
2. The event worker must signal ALL pending cancellers after each cycle, not just one.
3. The...

### `a4bc0db` — dbCa: CP link updates must set PUTF/RPRO via dbCaTask, not scanOnce callback
- **Date**: 2024-12-27 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `connection_callback`
- **Audit Rec**: In `base-rs/src/server/database/db_ca.rs`:
1. Verify that `connection_callback`, `event_callback`, and
   `access_rights_callback` for CP links do NOT directly call `db_process` from
   the callback context — they must post an action to the `dbCaTask` equivalent.
2. Verify `db_process` (or its Rust...

### `dac620a` — dbGet infinite recursion when input link points back to same field
- **Date**: 2024-11-29 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_link.rs` | **Function**: `db_db_get_control_limits`
- **Audit Rec**: In `db_link.rs` audit each metadata getter (get_control_limits,
get_graphic_limits, get_alarm_limits, get_precision, get_units): verify
that a per-link "visited" flag or equivalent is set before recursing into
the linked record and cleared on return.  Without this guard a
self-referential link will...

### `e9e576f` — Fix dbCaSync() and add testdbCaWaitForUpdateCount()
- **Date**: 2021-11-02 | **Category**: race | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `db_ca_sync`
- **Audit Rec**: - In `base-rs/src/server/database/db_ca.rs:db_ca_task`: verify that the sync
  response is sent only when the work channel is empty (e.g., after
  `channel.try_recv()` returns `Empty`), not at dequeue time.
- In `db_ca_sync()`: verify it awaits the oneshot before returning.

### `e1c1bb8` — dbEvent: correct eventsRemaining count — skip canceled events
- **Date**: 2023-01-22 | **Category**: race | **Severity**: high | **Verdict**: partial
- **File**: `src/server/database/event.rs` | **Function**: `event_read`
- **Audit Rec**: In `base-rs` event dispatch: confirm that the "more items" signal to the
CA/PVA server flush logic is snapshotted inside the same mutex guard as the
item dequeue, and that logically-canceled subscriptions are excluded from
the count.

### `3091f7c` — int64in: Monitor Delta Comparison Truncated to 32 Bits
- **Date**: 2021-07-29 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/int64in_record.rs` | **Function**: `monitor`
- **Audit Rec**: In `base-rs/src/server/database/int64in_record.rs`, check the `monitor()`
function's delta computation. Ensure the DELTA equivalent uses `u64`/`i64`
arithmetic throughout, and that no intermediate cast to `u32` or `i32` occurs.
Also verify `int64out` record if it exists and has the same MDEL logic.

### `6c914d1` — Validate dbrType before indexing conversion table to prevent OOB access
- **Date**: 2020-06-01 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: - In `base-rs/src/server/database/db_access.rs:db_get`, verify that `dbrType`
  received from a CA message is validated against known types before any
  conversion dispatch.
- In `base-rs/src/server/database/db_ca.rs:ca_get_callback`, same check.
- In any JSON link or const-link implementation:...

### `b6fffc2` — String-to-epicsUInt32 conversion uses ULONG_MAX bound instead of UINT_MAX
- **Date**: 2024-08-12 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_convert.rs` | **Function**: `get_string_ulong`
- **Audit Rec**: 1. Find all string-to-`u32` conversion sites in base-rs (`get_string_ulong`,
   `put_string_ulong`, `cvt_st_uint32` equivalents).
2. If conversion goes through `f64`, ensure the upper bound is `u32::MAX`
   (4294967295.0_f64), not `u64::MAX`.
3. Prefer `s.trim().parse::<u32>()` (direct integer...

### `b833f12` — epicsStrtod: use strtoll/strtoull for hex parsing on 32-bit architectures
- **Date**: 2025-04-04 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/util/stdlib.rs` | **Function**: `epics_strtod`
- **Audit Rec**: In `base-rs/src/util/stdlib.rs::epics_strtod` (or the string-to-double parse
utility), verify that hex prefixed strings (`0x…`) are parsed via 64-bit integer
conversion before casting to `f64`, not via `usize` or pointer-sized types.

### `b94afaa` — UTAG field widened from epicsInt32 to epicsUInt64
- **Date**: 2020-12-02 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `get_options`
- **Audit Rec**: 1. Search base-rs for any `utag` field typed as `i32`, `u32`, or `i64` — must be
   `u64`.
2. Verify the monitor-payload serialization writes utag as little-endian 8 bytes
   after the `nsec` field, with no additional padding.
3. Check that any link-layer callback signature for timestamp+tag uses...

### `c5012d9` — Make sure epicsInt8 is signed on all architectures
- **Date**: 2021-12-17 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_types.rs` | **Function**: `null`
- **Audit Rec**: - In `ca-rs/src/client/com_buf.rs`: verify `push_dbf_char` / analogous function
  uses `i8` not `u8` when writing CA type 4 data.
- In `ca-rs/src/client/com_que_recv.rs`: verify `copy_out_bytes` for DBF_CHAR
  reads back as `i8`.
- In `base-rs/src/server/database/db_types.rs`: verify `DBF_CHAR`...

### `f6e8a75` — DB link reads DBF_MENU field as DBF_ENUM due to wrong type query
- **Date**: 2021-08-12 | **Category**: type-system | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_link.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `base-rs/src/server/database/db_link.rs::db_db_get_value` (or equivalent):
verify the scalar fast-path uses `channel.field_type()` (pre-promotion) rather
than `channel.ca_type()` or `channel.exported_type()`. Add a unit test:
create a MENU field, link to it via a DB link, and verify the integer...

### `b1d9c57` — db_field_log::mask overwritten with actual event mask on post
- **Date**: 2021-10-03 | **Category**: wire-protocol | **Severity**: high | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_post_events`
- **Audit Rec**: - In `base-rs/src/server/database/db_event.rs:db_post_events`: after creating a
  `FieldLog` for a subscription, set `field_log.mask = event_mask & sub.select`.
- In any `pre_chain` filter equivalent: verify it reads the per-post mask, not
  the subscription's registered mask.

### `4a0f488` — histogramRecord wdog callback uses bptr instead of VAL field for db_post_events
- **Date**: 2021-02-25 | **Category**: bounds | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/record_support.rs` | **Function**: `monitor`
- **Audit Rec**: In `base-rs/src/server/database/record_support.rs`, any `post_event`
equivalent for array records must reference the field identity token, not the
raw buffer address.  Check histogram, compress, aai, and subArray record
equivalents.

### `5d808b7` — Introduce distinct error code for zero-element array reads
- **Date**: 2020-05-07 | **Category**: bounds | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: Search `src/server/database/db_access.rs` and `links.rs` for the error
enum returned when element count is zero. Verify there is a distinct
variant (e.g., `EmptyArray`) vs. field-type mismatch errors.

### `6e7a715` — Getting .DTYP from rectype with no devSup returns empty string instead of crash
- **Date**: 2022-08-16 | **Category**: bounds | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_fast_link_conv.rs` | **Function**: `cvt_device_st`
- **Audit Rec**: Audit `cvt_device_st` Rust equivalent in `db_fast_link_conv.rs`. Ensure that
`pdbDeviceMenu == NULL` (i.e., `Option::None` for device menu) is handled by
returning an empty string, not an error. Any `unwrap()` or `?` on a
`Option<DeviceMenu>` in a field-conversion path is suspect.

### `979dde8` — get_enum_strs uses pointer arithmetic that trips _FORTIFY_SOURCES=3
- **Date**: 2024-06-20 | **Category**: bounds | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `get_enum_strs`
- **Audit Rec**: In `db_access.rs::get_enum_strs` (or its equivalent CA DBR_ENUM_STRS
encoder), verify: (1) the buffer cursor advances at entry for this
option, not only on success; (2) string slots are written via indexed
access, not raw pointer arithmetic.

### `e5b4829` — lsi/lso SIZV Uncapped at 32767: Signed dbAddr::field_size Overflow
- **Date**: 2024-05-19 | **Category**: bounds | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/records/lsi_record.rs` | **Function**: `init_record`
- **Audit Rec**: In `base-rs/src/server/database/records/lsi_record.rs` and `lso_record.rs`:
check `init_record` for any `field_size` assignment from `sizv`. Ensure cap at
`0x7FFF` (32767) is applied. Cross-reference with printf record fix.

### `11a4bed` — compressRecord: compress_scalar average computation is incorrect
- **Date**: 2022-05-11 | **Category**: bounds | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/records/compress_record.rs` | **Function**: `compress_scalar`
- **Audit Rec**: In `base-rs` compress record: verify that the N-to-1 scalar average uses the
incremental mean formula, and that `inx` is not unconditionally reset to 0
when a partial push is triggered.

### `84f4771` — compressRecord: compress_array rejects valid partial input when PBUF=YES
- **Date**: 2022-05-11 | **Category**: bounds | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/records/compress_record.rs` | **Function**: `compress_array`
- **Audit Rec**: In `base-rs` compress record: verify that `compress_array()` does not
unconditionally reject input arrays smaller than `N` when partial-buffer mode
is enabled. Check the `pbuf` / partial-output flag handling in both array and
scalar paths.

### `baa4cb5` — callbackSetQueueSize: reject non-positive queue size before iocInit
- **Date**: 2025-09-30 | **Category**: bounds | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/callback.rs` | **Function**: `set_queue_size`
- **Audit Rec**: Audit `base-rs/src/server/database/callback.rs` (or equivalent queue
initialization) for a `capacity: usize` parameter that can be zero. Add an
explicit `assert!(capacity > 0)` or return `Err` for zero values, consistent
with the C fix.

### `ec650e8` — dbPutConvertJSON: empty JSON string not handled, passed to yajl causing parse error
- **Date**: 2022-07-26 | **Category**: bounds | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/db_convert_json.rs` | **Function**: `db_put_convert_json`
- **Audit Rec**: In `base-rs/src/server/database/db_convert_json.rs` (or equivalent), verify that `db_put_convert_json("")` returns `Ok(0)` and does not attempt to parse an empty buffer.

### `0a3427c` — logClient: Don't Discard Unsent Buffer on Disconnect
- **Date**: 2019-08-28 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/log/log_client.rs` | **Function**: `log_client_close`
- **Audit Rec**: In `base-rs/src/log/log_client.rs`, audit the disconnect/reconnect path:
confirm that the write buffer (channel, VecDeque, or BytesMut staging area) is
NOT cleared when the TCP stream closes. The reconnect loop should resume
draining from the same buffer position.

### `17a8dbc` — Filters not applied when reading via DB link (dbDbGetValue)
- **Date**: 2020-02-12 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/links.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `src/server/database/links.rs`: find the `db_db_get_value` or
equivalent function. Verify that when `channel.filters().is_empty()` is
false, the code invokes `run_pre_chain` / `run_post_chain` before the
final value conversion, and does NOT fall through to the scalar fast path.

### `4df48c9` — dbEvent queue accumulates duplicate reference-type events instead of compacting them
- **Date**: 2022-06-27 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_queue_event_log`
- **Audit Rec**: In `db_event.rs::queue_event_log` (or `SubscriptionQueue::push`), check
whether compaction of consecutive reference-type events is implemented.
Specifically: `if pending_count > 0 && !last.has_copy() && !new.has_copy() { drop(new); return; }`.

### `556de06` — epicsThreadGetCPUs overreports CPUs when affinity mask is restricted
- **Date**: 2026-02-06 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/callback.rs` | **Function**: `get_cpu_count`
- **Audit Rec**: In `base-rs/src/server/database/callback.rs`: if there is a function that
determines the default callback thread count, confirm it calls
`std::thread::available_parallelism()` (Rust ≥ 1.56, affinity-aware on Linux)
rather than a raw `num_cpus::get_physical()` or manual `sysconf` call.
On non-Linux...

### `8ac2c87` — compressRecord: post monitor event when reset via RES field
- **Date**: 2025-01-07 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/records/compress.rs` | **Function**: `special`
- **Audit Rec**: In `base-rs/src/server/database/records/compress.rs::special`, verify that
writing to the reset field:
1. Clears the internal buffer and sets `NORD=0`.
2. Calls the event-post function with `DBE_VALUE | DBE_LOG` mask immediately
   after the reset, without waiting for the next normal scan cycle.

### `b1f4459` — DB links stored DBADDR instead of dbChannel, bypassing filter metadata
- **Date**: 2020-02-11 | **Category**: flow-control | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/links.rs` | **Function**: `db_db_get_elements`
- **Audit Rec**: In `src/server/database/links.rs`: find `get_elements` / `get_dbf_type`
for DB link types. Verify they call `channel.final_elements()` /
`channel.final_field_type()` (post-filter) rather than reading from a
cached raw field descriptor. Also verify the link cleanup path calls the
channel destructor...

### `b6626e4` — dbEvent: detect possible queue stall when eventsRemaining is set
- **Date**: 2023-01-22 | **Category**: flow-control | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/event.rs` | **Function**: `event_read`
- **Audit Rec**: In `base-rs` event/subscription dispatch: verify that when the event channel
is full (backpressure), a warning is logged and the situation is surfaced
(e.g., via a metric or a one-shot log message), rather than silently dropping
events or blocking indefinitely.

### `4e4e55c` — dbDeleteRecordLinks only freed plink->text, skipping full link contents cleanup
- **Date**: 2024-06-19 | **Category**: leak | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_static_lib.rs` | **Function**: `delete_record_links`
- **Audit Rec**: 1. Find the Rust `delete_record` path — verify that all link fields in the
   record struct are properly dropped (not just the text/name field).
2. If links are represented as enums (CONSTANT / PV_LINK / JSON_LINK / etc.),
   ensure each variant's `Drop` implementation frees the variant-specific...

### `0f75e0a` — dbDbLink processTarget: replace assert() with errlogPrintf for procThread mismatches
- **Date**: 2019-03-13 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/dbDbLink.rs` | **Function**: `process_target`
- **Audit Rec**: In `base-rs/src/server/database/dbDbLink.rs`: if `process_target` has a
recursive-processing guard, use `tracing::error!` on invariant violation rather
than `panic!` or `unwrap()`.

### `13d6ca5` — initHookRegister: make idempotent and use mallocMustSucceed
- **Date**: 2025-02-05 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/init_hooks.rs` | **Function**: `init_hook_register`
- **Audit Rec**: In `base-rs/src/server/database/init_hooks.rs::init_hook_register`, verify:
1. Duplicate registration is detected (function pointer or comparable key
   equality check) before insertion.
2. The check and insert are atomic under the same lock (no TOCTOU window).
3. No hook is called twice for the...

### `1d85bc7` — longout special() sets link-changed flag before OUT link is updated
- **Date**: 2021-03-10 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/records/longout.rs` | **Function**: `special`
- **Audit Rec**: 1. Find the Rust `longout` record `special` hook — confirm the force-write flag
   is set only in the `after = true` branch.
2. Check other output records with OOPT/On-Change logic for the same pattern.

### `23d9176` — aai/waveform record cleanup: nord initialization and waveform returns readValue status
- **Date**: 2018-10-26 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/rec/waveform.rs` | **Function**: `process`
- **Audit Rec**: In `base-rs/src/server/database/rec/waveform.rs::process` and `aai.rs::process`, verify that `read_value()` result is captured and returned. Verify that NORD change detection uses direct element-count comparison, not a link-type check.

### `3fb10b6` — dbNotify must set PUTF on the first-record call only
- **Date**: 2018-12-29 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_notify.rs` | **Function**: `process_notify_common`
- **Audit Rec**: When implementing `db_notify.rs` `process_notify_common`, ensure the
"initiated by put" flag (equivalent to `putf`) is only set on the initial
invocation.  Do not re-set it on recursive/restart re-entries.

### `4737901` — devAiSoft read_ai returns error status on device read failure
- **Date**: 2020-02-13 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/dev_ai_soft.rs` | **Function**: `read_ai`
- **Audit Rec**: In `base-rs/src/server/database/dev_ai_soft.rs::read_ai` (or equivalent), verify that the "no-convert" success indicator is returned only within the `Ok` branch, and errors are propagated via `Err` or a non-OK status code to trigger alarm setting.

### `51c5b8f` — subArray process: missing NORD db_post_events when element count changes
- **Date**: 2023-03-09 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/rec/subarray.rs` | **Function**: `process`
- **Audit Rec**: In `base-rs/src/server/database/rec/subarray.rs::process`,
`rec/aai.rs::process`, `rec/aao.rs::process`, and `rec/waveform.rs::process`:
confirm NORD change detection and event posting is in the record layer.
Verify device support implementations do NOT duplicate NORD event posting.

### `5d1f572` — Remove NORD db_post_events from aai and waveform device support layers
- **Date**: 2023-03-08 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/dev/aai_soft.rs` | **Function**: `read_aai`
- **Audit Rec**: In `base-rs/src/server/database/dev/aai_soft.rs` and `dev/wf_soft.rs`:
confirm no `post_events` calls exist. In `rec/aai.rs::process`,
`rec/waveform.rs::process`: confirm NORD change detection and posting is
present (mirrors fix aff7463 for aai).

### `62c3b0a` — iocLog: errlog Listener Registered on Wrong Object (All Clients)
- **Date**: 2019-08-27 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/log/ioc_log.rs` | **Function**: `ioc_log_client_init`
- **Audit Rec**: In `base-rs/src/log/ioc_log.rs`, verify that `errlog` listener registration is
done by the IOC-level wrapper only, not by the generic `LogClient` constructor.
Check whether there is a `Drop` impl that correctly unregisters the listener.

### `64011ba` — Remove duplicate NORD db_post_events from subArray device support
- **Date**: 2023-03-09 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/dev/sa_soft.rs` | **Function**: `read_sa`
- **Audit Rec**: In `base-rs/src/server/database/dev/`: grep for any call to `post_events` or
equivalent monitor notification within device support implementations. These
should not exist; only record support `process()` functions should call them.

### `6c573b4` — longout with OOPT=On Change skips output write on first process
- **Date**: 2021-03-10 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/records/longout.rs` | **Function**: `init_record`
- **Audit Rec**: 1. Find the Rust `longout` record `init` function — verify the write-state is
   initialized to "execute" (not "skip").
2. Check that any equivalent of `special()` sets the force-write flag only after
   the link change completes.
3. Check all other output record types (`longoutRecord`, `aoRecord`,...

### `8c08c57` — errSymbolAdd fails if called before errSymBld (init ordering bug)
- **Date**: 2023-03-08 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/log/err_sym.rs` | **Function**: `err_symbol_add`
- **Audit Rec**: In `base-rs/src/log/err_sym.rs`, verify:
1. The symbol table is initialized via `OnceLock`/`Lazy` (not a manual `bool initialized` flag).
2. `err_symbol_add()` acquires a lock before insertion.
3. Duplicate code with different message returns `Err(CodeExists)`.
4. Module number < 501 returns...

### `8e7d3e9` — initHookName: Shutdown States Missing from Name Table
- **Date**: 2021-06-30 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/init_hooks.rs` | **Function**: `init_hook_name`
- **Audit Rec**: In `base-rs/src/server/database/init_hooks.rs`, verify that `init_hook_name()`
or equivalent uses an exhaustive `match` on the enum rather than numeric
indexing. If it uses a `&[&str]` array, confirm the array length equals
`InitHookState::COUNT` (or equivalent) and that shutdown variants are...

### `8fdaa13` — errlog: restore errlogFlush() call in eltc()
- **Date**: 2021-02-22 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/log/errlog.rs` | **Function**: `eltc`
- **Audit Rec**: - In `base-rs/src/log/errlog.rs:eltc`: verify that after toggling `to_console`,
  the internal log queue is flushed before returning.
- If errlog uses an async mpsc channel, check that the flush is awaited, not
  just a non-blocking signal.

### `aff7463` — aai and aao process: add NORD db_post_events when element count changes
- **Date**: 2023-03-08 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/rec/aai.rs` | **Function**: `process`
- **Audit Rec**: Check all array-type record `process()` implementations in
`base-rs/src/server/database/rec/`: verify each one snapshots NORD before
device support call and posts NORD events if changed. The four records
affected by this commit cluster (51c5b8f, 64011ba, 5d1f572, aff7463) are:
subArray, aai, aao,...

### `d0cf47c` — AMSG alarm message not propagated through MSS links
- **Date**: 2024-11-19 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_link.rs` | **Function**: `db_db_get_value`
- **Audit Rec**: In `db_link.rs` check both the get-value and put-value link paths:
wherever alarm severity is inherited from the linked record, verify that
the alarm message (`amsg`) is also copied when link mode is MSS.
In `rec_gbl.rs` confirm that `inherit_sevr` or its equivalent accepts
and propagates an...

### `dabcf89` — mbboDirect: fix init priority — B0-B1F bits override VAL when VAL is UDF
- **Date**: 2021-10-03 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/rec/mbbodirect.rs` | **Function**: `init_record`
- **Audit Rec**: In `base-rs/src/server/database/rec/mbbodirect.rs::init_record`: confirm that
initialization checks whether `val` is UDF before deciding which direction to
sync (`VAL → bits` or `bits → VAL`). The correct precedence is:
`VAL` (if not UDF) → bits; else bits → `VAL`.

### `f1e83b2` — Timestamp updated after outlinks: downstream TSEL reads stale timestamp
- **Date**: 2017-02-18 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/records/ao.rs` | **Function**: `process`
- **Audit Rec**: In `src/server/records/ao.rs` (and bo.rs, calcout.rs, waveform.rs):
find the `process()` function. Verify `recgbl_get_timestamp()` /
`get_timestamp()` is called BEFORE the `write_value()` / `fwd_link()`
calls. For async records (pact=true on re-entry), verify timestamp is
refreshed after async...

### `f57acd2` — Add testdbCaWaitForConnect() for CA link connection synchronization
- **Date**: 2021-11-05 | **Category**: lifecycle | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_ca.rs` | **Function**: `testdb_ca_wait_for_connect`
- **Audit Rec**: - In `base-rs/src/server/database/db_ca.rs`: verify that connection and first
  data update notifications are separate signals, allowing tests to await each
  independently.
- Check that `CaLink` exposes a `connected()` async method or notifier that
  fires when the CA channel first connects (not...

### `1c566e2` — aai record: allow device support to defer init_record to pass 1
- **Date**: 2021-02-27 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/record_support.rs` | **Function**: `init_record`
- **Audit Rec**: Verify `base-rs/src/server/database/record_support.rs` `init_record` handles
two-phase initialization for aai-equivalent records.  Ensure device support
cannot capture a buffer pointer before the linked record has allocated it.

### `280aa0b` — Initialize errSymTable before database errors can occur in dbReadCOM
- **Date**: 2025-10-08 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/static_lib.rs` | **Function**: `db_read_database`
- **Audit Rec**: Audit `base-rs/src/server/database/static_lib.rs::db_read_database` (or the
equivalent entry point for loading `.db` files) to verify that the error
reporting subsystem is fully initialized before parsing begins.

### `5d5e552` — Add de-init hook announcements to iocShutdown sequence
- **Date**: 2019-11-14 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/ioc_init.rs` | **Function**: `ioc_shutdown`
- **Audit Rec**: In `base-rs/src/server/database/ioc_init.rs:ioc_shutdown`, verify that:
1. Scan tasks are fully stopped (awaited) before record memory is freed.
2. CA link tasks are cancelled before the record db is dropped.
3. Any registered shutdown callbacks are invoked in the correct order relative
   to...

### `7448a8b` — errlog worker exits loop before draining buffer at shutdown
- **Date**: 2022-11-14 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/log/errlog.rs` | **Function**: `errlog_thread`
- **Audit Rec**: In `base-rs/src/log/errlog.rs`, verify the errlog worker task: (1) drains remaining buffered messages when `atExit` signal arrives, (2) notifies any waiting `flush()` callers after the drain completes.

### `832abbd` — subRecord: propagate error from bad INP links instead of silently succeeding
- **Date**: 2022-12-20 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/records/sub_record.rs` | **Function**: `process`
- **Audit Rec**: In `base-rs` subRecord (or equivalent): verify that the `process` function
propagates `Err` from each input link fetch to the caller rather than always
returning `Ok(())`.

### `9df98c1` — logClient pending messages not flushed immediately after reconnect
- **Date**: 2019-08-28 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/log/log_client.rs` | **Function**: `reconnect_loop`
- **Audit Rec**: In `base-rs/src/log/log_client.rs`: verify that the reconnect loop calls
flush immediately after a successful `TcpStream::connect`, not just on
subsequent loop iterations. Specifically check for a `select!` or `match`
where connect and flush are in different arms with no tail-flush.

### `e11f880` — ts Filter Uses Stale db_field_log API — dtor Field Moved Out of Union
- **Date**: 2022-10-18 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/filters/ts.rs` | **Function**: `replace_fl_value`
- **Audit Rec**: In `base-rs`, if a `DbFieldLog` struct is defined with a destructor callback,
confirm the callback field is at the top struct level and not nested inside
a variant/union sub-struct. Verify ts filter equivalent registers its
cleanup via the same path the runtime uses to invoke cleanup.

### `eeb198d` — arrRecord: Move pfield assignment from cvt_dbaddr to get_array_info
- **Date**: 2020-03-30 | **Category**: lifecycle | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/db_access.rs` | **Function**: `cvt_dbaddr`
- **Audit Rec**: In `base-rs/src/server/database/db_access.rs:cvt_dbaddr`, verify that
`pfield` (or its Rust equivalent field reference) is not populated until
`get_array_info` time for array-type records. Prefer using an `Arc<Vec<T>>`
or snapshot reference obtained inside the scan-lock at read time.

### `932e9f3` — asLib: soft-fail DNS lookup, store "unresolved:<host>" instead of aborting
- **Date**: 2019-06-04 | **Category**: network-routing | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/access_security.rs` | **Function**: `hag_add_host`
- **Audit Rec**: In `base-rs/src/server/access_security.rs` (or equivalent), if DNS resolution
of HAG entries is implemented: verify that resolution failure stores a
never-matching sentinel and logs a warning, rather than propagating an error
that aborts ACF loading.

### `c9b6709` — logClient zero-byte send to detect broken TCP connections
- **Date**: 2019-09-18 | **Category**: network-routing | **Severity**: medium | **Verdict**: partial
- **File**: `src/log/log_client.rs` | **Function**: `flush`
- **Audit Rec**: In `base-rs/src/log/log_client.rs`: verify TCP keepalive is configured (via
`socket2` before handing the socket to tokio) so broken connections are
detected within seconds, not minutes. Alternatively check if the flush loop
explicitly tests for broken-pipe after each write.

### `4c20518` — recGblRecordError Skips Error Symbol Lookup for Negative Status Codes
- **Date**: 2024-02-26 | **Category**: other | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/recgbl.rs` | **Function**: `recgbl_record_error`
- **Audit Rec**: In `base-rs/src/server/database/recgbl.rs` or equivalent error reporting:
verify that any mapping from numeric EPICS status to error string correctly
handles both positive and negative error codes. In particular, any call to an
EPICS C FFI `errSymLookup` wrapper should guard with `status > 0`.

### `3dbc9ea` — iocsh argument splitter: EOF sentinel (-1) misread as valid char
- **Date**: 2023-02-01 | **Category**: other | **Severity**: medium | **Verdict**: partial
- **File**: `src/iocsh/tokenize.rs` | **Function**: `tokenize`
- **Audit Rec**: If base-rs implements an iocsh command parser/tokenizer, verify that the "no active quote" state is represented as `Option<char>` (None = not quoting) rather than a magic byte value.

### `d47fa4c` — aSub record: dbGetLink called on constant input links causing error
- **Date**: 2022-08-08 | **Category**: other | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/records/asub_record.rs` | **Function**: `fetch_values`
- **Audit Rec**: In `base-rs/src/server/database/records/asub_record.rs`, verify the input link fetch loop skips constant links (`dbLinkIsConstant` equivalent check) before calling the runtime DB get.

### `5aca4c6` — dbEvent: clear callBackInProgress before signaling pflush_sem
- **Date**: 2023-09-13 | **Category**: race | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `event_read`
- **Audit Rec**: - In `base-rs/src/server/database/db_event.rs`, verify that any "in-progress"
  atomic flag is cleared with `Release` ordering before the corresponding
  `Notify` or channel send that wakes the cancel waiter.
- Pattern: `flag.store(false, Release); notify.notify_one();` — never interleave
  notify...

### `5ba8080` — Waveform NORD posted before timestamp update causes undefined timestamp on first CA monitor update
- **Date**: 2022-05-13 | **Category**: race | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/waveform_record.rs` | **Function**: `process`
- **Audit Rec**: In `waveform_record.rs::process()`, verify that NORD-change notifications are
posted only after the record timestamp has been stamped (i.e., after the
equivalent of `recGblGetTimeStampSimm`). Snapshot `nord` before calling into
device support, then post the event after the timestamp is set.

### `059d32a` — dbChannel Type Probe Struct Has Uninitialized Members
- **Date**: 2023-05-25 | **Category**: race | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/channel.rs` | **Function**: `channel_open`
- **Audit Rec**: In `base-rs` `channel.rs`, verify that the `DbFieldLog` probe struct used
during channel filter chain construction is fully zero-initialized before
being passed to filter `register` callbacks. Prefer `Default::default()`
construction over manual field assignment.

### `333446e` — dbDbLink: Assert lockset ownership before dbPutLink
- **Date**: 2025-06-16 | **Category**: race | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/db_link.rs` | **Function**: `process_target`
- **Audit Rec**: In `base-rs/src/server/database/db_link.rs`, check that `process_target` (or
its equivalent) acquires the destination record's scan-lock before invoking
`db_process`. Add a debug assertion or a structured lock guard type that
enforces this statically.

### `4966baf` — SIZV Field Uncapped at 32767: Signed field_size Overflow
- **Date**: 2024-05-19 | **Category**: type-system | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/records/printf_record.rs` | **Function**: `init_record`
- **Audit Rec**: In `base-rs/src/server/database/records/printf_record.rs` (or lsi/lso
equivalents): verify that `sizv` is capped at `0x7FFF` before being stored
in `DbAddr::field_size` (an `i16`). Check that `u16::try_into::<i16>()` is
used with proper error handling rather than `as i16`.

### `5485ada` — Make epicsNAN and epicsINF truly const on all platforms
- **Date**: 2022-04-15 | **Category**: type-system | **Severity**: medium | **Verdict**: applies
- **File**: `src/types/epics_math.rs` | **Function**: `null`
- **Audit Rec**: - Search `base-rs` and `ca-rs` for any `static mut` float constants used as
  sentinels (NaN/Inf representations for EPICS undefined values).
- Verify that `DBF_FLOAT`/`DBF_DOUBLE` "undefined" encoding uses `f32::NAN` /
  `f64::NAN` directly, not a stored global.

### `b36e526` — Const link string init fails for DBF_CHAR waveform fields
- **Date**: 2020-08-21 | **Category**: type-system | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_loader/mod.rs` | **Function**: `null`
- **Audit Rec**: In `src/server/database/db_loader/mod.rs` or wherever const-link
string values are loaded into waveform/aai records: search for
`DBF_CHAR` or `FieldType::Char` handling. Verify that string-to-char-array
conversion uses the full buffer length (`nelm`) rather than a 40-byte
limit. Look for use of a...

### `e88a186` — Signed bit field UB in struct link::flags
- **Date**: 2023-11-24 | **Category**: type-system | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/link.rs` | **Function**: `null`
- **Audit Rec**: In `base-rs/src/server/database/link.rs` (or wherever `struct link` / `DbLink` is defined), verify:
1. The `flags` field is `u32` (or equivalent unsigned type), not `i16` or `i32`.
2. Any bitmask constants used with `flags` are unsigned literals.
3. If using `bitflags!` crate, confirm the backing...

### `b460c26` — Menu field conversion returns error for out-of-range enum index instead of numeric string
- **Date**: 2022-11-01 | **Category**: type-system | **Severity**: medium | **Verdict**: partial
- **File**: `src/server/database/db_fast_link_conv.rs` | **Function**: `cvt_menu_st`
- **Audit Rec**: In `base-rs/src/server/database/db_fast_link_conv.rs` (or equivalent): verify `cvt_menu_st` / `db_get_string_num` handles `value >= n_choice` by formatting the raw integer, not returning an error.

### `235f8ed` — db_field_log missing DBE_* mask prevents filter from distinguishing DBE_PROPERTY
- **Date**: 2020-04-20 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_event.rs` | **Function**: `db_create_event_log`
- **Audit Rec**: 1. Locate the Rust `DbFieldLog` / `FieldLog` struct — verify it contains an
   event-mask field.
2. Locate `db_create_event_log` equivalent — verify the subscription's select
   mask is stored into the log.
3. Any filter implementation that guards on `DBE_PROPERTY` must read the field,
   not...

### `3b3261c` — Revert S_db_emptyArray — empty array must return S_db_badField for compatibility
- **Date**: 2020-05-22 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `dbGet`
- **Audit Rec**: Search base-rs for any `EmptyArray` or separate empty-array error path in
`db_access.rs` and `db_db_link.rs`. Ensure `dbGet` returns `Err(DbError::BadField)`
when element count is zero, matching the canonical C behavior post-revert.

### `82ec539` — dbPut: long-string (nRequest>1) skips get_array_info, corrupts write path
- **Date**: 2021-08-08 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_put`
- **Audit Rec**: In `base-rs/src/server/database/db_access.rs::db_put`: check the branch
condition for entering the `get_array_info` path. Must cover `nRequest > 1`
in addition to `SPC_DBADDR`. Verify the `put_array_info` guard is not called
for long-string writes (it is only meaningful for true array fields). Add...

### `88bfd6f` — dbConvert: allow hex/octal string-to-integer conversion in dbPut/dbGet
- **Date**: 2025-11-05 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/convert.rs` | **Function**: `put_string_to_integer`
- **Audit Rec**: Audit `base-rs/src/server/database/convert.rs` and any function that
converts a string field value to an integer type during CA put processing.
Verify that hex (`0x...`) and octal (`0...`) prefixes are handled or that
the omission is a documented deliberate choice.

### `9e7cd24` — DBE_PROPERTY events missing for mbbi/mbbo when val != changed string index
- **Date**: 2024-09-02 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/rec/mbbi_record.rs` | **Function**: `special`
- **Audit Rec**: In `mbbi_record.rs` and `mbbo_record.rs`, verify that the `special`
handler for ZRST–FFST field writes does NOT call `db_post_events` with
`DBE_PROPERTY`.  That event must be posted by `db_access.rs::dbPut`.

### `b7cc33c` — DBE_PROPERTY event posted after DBE_VALUE instead of before
- **Date**: 2024-09-02 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `dbPut`
- **Audit Rec**: In `db_access.rs` (or equivalent write path), verify the event-posting
order: `DBE_PROPERTY` must be dispatched before `DBE_VALUE | DBE_LOG`
when `propertyUpdate` is true.

### `f2fe9d1` — bi "Raw Soft Channel" did not apply MASK to RVAL
- **Date**: 2023-11-02 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/dev/bi_soft_raw.rs` | **Function**: `read_locked`
- **Audit Rec**: In `base-rs/src/server/database/dev/bi_soft_raw.rs` (or equivalent), verify that `rval &= mask` is applied when `mask != 0` after reading the raw value from the input link. This must happen before the standard convert-RVAL-to-VAL pipeline.

### `faac1df` — Spurious DBE_PROPERTY events posted even when property field value unchanged
- **Date**: 2024-08-30 | **Category**: wire-protocol | **Severity**: medium | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `dbPut`
- **Audit Rec**: In `db_access.rs::dbPut`, verify that a `propertyUpdate` flag is only
set when the property field content actually changes.  A `memcmp` (or
`PartialEq` check in Rust) of old vs. new value must gate the
`DBE_PROPERTY` post.

### `275c4c7` — Wrong pointer deref in empty-array guard in dbGet
- **Date**: 2020-05-07 | **Category**: bounds | **Severity**: low | **Verdict**: applies
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: Grep for `no_elements` in `src/server/database/db_access.rs`. Verify
that the empty-array guard after filter application uses the
locally-scoped count variable, not a struct field from the filter log.

### `4ab9808` — arr filter: wrapArrayIndices early-return clarifies empty-slice path
- **Date**: 2020-03-30 | **Category**: bounds | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/filters/arr.rs` | **Function**: `wrap_array_indices`
- **Audit Rec**: Verify that `wrap_array_indices` (or equivalent range-clamping helper) in
`base-rs/src/server/database/filters/arr.rs` returns `0` (not an underflow or
panic) when the computed end index is less than start. Add a unit test for the
backward-range case.

### `8488c9e` — initHookName() Missing Compile-Time Array Length Consistency Check
- **Date**: 2023-09-03 | **Category**: bounds | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/init_hooks.rs` | **Function**: `init_hook_name`
- **Audit Rec**: In `base-rs` `init_hooks.rs`, confirm `init_hook_name()` uses an exhaustive
`match` on the `InitHookState` enum. If it uses a `&[&str]` slice with
numeric indexing, add either a `const` assertion on slice length or convert
to `match`.

### `8483ff9` — NAMSG not cleared after alarm promoted to AMSG, leaving stale message
- **Date**: 2024-11-14 | **Category**: lifecycle | **Severity**: low | **Verdict**: applies
- **File**: `src/server/database/rec_gbl.rs` | **Function**: `recGblResetAlarms`
- **Audit Rec**: In `rec_gbl.rs` audit `recGblResetAlarms` (or its Rust equivalent):
after `amsg = namsg`, verify `namsg` is reset to an empty string.  Also
check that the `DBE_ALARM` event-trigger comparison accounts for this
clear.

### `bc7ee94` — Remove spurious warning when PUTF is set on target with PACT false
- **Date**: 2019-01-03 | **Category**: lifecycle | **Severity**: low | **Verdict**: applies
- **File**: `src/server/database/db_link.rs` | **Function**: `process_target`
- **Audit Rec**: When implementing link traversal in `process_target` (or equivalent), ensure
no diagnostic fires for the combination `putf=true` + `pact=false` on the
destination — this is the normal state at the start of a notify chain.

### `372e937` — dbGet: duplicated dbfl_type_val/ref dispatch replaced with dbfl_pfield macro
- **Date**: 2021-01-14 | **Category**: lifecycle | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/db_access.rs` | **Function**: `db_get`
- **Audit Rec**: If base-rs has a `DbFieldLog` or equivalent struct with a val/ref union
pattern, ensure field-data access is centralized in a method rather than
duplicated at call sites.

### `550e902` — iocLogPrefix warns on identical re-set instead of accepting silently
- **Date**: 2023-01-19 | **Category**: lifecycle | **Severity**: low | **Verdict**: partial
- **File**: `src/log/log_client.rs` | **Function**: `ioc_log_prefix`
- **Audit Rec**: Check `base-rs/src/log/log_client.rs::ioc_log_prefix` — verify the "already set" guard compares prefix values, not just checks for `Some(_)`.

### `acd1aef` — Silent CP/CPP Modifier Discard on Output Links
- **Date**: 2025-10-08 | **Category**: lifecycle | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/link.rs` | **Function**: `parse_link`
- **Audit Rec**: When `base-rs` implements DB link parsing, audit the output-link modifier
filtering path to ensure unsupported modifiers produce a log warning rather
than silent discard. The function signature should carry record/field
context for diagnostics.

### `73cdea5` — rsrv/asLib: rename asUseIP→asCheckClientIP, ignore client hostname when set
- **Date**: 2019-05-08 | **Category**: network-routing | **Severity**: low | **Verdict**: partial
- **File**: `src/server/access_security.rs` | **Function**: `null`
- **Audit Rec**: In ca-rs server host_name command handler: if `asCheckClientIP` is enabled,
verify the handler ignores the client-provided name and the peer IP is used for
all AS checks.

### `144f975` — iocsh: propagate error codes from db/libcom commands via iocshSetError
- **Date**: 2024-06-13 | **Category**: other | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/ioc_register.rs` | **Function**: `null`
- **Audit Rec**: In `base-rs/src/server/database/ioc_register.rs` (or equivalent iocsh
registration), verify that iocsh command wrappers check their return values and
set the iocsh error state. Ensure the iocsh runner exits with a non-zero code
if any command returns an error.

### `3b484f5` — dbConstLink: treat empty string same as unset link
- **Date**: 2023-03-06 | **Category**: other | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/const_link.rs` | **Function**: `load_scalar`
- **Audit Rec**: In `base-rs` const-link parsing: confirm that `""` as a link value is
rejected early with an appropriate error, not silently treated as a zero or
default. Look for `str.parse::<f64>()` or `serde_json::from_str` called
without an upfront `is_empty()` guard.

### `5c77c84` — Test Harness Cannot Detect NaN Equality; DBR Type IDs Not Human-Readable
- **Date**: 2025-07-31 | **Category**: other | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/unit_test.rs` | **Function**: `null`
- **Audit Rec**: Search base-rs and ca-rs test code for float equality assertions (`assert_eq!`
on `f32`/`f64` values) and confirm NaN cases are handled with
`f64::is_nan()` guards or `assert!(result.is_nan())`.

### `27918cb` — dbPutString: insufficient error message for DBF_MENU/DEVICE invalid choice
- **Date**: 2021-02-04 | **Category**: type-system | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/static_run.rs` | **Function**: `put_string_num`
- **Audit Rec**: If base-rs implements DBD field parsing, ensure error types carry the menu
name or device type name when an invalid choice is rejected, rather than only
surfacing a raw "parse error" code.

### `2c1c352` — DBF_MENU/DEVICE: missing "did you mean" suggestion on parse error
- **Date**: 2021-02-05 | **Category**: type-system | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/static_lib.rs` | **Function**: `put_string_suggest`
- **Audit Rec**: No correctness audit needed. If base-rs implements a DBD parser with field
validation, consider adding a fuzzy-match suggestion for menu/device fields
to improve operator experience.

### `d1491e0` — dbpf switches from whitespace-delimited to JSON array format for array puts
- **Date**: 2020-07-17 | **Category**: wire-protocol | **Severity**: low | **Verdict**: partial
- **File**: `src/server/database/db_test.rs` | **Function**: `dbpf`
- **Audit Rec**: Check base-rs test utilities and any CLI `put` helpers for array argument parsing. Ensure they use a typed JSON path, not a raw `split_whitespace()` approach that cannot represent empty arrays.

## bridge-rs

_(none)_
