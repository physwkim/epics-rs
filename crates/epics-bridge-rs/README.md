# epics-bridge-rs

EPICS protocol bridge/adapter hub for [epics-rs](https://github.com/epics-rs/epics-rs).

Hosts multiple bridge implementations as feature-gated sub-modules:

- **`qsrv`** (default) — Record ↔ pvAccess channels (C++ QSRV equivalent)
- **`ca_gateway`** (default) — CA fan-out gateway (C++ ca-gateway equivalent)
- **`pvalink`** (planned) — PVA links for record INP/OUT
- **`pva_gateway`** (planned) — PVA-to-PVA proxy

No C dependencies. Just `cargo build`.

**Repository:** <https://github.com/epics-rs/epics-rs>

## qsrv — Record ↔ PVA bridge

Corresponds to C++ EPICS QSRV (`modules/pva2pva/pdbApp/`). Translates between `epics-base-rs` record state and `epics-pva-rs` PVA data structures, allowing pvAccess clients to read, write, and monitor EPICS database records.

```
PVA Client <--> [epics-pva-rs server] <--> BridgeProvider <--> PvDatabase
```

**Status: Experimental** — the PVA server side (socket, protocol handling) will be implemented in `epics-pva-rs` by the spvirit maintainer. This crate provides the application-layer bridge that the server calls into.

## ca_gateway — CA fan-out gateway

Pure Rust port of [EPICS ca-gateway](https://github.com/epics-modules/ca-gateway). A Channel Access proxy that:

- Accepts downstream client connections (CA server side, via `epics-ca-rs`)
- Connects to upstream IOCs (CA client side, via `epics-ca-rs`)
- Caches PV values and fans out monitor events to multiple clients
- Applies access security rules from `.pvlist` (regex-based, with alias backreferences)
- Tracks per-PV statistics and exposes them as PVs (`gateway:totalPvs`, etc.)
- Supports auto-restart supervisor (NRESTARTS pattern)
- Logs put events to a configurable putlog file

```
Upstream IOCs                Gateway                 Downstream Clients
┌─────────┐                ┌─────────┐               ┌─────────┐
│ IOC #1  │ ◄── CaClient ──┤         ├── CaServer ──►│ caget   │
└─────────┘                │ PvCache │               └─────────┘
┌─────────┐                │  + ACL  │               ┌─────────┐
│ IOC #2  │ ◄── CaClient ──┤  + Stats├── CaServer ──►│  CSS    │
└─────────┘                │         │               └─────────┘
                           └─────────┘                  (~1000)
```

### Modules

- `cache` — `PvCache`, `GwPvEntry`, `PvState` (5-state FSM: Dead/Connecting/Inactive/Active/Disconnect), timeout-based cleanup
- `pvlist` — `.pvlist` parser (ALLOW/DENY/ALIAS, regex backreferences, EVALUATION ORDER)
- `access` — `.access` ACF parser adapter (via `epics-base-rs`)
- `upstream` — CaClient adapter, manages per-PV monitor tasks
- `downstream` — CaServer adapter, hosts shadow `PvDatabase`
- `stats` — gateway runtime statistics + PV publication
- `beacon` — beacon anomaly throttle (5-min reconnect inhibit)
- `putlog` — put-event audit log
- `command` — runtime command interface (R1/R2/R3/AS/PVL/V)
- `master` — auto-restart supervisor (NRESTARTS=10, RESTART_INTERVAL=10min)
- `server` — `GatewayServer` top-level + main event loop

### Binary

```bash
cargo build --release -p epics-bridge-rs --bin ca-gateway-rs
./target/release/ca-gateway-rs \
    --pvlist  example/gateway.pvlist \
    --access  example/gateway.access \
    --preload example/preload.txt \
    --putlog  /var/log/ca-gateway.log
```

CLI options:

| Option | Description |
|--------|-------------|
| `--pvlist <FILE>` | Path to `.pvlist` access list |
| `--access <FILE>` | Path to `.access` ACF file |
| `--preload <FILE>` | Pre-subscribe upstream PVs (one per line) |
| `--putlog <FILE>` | Put-event audit log file |
| `--port <N>` | CA server TCP port (0 = default 5064) |
| `--read-only` | Reject all client puts |
| `--no-stats` | Disable `gateway:*` stats PVs |
| `--stats-prefix <S>` | Custom stats PV prefix (default `"gateway:"`) |
| `--heartbeat-interval <N>` | Heartbeat counter period (s; 0 = disable) |
| `--cleanup-interval <N>` | Cache eviction sweep period (s) |
| `--stats-interval <N>` | Stats refresh period (s) |
| `--supervised` | Run under NRESTARTS auto-restart supervisor |
| `--max-restarts <N>` | Max restarts in window (default 10) |
| `--restart-window <N>` | Restart window in seconds (default 600) |
| `--restart-delay <N>` | Delay between restarts in seconds (default 10) |

### Status

Working skeleton with:
- ✅ Full `.pvlist` parser (ALLOW/DENY/ALIAS, regex backreferences)
- ✅ ACF integration via `epics-base-rs`
- ✅ 5-state FSM PV cache with timeout-based cleanup
- ✅ Upstream client (subscribe/get/put) wired to `epics-ca-rs`
- ✅ Downstream server hosting a shadow `PvDatabase`
- ✅ Lazy on-demand search resolution via `PvDatabase::set_search_resolver` hook (no preload required)
- ✅ Per-host connection tracking via `CaServer::connection_events` broadcast
- ✅ SIGUSR1 signal handler for runtime command file processing (Unix)
- ✅ Statistics PVs published by the gateway itself
- ✅ Heartbeat, cleanup, stats refresh timers
- ✅ Beacon anomaly throttle
- ✅ Put-event logger
- ✅ Runtime command interface (R1/R2/R3/AS/PVL/V)
- ✅ Auto-restart supervisor

The `--preload` file is still supported as an optional warm-cache mechanism but is no longer required: any name that matches an `ALLOW`/`ALIAS` rule in `.pvlist` is resolved on first downstream search.

## Features

### Single Record Channels
- **NTScalar** — ai, ao, longin, longout, stringin, stringout, calc, calcout
- **NTEnum** — bi, bo, mbbi, mbbo (with enum choices)
- **NTScalarArray** — waveform, compress, histogram
- Full metadata: alarm, timeStamp (with userTag), display (units, precision, form, description), control limits, valueAlarm thresholds
- pvRequest field selection (`field.value`, `field.alarm`, etc.)
- Process control via `record._options.process` (true/false/passive) and `record._options.block`

### Group PV Channels
- Composite PvStructure from multiple records (C++ QSRV JSON format compatible)
- 5 field mapping types: Scalar, Plain, Meta, Any, Proc
- Trigger rules: `"*"` (all), `"field1,field2"` (selective), `""` (none)
- Atomic put mode (sequential write without yielding)
- Per-member `+id`, `+putorder`, `+type` configuration
- info(Q:group, ...) parsing from record definitions + external JSON file merging
- Nested field paths (`a.b.c` dot notation)

### Monitor Bridge
- Full Snapshot on every update (alarm, display, control, enums — not just value)
- Initial complete snapshot on start (C++ BaseMonitor::connect pattern)
- Group monitor: fan-in channel across all members with trigger rule evaluation
- Partial updates for `TriggerDef::Fields` (only re-read triggered members)
- Overflow counter for tracking lost events

### Infrastructure
- **ChannelProvider** trait — channel search, list, create
- **Channel** trait — get, put, getField, createMonitor
- **PvaMonitor** trait — start, poll, stop
- Record metadata cache (avoids repeated introspection)
- Pluggable AccessControl trait (default: AllowAllAccess)
- Enum index <-> string bidirectional conversion
- DBF type-aware value conversion (`scalar_to_epics_typed`)

## Architecture

```
epics-bridge-rs/src/
  lib.rs            # Module re-exports + public API
  error.rs          # BridgeError, BridgeResult
  convert.rs        # EpicsValue <-> ScalarValue conversion (Enum=UShort, DBF-aware)
  pvif.rs           # Snapshot -> NTScalar/NTEnum/NTScalarArray + FieldDesc + pvRequest filter
  provider.rs       # ChannelProvider/Channel/PvaMonitor traits + BridgeProvider + AnyChannel
  channel.rs        # BridgeChannel (single record) + PutOptions (process/block)
  monitor.rs        # BridgeMonitor (DbSubscription -> PVA monitor)
  group.rs          # GroupChannel + GroupMonitor + AnyMonitor + nested field paths
  group_config.rs   # Group JSON parser (C++ QSRV format) + info(Q:group) + merge
```

### Type Mapping

| Record Type | NormativeType | Value Type |
|-------------|---------------|------------|
| ai, ao, calc, calcout | NTScalar | Double |
| longin, longout | NTScalar | Int |
| stringin, stringout | NTScalar | String |
| bi, bo, mbbi, mbbo | NTEnum | UShort (index) + choices[] |
| waveform, compress, histogram | NTScalarArray | element type from record |

### C++ QSRV Correspondence

| C++ QSRV | Rust epics-bridge-rs |
|-----------|---------------------|
| PDBProvider | BridgeProvider |
| PDBSinglePV / PDBSingleChannel | BridgeChannel |
| PDBGroupPV / PDBGroupChannel | GroupChannel |
| PDBSingleMonitor / BaseMonitor | BridgeMonitor |
| PDBGroupMonitor | GroupMonitor |
| PVIF / PVIFBuilder / ScalarBuilder | pvif module (snapshot_to_nt_*, filter_by_request) |
| configparse.cpp | group_config module |
| dbf_copy.cpp | convert module |

## Usage

```toml
[dependencies]
epics-rs = { version = "0.8", features = ["bridge"] }
```

Or directly:

```toml
[dependencies]
epics-bridge-rs = "0.8"
```

### Example

```rust
use epics_bridge_rs::{BridgeProvider, ChannelProvider, Channel};

// Create bridge from an existing PvDatabase
let mut bridge = BridgeProvider::new(db.clone());

// Load group PV definitions
bridge.load_group_file("groups.json")?;

// Or from record info tags
bridge.load_info_group("TEMP:sensor", r#"{
    "TEMP:group": {
        "temperature": {"+channel": "VAL", "+type": "plain", "+trigger": "*"}
    }
}"#)?;

// Search and create channels
if bridge.channel_find("TEMP:ai").await {
    let ch = bridge.create_channel("TEMP:ai").await?;
    let value = ch.get(&request).await?;
}
```

## Testing

```bash
cargo test -p epics-bridge-rs    # 39 tests
```

Tests cover: type conversion roundtrips, NormativeType structure building, pvRequest field filtering, group JSON parsing, info(Q:group) parsing with prefix, group merging, PutOptions parsing (process/block), nested field path operations, NTEnum UShort index.

## Dependencies

- epics-base-rs — Record trait, PvDatabase, Snapshot, DbSubscription
- epics-pva-rs — PvStructure, ScalarValue, FieldDesc
- tokio — async runtime (fan-in channels, spawned tasks)
- serde / serde_json — group config JSON parsing
- thiserror — error types

## Requirements

- Rust 1.85+ (edition 2024)

## License

[EPICS Open License](../../LICENSE)
