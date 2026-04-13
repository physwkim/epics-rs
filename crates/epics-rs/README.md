# epics-rs

Umbrella crate that re-exports all epics-rs sub-crates. Use feature flags to select which modules you need.

**Repository:** <https://github.com/epics-rs/epics-rs>

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `ca` | Channel Access client & server | **yes** |
| `pva` | pvAccess client (experimental) | no |
| `bridge` | Record <-> PVA bridge (QSRV equivalent) | no |
| `asyn` | Async port driver framework | no |
| `motor` | Motor record + SimMotor | no |
| `ad` | areaDetector (core + 23 plugins) | no |
| `calc` | Calc expression engine | always |
| `autosave` | PV save/restore | always |
| `busy` | Busy record | always |
| `std` | Standard records (epid, throttle, timestamp) | no |
| `scaler` | Scaler record (64-channel counter) | no |
| `optics` | Beamline optics (table, monochromator, filters) | no |
| `full` | Everything | no |

## Usage

```toml
[dependencies]
epics-rs = { version = "0.8", features = ["motor", "ad"] }
```

```rust
use epics_rs::base;        // IOC runtime, records, iocsh
use epics_rs::ca;          // Channel Access (feature = "ca")
use epics_rs::pva;         // pvAccess client (feature = "pva")
use epics_rs::bridge;      // Record <-> PVA bridge (feature = "bridge")
use epics_rs::asyn;        // port driver framework (feature = "asyn")
use epics_rs::motor;       // motor record (feature = "motor")
use epics_rs::ad_core;     // areaDetector core (feature = "ad")
use epics_rs::ad_plugins;  // areaDetector plugins (feature = "ad")
```

## License

[EPICS Open License](../../LICENSE)
