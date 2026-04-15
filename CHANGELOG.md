# Changelog

## v0.9.3 — 2026-04-15

### Examples

- All example IOCs now serve both Channel Access and pvAccess
  simultaneously:
  - `ophyd-test-ioc`: switch `AdIoc` to `run_from_args_with_pva`
    (enable `ad-plugins-rs/pva`)
  - `scope-ioc`: swap `run_ca_ioc` for
    `epics_bridge_rs::qsrv::run_ca_pva_qsrv_ioc`
  - `mqtt-ioc`: same protocol-runner swap
- Remove `random-signals` example — it was a programmatic
  `IocBuilder` demo rather than a st.cmd-style IOC, and its
  unused `db/` file made the divergence more confusing than
  useful. The seven remaining examples are all st.cmd-driven
  dual-protocol IOCs.

### Documentation

- Drop "experimental" status from `epics-pva-rs`,
  `epics-bridge-rs`, and the pvAccess CLI tool section
  (`pvget-rs`, `pvput-rs`, `pvmonitor-rs`, `pvinfo-rs`) in the
  top-level and per-crate READMEs.
- Refresh stale "server-side planned" notes in `epics-pva-rs`
  README now that the server is shipped.

## v0.9.2 — 2026-04-16

### pvAccess / QSRV

