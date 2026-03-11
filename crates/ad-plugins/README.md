# ad-plugins

NDPlugin implementations for [areaDetector-rs](../ad-core/). 23 image processing and data handling plugins for real-time detector data pipelines.

No C dependencies. Pure Rust with real encoding libraries (JPEG, TIFF, LZ4, FFT).

## Plugins

### Statistical Analysis

| Plugin | Description |
|--------|-------------|
| **NDStats** | Min/max/mean/sigma/total, centroid with threshold, histogram, row/column profiles, higher-order moments |
| **NDROIStat** | Multi-ROI statistics with background subtraction and time series |
| **NDAttrPlot** | Circular buffer tracking of numeric NDArray attributes |
| **NDAttribute** | Single attribute extraction with cumulative sum |

### Image Transformation

| Plugin | Description |
|--------|-------------|
| **NDROIPlugin** | Region-of-interest extraction with auto-center (CoM/Peak), binning, per-dimension enable |
| **NDTransform** | 8 geometric transforms: rotations (90/180/270), flips (H/V/diagonal) |
| **NDColorConvert** | RGB/Mono conversion, Bayer demosaic (bilinear), false-color jet LUT |
| **NDOverlay** | Draw shapes (cross, rectangle, ellipse, text) with 5x7 bitmap font |

### Array Processing

| Plugin | Description |
|--------|-------------|
| **NDProcess** | 4-tap recursive filter, background/flatfield save, offset/scale, clipping |
| **NDFFT** | 1D (rows) and 2D (separable) FFT via rustfft, DC suppression, frame averaging |

### Data Buffering

| Plugin | Description |
|--------|-------------|
| **NDCircularBuff** | Pre/post-trigger ring buffer with Calc expression evaluator |
| **NDTimeSeries** | OneShot/RingBuffer accumulation for statistics channels |
| **NDStdArrays** | Latest array storage (passthrough with caching) |

### File I/O

| Plugin | Description |
|--------|-------------|
| **NDFileHDF5** | HDF5 file writer (feature-gated `hdf5` crate, with binary fallback) |
| **NDFileJPEG** | JPEG encoding/decoding via jpeg-encoder/jpeg-decoder |
| **NDFileTIFF** | TIFF encoding/decoding via tiff crate |

### Codec

| Plugin | Description |
|--------|-------------|
| **NDCodec** | Lossless compression: LZ4 (lz4_flex) + JPEG, preserves original data type |

### Position & Pixel Correction

| Plugin | Description |
|--------|-------------|
| **NDPos** | Attach position metadata from JSON list (Discard/Keep modes) |
| **NDBadPixel** | Bad pixel correction: Set (fixed value), Replace (neighbor), Median (kernel). JSON config |

### Multiplexing

| Plugin | Description |
|--------|-------------|
| **NDGather** | Passthrough multiplexer (many → one) |
| **NDScatter** | Round-robin splitter (one → many) |
| **Passthrough** | No-op stub for unimplemented plugin types |

## Features

```toml
[features]
default = []
hdf5 = ["dep:hdf5"]    # HDF5 file format (requires libhdf5)
ioc  = ["ad-core/ioc"]  # IOC startup commands (NDStatsConfigure, etc.)
```

## Usage

Each plugin implements `NDPluginProcess`:

```rust
use ad_plugins::stats::NDStatsPlugin;
use ad_core::plugin::NDPluginProcess;

let mut stats = NDStatsPlugin::new();
let result = stats.process_array(&array, &pool);
// result.arrays contains processed output
```

With the `ioc` feature, plugins register as st.cmd startup commands:

```bash
# In st.cmd
NDStatsConfigure("STATS1", 5, "SIM1")
NDROIConfigure("ROI1", 5, "SIM1")
NDStdArraysConfigure("IMAGE1", 5, "SIM1")
```

## Build

```bash
cargo build -p ad-plugins                    # plugins only
cargo build -p ad-plugins --features ioc     # with IOC commands
cargo build -p ad-plugins --features hdf5    # with HDF5 support
cargo test -p ad-plugins                     # 205 tests
```

## License

MIT
