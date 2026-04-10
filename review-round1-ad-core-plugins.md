# AD-Core-RS / AD-Plugins-RS C Parity Review — Round 1

Compared Rust implementations against C++ originals in `~/codes/epics-modules/ADCore/`.

## Critical Bugs (Wrong behavior)

### 1. Process plugin: filter equations are fundamentally different
- **File:** `ad-plugins-rs/src/process.rs:443-458`
- **C++ ref:** `NDPluginProcess.cpp:205-228`
- C++ uses a 2-buffer system (current data + filter state) with coefficients that depend on `numFiltered`:
  ```c
  O1 = oScale * (oc1 + oc2/numFiltered);
  newData   = oOffset + O1*filter[i] + O2*data[i];
  newFilter = fOffset + F1*filter[i] + F2*data[i];
  ```
- Rust implements a 6-buffer 4-tap IIR with fixed coefficients:
  ```rust
  f_new[i] = FC1*I + FC2*F_{n-1} + FC3*(F_{n-2}-FOffset) + FC4*(F_{n-3}-FOffset)
  ```
- These are completely different mathematical operations. Filter presets are also wrong.

### 2. Process plugin: offset/scale order reversed
- **File:** `ad-plugins-rs/src/process.rs:360`
- **C++ ref:** `NDPluginProcess.cpp:173`
- C++: `value = (value + offset) * scale` (offset first)
- Rust: `*v = *v * scale + offset` (scale first)
- Example: value=10, offset=5, scale=2 → C++ gives 30, Rust gives 25.

### 3. ROI plugin: scale is multiplication instead of division
- **File:** `ad-plugins-rs/src/roi.rs:212-215`
- **C++ ref:** `NDPluginROI.cpp:168`
- C++: `pData[i] = pData[i] / scale` (divisor)
- Rust: `scaled = val * config.scale` (multiplier)
- C++ header documents SCALE_VALUE as "Scaling value, used as divisor."

### 4. ROI plugin: binning averages instead of sums
- **File:** `ad-plugins-rs/src/roi.rs:202`
- **C++ ref:** `NDPluginROI.cpp:159-174` via `NDArrayPool::convert()`
- C++ sums binned pixels; Rust averages them.
- Combined with issue #3, this is doubly wrong.

### 5. Stats plugin: sigma_xy is covariance, not correlation coefficient
- **File:** `ad-plugins-rs/src/stats.rs:546`
- **C++ ref:** `NDPluginStats.cpp:266`
- C++: `sigmaXY = varXY / (sigmaX * sigmaY)` (correlation coefficient)
- Rust: `sigma_xy = mu11 / m00` (covariance)

### 6. Stats plugin: eccentricity formula has wrong sign
- **File:** `ad-plugins-rs/src/stats.rs:573-578`
- **C++ ref:** `NDPluginStats.cpp:281-283`
- C++: `((mu20-mu02)^2 - 4*mu11^2) / (mu20+mu02)^2` (minus sign)
- Rust: `((mu20-mu02)^2 + 4*mu11^2) / (mu20+mu02)^2` (plus sign)

### 7. Stats plugin: histogram entropy uses completely different formula
- **File:** `ad-plugins-rs/src/stats.rs:686-698`
- **C++ ref:** `NDPluginStats.cpp:56-63`
- C++: `-(sum(count * ln(count))) / nElements` (custom formula)
- Rust: `-sum(p * ln(p))` where `p = count/total_in_bins` (Shannon entropy)

### 8. Stats plugin: does not forward input arrays
- **File:** `ad-plugins-rs/src/stats.rs:995`
- **C++ ref:** `NDPluginStats.cpp:643`
- C++ calls `endProcessCallbacks(pArray, true, true)` to forward the array.
- Rust returns `ProcessResult::sink(updates)` — swallows the array.
- Any plugins chained after Stats receive nothing.

### 9. NDArrayPool: max_memory=0 means "reject all" instead of "unlimited"
- **File:** `ad-core-rs/src/ndarray_pool.rs:96-97`
- **C++ ref:** `NDArrayPool.cpp:214`
- C++: `maxMemory_ == 0` means unlimited (`if (maxMemory_ > 0) && ...`)
- Rust: `current + needed > self.max_memory` always true when max_memory=0.

### 10. NDArrayPool: TOCTOU race in allocation memory check
- **File:** `ad-core-rs/src/ndarray_pool.rs:94-102`
- **C++ ref:** `NDArrayPool.cpp:153` (protected by `listLock_`)
- Rust load+check+fetch_add is racy under concurrent access. Two threads can both pass the check and both add, exceeding max_memory.

### 11. Driver: create_file_name() template parsing is broken
- **File:** `ad-core-rs/src/driver/ndarray_driver.rs:118-139`
- **C++ ref:** `asynNDArrayDriver.cpp:192-221`
- C++ uses `epicsSnprintf(fullFileName, maxChars, fileTemplate, filePath, fileName, fileNumber)` with printf format strings like `%s%s_%3.3d.dat`.
- Rust does `template.replace("%s%s", ...).replace("%d", ...)` — can't handle `%3.3d` or separate `%s` occurrences.

