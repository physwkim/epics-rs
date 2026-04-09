# epics-bridge-rs

Pure Rust QSRV equivalent — bridges EPICS database records to pvAccess channels (NTScalar, NTEnum, NTScalarArray, Group PV).

No C dependencies. Just `cargo build`.

**Repository:** <https://github.com/epics-rs/epics-rs>

## Overview

epics-bridge-rs corresponds to C++ EPICS QSRV (`modules/pva2pva/pdbApp/`). It translates between `epics-base-rs` record state and `epics-pva-rs` PVA data structures, allowing pvAccess clients to read, write, and monitor EPICS database records.

```
PVA Client <--> [epics-pva-rs server] <--> BridgeProvider <--> PvDatabase
```

**Status: Experimental** — the PVA server side (socket, protocol handling) will be implemented in `epics-pva-rs` by the spvirit maintainer. This crate provides the application-layer bridge that the server calls into.

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