- **pvAccess protocol support** — full client & server via [spvirit](https://crates.io/crates/spvirit-server) integration
- **QSRV bridge** — map EPICS records to PVA NormativeTypes (NTScalar, NTEnum, NTNDArray) via `info(Q:group)` JSON configuration
- **NDPluginPva** — serve AreaDetector NDArray as NTNDArray over pvAccess, compatible with C++ `pvget -m`
- **Dual-protocol CA+PVA runner** — `run_ca_pva_qsrv_ioc()` for all example IOCs
- **PVA CLI tools** — `pvget-rs`, `pvmonitor-rs`, `pvput-rs`, `pvinfo-rs` (renamed from `pvaget-rs` etc.)
- **spvirit 0.1.9** from crates.io (removed `[patch.crates-io]` path overrides)

### xrt-beamline example

- **Real-time ray tracing simulation** — Undulator → DCM Si(111) → HFM → VFM → Sample at 8 keV
- 25 motors driving [xrt-rs](https://github.com/physwkim/xrt-rs) ray tracing with AreaDetector output
- Accumulation over `AcquireTime` for improved statistics
- PyDM viewer with contrast control, xrtGlow 3D viewer with pyepics PV monitoring
- Coddington-calculated mirror radii (HFM R=3.27 km, VFM R=1.82 km)

### xrt-rs fixes (companion repo)

- **position_roll**: implement as roll addition matching xrt Python behavior
- **bracketing**: increase t_min clamp from -1e-6 to -100 mm for large pitch angles (DCM at 14°)
- **reflect()**: use `state==1` filter to prevent Over ray reprocessing

### Other

- Upgrade spvirit dependencies 0.1.8 → 0.1.9
- Fix clippy warnings across workspace

## v0.9.1 — 2026-04-13

### motor-rs

- **Fix RBV monitor updates during motion**: `process()` was returning
  `AsyncPendingNotify` on every poll cycle with only DMOV/VAL/DVAL/RVAL
  fields — RBV and DRBV were missing. Now uses `AsyncPendingNotify` only
  for the initial DMOV 1→0 transition; subsequent polls return `Complete`
  which posts monitors for all changed fields including RBV.
- **Fix missing DMOV monitor on back-to-back motions**: When a new put
  arrives while the previous motion's done status is consumed in the same
  process cycle, `dmov_notified` was not reset. Fixed by resetting the
  flag in `plan_motion()`.
- **Fix same-direction NTM retarget**: `ExtendMove` accepted the new
  DVAL but never re-dispatched a `MoveAbsolute` to the driver. On
  completion, `evaluate_position_error()` only retried under retry
  conditions (RTRY>0, RDBD>0). Now sets `verify_retarget_on_completion`
  so the completion path replans if DVAL ≠ DRBV regardless of retry
  settings.

### epics-ca-rs

- **CA repeater**: Rewrite to use per-client connected UDP sockets
  matching C EPICS architecture. Fixes compatibility with C CA clients
  (camonitor, caget) that could not register with the Rust repeater.
- **Pre-connection subscription**: `subscribe()` now registers
  subscriptions even when disconnected. On connect, the coordinator
  fills in native type and element count and issues `CA_PROTO_EVENT_ADD`.
  Eliminates the need for application-level resubscribe loops.
- **Add `get_with_timeout()`** for explicit timeout control on reads.
- **Monitor flow control**: Client-side backlog tracking replaces TCP
  read count heuristic. Server-side `FlowControlGate` with
  `coalesce_while_paused()` matching C EPICS `dbEvent.c` behavior.
- **Add `ioc` feature** to umbrella crate for IOC builds.
- **Fix proc macro path resolution**: `epics_main`/`epics_test` now
  resolve `epics_base_rs` path for umbrella crate users via
  `proc-macro-crate`.

### CA tools (C parity)

- **camonitor-rs**: Use server timestamp, print disconnect to stdout
  as `*** disconnected`, add `-w` initial connection timeout. Subscribe
  once and rely on library auto-restore (no resubscribe loop).
- **caput-rs**: Re-read value from server for `New` line. Apply `-w`
  timeout to all reads. Fix `-c` description.
- **caget-rs**: Parallel PV connect+read via `tokio::spawn`. Add `-w`
  timeout. Distinguish "Not connected" from "timeout" errors.
- **cainfo-rs**: Add `-w` timeout, use explicit channel connect.
- All tools: Rename help text from `rcaXXX` to `caXXX`.

## v0.9.0

### motor-rs — Complete C parity (~95 fixes across 12 review rounds)

#### State machine
- Fix MSTA bit positions for wire compatibility with C clients
- Fix all 4 retry modes (Default/Arithmetic/Geometric/InPosition)
- Fix SPMG Pause/Stop/Go transitions to match C postProcess pipeline
- Add MIP_EXTERNAL detection for externally-initiated motion
- Add clear_buttons on limit switch hit or PROBLEM
- Add stop-first pattern for home-while-moving and jog-while-moving
- Add DLY → DELAY_ACK → fresh poll → retry evaluation flow
- Add limit switch direction guard before retries (user_cdir)
- Implement two-phase jog backlash (BL1 slew + BL2 backlash velocity)
- Add sub-step deadband check with DMOV pulse for ophyd compatibility

#### Coordinate system
- Fix CDIR to account for MRES sign
- Fix DIR handler FOFF branching (Variable preserves VAL)
- Fix SET+FOFF=Frozen cascade for VAL/DVAL/RVAL
- Fix FOFF=Frozen in non-SET mode (no effect, matches C)
- Fix RDIF type (i32) and formula (NINT(diff/mres))
- Fix LVIO escape logic using ldvl, pretarget only for non-preferred direction
- Fix soft limit disable only when dhlm==dllm==0
- Add RHLM/RLLM fields for MRES cascade invariance

#### New features
- Add MoveRelative command and use_rel logic (ueip/urip)
- Add FRAC progressive approach scaling
- Add dual poll rate (moving/idle intervals, forced fast polls)
- Add auto power on/off with configurable delays
- Add deferred moves and profile moves framework
- Add RDBL/URIP readback link support
- Add velocity cross-calculation and range validation

#### Driver interface
- Expand MotorStatus with direction, slip_stall, comms_error, homed, gain_support, has_encoder, velocity
- Add move_velocity, move_relative, set_deferred_moves trait methods
- Add profile move trait methods (initialize, define, build, execute, abort, readback)
- Fix SetPosition to send dial coordinates (not raw steps)
- Fix MOVN ls_active to use raw limit switches before user mapping

### asyn-rs
- Fix race condition in PortManager register/unregister
- Fix COMM_ALARM constant, HTTP connect-per-transaction
- Fix write retry timeout, HTTP write reconnect, EOS storage
- Fix param defined tracking, IP port auto-disconnect
- Fix trace masks, serial flush, baud rates, break/ixany
- Fix asyn_record connect_device clearing drv_user_create error
- Add PortHandle convenience methods for new operations
- Add `set_params_and_notify` for atomic background thread parameter updates
- Add ParamSetValue::Float64Array for waveform parameter updates
- Add AsynMotor::move_relative, set_deferred_moves, profile move methods
- Move set_rs485_option out of PortDriver trait impl
- Document `set_params_and_notify` vs `write_int32_no_wait` for driver authors

### epics-base-rs
- Fix ai/ao conversion pipeline (ASLO/AOFF/ESLO/EOFF)
- Fix bi/bo records and COS alarm
- Fix calc division by zero to return NaN
- Fix mbbi/mbbo state handling and field access
- Fix sel record High/Low/Median algorithms
- Fix calcout missing OUT link write (pval timing + cached should_output)
- Fix WriteDbLink to use resolve_field for common fields (OUT/DOL)
- Fix monitor deadband for binary records (bi/bo/busy/mbbi/mbbo always post)
- Document DeviceReadOutcome ok() vs computed() convention

### ad-core-rs
- Fix ADDriverBase MaxSizeX/Y init from constructor args
- Fix NDArrayPool threshold and free-list logic
- Fix plugin runtime interrupt notifications
- Add ParamUpdate::Float64Array for waveform param updates in plugins

### ad-plugins-rs
- Fix ROIStat time series waveform readback (was accumulating but never writing to params)
- Fix ROI, Stats, Process, HDF5, TIFF, JPEG, NetCDF, Nexus plugins
- Add attr_plot param indices and buffer output infrastructure

### examples
- Migrate all acquisition tasks to set_params_and_notify
- Fix beam_current and time_of_day DeviceReadOutcome to skip ai conversion
- Fix moving_dot acquire_busy and status in writeInt32

## v0.8.3

### asyn-rs

- Remove unbounded sync channel from `InterruptManager`, replacing it with a simpler notification mechanism to eliminate memory leaks when interrupt callbacks accumulate faster than consumed.

### motor-rs

- Fix tight poll loop consuming excessive CPU when motor is in motion.
- Defer `StartPolling` to `after_init` hook to prevent premature polling during st.cmd and autosave restore.
- Throttle `StartPolling` and send only on idle-to-active transition, removing redundant poll requests.
- Clear `last_write` in init to prevent restore-triggered moves.
- Sync driver position from pass0-restored VAL during initialization.

### epics-base-rs

- Add `after_init` hooks that run after PINI processing, matching C EPICS `initHookAfterIocRun` timing.

### epics-ca-rs

#### Client

- **Fix**: Slow reconnection after IOC restart (~50s → ~5s). Beacon monitor was skipping `available=INADDR_ANY` beacons (all modern IOCs), reading the wrong header field for server port, and doing per-server rescan instead of global rescan.
- **Fix**: ECHO ping-pong loop causing 50%+ CPU usage. Client was echoing back the server's echo responses, creating a tight infinite loop after the first 30-second idle timeout.
- **Fix**: Search response `INADDR_ANY` check (`0xFFFFFFFF` → `0`) for C server interoperability.
- **Fix**: `handle_disconnect` operator precedence bug causing channels on unrelated servers to be incorrectly disconnected.
- **Fix**: Pending read/write waiters now receive `CaError::Disconnected` on server disconnect instead of hanging forever.
- **Fix**: `DropChannel` now properly cleans up all channel states (Connecting, Disconnected, Unresponsive).
- Beacon-TCP watchdog integration: immediate echo probe on beacon anomaly detects dead connections in ~5s instead of ~35s.
- Send buffer backpressure: close stalled connections at 4096 pending frames.
- Search datagram sequence validation to reject stale responses from previous rounds.
- TCP read buffer capped at 1MB to protect against malformed servers.
- Defensive bounds checks and malformed message logging.
- `align8` overflow protection with `saturating_add`.

#### Server

- **Fix**: Beacon header field swap (`data_type`/`count` were swapped), breaking C client interop.
- **Fix**: Search response `INADDR_ANY` sentinel (`0xFFFFFFFF` → `0`), matching C protocol.
- **Fix**: `WRITE_NOTIFY` response `count` field was hardcoded to 1 instead of echoing the request count.
- **Fix**: `CLEAR_CHANNEL` response was missing `data_type` and `count` fields.

#### Repeater

- **Fix**: Accept zero-length UDP registration for C client backward compatibility (pre-3.12 protocol).
- **Fix**: Fill in beacon `available` field with source IP on relay, matching C repeater behavior.

### optics-rs

- Add HSC and QXBPM async driver support with deferred poll start.

## v0.8.2

### epics-bridge-rs (new crate)

New umbrella crate for EPICS protocol bridges. Hosts feature-gated sub-modules:

- **`qsrv`** (default) — Record ↔ pvAccess channels (C++ EPICS QSRV equivalent). Single PVs (NTScalar/NTEnum/NTScalarArray) and multi-record group PVs with full metadata, pvRequest filtering, process/block put options, AccessControl enforcement on get/put/monitor, nested field paths, info(Q:group, ...) parsing, and trigger validation.
- **`ca-gateway`** (default) — CA fan-out gateway (C++ ca-gateway equivalent). Includes `.pvlist` parser with regex backreferences, ACF integration, lazy on-demand resolution via search hook, per-host connection tracking, statistics PVs, beacon throttle, putlog, runtime command interface, and an auto-restart supervisor.
- **`pvalink`**, **`pva-gateway`** — placeholders for future implementations.

The `ca-gateway-rs` daemon binary builds via `cargo build --release -p epics-bridge-rs --bin ca-gateway-rs` and lands in `target/release/ca-gateway-rs`.

The umbrella `epics-rs` crate gains a `bridge` feature that re-exports `epics-bridge-rs` as `epics_rs::bridge`.

### epics-base-rs

#### **Behavior change**: `PvDatabase::has_name()` / `find_entry()` now invoke an optional async search resolver on miss

`PvDatabase` gained `set_search_resolver(SearchResolver)` / `clear_search_resolver()` plus a new `SearchResolver` type alias. When set, both `has_name()` and `find_entry()` invoke the resolver on a database miss; the resolver may populate the database (e.g. by subscribing to an upstream IOC) and return `true` to make the lookup succeed on the immediate re-check.

**Compatibility**: with no resolver installed (the default), behavior is unchanged. However, callers that previously assumed `has_name()`/`find_entry()` were *cheap, side-effect-free* lookups should be aware these methods can now `.await` arbitrary work when a resolver is registered. The current in-tree usage (CA UDP search responder, TCP create-channel handler) is consistent with this design.

This hook is what enables `epics-bridge-rs::ca_gateway` to lazily subscribe upstream PVs on first downstream search instead of requiring a `--preload` file.

#### `Snapshot` / `DisplayInfo` — additive fields

- `DisplayInfo` gained `form: i16` (display format hint, from `Q:form` info tag) and `description: String` (DESC). Existing initializers need `..Default::default()` to remain forward-compatible — internal call sites have been updated.
- `Snapshot` gained `user_tag: i32` (from `Q:time:tag` nsec LSB splitting). Defaults to 0.

These fields propagate into PVA NTScalar `display.form` / `display.description` and `timeStamp.userTag` via `epics-bridge-rs::qsrv::pvif`.

### epics-ca-rs

#### **Breaking**: `tcp::run_tcp_listener()` signature changed

Added a 6th parameter:

```rust
pub async fn run_tcp_listener(
    db: Arc<PvDatabase>,
    port: u16,
    acf: Arc<Option<AccessSecurityConfig>>,
    tcp_port_tx: tokio::sync::oneshot::Sender<u16>,
    beacon_reset: Arc<tokio::sync::Notify>,
    conn_events: Option<broadcast::Sender<ServerConnectionEvent>>, // ← new
) -> CaResult<()>;
```

External callers of `run_tcp_listener()` must pass `None` (opt out of connection lifecycle events) or a `broadcast::Sender` to subscribe.

In-workspace consumers (`server::ca_server::CaServer::run` and `crates/epics-base-rs/tests/client_server.rs`) have been updated.

#### Additive: `CaServer::connection_events()` and `ServerConnectionEvent`

`CaServer` now exposes `connection_events()` which returns a `broadcast::Receiver<ServerConnectionEvent>` (`Connected(SocketAddr)` / `Disconnected(SocketAddr)`). Used by `epics-bridge-rs::ca_gateway` for per-host downstream client tracking. Servers that don't subscribe see no behavior change.

## v0.8.1

### Fix: Plugin param update re-entrancy (CPU 100% on idle)

Plugin `on_param_change` handlers that return `ParamUpdate` values (readback pushes)
previously used `write_int32_no_wait` which sends `Int32Write` to the port actor.
The port actor then calls `io_write_int32` → `on_param_change` again, causing
**infinite re-entrancy loops** (e.g., Overlay Position↔Center bidirectional update).

This is now fixed by introducing `ParamSetValue` and `set_params_and_notify()`,
which mirrors C ADCore's `setIntegerParam()` + `callParamCallbacks()` pattern:
values are stored directly in the param store without going through the driver's
write path, so `on_param_change` is never re-triggered.

- **asyn-rs**: Add `ParamSetValue` enum, extend `CallParamCallbacks` with inline param updates, add `PortHandle::set_params_and_notify()`
- **ad-core-rs**: `publish_result` now uses `set_params_and_notify` instead of `write_int32_no_wait` for plugin readback values
- **ad-plugins-rs**: Restore Overlay Position↔Center bidirectional readback (safe with new path)
- **commonPlugins.cmd**: Add missing `NDTimeSeriesConfigure` commands for Stats/ROIStat/Attr TS ports

## v0.8.0

### HDF5 Plugin — Complete Rewrite
- **Pure Rust HDF5**: Switch from fallback binary format to real HDF5 via `rust-hdf5` (crates.io `0.2`). No C dependencies.
- **Compression**: zlib, SZIP, LZ4, Blosc (with sub-codecs: BloscLZ, LZ4, LZ4HC, Snappy, Zlib, Zstd). All via `rust-hdf5` filter pipeline.
- **SWMR streaming**: Single Writer Multiple Reader support — `SwmrFileWriter` with `append_frame`, periodic flush, ordered fsyncs.
- **Store performance**: Write timing measurement with Run time / I/O speed readback.
- **Store attributes**: Controllable via param (on/off).
- **File number fix**: Last filename now shows the actual written file, not the next incremented number.

### NeXus File Plugin (New)
- **NDFileNexus**: HDF5-based NeXus format writer with `/entry/instrument/detector/data` group hierarchy via `rust-hdf5` group API.

### Plugin on_param_change — All Plugins Complete
- **Process**: Full `on_param_change` for all 34 params. Filter type presets (RecursiveAve, Average, Sum, Difference, RecursiveAveDiff, CopyToFilter). Auto offset/scale calc. Separate low/high clip threshold and value. Scale flat field param.
- **Transform**: `on_param_change` for TRANSFORM_TYPE.
- **ColorConvert**: `on_param_change` for COLOR_MODE_OUT and FALSE_COLOR.
- **Overlay**: 8 runtime-configurable overlay slots via addr, with Position↔Center bidirectional readback.
- **FFT**: `on_param_change` for direction, suppress DC, num_average, reset_average. Num averaged readback.
- **CircularBuff**: `on_param_change` for Start/Stop, trigger A/B attributes, calc expression, pre/post count, preset triggers, soft trigger, flush on trigger. Status/triggered/trigger count readback.
- **Codec**: `on_param_change` for mode, compressor (LZ4/JPEG/Blosc), JPEG quality, Blosc sub-compressor/level/shuffle. Compression factor and status readback. Blosc compress/decompress via `rust-hdf5` filter pipeline.
- **Stats**: `on_param_change` for compute_statistics toggle.
- **BadPixel**: `on_param_change` for BAD_PIXEL_FILE_NAME — loads JSON bad pixel list at runtime. Moved from stub to real processor.
- **Attribute**: 8-channel multi-addr attribute extraction with TimeSeries integration. Moved from stub to real processor.

### Scatter/Gather — C ADCore Compatible
- **Scatter**: Round-robin distribution via `ProcessResult::scatter_index`. New `NDArrayOutput::publish_to(index)` for selective delivery.
- **Gather**: Multi-upstream wiring in `NDGatherConfigure` — accepts multiple port names.

### TimeSeries Refactor
- **`TsReceiverRegistry`**: Shared registry pattern. Stats/ROIStat/Attribute store TS receivers; `NDTimeSeriesConfigure` picks them up. Eliminates duplicate TS port creation code.
- **`NDTimeSeriesConfigure`**: Fully implemented (no longer a stub).

### File Plugin Infrastructure
- **Lazy open / Delete driver file / Free buffer**: Params wired in `FilePluginController` (shared by all file plugins).
- **ROIStat**: 32 ROIs (up from 8), with `NDROIStatN.template` × 32 in commonPlugins.cmd.

### Dependencies
- **rust-hdf5**: Switch from git dependency to crates.io `0.2`. Pure Rust HDF5 with all compression filters.

## v0.7.12

### CA Client Connection Stability
- **TCP keepalive**: Enable `SO_KEEPALIVE` with 15s idle time and 5s probe interval on all CA TCP connections. OS detects dead sockets within ~30s on idle circuits.
- **Client-side echo heartbeat**: Send `CA_PROTO_ECHO` after 30s of idle (matching C EPICS `CA_CONN_VERIFY_PERIOD`). If no response within 5s (`CA_ECHO_TIMEOUT`), declare connection dead and trigger automatic re-search + subscription recovery. Detects hung server processes that TCP keepalive alone cannot catch.
- **`EPICS_CA_CONN_TMO` support**: Echo interval configurable via environment variable, matching C EPICS behavior.

### Motor Record
- **Fix MOVN not resetting to 0**: `finalize_motion()` now clears MOVN when motion completes. Previously MOVN was computed before the phase transition to Idle and never updated, causing ophyd `PVPositionerPC` (which reads `.MOVN`) to report moving=true after `move(wait=True)` returned.

### areaDetector Plugins
- **NDFileMagick plugin**: New file writer using the `image` crate. Supports PNG, JPEG, BMP, GIF, TIFF (format determined by file extension), UInt8/UInt16 data, mono and RGB color modes. Parameters: `MAGICK_QUALITY`, `MAGICK_BIT_DEPTH`, `MAGICK_COMPRESS_TYPE`.
- **Idempotent plugin Configure commands**: Skip if port already exists, allowing `commonPlugins.cmd` to be loaded multiple times with different `PREFIX` for alias records.
- **Activate NDFileMagick** in `commonPlugins.cmd`.

### Asyn Device Support
- **Initial readback for input records**: Enable `with_initial_readback()` for input records (stringin, longin, etc.), matching C EPICS `devAsynXxx` `init_common()` behavior. Fixes `PluginType_RBV` and other I/O Intr input records returning template defaults ("Unknown") instead of the driver's current value.

### Wiring
- **Fix sender loss on failed rewire**: Validate new upstream exists before extracting sender from old upstream. Previously a failed rewire (e.g., invalid port name) would drop the sender, causing all subsequent rewires to fail.

## v0.7.11

### CA Client Transport Rewrite
- **Single-owner writer task**: Replace `Arc<Mutex<OwnedWriteHalf>>` with a dedicated `write_loop` task + mpsc channel. Eliminates writer lock contention between command dispatch and read_loop (ECHO responses).
- **Batch coalescing**: Writer task drains all pending frames via `try_recv` before issuing a single `write_all`, reducing TCP segment count under burst load.
- **TCP_NODELAY**: Set on all CA transport connections. Fixes ~45ms stall on `get()` immediately after `put()` caused by Nagle's algorithm + delayed ACK interaction.
- **Immediate write-error propagation**: `write_loop` sends `TcpClosed` on socket write failure, so pending `get()`/`put()` waiters fail immediately instead of hanging until timeout.

### CA Client Connection Fix
- **Channel starvation during concurrent PV creation**: `WaitConnected` and `Found` responses arriving before `RegisterChannel` are now buffered in `pending_wait_connected` / `pending_found` maps and drained on registration, preventing lost connections and infinite search loops.

## v0.7.10

### CA Client Search Engine Rewrite (libca++ level)
- **Adaptive deadline scheduler**: BTreeSet-based global scheduler replaces per-PV exponential backoff — lane-indexed retry with `period = (1 << lane) * RTT estimate`, max 5 min (configurable via `EPICS_CA_MAX_SEARCH_PERIOD`, floor 60s)
- **Per-path RTT estimation**: Jacobson/Karels algorithm (RFC 6298) per server address, 32ms floor — backoff adapts to actual network conditions instead of fixed 100ms→2s
- **Batch UDP search**: multiple SEARCH commands packed into single datagrams (≤1024 bytes), reducing packet count by ~30-50x for large PV sets
- **AIMD congestion control**: `frames_per_try` with additive increase (+1 on >50% response rate) / multiplicative decrease (reset to 1 on <10%) — prevents network flooding during mass PV search
- **Beacon anomaly detection**: dedicated `BeaconMonitor` task registers with CA repeater, tracks per-server beacon sequence/period, detects IOC restart (ID gap or period drop) and triggers selective rescan with 5s fast-rescan window
- **Connect-feedback penalty box**: servers that fail TCP create are deprioritized for 30s — prevents repeated connection attempts to unreachable servers
- **Selective rescan**: coordinator maintains server→channel reverse index, beacon anomaly rescans only affected channels (not global storm)
- **Immediate search on Schedule**: drain queued requests and send in same event loop iteration — fixes starvation where burst `create_channel` calls could delay first UDP search indefinitely

### CA Client Connection Improvements
- **Keep connect waiters on ChannelCreateFailed**: waiters stay pending so immediate re-search can still resolve before caller timeout (was: drain waiters on first failure)
- **AccessRightsChanged on channel create and reconnect**: fire event immediately after channel becomes connected
- **DBE_LOG in monitor mask**: match pyepics default (DBE_VALUE | DBE_LOG | DBE_ALARM)
- **Search recv buffer**: 256KB SO_RCVBUF for burst search response handling
- **Internal CA timeouts**: read/subscribe raised from 5s to 30s

### CA Client API
- **`CaChannel::info()`**: get channel metadata (native type, element count, host, access rights) without performing a CA read
- **`Snapshot` monitors**: `CaChannel::subscribe()` returns `Snapshot` with EPICS timestamp and alarm status

### IOC Shell
- **Output redirection**: `> file` and `>> file` support in iocsh without libc dependency

### Asyn
- **Synchronous write**: `can_block=false` ports use direct write instead of async channel, fixing write_op type coercion

## v0.7.9

### File Plugin Architecture (C ADCore NDPluginFile pattern)
- **`FilePluginController<W: NDFileWriter>`**: generic file plugin controller extracted to `ad-core-rs`, matching C ADCore's `NDPluginFile` base class — all file control logic (auto_save, capture, stream, temp_suffix rename, create_dir, param updates, error reporting) in one place
- All file plugins (TIFF, HDF5, JPEG, NetCDF) now delegate to `FilePluginController` via composition, eliminating ~300 lines of duplicated control logic
- **Auto-save**: write each incoming array as a single file when `AutoSave=Yes` (matches C `processCallbacks` autoSave)
- **Stream mode auto-stop**: close stream when `NumCaptured >= NumCapture` (NumCapture > 0), matching C `doCapture(0)` pattern
- **Capture mode**: full buffer → flush → close cycle with `NumCaptured` tracking
- **Temp suffix rename**: write to `path.tmp`, rename to `path` on close (all three modes)
- **Create dir**: `create_dir != 0` triggers `create_dir_all` (was `> 0` only, negative values like `-5` were ignored)
- **Write message cleared on success**: prevents stale error messages from persisting after successful writes
- **printf-style file template**: proper `%s%s_%3.3d.tif` expansion with sequential `%s` → filePath/fileName, `%d` with width/precision

### Waveform FTVL=CHAR Support
- asynOctetWrite device support for waveform records with `FTVL=CHAR`
- `write_only` flag: `read()` performs write (waveform is input record type in EPICS)
- Dynamic `field_list()` returns FTVL-appropriate VAL type (prevents CA write coercion errors)
- String → CharArray coercion in `put_field` for FTVL=CHAR
- NELM padding preserved on put (resize to NELM, prevents element count shrink)
- Trailing null trimming from CharArray before OctetWrite

### Plugin Infrastructure
- `register_params` implemented for all 12+ areaDetector plugins (was missing, causing silent `drv_user_create` failures)
- `on_param_change` with `Vec<ParamUpdate>` return for immediate param feedback (FILE_PATH_EXISTS, FULL_FILE_NAME, etc.)
- `ParamUpdate::Octet` variant for string param updates from data plane
- Fix NDArrayPort rewire: skip no-op rewire when `new_port == current_upstream` (eliminates startup race condition errors)

### Other
- `AdIoc::register_record_type()` for custom record type registration
- `put_notify` completion: `complete_async_record` fires `put_notify_tx.send(())` for CA WRITE_NOTIFY responses
- ophyd-test-ioc: all plugin ports reused for ADSIM prefix, motor record type registered

## v0.7.8

### Universal Asyn Device Support (C EPICS pattern)
- **`universal_asyn_factory`**: single factory handles all standard asyn DTYPs (`asynInt32`, `asynFloat64`, `asynOctet`, all array types) by parsing `@asyn(PORT,ADDR,TIMEOUT)DRVINFO` links and resolving params via `drv_user_create` → `find_param`, matching C EPICS asyn behavior exactly
- **All custom device support eliminated**: `MovingDotDeviceSupport`, `PointDetectorDeviceSupport`, `SimDeviceSupport`, `ScopeDeviceSupport`, `PluginDeviceSupport` — replaced by universal factory (~1,800 lines removed)
- **`ParamRegistry` infrastructure removed**: `ParamRegistry`, `ParamInfo`, `RegistryParamType`, all `build_param_registry` functions — `drv_user_create`/`find_param` replaces them
- **Plugin dynamic factory removed**: `PluginManager` no longer provides device support dispatch — only manages lifecycle, port registration, and NDArray wiring

### Template Migration
- All templates converted from `$(DTYP)` to standard asyn DTYPs with `@asyn(PORT,...)DRVINFO` links
- CP-linked records use 2-stage pattern (C ADCore `NDOverlayN` pattern): Soft Channel link receiver → asyn record via `OUT PP`
- `commonPlugins_settings.req` aligned with C ADCore (added StdArrays, Scatter/Gather, AttributeN, file-type-specific .req)

### Array Data (C EPICS pattern)
- Full array type support: `Int8`, `Int16`, `Int32`, `Int64`, `Float32`, `Float64` (read + write)
- `PluginPortDriver::read_*_array` overrides serve pixel data from NDArray (matching C `NDPluginStdArrays::readArray`)
- Array data pushed via direct interrupt (bypasses port actor channel), matching C `arrayInterruptCallback` pattern
- `param_value_to_epics_value` handles all array `ParamValue` variants

### Param Names (C ADCore alignment)
- All `create_param` names aligned with C ADCore `#define` strings: `ACQ_TIME`, `ACQ_PERIOD`, `NIMAGES`, `STATUS`, `ENABLE_CALLBACKS`, `ARRAY_NDIMENSIONS`, etc.
- Added missing `NDPluginDriver` params: `MAX_THREADS`, `NUM_THREADS`, `SORT_MODE`, `SORT_TIME`, `SORT_SIZE`, `SORT_FREE`, `DISORDERED_ARRAYS`, `DROPPED_OUTPUT_ARRAYS`, `PROCESS_PLUGIN`, `MIN_CALLBACK_TIME`, `MAX_BYTE_RATE`

### Other
- Per-parameter callback flush (`call_param_callback`) to avoid unintended side-flush
- `normalize_asyn_dtyp`: strips direction suffixes (`asynOctetRead` → `asynOctet`, `asynFloat64ArrayIn` → `asynFloat64Array`)
- Graceful `drv_user_create` failure: silently disables device support for records without matching driver param
- MovingDot: binning support (BinX/BinY), fix NDArray dims order
- Autosave for MovingDot cam1, `commonPlugins_settings.req` fixes
- `PvDatabase::get_pv_blocking` for sync access from std::threads
- `AdIoc::keep_alive` for driver runtime lifetime management
- `EpicsTimestamp::to_system_time` for interrupt timestamp consistency
- Fix array interrupt: handle I64/U64 types, use NDArray timestamp (not wall clock)
- Fix ADCORE path in AdIoc (`ad-core` → `ad-core-rs`)
- ophyd-test-ioc: switch from MovingDot to SimDetector (provides GainX/Y, Noise, etc.)
- ophyd-test-ioc: use AdIoc, add ADSIM: prefix for ophyd test compatibility
- All crate READMEs: fix license to EPICS Open License, add missing READMEs

## v0.7.7

_Superseded by v0.7.8 — v0.7.7 was an intermediate release._

## v0.7.6

### Runtime Facade
- **asyn-rs**: add `runtime::sync` (mpsc, oneshot, broadcast, Notify, Mutex, RwLock), `runtime::task` (spawn, sleep, interval, RuntimeHandle), and `runtime::select!` re-exports — driver authors no longer need to depend on tokio directly
- **epics-base-rs**: add matching re-exports in `runtime::sync` and `runtime::task`, plus `select!` macro re-export and hidden `__tokio` re-export for macro hygiene

### Proc Macros
- **`#[epics_main]`**: attribute macro replacing `#[tokio::main]` — validates `async fn main()`, no args, no generics, no attribute arguments; builds multi-thread runtime via `epics_base_rs::__tokio`
- **`#[epics_test]`**: attribute macro replacing `#[tokio::test]` — validates async fn with no args/generics, rejects duplicate `#[test]`; builds current-thread runtime (matching `#[tokio::test]` default)

### Examples Modernized
- All examples (`mini-beamline`, `scope-ioc`, `sim-detector`, `ophyd-test-ioc`, `random-signals`) now use the runtime facade instead of tokio directly
- `scope-ioc`: `epics-base-rs` promoted from optional to required dependency
- Zero `tokio::` references remain in example code (except `#[tokio::main]` → `#[epics_main]`)

### Docs
- Quick Start: add binary location (`target/release/`) and PATH setup
- Quick Start: fix build command to use `--release`
- Update copyright name in LICENSE

## v0.7.5

### areaDetector PV Convention
- Adopt standard areaDetector PV convention (`P=mini:dot:`, `R=cam1:`) in mini-beamline
- Add NDStdArrays `image1` plugin to `commonPlugins.cmd`
- Include `ADBase.template` for full ADBase PV set (TriggerMode, Gain, etc.)
- Add missing param registry entries for NDArrayBase PVs
- Fix param name mismatches with C ADCore templates

### CA Server
- Non-blocking WRITE_NOTIFY: spawn background task for completion instead of blocking `dispatch_message`, matching C EPICS rsrv behavior
- Remove arbitrary 30s timeout — wait indefinitely for record completion

### MovingDot Driver
- Non-blocking port writes in device support and acquisition task to prevent tokio thread starvation
- Remove `call_param_callbacks` from driver write methods to prevent re-entrant message storms
- Add slit aperture simulation (SlitLeft/Right/Top/Bottom in pixels)
- Output UInt16 image data (realistic photon counts)
- Tolerate read failures during config refresh instead of aborting acquisition

### Waveform Record
- Add SHORT/USHORT and FLOAT FTVL support (was falling through to DOUBLE)
- Fix `DbFieldType`-to-`menuFtype` mapping in `new()`
- `PluginDeviceSupport`: native `EpicsValue` types for NDArray data

### AsynDeviceSupport
- Add public accessors (`reason`, `addr`, `handle`, `write_op_pub`)

### Docs
- Quick Start: add binary location (`target/release/`) and PATH setup
- Quick Start: fix build command to use `--release`
- Update copyright name in LICENSE

## v0.7.4

### New Crate
- **optics-rs**: Port of EPICS optics synApps module — table record (6-DOF, 4 geometry modes), Kohzu/HR/ML-mono DCM controllers, 4-circle orientation matrix, XIA PF4 dual filter, auto filter drive, HSC-1 slit, quad BPM, ion chamber, Chantler X-ray absorption data (22 elements), 36 database templates, PyDM UI screens, 362 tests including 46 golden tests vs C tableRecord.c

### dbAccess: C EPICS Parity
- **Three-tier DB write API** matching C EPICS semantics:
  - `put_pv` / `put_f64` = C `dbPut` — value + special, no monitor, no process
  - `put_pv_and_post` / `put_f64_post` = C `dbPut` + `db_post_events` — value + monitor on change
  - `put_record_field_from_ca` / `put_f64_process` = C `dbPutField` — value + process + monitor
- **Event source tagging** — origin ID prevents sequencer self-feedback loops; `DbChannel::with_origin()`, `DbMultiMonitor::new_filtered()`, origin-aware `DbSubscription`
- **DbChannel API**: add `put_i16_process`, `put_i32_process`, `put_string_process`, `get_i32`
- **TPRO** trace processing output when field is set
- **Pre-write special** hook in CA put path (`special(field, false)` before write)
- **Read-only field** enforcement in `put_record_field_from_ca`
- **ACKS/ACKT** alarm acknowledge with severity comparison
- **Menu string resolution** in type conversion (String → Enum/Short)
- **dbValueSize / dbBufferSize** equivalents
- **is_soft_dtyp**: recognize "Raw Soft Channel", "Async Soft Channel", "Soft Timestamp", "Sec Past Epoch"
- **stringout**: add OMSL/DOL fields and framework DOL processing support

### SNL Programs: CA → DbChannel Migration
- All 7 optics-rs SNL programs converted from CA client to direct database access:
  kohzu_ctl, hr_ctl, ml_mono_ctl, kohzu_ctl_soft, orient, pf4, filter_drive
- Origin tagging + filtered monitors prevent write-back loops
- Kohzu DCM: non-blocking move with `tokio::select!` retarget support

### Bug Fixes
- **I/O Intr read timeout**: cache interrupt value in adapter, skip blocking read on cache miss
- **ao DOL/OIF conflict**: remove duplicate DOL handling from ao process() (framework handles it)
- **put_pv_and_post timestamp**: update `common.time` before posting monitor events
- **Redundant monitors**: suppress duplicate events when value unchanged

### Breaking Changes
- Remove `epics-seq-rs`, `snc-core-rs`, `snc-rs` (replaced by native Rust async state machines in optics-rs and std-rs)

## v0.7.3

### New Crates
- **std-rs**: Port of EPICS std module — epid (PID/MaxMin feedback), throttle (rate-limited output), timestamp (formatted time strings) records, plus device support (Soft/Async/Fast Epid, Time of Day, Sec Past Epoch) and SNL programs (femto gain control, delayDo state machine)
- **scaler-rs**: Port of EPICS scaler module — 64-channel 32-bit counter record with preset-based counting, OneShot/AutoCount modes, DLY/DLY1 delayed start, RATE periodic display update, asyn device support, and software scaler driver

### Framework: ProcessOutcome / ProcessAction
- **Breaking**: `Record::process()` now returns `CaResult<ProcessOutcome>` instead of `CaResult<RecordProcessResult>`
- `ProcessOutcome` contains `result` (Complete/AsyncPending) + `actions` (side-effect requests)
- `ProcessAction::WriteDbLink` — record requests a DB link write without direct DB access
- `ProcessAction::ReadDbLink` — record requests a DB link read (pre-process execution)
- `ProcessAction::ReprocessAfter(Duration)` — delayed self re-process (replaces C `callbackRequestDelayed` + `scanOnce`)
- `ProcessAction::DeviceCommand` — record sends named commands to device support via `handle_command()`
- Processing layer executes actions at the correct point in the cycle (ReadDbLink before process, WriteDbLink/DeviceCommand after, ReprocessAfter via tokio::spawn)

### Framework: DeviceReadOutcome
- **Breaking**: `DeviceSupport::read()` now returns `CaResult<DeviceReadOutcome>` instead of `CaResult<()>`
- `DeviceReadOutcome` carries `did_compute` flag and `actions` list
- `did_compute`: signals that device support already performed the record's compute step (e.g., PID), passed to record via `set_device_did_compute()` before `process()`
- Device support actions are merged into the record's ProcessOutcome by the framework

### Framework: Other Improvements
- `Record::pre_process_actions()` — return ReadDbLink actions executed BEFORE process() (matches C `dbGetLink` immediate semantics)
- `Record::put_field_internal()` — bypasses read-only checks for framework-internal writes
- `Record::set_device_did_compute()` — framework signals device support compute status
- `DeviceSupport::handle_command()` — handle named commands from ProcessAction::DeviceCommand
- `field_io.rs`: `put_pv()` and `put_record_field_from_ca()` now call `on_put()` + `special()` for record-owned fields (was previously only for common fields)
- ReprocessAfter timer cancellation via generation counter in RecordInstance (prevents stale timer accumulation)

### Workspace Integration
- Add `std-rs` and `scaler-rs` to workspace members and default-members
- Add `std` and `scaler` feature flags to epics-rs umbrella crate
- Bundle 70+ database templates (.db) and autosave request files (.req)

### Testing
- Add 390+ new tests across all crates:
  - std-rs: 94 tests (epid PID algorithm, throttle rate limiting, timestamp formats, SNL state machines, framework integration, e2e autosave)
  - scaler-rs: 40 tests (64-channel field access, state machine, TP↔PR1 conversion, soft driver, DLY delayed start, COUT/COUTP link firing)
  - asyn-rs: 20 integration tests (port driver parameters, octet echo, error handling, interrupt callbacks, enum, blocking API)
  - ad-core-rs: 47 tests (NDArray types/dimensions, pool allocation/reuse/memory limits, attributes, concurrent access)
  - epics-macros-rs: 27 tests (derive macro field generation, type mapping, read-only, snake_case conversion)
  - epics-ca-rs: 30 tests (protocol header encoding, server builder, get/put API, field access, multiple record types)
  - epics-pva-rs: 49 tests (scalar types, PvStructure, serialization roundtrip, protocol header, codec)
  - epics-seq-rs: 30 tests (event flags, channel store, program builder, variable traits)
  - snc-core-rs: 42 tests (lexer tokenization, parser AST, codegen output, end-to-end pipeline)
  - snc-rs: 11 tests (CLI help, compilation, error handling, debug flags)

## v0.7.2

- Fix asyn-rs epics feature compilation (get_port export, AsynRecord import)
- Migrate record factory registration from global registry to IocApplication injection
- Replace global port registry with shared PortRegistry instance
- Add feature matrix to CI (asyn-rs/epics, ad-core-rs/ioc, ad-plugins-rs/ioc)
- Add IocApplication::register_record_type() method
- Add motor_record_factory() and asyn_record_factory() returning injectable tuples

## v0.7.1

### Architecture
- Extract `IocBuilder` from `CaServerBuilder` into epics-base-rs (protocol-agnostic IOC bootstrap)
- Move `IocApplication` to epics-base-rs with pluggable protocol runner closure
- Split `database.rs` into modules: field_io, processing, links, scan_index
- Split `record.rs` into modules: alarm, scan, link, common_fields, record_trait, record_instance
- Split `types.rs` into modules: value, dbr, codec
- Split `db_loader.rs` into parser + include expander modules
- Split `asyn_record.rs` registry into separate module
- Extract motor field dispatch to `field_access.rs`
- Remove thin wrapper crates (autosave-rs, busy-rs, epics-calc-rs) — now re-exported from epics-base-rs
- Remove legacy autosave API, migrate to SaveSetConfig/AutosaveManager
- Remove unused calc feature flags
- Crate directory names now match crate names (crates/motor → crates/motor-rs, etc.)

### API
- Reduce public API surface: 7 internal modules → pub(crate) (recgbl, scan_event, exception, interpose, protocol, transport, channel)
- Motor lib.rs: fields, coordinate → pub(crate); remove pub use fields::*, flags::*
- Add `create_record_with_factories()` for dependency injection (avoids global registry)
- `IocApplication::run()` now accepts a protocol runner: `.run(run_ca_ioc).await`

### Testing
- Move large inline test blocks to tests/ directory (3,337 lines)
- Add autosave integration test with mini-beamline (save + restore on restart)

### Fixes
- Fix ad-core path references after directory rename
- Fix remaining old crate directory references in README and examples
- Clean all clippy warnings

## v0.7.0

- **Breaking**: Separate Channel Access into `epics-ca-rs` crate
- **Breaking**: Separate pvAccess into `epics-pva-rs` crate
- **Breaking**: Rename crates for consistent `-rs` suffix (ad-core-rs, ad-plugins-rs, epics-macros-rs, epics-seq-rs, snc-core-rs, snc-rs)
- Add `epics-rs` umbrella crate with feature flags (ca, pva, motor, ad, calc, full, etc.)
- Remove msi from workspace (moved to separate repo)
- Add 113 C EPICS parity tests (ai/bi/bo record, deadband, alarm, calc engine, FLNK chains, CA wire protocol, .db parsing, autosave)
- Add SAFETY comments for production unwrap sites
- Clippy lint cleanup across all crates

## v0.6.1

- Fix monitor deadband for records without MDEL field
- Reset beacon interval on TCP connect/disconnect (C EPICS parity)
- Fix caput-rs to use fire-and-forget write like C caput, add `-c` flag for callback mode
- Show Old/New values in caput-rs output
- Support multiple PV names in CA/PVA CLI tools (caget, camonitor, cainfo, pvget, etc.)
- Add per-field change detection for monitor notifications
- Add DMOV same-position transition tests
- Poll motor immediately on StartPolling for faster DMOV response
- Add motor tests ported from ophyd (sequential moves, calibration, RBV updates, homing)
- Update minimum Rust version to 1.85+ for edition 2024

## v0.6.0

- Deferred write_notify via callback for motor records
- Motor display/ctrl metadata support
- SET mode RBV updates

## v0.5.2

- Fix monitor notify, DMOV transition, timestamp, and IPv4 resolution

## v0.5.1

- Add DMOV 1->0->1 monitor transition for motor moves

## v0.5.0

- Fix motor record process chain, client error handling, and connection speed
- Add ophyd-test-ioc example

## v0.4.6

- Add client-side DBR_TIME/CTRL decode and get_with_metadata() API

## v0.4.5

- Upgrade Rust edition 2021 -> 2024

## v0.4.4

- Bug fixes

## v0.4.3

- Add generalTime framework for priority-based time providers
- Add random-signals example
- Add GitHub Actions CI workflow

## v0.4.2

- Implement C-compatible autosave iocsh commands and request file infrastructure

## v0.4.1

- Implement full YUV color mode support and refactor color convert plugin

## v0.4.0

- Initial crates.io publish
- Move to epics-rs GitHub organization

## v0.3.0

- Unify workspace version management