### 12. ROI plugin: auto_size semantics differ
- **File:** `ad-plugins-rs/src/roi.rs:122-124`
- **C++ ref:** `NDPluginROI.cpp:92`
- C++: autoSize gives `size = full_dimension_size`
- Rust: autoSize gives `size = src_dim - min` (subtracts offset)


## High Severity (Missing critical features / wrong defaults)

### 13. NDArray::info() hardcodes RGB1 for all 3D arrays
- **File:** `ad-core-rs/src/ndarray.rs:313-337`
- **C++ ref:** `NDArray.cpp:131-203`
- C++ reads ColorMode attribute to determine RGB1/RGB2/RGB3 layout.
- Rust always assumes RGB1 for 3D arrays.

### 14. NDArrayInfo missing colorMode, dim indices, stride fields
- **File:** `ad-core-rs/src/ndarray.rs:277-285`
- **C++ ref:** `NDArray.h:78-94`
- Missing: `colorMode`, `xDim`, `yDim`, `colorDim`, `xStride`, `yStride`, `colorStride`.
- These are critical for any plugin that processes color images.

### 15. No RGB/color mode support in ROI, Transform, Overlay plugins
- ROI (`roi.rs`): Hardcodes dim[0]=X, dim[1]=Y, no color dimensions.
- Transform (`transform.rs`): Only handles mono 2D images.
- Overlay (`overlay.rs`): Only writes single channel value (red), not RGB planes.

### 16. Missing ADAcquire/ADAcquireBusy interlock logic
- **File:** `ad-core-rs/src/driver/ad_driver.rs`
- **C++ ref:** `asynNDArrayDriver.cpp:636-663`
- C++ automatically manages ADAcquireBusy when ADAcquire changes and when plugin queues drain.
- Rust has no equivalent logic.

### 17. Plugin runtime: missing MinCallbackTime/MaxByteRate throttling
- **File:** `ad-core-rs/src/plugin/runtime.rs:922-998`
- **C++ ref:** `NDPluginDriver.cpp:396-405, 342-358`
- Params are registered but never enforced. Every array is processed.

### 18. Plugin runtime: missing sort mode implementation
- **File:** `ad-core-rs/src/plugin/runtime.rs` (entire file)
- **C++ ref:** `NDPluginDriver.cpp:249-330, 619-670`
- C++ has sorted output with a separate sorting thread.
- Rust has no sorting logic. SORT_MODE PV is non-functional.

### 19. Plugin runtime: DroppedArrays/QueueFree/DroppedOutputArrays never updated
- **File:** `ad-core-rs/src/plugin/runtime.rs:519-520`
- **C++ ref:** `NDPluginDriver.cpp:384-389, 431-442`
- All these PVs always read 0 regardless of actual queue state.

### 20. Missing NDArrayPool::convert() (binning/reversal/type conversion)
- **File:** `ad-core-rs/src/ndarray_pool.rs`
- **C++ ref:** `NDArrayPool.cpp:574-737`
- Critical for ROI, ColorConvert, etc. Handles data type conversion, pixel binning/summation, axis reversal, and cumulative offset/binning tracking.

### 21. Missing NDArrayPool::copy() with selective field copying
- **File:** `ad-core-rs/src/ndarray_pool.rs`
- **C++ ref:** `NDArrayPool.cpp:269-304`
- C++ copy() supports `copyData`, `copyDimensions`, `copyDataType` flags.
- Rust alloc_copy() always does a full clone and bypasses the free list.

### 22. set_shutter() missing delay logic and callParamCallbacks
- **File:** `ad-core-rs/src/driver/ad_driver.rs:148-176`
- **C++ ref:** `ADDriver.cpp:29-52`
- Missing: callParamCallbacks before delay, sleep for shutterOpenDelay-shutterCloseDelay.

### 23. Missing writeInt32/writeOctet handlers
- No handler for ADShutterControl → setShutter() (`ADDriver.cpp:84-116`)
- No handler for NDPoolEmptyFreeList, NDPoolPreAllocBuffers, NDPoolPollStats (`asynNDArrayDriver.cpp:671-708`)
- No handler for NDFilePath → checkPath() + createFilePath() (`asynNDArrayDriver.cpp:511-552`)
- No handler for NDAttributesFile → readNDAttributesFile()

### 24. File plugin: missing NDFileOpenMode bitmask
- **File:** `ad-core-rs/src/plugin/file_base.rs:7-23`
- **C++ ref:** `NDPluginFile.h:10-17`
- C++ uses bitmask (Read|Write|Append|Multiple). Rust conflates write mode with open mode.

### 25. File plugin: capture mode ignores supportsMultipleArrays
- **File:** `ad-core-rs/src/plugin/file_base.rs:237-256`
- **C++ ref:** `NDPluginFile.cpp:280-331`
- Rust always opens once + writes all + closes once.
- Single-image formats (JPEG, TIFF) need open/write/close per frame.

