# epics-pva-rs

Pure Rust implementation of the [pvAccess](https://docs.epics-controls.org/projects/pvaccess/en/latest/) protocol — modern EPICS structured data transport.

No C dependencies. Just `cargo build`.

**Repository:** <https://github.com/epics-rs/epics-rs>

**Status: Experimental** — client side is functional (search, get, put, monitor, info). The server side and full pvData type system are under development by the spvirit maintainer. See `epics-bridge-rs` for the application-layer Record↔PVA bridge that the server will use.

## Overview

pvAccess is the next-generation EPICS protocol that supersedes Channel Access for structured data. Where CA carries primitive scalars and 1D arrays, PVA carries arbitrary nested structures (NormativeTypes like NTScalar, NTEnum, NTTable, NTNDArray, NTMatrix, ...) — making it the natural choice for areaDetector images, MASAR snapshots, structured machine state, and any data richer than a single scalar.

```
PVA Client (pvaget-rs, OPI, Python)
       │
       │  UDP search (port 5076)
       │  TCP virtual circuit (port 5075)
       │  pvData wire format (FieldDesc + values, BitSet deltas)
       │
       ▼
PVA Server (epics-pva-rs server, planned)
       │
       ▼
ChannelProvider (epics-bridge-rs BridgeProvider)
       │
       ▼
PvDatabase (epics-base-rs records)
```

## Features

### Wire Protocol (`protocol.rs`)
- **PVA header** — magic byte `0xCA`, version 2, control/application messages, segmentation flags
- **Endianness negotiation** — `SET_BYTE_ORDER` handshake
- **Connection validation** — auth + buffer size negotiation
- **All commands**: `CMD_BEACON` (0), `CMD_CONNECTION_VALIDATION` (1), `CMD_ECHO` (2), `CMD_SEARCH` (3), `CMD_SEARCH_RESPONSE` (4), `CMD_AUTHNZ` (5), `CMD_ACL_CHANGE` (6), `CMD_CREATE_CHANNEL` (7), `CMD_DESTROY_CHANNEL` (8), `CMD_CONNECTION_VALIDATED` (9), `CMD_GET` (10), `CMD_PUT` (11), `CMD_PUT_GET` (12), `CMD_MONITOR` (13), `CMD_ARRAY` (14), `CMD_DESTROY_REQUEST` (15), `CMD_PROCESS` (16), `CMD_GET_FIELD` (17), `CMD_RPC` (20)
- **QoS subcommand flags** — `QOS_INIT` (0x08), `QOS_DESTROY` (0x10), `QOS_PROCESS` (0x04), `QOS_GET` (0x40)
- **Constants** — `PVA_SERVER_PORT=5075`, `PVA_BROADCAST_PORT=5076`, `PVA_VERSION=2`

### pvData (`pvdata.rs`)
- **ScalarType** enum — Boolean, Byte/UByte, Short/UShort, Int/UInt, Long/ULong, Float, Double, String, with type code lookup table from C++ FieldCreateFactory
- **ScalarValue** — runtime value of any scalar type, with `parse(scalar_type, &str)` and `Display` impl
- **PvField** — recursive runtime field: `Scalar(ScalarValue) | ScalarArray(Vec<ScalarValue>) | Structure(PvStructure)`
- **PvStructure** — composite with `struct_id` (e.g., `"epics:nt/NTScalar:1.0"`) and ordered named fields, with `get_field`, `get_value`, `get_alarm`, `get_timestamp` helpers
- **FieldDesc** — type description (no values) for `getField` introspection: `Scalar | ScalarArray | Structure`

### Serialization (`serialize.rs`)
- **Variable-length size encoding** — `read_size`/`write_size`
- **String I/O** — UTF-8 with length prefix
- **Numeric primitives** — u8/u16/u32/u64/i32/i64/f32/f64 with endianness parameter
- All primitives match C++ pvDataCPP byte-for-byte

### Codec (`codec.rs`)
- **PvaCodec** — message builder with `big_endian` flag
- `build_search`, `build_create_channel`, `build_get_init`, `build_get`, `build_put_init`, `build_put`, `build_monitor_init`, `build_monitor_start`, `build_destroy_request`

### Client (`client.rs`)
- **PvaClient** — async client backed by `EPICS_PVA_ADDR_LIST` env var
- UDP search broadcast → server response
- TCP connection with `SET_BYTE_ORDER` + `CONNECTION_VALIDATION` handshake
- Get / Put / Monitor / GetField operations

### CLI Tools
- **pvaget-rs** — read PVA channel value (single shot)
- **pvaput-rs** — write PVA channel value
- **pvamonitor-rs** — subscribe to PVA channel updates
- **pvainfo-rs** — display PVA channel structure metadata

## Architecture

```
epics-pva-rs/src/
├── lib.rs
├── error.rs            # PvaError, PvaResult
├── protocol.rs         # PvaHeader, command codes, QoS flags, constants
├── pvdata.rs           # ScalarType, ScalarValue, PvField, PvStructure, FieldDesc
├── serialize.rs        # primitive read/write with endianness
├── codec.rs            # PvaCodec message builders
├── client.rs           # PvaClient
└── bin/                # pvaget-rs, pvaput-rs, pvamonitor-rs, pvainfo-rs
```

## Quick Start

```bash
# Read a PVA channel (a CA channel served via the future PVA bridge will work the same way)
pvaget-rs MY:PV

# Subscribe
pvamonitor-rs MY:PV

# Get field type info
pvainfo-rs MY:PV

# Put
pvaput-rs MY:PV 42.5
```

### Library

```rust
use epics_pva_rs::client::PvaClient;
use epics_pva_rs::pvdata::ScalarValue;

let client = PvaClient::new()?;
let pv = client.get("MY:PV").await?;
if let Some(val) = pv.get_value() {
    println!("{val}");
}
```

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `EPICS_PVA_ADDR_LIST` | (empty) | Comma-separated list of PVA server addresses |
| `EPICS_PVA_AUTO_ADDR_LIST` | `YES` | Append broadcast addresses |
| `EPICS_PVA_SERVER_PORT` | `5075` | TCP server port |
| `EPICS_PVA_BROADCAST_PORT` | `5076` | UDP search/beacon port |

## Server-Side Status

The server-side implementation (TCP listener, UDP beacon, ChannelProvider integration, segmentation, FieldDesc/PvStructure full serialization) is being developed by the [spvirit](https://github.com/spvirit) maintainer. Once available, IOCs will be able to run CA and PVA simultaneously:

```rust
app.run(|config| async move {
    let bridge = epics_bridge_rs::BridgeProvider::new(config.db.clone());
    let ca = CaServer::from_parts(config.db.clone(), config.port, ...);
    let pva = PvaServer::new().with_provider(bridge);
    epics_base_rs::runtime::select! {
        _ = ca.run() => {},
        _ = pva.run() => {},
    }
}).await
```

## Testing

```bash
cargo test -p epics-pva-rs
```

Test coverage: scalar type code lookup, value parse/display roundtrip, FieldDesc introspection, codec message construction.

## Dependencies

- tokio — async runtime
- chrono — timestamps
- clap — CLI argument parsing
- thiserror — error types

## Requirements

- Rust 1.85+ (edition 2024)

## License

[EPICS Open License](../../LICENSE)
