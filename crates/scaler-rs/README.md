# scaler-rs

Pure Rust port of the EPICS [scaler](https://github.com/epics-modules/scaler) module — multi-channel counter/timer record with preset and auto-count support.

No C dependencies. Just `cargo build`.

**Repository:** <https://github.com/epics-rs/epics-rs>

## Overview

The scaler record represents a multi-channel counter (typically a VME or PCI scaler card) with up to 64 input channels. Each channel counts pulses, with one channel often dedicated to a time-base that gates the others. The record supports two modes:

- **OneShot** — count for a configurable time then stop
- **AutoCount** — periodically count with display refresh, often used while idle

scaler-rs is a faithful Rust port of the C++ scalerRecord, including the full state machine, preset/gate/direction/name per-channel configuration, periodic display update during counting, and asyn-based device support.

## Features

### scaler Record
- **64-channel 32-bit counters** — `S1`–`S64` (counts), `PR1`–`PR64` (presets), `G1`–`G64` (gates), `D1`–`D64` (directions), `NM1`–`NM64` (names)
- **Time base** — channel 1 typically scales TP (time preset) for fixed-time counting
- **Two count modes** — OneShot (CNT=1 → count then stop) and AutoCount (continuous with rate-limited display)
- **Delayed start** — DLY (OneShot delay) and DLY1 (AutoCount delay) for synchronizing with external events
- **Display rate** — RATE/RAT1 control how often counts are read while counting
- **Output links** — COUT (count direction toggle) and COUTP (count start/stop transition) fired on state changes
- **Per-channel naming** — NMx fields populate display labels in OPI screens
- **Status fields** — CNT (count enable), CONT (continuous counting), TCNT (counts when finished), VAL (elapsed time)

### Device Support
- **Asyn device support** (`scaler_asyn.rs`) — bridges scaler record to a `ScalerDriver` trait with `reset`, `read`, `write_preset`, `arm`, `done` operations
- **Software scaler** (`scaler_soft.rs`) — pure Rust simulation driver for testing (configurable count rates per channel)
- **DeviceCommand actions** — record expresses commands as data (Reset, Arm, WritePreset) which the framework dispatches to the driver

### Database Templates (bundled)
- `scaler.db` — base scaler record (64-channel)
- `scaler16.db` / `scaler32.db` / `scaler16m.db` — sized variants
- `scalerSoftCtrl.db` — software-only test scaler
- `scaler*_settings.req` — autosave request files for all sized variants

### PyDM Screens (bundled in `ui/`)
- 16-channel: full, more, calc variants
- 32-channel: full, more, calc variants
- 64-channel: split into two halves with main + 33–64 sub-screens

## Architecture

```
scaler-rs/
├── src/
│   ├── lib.rs                  # public API + factory
│   ├── records/
│   │   ├── mod.rs              # re-exports
│   │   └── scaler.rs           # ScalerRecord (Record trait + state machine)
│   └── device_support/
│       ├── mod.rs              # ScalerDriver trait
│       ├── scaler_asyn.rs      # asyn-based device support
│       └── scaler_soft.rs      # software simulation driver
├── db/                         # database templates + autosave .req files
└── ui/                         # PyDM .ui screens (16/32/64 channel)
```

## Usage

```toml
[dependencies]
epics-rs = { version = "0.8", features = ["scaler"] }
```

### Register the Record Type

```rust
use epics_base_rs::server::ioc_app::IocApplication;
use epics_ca_rs::server::run_ca_ioc;
use scaler_rs::scaler_record_factory;

#[epics_base_rs::epics_main]
async fn main() -> epics_base_rs::error::CaResult<()> {
    let (name, factory) = scaler_record_factory();

    IocApplication::new()
        .register_record_type(name, factory)
        .db_file("db/scaler16.db", &macros)?
        .run(run_ca_ioc)
        .await
}
```

### Software Driver (Simulation)

```rust
use scaler_rs::device_support::scaler_soft::SoftScalerDriver;

let driver = SoftScalerDriver::new(16, vec![
    1_000_000.0,  // channel 1: 1 MHz time base
    100_000.0,    // channel 2: 100 kHz signal
    50_000.0,     // channel 3: 50 kHz signal
    // ... up to 16 channels
]);
```

### Operating the Scaler (CA)

```bash
# Set up
caput SCALER:TP 1.0       # 1 second count time
caput SCALER:PR1 1000000  # channel 1 preset (time base)
caput SCALER:CNT 1        # start counting

# Watch results
camonitor SCALER:S1 SCALER:S2 SCALER:S3
```

## Testing

```bash
cargo test -p scaler-rs
```

Test coverage: state machine transitions (OneShot → arm → counting → done), AutoCount periodic refresh, preset writing, gate/direction handling, COUT/COUTP link firing, soft driver count generation, asyn device support bridge.

## Dependencies

- epics-base-rs — Record trait, DeviceSupport, ProcessAction
- asyn-rs — port driver framework
- epics-macros-rs — `#[derive(EpicsRecord)]`
- chrono — timestamps

## Requirements

- Rust 1.85+ (edition 2024)

## License

[EPICS Open License](../../LICENSE)