### 26. File plugin: missing frame validation in stream mode
- **File:** `ad-core-rs/src/plugin/file_controller.rs`
- **C++ ref:** `NDPluginFile.cpp:653-705`
- C++ validates each frame against initial dimensions/type. Rust accepts any array.

### 27. File plugin: missing attribute-based features
- No `FILEPLUGIN_DESTINATION` attribute filtering
- No `FilePluginFileName`/`FilePluginFileNumber` attribute override
- No `FilePluginClose` attribute for mid-stream close
- **C++ ref:** `NDPluginFile.cpp:496-651`

### 28. create_file_name() missing auto-increment
- **File:** `ad-core-rs/src/driver/ndarray_driver.rs:118-139`
- **C++ ref:** `asynNDArrayDriver.cpp:216-219`
- C++ checks NDAutoIncrement and increments NDFileNumber after creating filename.
- Rust never reads auto_increment and never increments file_number.


## Medium Severity

### 29. NDArray: missing separate f64 timeStamp field
- C++ has both `double timeStamp` (EPICS seconds) and `epicsTimeStamp epicsTS` (sec+nsec). They are independently set.
- Rust only has `timestamp: EpicsTimestamp`.

### 30. NDArray: compressedSize restructured into Codec
- C++ has `compressedSize` always present on NDArray (defaults to dataSize).
- Rust puts it inside `Option<Codec>`, so when codec is None, there's no compressed_size.

### 31. NDArrayPool: alloc_copy() bypasses free-list reuse
- Always does a fresh clone, never reuses pooled buffers.

### 32. NDArrayPool: no THRESHOLD_SIZE_RATIO for oversized buffers
- C++ frees buffers > 1.5x needed size. Rust reuses any buffer that fits.

### 33. NDArrayPool: allocated_bytes tracking uses logical size on alloc but capacity on release
- Causes drift over time.

### 34. NDArrayPool: pool fails instead of reclaiming free-list entries
- C++ tries to free entries to make room before failing. Rust returns error immediately.

### 35. Driver: multiple initial param values differ from C++
- NDMaxSizeX/Y: C++ initializes to 1, Rust to constructor arg
- NDArraySizeX/Y: C++ initializes to 0, Rust to max_size
- NDArraySize: C++ initializes to 0, Rust to max_x * max_y
- NDPoolMaxMemory: C++ initializes to 0.0, Rust computes from max_memory
- NDFileTemplate: C++ sets "%s%s_%3.3d.dat", Rust doesn't set

### 36. Driver: ADDriverBase::publish_array() missing unique_id and full pool stats
- Doesn't update unique_id param or pool_free_buffers/pool_alloc_buffers.

### 37. Driver: wait_for_plugins param duplicated across base and derived params
- Both NDArrayDriverParams and ADDriverParams create "WAIT_FOR_PLUGINS".

### 38. Driver: acquire/acquire_busy in wrong param level
- C++ puts them in asynNDArrayDriver (base). Rust puts them in ADDriverParams (derived).

### 39. Overlay: uses red channel for mono instead of green
- **File:** `ad-plugins-rs/src/overlay.rs:167`
- C++ uses `pOverlay->green` for mono overlays.

### 40. Overlay: cross/rectangle/ellipse missing WidthX/WidthY support
- All shapes are drawn as 1px wide. C++ supports configurable line thickness.

### 41. Overlay: missing I8, I64, U64 data type support
- Silently skips these data types.

### 42. Stats: missing cursorValue, PROFILE_SIZE_X/Y, HIST_ARRAY, HIST_X_ARRAY params
- Cursor position value readback and array PVs for profiles/histograms not registered.

### 43. Stats: profile arrays not published as Float64Array params
- Data is computed but not exposed over Channel Access.

### 44. ROI: collapseDims only collapses Y dimension
- C++ collapses ALL dimensions of size 1. Rust only checks Y.

### 45. Process: auto_offset_scale formula differs and doesn't enable clipping
- C++ also sets enableLowClip, lowClipThresh, enableHighClip, highClipThresh.

### 46. Process: rOffset (reset offset) not implemented
- Param exists but is ignored.

### 47. Process: filterCallbacks not implemented
- Field exists in FilterConfig but never checked.

### 48. Plugin runtime: ProcessPlugin (reprocess last array) not implemented
- No cached input array, no handler for process_plugin param.

### 49. File plugin: lazy_open PV has no effect
- Field exists but is never checked during write path.

### 50. File plugin: delete_driver_file not implemented
- Param is set but never checked during writes.

### 51. Driver: check_path() missing path normalization (trailing delimiter)
- C++ normalizes path and writes back; Rust only checks existence.

### 52. Driver: missing createFilePath() method
- C++ creates directory hierarchies based on NDFileCreateDir depth.

### 53. Codec uses enum instead of string name
- Custom/third-party codecs can't be represented.

### 54. ShutterMode enum has extra EpicsAndDetector variant
- C++ only has 3 modes; Rust adds a 4th that doesn't exist in C++.
- Also: catch-all mapping of invalid values to EpicsAndDetector is dangerous.

### 55. Driver: phantom shutter_status_epics param
- "SHUTTER_STATUS_EPICS" param doesn't exist in C++.
