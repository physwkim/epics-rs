# Simulated Detector

A full-featured simulated areaDetector driver for epics-rs, matching the C++ [ADSimDetector](https://github.com/areaDetector/ADSimDetector) architecture.

Generates synthetic images for testing detector pipelines without real hardware.

## Simulation Modes

| Mode | Description |
|------|-------------|
| LinearRamp | Ramp from min to max across image |
| Peaks | Gaussian peaks with configurable position, count, width, height variation |
| Sine | Separable X/Y sine waves (2 components per axis, Add or Multiply) |
| OffsetNoise | Noise floor with configurable offset |

## Architecture

```
SimDetector (PortDriver)
    │
    ├── params.rs        35+ asyn parameters (gains, offsets, modes)
    ├── compute.rs       Pixel-level image generation per mode
    ├── task.rs          Acquisition thread (continuous frame generation)
    ├── types.rs         SimMode, SineOperation, DirtyFlags
    └── ioc_support.rs   Device support bridge, 150+ PV mappings
```

The driver uses a dirty flag system to avoid recomputing caches unnecessarily. Changing a gain parameter only invalidates the gain cache, not the peak positions.

## Parameters

### Gains

| Parameter | Type | Description |
|-----------|------|-------------|
| `SIM_GAIN_X`, `SIM_GAIN_Y` | Float64 | Per-axis gain multipliers |
| `SIM_GAIN_RED/GREEN/BLUE` | Float64 | Per-color gains (RGB mode) |

### Peak Mode

| Parameter | Type | Description |
|-----------|------|-------------|
| `SIM_PEAK_START_X/Y` | Int32 | Initial peak position |
| `SIM_PEAK_NUM_X/Y` | Int32 | Number of peaks per dimension |
| `SIM_PEAK_STEP_X/Y` | Int32 | Spacing between peaks |
| `SIM_PEAK_WIDTH_X/Y` | Int32 | Peak width (sigma) |
| `SIM_PEAK_HEIGHT_VARIATION` | Float64 | Random height variation |

### Sine Mode

| Parameter | Type | Description |
|-----------|------|-------------|
| `SIM_XSINE1/2_AMPLITUDE` | Float64 | X sine component amplitudes |
| `SIM_XSINE1/2_FREQUENCY` | Float64 | X sine component frequencies |
| `SIM_XSINE1/2_PHASE` | Float64 | X sine component phases |
| `SIM_YSINE1/2_*` | Float64 | Y sine equivalents |
| `SIM_XSINE_OPERATION` | Int32 | Add (0) or Multiply (1) |

### General

| Parameter | Type | Description |
|-----------|------|-------------|
| `SIM_MODE` | Int32 | Simulation mode (0-3) |
| `SIM_OFFSET` | Float64 | DC offset added to all pixels |
| `SIM_NOISE` | Float64 | Random noise amplitude |
| `RESET_IMAGE` | Int32 | Trigger buffer reallocation |

## Build and Run

```bash
# Library only
cargo build -p sim-detector

# Full IOC with plugins
cargo build -p sim-detector --features ioc --bin sim_ioc

# Run IOC
./target/release/sim_ioc examples/sim-detector/ioc/st.cmd

# Standalone demo (no EPICS IOC, just PortHandle API)
cargo run -p sim-detector --example demo
```

## st.cmd Configuration

```bash
epicsEnvSet("PREFIX", "SIM1:")
epicsEnvSet("CAM",    "cam1:")

# simDetectorConfig(portName, sizeX, sizeY, maxMemory)
simDetectorConfig("SIM1", 1024, 1024, 50000000)

dbLoadRecords("$(SIM_DETECTOR)/db/simDetector.template", "P=$(PREFIX),R=$(CAM),PORT=SIM1")

# Standard areaDetector plugins
< $(ADCORE)/ioc/commonPlugins.cmd
```

## Verify

```bash
# Set simulation mode to Peaks
caput SIM1:cam1:SimMode 1

# Acquire single image
caput SIM1:cam1:ImageMode 0
caput SIM1:cam1:Acquire 1

# Monitor stats
camonitor SIM1:Stats1:MeanValue_RBV
```

## License

MIT
