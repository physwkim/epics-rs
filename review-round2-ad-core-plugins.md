# AD-Core-RS / AD-Plugins-RS C Parity Review — Round 2

Reviewed remaining plugins, core modules, and file plugins.

## Critical / High Severity Bugs

### FFT Plugin
1. **FFT magnitudes not normalized by N** — values are N (or N*M for 2D) times too large vs C++
2. **Full spectrum output (N bins) instead of half (N/2)** — C++ only outputs positive frequencies
3. **Averaging algorithm completely different** — Rust uses block accumulator; C++ uses exponential moving average
4. **Missing power-of-2 padding** — C++ pads to next power of 2
5. **Missing real/imaginary/time-series output arrays** — params registered but never written
6. **Missing time/frequency axis computation**

### Codec Plugin
7. **LZ4 wire format incompatible** — Rust uses `compress_prepend_size` (4-byte header); C++ uses raw LZ4
8. **Compressor enum mapping swapped** — C++: 1=JPEG,2=Blosc,3=LZ4; Rust: 1=LZ4,2=JPEG,3=Blosc
9. **Missing BSLZ4 (Bitshuffle-LZ4) codec** — commonly used in scientific detectors
10. **Failed operations sink array instead of passing through** — C++ forwards original on failure
11. **No check for already-compressed arrays** — Rust will attempt double-compression
12. **Original dataType stored in attribute instead of preserved** — interop issue with C++

### CircularBuff Plugin
13. **Post-trigger frames batched instead of streamed** — C++ emits one-by-one; Rust batches all
14. **Trigger frame counted as last pre-trigger instead of first post-trigger**
15. **CalcExpression only supports A,B variables** — C++ uses full EPICS calc with A-L, math functions
16. **Missing NaN default for trigger attributes** — uses 0.0 instead of NaN

### HDF5 Plugin
17. **Compression enum off-by-one** — missing BSHUF=5; JPEG mapped to 5 should be 7
18. **XML layout files not implemented** — core feature, params are dead
19. **Several data types fall to raw-byte path** (I8, I16, U32, I64, U64)

### NetCDF Plugin
20. **Dimension ordering reversed** — C++ reverses dims; Rust uses forward order
21. **No per-frame uniqueId/timeStamp/epicsTS variables**
22. **Per-frame attribute variables missing**
23. **Missing dimension metadata and file version global attributes**
24. **Int64/UInt64 rejected instead of cast to double**

### Nexus Plugin
25. **XML template not implemented** — core feature; hardcoded hierarchy instead
26. **Missing NX_class attributes** — output is not valid NeXus
27. **Multi-frame stored as separate datasets** — should use slabs in single dataset

### TIFF Plugin
28. **No custom TIFF tags for attributes/timestamps/uniqueId**
29. **RGB2/RGB3 force-converted to RGB1**

### JPEG Plugin
30. **RGB2/RGB3 data produces garbled output** — no interleaving conversion
31. **Int8 data rejected** — C++ accepts it

### Magick Plugin
32. **Only U8/U16 supported** — C++ supports all types via GraphicsMagick
33. **bit_depth and compress_type params non-functional**

### ColorConvert Plugin
34. **ColorMode attribute not set on output** — breaks downstream plugins
35. **Bayer demosaic ignores dimension offsets** — wrong phase for sub-regions
36. **Bayer only supports U8/U16** — C++ supports all types
37. **False color uses wrong colormap** — "jet" instead of Rainbow/Iron
38. **No false_color mode selection** — boolean instead of C++ enum (0=off,1=Rainbow,2=Iron)

### Gather Plugin
39. **Multi-source subscription not implemented** — plugin is effectively passthrough

### BadPixel Plugin
40. **Missing binning/offset handling in pixel coordinates**
41. **Incompatible JSON format** — different from C++ JSON structure
42. **Median kernel size semantics differ** — full-size vs half-extent

### PosPlugin
43. **Keep mode wraps around forever** — C++ stops at end of list
44. **Missing IDDifference, IDStart, IDName parameters**
45. **Position file format: JSON vs C++ XML**

### TimeSeries Plugin
46. **Missing per-point averaging** — C++ averages N samples per time point
47. **Missing multi-signal support from 2D arrays**
48. **Missing timestamp array per time point**
49. **Time axis not scaled by averaging time** — uses raw index instead

### Core: Attributes
50. **Missing EpicsPV, Function, Undefined source types**
51. **Missing Undefined data type variant**
52. **Missing copy (merge) method on attribute list**

### Core: Codec struct
53. **Missing level, shuffle, compressor fields** — only has name + compressed_size
54. **compressed_size in wrong location** — C++ has it on NDArray, not in Codec

### Core: Color
55. **mono_to_rgb1/rgb1_to_mono only handle U8/U16** — missing 8 other types
56. **ColorLayout only supports Mono and RGB1** — panics on RGB2/RGB3

### Core: ROI
57. **No compressed data check** — silently processes compressed bytes as pixels
58. **No bounds checking** — will panic on out-of-bounds ROI
59. **Doesn't track cumulative offset/binning in output dimensions**

## Medium Severity

### Attribute Plugin
60. **Param name ATTR_NAME vs C++ ATTR_ATTRNAME** — database template mismatch
61. **Reset only resets single channel sum** — C++ resets all channels' value+sum
62. **Missing NDArrayEpicsTSSec/NDArrayEpicsTSnSec pseudo-attributes**

### ROIStat Plugin
63. **Time series uses circular buffer vs C++ fixed-length** — different retention model
64. **TSControl simplified from 5 commands to binary start/stop**
65. **Missing timestamp time series channel**

### AttrPlot Plugin
66. **Missing data selection/block concept and periodic data exposure**
67. **Missing NaN fill** — uses 0.0 for absent attributes

### Scatter Plugin
68. **Round-robin index grows unbounded** — no wrap-around
69. **Missing "try next on queue full" fallback**

### HDF5 Plugin
70. **Missing fill value support** — param registered but unused
71. **Missing extraDimChunkN param**

### JPEG Plugin
72. **Default quality 90 vs C++ default 50**

### Magick Plugin
73. **RGB2/RGB3 force-converted** — actually more complete than C++ which has empty cases
