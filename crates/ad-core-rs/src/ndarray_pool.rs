use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

use parking_lot::Mutex;

use crate::error::{ADError, ADResult};
use crate::ndarray::{NDArray, NDDataBuffer, NDDataType, NDDimension};
use crate::ndarray_handle::{NDArrayHandle, pooled_array};
use crate::timestamp::EpicsTimestamp;

/// If a free-list buffer is more than this ratio larger than needed, discard
/// it and allocate fresh to avoid wasting memory.
const THRESHOLD_SIZE_RATIO: f64 = 1.5;

/// NDArray factory with free-list reuse and memory tracking.
///
/// Mimics C++ ADCore's NDArrayPool: on alloc, checks the free list for a
/// buffer with sufficient capacity. On release, returns the buffer to the
/// free list for future reuse. The free list is sorted by capacity (descending)
/// and excess entries are dropped when max_memory is exceeded.
pub struct NDArrayPool {
    max_memory: usize,
    allocated_bytes: AtomicU64,
    next_unique_id: AtomicI32,
    free_list: Mutex<Vec<NDArray>>,
    num_alloc_buffers: AtomicU32,
    num_free_buffers: AtomicU32,
}

impl NDArrayPool {
    pub fn new(max_memory: usize) -> Self {
        Self {
            max_memory,
            allocated_bytes: AtomicU64::new(0),
            next_unique_id: AtomicI32::new(1),
            free_list: Mutex::new(Vec::new()),
            num_alloc_buffers: AtomicU32::new(0),
            num_free_buffers: AtomicU32::new(0),
        }
    }

    /// Allocate an NDArray. Tries to reuse a free-list entry with sufficient capacity.
    pub fn alloc(&self, dims: Vec<NDDimension>, data_type: NDDataType) -> ADResult<NDArray> {
        let num_elements: usize = dims.iter().map(|d| d.size).product();
        let needed_bytes = num_elements * data_type.element_size();

        // Try to find a reusable buffer in the free list
        let reused = {
            let mut free = self.free_list.lock();
            // Find smallest buffer that is large enough (free list sorted descending by capacity)
            let mut best_idx = None;
            let mut best_cap = usize::MAX;
            for (i, arr) in free.iter().enumerate() {
                let cap = arr.data.capacity_bytes();
                if cap >= needed_bytes && cap < best_cap {
                    best_cap = cap;
                    best_idx = Some(i);
                }
            }
            if let Some(idx) = best_idx {
                if best_cap as f64 > needed_bytes as f64 * THRESHOLD_SIZE_RATIO {
                    let dropped = free.swap_remove(idx);
                    let dropped_cap = dropped.data.capacity_bytes();
                    self.num_free_buffers.fetch_sub(1, Ordering::Relaxed);
                    self.allocated_bytes
                        .fetch_sub(dropped_cap as u64, Ordering::Relaxed);
                    self.num_alloc_buffers.fetch_sub(1, Ordering::Relaxed);
                    None
                } else {
                    let arr = free.swap_remove(idx);
                    self.num_free_buffers.fetch_sub(1, Ordering::Relaxed);
                    Some(arr)
                }
            } else {
                None
            }
        };

        let mut arr = if let Some(mut reused) = reused {
            // Reuse: retype the buffer if needed, resize to match
            if reused.data.data_type() != data_type {
                // Must reallocate with new type, but we keep the allocation tracked
                let old_cap = reused.data.capacity_bytes();
                reused.data = NDDataBuffer::zeros(data_type, num_elements);
                let new_cap = reused.data.capacity_bytes();
                // Adjust allocated_bytes for the difference
                if new_cap > old_cap {
                    let diff = new_cap - old_cap;
                    let current = self.allocated_bytes.load(Ordering::Relaxed);
                    if self.max_memory > 0 && current + diff as u64 > self.max_memory as u64 {
                        return Err(ADError::PoolExhausted(needed_bytes, self.max_memory));
                    }
                    self.allocated_bytes
                        .fetch_add(diff as u64, Ordering::Relaxed);
                } else {
                    let diff = old_cap - new_cap;
                    self.allocated_bytes
                        .fetch_sub(diff as u64, Ordering::Relaxed);
                }
            } else {
                reused.data.resize(num_elements);
            }
            reused.dims = dims;
            reused.attributes.clear();
            reused.codec = None;
            reused
        } else {
            // Fresh allocation with CAS loop to avoid TOCTOU race
            if self.max_memory > 0 {
                loop {
                    let current = self.allocated_bytes.load(Ordering::Relaxed);
                    if current + needed_bytes as u64 > self.max_memory as u64 {
                        let mut freed_enough = false;
                        {
                            let mut free = self.free_list.lock();
                            free.sort_by(|a, b| {
                                b.data.capacity_bytes().cmp(&a.data.capacity_bytes())
                            });
                            let mut reclaimed = 0u64;
                            let over = (current + needed_bytes as u64)
                                .saturating_sub(self.max_memory as u64);
                            while !free.is_empty() && reclaimed < over {
                                let dropped = free.remove(0);
                                let dropped_cap = dropped.data.capacity_bytes();
                                self.allocated_bytes
                                    .fetch_sub(dropped_cap as u64, Ordering::Relaxed);
                                self.num_free_buffers.fetch_sub(1, Ordering::Relaxed);
                                self.num_alloc_buffers.fetch_sub(1, Ordering::Relaxed);
                                reclaimed += dropped_cap as u64;
                            }
                            if reclaimed >= over {
                                freed_enough = true;
                            }
                        }
                        if !freed_enough {
                            return Err(ADError::PoolExhausted(needed_bytes, self.max_memory));
                        }
                        continue;
                    }
                    if self
                        .allocated_bytes
                        .compare_exchange_weak(
                            current,
                            current + needed_bytes as u64,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        break;
                    }
                }
            } else {
                self.allocated_bytes
                    .fetch_add(needed_bytes as u64, Ordering::Relaxed);
            }
            self.num_alloc_buffers.fetch_add(1, Ordering::Relaxed);
            let new_arr = NDArray::new(dims, data_type);
            let actual_cap = new_arr.data.capacity_bytes();
            if actual_cap > needed_bytes {
                self.allocated_bytes
                    .fetch_add((actual_cap - needed_bytes) as u64, Ordering::Relaxed);
            }
            new_arr
        };

        arr.unique_id = self.next_unique_id.fetch_add(1, Ordering::Relaxed);
        arr.timestamp = EpicsTimestamp::now();
        Ok(arr)
    }

    /// Allocate a copy of an existing NDArray (new unique_id, data cloned).
    /// Tries the free list first (via alloc()), then copies data from source.
    pub fn alloc_copy(&self, source: &NDArray) -> ADResult<NDArray> {
        let dims = source.dims.clone();
        let data_type = source.data.data_type();
        let mut copy = self.alloc(dims, data_type)?;
        copy.data = source.data.clone();
        copy.time_stamp = source.time_stamp;
        copy.attributes = source.attributes.clone();
        copy.codec = source.codec.clone();
        Ok(copy)
    }

    /// Return an array to the free list for future reuse.
    pub fn release(&self, array: NDArray) {
        let cap = array.data.capacity_bytes();
        let mut free = self.free_list.lock();
        free.push(array);
        self.num_free_buffers.fetch_add(1, Ordering::Relaxed);

        // If total allocated exceeds max_memory, drop largest free entries
        // (max_memory == 0 means unlimited, skip trimming)
        let total = self.allocated_bytes.load(Ordering::Relaxed) as usize;
        if self.max_memory > 0 && total > self.max_memory && !free.is_empty() {
            // Sort descending by capacity so we drop largest first
            free.sort_by(|a, b| b.data.capacity_bytes().cmp(&a.data.capacity_bytes()));
            let mut excess = total.saturating_sub(self.max_memory);
            while excess > 0 && !free.is_empty() {
                let dropped = free.remove(0);
                let dropped_cap = dropped.data.capacity_bytes();
                self.allocated_bytes
                    .fetch_sub(dropped_cap.min(total) as u64, Ordering::Relaxed);
                self.num_free_buffers.fetch_sub(1, Ordering::Relaxed);
                self.num_alloc_buffers.fetch_sub(1, Ordering::Relaxed);
                if dropped_cap >= excess {
                    break;
                }
                excess -= dropped_cap;
            }
        }
        let _ = cap;
    }

    /// Clear all entries from the free list.
    pub fn empty_free_list(&self) {
        let mut free = self.free_list.lock();
        let count = free.len() as u32;
        for arr in free.drain(..) {
            let cap = arr.data.capacity_bytes();
            self.allocated_bytes
                .fetch_sub(cap as u64, Ordering::Relaxed);
            self.num_alloc_buffers.fetch_sub(1, Ordering::Relaxed);
        }
        self.num_free_buffers.fetch_sub(count, Ordering::Relaxed);
    }

    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    pub fn num_alloc_buffers(&self) -> u32 {
        self.num_alloc_buffers.load(Ordering::Relaxed)
    }

    pub fn num_free_buffers(&self) -> u32 {
        self.num_free_buffers.load(Ordering::Relaxed)
    }

    pub fn max_memory(&self) -> usize {
        self.max_memory
    }

    /// Allocate an NDArray wrapped in a pool-aware handle.
    /// On final drop, the array is returned to this pool's free list.
    pub fn alloc_handle(
        pool: &Arc<Self>,
        dims: Vec<NDDimension>,
        data_type: NDDataType,
    ) -> ADResult<NDArrayHandle> {
        let array = pool.alloc(dims, data_type)?;
        Ok(pooled_array(array, pool))
    }

    /// Convert data type only (no dimension changes).
    /// Allocates from pool, converts data, copies metadata.
    pub fn convert_type(&self, src: &NDArray, target_type: NDDataType) -> ADResult<NDArray> {
        if src.data.data_type() == target_type {
            return self.alloc_copy(src);
        }
        let mut out = crate::color::convert_data_type(src, target_type)?;
        out.unique_id = self
            .next_unique_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(out)
    }

    /// Full convert with dimension changes: extract sub-region, bin, reverse.
    /// `dims_out` specifies offset/size/binning/reverse for each dimension.
    /// Allocates from pool with output dimensions.
    ///
    /// Matches the C++ `NDArrayPool::convert()` semantics:
    /// - Output size for each dim = `dims_out[i].size / dims_out[i].binning`
    /// - Source pixels are summed (not averaged) across each binning window
    /// - Reverse flips the output along that dimension
    /// - Cumulative offset: `out.dims[i].offset = src.dims[i].offset + dims_out[i].offset`
    /// - Cumulative binning: `out.dims[i].binning = src.dims[i].binning * dims_out[i].binning`
    pub fn convert(
        &self,
        src: &NDArray,
        dims_out: &[NDDimension],
        target_type: NDDataType,
    ) -> ADResult<NDArray> {
        let ndims = src.dims.len();
        if dims_out.len() != ndims {
            return Err(ADError::InvalidDimensions(format!(
                "convert: dims_out length {} != source ndims {}",
                dims_out.len(),
                ndims,
            )));
        }

        // Compute output sizes and validate
        let mut out_sizes = Vec::with_capacity(ndims);
        for (i, d) in dims_out.iter().enumerate() {
            let bin = d.binning.max(1);
            if d.size == 0 {
                return Err(ADError::InvalidDimensions(format!(
                    "convert: dims_out[{}].size is 0",
                    i,
                )));
            }
            let out_size = d.size / bin;
            if out_size == 0 {
                return Err(ADError::InvalidDimensions(format!(
                    "convert: dims_out[{}] size {} / binning {} = 0",
                    i, d.size, bin,
                )));
            }
            // Validate that offset + size fits within source dimension
            if d.offset + d.size > src.dims[i].size {
                return Err(ADError::InvalidDimensions(format!(
                    "convert: dims_out[{}] offset {} + size {} > src dim size {}",
                    i, d.offset, d.size, src.dims[i].size,
                )));
            }
            out_sizes.push(out_size);
        }

        let src_type = src.data.data_type();

        // Build output dimension metadata
        let mut out_dims = Vec::with_capacity(ndims);
        for i in 0..ndims {
            let bin = dims_out[i].binning.max(1);
            out_dims.push(NDDimension {
                size: out_sizes[i],
                offset: src.dims[i].offset + dims_out[i].offset,
                binning: src.dims[i].binning * bin,
                reverse: dims_out[i].reverse,
            });
        }

        let total_out: usize = out_sizes.iter().product();

        // Precompute source strides (row-major: dim[0] varies fastest)
        let mut src_strides = vec![1usize; ndims];
        for i in 1..ndims {
            src_strides[i] = src_strides[i - 1] * src.dims[i - 1].size;
        }

        // Precompute output strides
        let mut out_strides = vec![1usize; ndims];
        for i in 1..ndims {
            out_strides[i] = out_strides[i - 1] * out_sizes[i - 1];
        }

        // Macro to handle binning/offset/reverse for a specific typed buffer
        macro_rules! convert_buf {
            ($src_vec:expr, $T:ty, $zero:expr, $variant:ident) => {{
                let mut out = vec![$zero; total_out];

                // Iterate over all output pixels
                for out_idx in 0..total_out {
                    // Decompose flat output index into per-dim coordinates
                    let mut remaining = out_idx;
                    let mut out_coords = [0usize; 10]; // up to 10 dims
                    for i in (0..ndims).rev() {
                        out_coords[i] = remaining / out_strides[i];
                        remaining %= out_strides[i];
                    }

                    // Apply reverse: flip coordinate in output space
                    let mut eff_coords = [0usize; 10];
                    for i in 0..ndims {
                        eff_coords[i] = if dims_out[i].reverse {
                            out_sizes[i] - 1 - out_coords[i]
                        } else {
                            out_coords[i]
                        };
                    }

                    // Sum over binning window
                    let mut sum = 0.0f64;
                    let bin_total: usize = dims_out.iter().map(|d| d.binning.max(1)).product();

                    // Iterate over all bin offsets
                    for bin_flat in 0..bin_total {
                        let mut br = bin_flat;
                        let mut src_flat = 0usize;
                        let mut valid = true;

                        for i in (0..ndims).rev() {
                            let bin = dims_out[i].binning.max(1);
                            let bin_off = br % bin;
                            br /= bin;

                            let src_coord = dims_out[i].offset + eff_coords[i] * bin + bin_off;
                            if src_coord >= src.dims[i].size {
                                valid = false;
                                break;
                            }
                            src_flat += src_coord * src_strides[i];
                        }

                        if valid {
                            sum += $src_vec[src_flat] as f64;
                        }
                    }

                    out[out_idx] = sum as $T;
                }

                NDDataBuffer::$variant(out)
            }};
        }

        let out_data = match &src.data {
            NDDataBuffer::I8(v) => convert_buf!(v, i8, 0i8, I8),
            NDDataBuffer::U8(v) => convert_buf!(v, u8, 0u8, U8),
            NDDataBuffer::I16(v) => convert_buf!(v, i16, 0i16, I16),
            NDDataBuffer::U16(v) => convert_buf!(v, u16, 0u16, U16),
            NDDataBuffer::I32(v) => convert_buf!(v, i32, 0i32, I32),
            NDDataBuffer::U32(v) => convert_buf!(v, u32, 0u32, U32),
            NDDataBuffer::I64(v) => convert_buf!(v, i64, 0i64, I64),
            NDDataBuffer::U64(v) => convert_buf!(v, u64, 0u64, U64),
            NDDataBuffer::F32(v) => convert_buf!(v, f32, 0.0f32, F32),
            NDDataBuffer::F64(v) => convert_buf!(v, f64, 0.0f64, F64),
        };

        // Build intermediate array in source type with binned data
        let mut arr = NDArray {
            unique_id: self
                .next_unique_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            timestamp: src.timestamp,
            time_stamp: src.time_stamp,
            dims: out_dims,
            data: out_data,
            attributes: src.attributes.clone(),
            codec: src.codec.clone(),
        };

        // Convert data type if needed
        if target_type != src_type {
            let converted = crate::color::convert_data_type(&arr, target_type)?;
            arr.data = converted.data;
        }

        Ok(arr)
    }
}

// Compile-time check: NDArrayPool is Send + Sync
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NDArrayPool>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_auto_id() {
        let pool = NDArrayPool::new(1_000_000);
        let a1 = pool
            .alloc(vec![NDDimension::new(10)], NDDataType::UInt8)
            .unwrap();
        let a2 = pool
            .alloc(vec![NDDimension::new(10)], NDDataType::UInt8)
            .unwrap();
        assert_eq!(a1.unique_id, 1);
        assert_eq!(a2.unique_id, 2);
    }

    #[test]
    fn test_alloc_tracks_bytes() {
        let pool = NDArrayPool::new(1_000_000);
        let _ = pool
            .alloc(vec![NDDimension::new(100)], NDDataType::Float64)
            .unwrap();
        assert!(pool.allocated_bytes() >= 800);
    }

    #[test]
    fn test_alloc_exceeds_max() {
        let pool = NDArrayPool::new(100);
        let result = pool.alloc(vec![NDDimension::new(200)], NDDataType::UInt8);
        assert!(result.is_err());
    }

    #[test]
    fn test_alloc_copy_preserves_data() {
        let pool = NDArrayPool::new(1_000_000);
        let mut source = pool
            .alloc(vec![NDDimension::new(4)], NDDataType::UInt8)
            .unwrap();
        if let NDDataBuffer::U8(ref mut v) = source.data {
            v[0] = 1;
            v[1] = 2;
            v[2] = 3;
            v[3] = 4;
        }

        let copy = pool.alloc_copy(&source).unwrap();
        assert_ne!(copy.unique_id, source.unique_id);
        assert_eq!(copy.dims.len(), source.dims.len());
        if let NDDataBuffer::U8(ref v) = copy.data {
            assert_eq!(v, &[1, 2, 3, 4]);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_alloc_copy_tracks_bytes() {
        let pool = NDArrayPool::new(1_000_000);
        let source = pool
            .alloc(vec![NDDimension::new(10)], NDDataType::UInt16)
            .unwrap();
        assert_eq!(pool.allocated_bytes(), 20);
        let _ = pool.alloc_copy(&source).unwrap();
        assert!(pool.allocated_bytes() >= 40);
    }

    #[test]
    fn test_alloc_copy_exceeds_max() {
        let pool = NDArrayPool::new(60);
        let source = pool
            .alloc(vec![NDDimension::new(50)], NDDataType::UInt8)
            .unwrap();
        assert!(pool.alloc_copy(&source).is_err());
    }

    // --- Free-list reuse tests ---

    #[test]
    fn test_release_and_reuse() {
        let pool = NDArrayPool::new(1_000_000);
        let arr = pool
            .alloc(vec![NDDimension::new(100)], NDDataType::UInt8)
            .unwrap();
        let _alloc_bytes_after_first = pool.allocated_bytes();
        assert_eq!(pool.num_alloc_buffers(), 1);

        // Release back to free list
        pool.release(arr);
        assert_eq!(pool.num_free_buffers(), 1);

        // Alloc again — reuse within 1.5x ratio
        let arr2 = pool
            .alloc(vec![NDDimension::new(80)], NDDataType::UInt8)
            .unwrap();
        assert_eq!(arr2.data.len(), 80);
    }

    #[test]
    fn test_free_list_prefers_smallest_sufficient() {
        let pool = NDArrayPool::new(10_000_000);
        let small = pool
            .alloc(vec![NDDimension::new(100)], NDDataType::UInt8)
            .unwrap();
        let large = pool
            .alloc(vec![NDDimension::new(10000)], NDDataType::UInt8)
            .unwrap();
        let medium = pool
            .alloc(vec![NDDimension::new(1000)], NDDataType::UInt8)
            .unwrap();

        pool.release(large);
        pool.release(medium);
        pool.release(small);
        assert_eq!(pool.num_free_buffers(), 3);

        // Request 900 bytes — medium (1000 cap) is within 1.5x ratio
        let reused = pool
            .alloc(vec![NDDimension::new(900)], NDDataType::UInt8)
            .unwrap();
        assert!(reused.data.capacity_bytes() >= 900);
    }

    #[test]
    fn test_empty_free_list() {
        let pool = NDArrayPool::new(1_000_000);
        let a1 = pool
            .alloc(vec![NDDimension::new(100)], NDDataType::UInt8)
            .unwrap();
        let a2 = pool
            .alloc(vec![NDDimension::new(200)], NDDataType::UInt8)
            .unwrap();
        pool.release(a1);
        pool.release(a2);
        assert_eq!(pool.num_free_buffers(), 2);

        pool.empty_free_list();
        assert_eq!(pool.num_free_buffers(), 0);
        assert_eq!(pool.num_alloc_buffers(), 0);
    }

    #[test]
    fn test_num_free_buffers_tracking() {
        let pool = NDArrayPool::new(1_000_000);
        assert_eq!(pool.num_free_buffers(), 0);

        let a = pool
            .alloc(vec![NDDimension::new(10)], NDDataType::UInt8)
            .unwrap();
        assert_eq!(pool.num_free_buffers(), 0);

        pool.release(a);
        assert_eq!(pool.num_free_buffers(), 1);

        let _ = pool
            .alloc(vec![NDDimension::new(10)], NDDataType::UInt8)
            .unwrap();
        assert_eq!(pool.num_free_buffers(), 0);
    }

    #[test]
    fn test_concurrent_alloc_release() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(NDArrayPool::new(10_000_000));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let pool = pool.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let arr = pool
                        .alloc(vec![NDDimension::new(100)], NDDataType::UInt8)
                        .unwrap();
                    pool.release(arr);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All should be released back
        assert!(pool.num_free_buffers() > 0);
    }

    #[test]
    fn test_max_memory() {
        let pool = NDArrayPool::new(42);
        assert_eq!(pool.max_memory(), 42);
    }

    // --- convert_type tests ---

    #[test]
    fn test_convert_type_same_type() {
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(vec![NDDimension::new(4)], NDDataType::UInt8);
        if let NDDataBuffer::U8(ref mut v) = src.data {
            v[0] = 10;
            v[1] = 20;
            v[2] = 30;
            v[3] = 40;
        }

        let out = pool.convert_type(&src, NDDataType::UInt8).unwrap();
        assert_eq!(out.data.data_type(), NDDataType::UInt8);
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v, &[10, 20, 30, 40]);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_type_u8_to_f32() {
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(vec![NDDimension::new(3)], NDDataType::UInt8);
        if let NDDataBuffer::U8(ref mut v) = src.data {
            v[0] = 0;
            v[1] = 128;
            v[2] = 255;
        }

        let out = pool.convert_type(&src, NDDataType::Float32).unwrap();
        assert_eq!(out.data.data_type(), NDDataType::Float32);
        if let NDDataBuffer::F32(ref v) = out.data {
            assert_eq!(v[0], 0.0);
            assert_eq!(v[1], 128.0);
            assert_eq!(v[2], 255.0);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_type_u16_to_u8() {
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(vec![NDDimension::new(2)], NDDataType::UInt16);
        if let NDDataBuffer::U16(ref mut v) = src.data {
            v[0] = 100;
            v[1] = 300; // clamps to 255
        }

        let out = pool.convert_type(&src, NDDataType::UInt8).unwrap();
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v[0], 100);
            assert_eq!(v[1], 255); // clamped
        } else {
            panic!("wrong type");
        }
    }

    // --- convert tests ---

    /// Helper: create a 4x4 UInt8 array with values 0..15.
    fn make_4x4_u8() -> NDArray {
        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = arr.data {
            for i in 0..16 {
                v[i] = i as u8;
            }
        }
        arr
    }

    #[test]
    fn test_convert_identity() {
        // Identity conversion: no offset, no binning, no reverse
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        assert_eq!(out.dims[0].size, 4);
        assert_eq!(out.dims[1].size, 4);
        if let NDDataBuffer::U8(ref v) = out.data {
            for i in 0..16 {
                assert_eq!(v[i], i as u8);
            }
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_offset_extraction() {
        // Extract 2x2 sub-region starting at offset (1, 1)
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        let dims_out = vec![
            NDDimension {
                size: 2,
                offset: 1,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 2,
                offset: 1,
                binning: 1,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        assert_eq!(out.dims[0].size, 2);
        assert_eq!(out.dims[1].size, 2);
        // Source layout (row-major, dim0=x fastest):
        //   row0: [0,1,2,3], row1: [4,5,6,7], row2: [8,9,10,11], row3: [12,13,14,15]
        // offset (1,1) -> src[1+1*4]=5, src[2+1*4]=6, src[1+2*4]=9, src[2+2*4]=10
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v[0], 5);
            assert_eq!(v[1], 6);
            assert_eq!(v[2], 9);
            assert_eq!(v[3], 10);
        } else {
            panic!("wrong type");
        }

        // Verify cumulative offset tracking
        assert_eq!(out.dims[0].offset, 1); // src offset 0 + dims_out offset 1
        assert_eq!(out.dims[1].offset, 1);
    }

    #[test]
    fn test_convert_binning_2x2() {
        // 4x4 -> 2x2 with 2x2 binning (sum)
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 0,
                binning: 2,
                reverse: false,
            },
            NDDimension {
                size: 4,
                offset: 0,
                binning: 2,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        assert_eq!(out.dims[0].size, 2);
        assert_eq!(out.dims[1].size, 2);
        // top-left 2x2: sum = 0+1+4+5 = 10
        // top-right 2x2: sum = 2+3+6+7 = 18
        // bottom-left 2x2: sum = 8+9+12+13 = 42
        // bottom-right 2x2: sum = 10+11+14+15 = 50
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v[0], 10);
            assert_eq!(v[1], 18);
            assert_eq!(v[2], 42);
            assert_eq!(v[3], 50);
        } else {
            panic!("wrong type");
        }

        // Verify cumulative binning
        assert_eq!(out.dims[0].binning, 2); // src binning 1 * dims_out binning 2
        assert_eq!(out.dims[1].binning, 2);
    }

    #[test]
    fn test_convert_reverse_x() {
        // 4x1 with X-reverse
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(1)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = src.data {
            v[0] = 10;
            v[1] = 20;
            v[2] = 30;
            v[3] = 40;
        }

        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: true,
            },
            NDDimension {
                size: 1,
                offset: 0,
                binning: 1,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v[0], 40);
            assert_eq!(v[1], 30);
            assert_eq!(v[2], 20);
            assert_eq!(v[3], 10);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_reverse_y() {
        // 2x2 with Y-reverse
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(
            vec![NDDimension::new(2), NDDimension::new(2)],
            NDDataType::UInt16,
        );
        if let NDDataBuffer::U16(ref mut v) = src.data {
            // row0: [1, 2], row1: [3, 4]
            v[0] = 1;
            v[1] = 2;
            v[2] = 3;
            v[3] = 4;
        }

        let dims_out = vec![
            NDDimension {
                size: 2,
                offset: 0,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 2,
                offset: 0,
                binning: 1,
                reverse: true,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt16).unwrap();
        if let NDDataBuffer::U16(ref v) = out.data {
            // Y reversed: row0 now has row1 data, row1 has row0 data
            assert_eq!(v[0], 3);
            assert_eq!(v[1], 4);
            assert_eq!(v[2], 1);
            assert_eq!(v[3], 2);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_with_type_change() {
        // Convert 4x4 UInt8 -> Float32, with 2x2 binning
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 0,
                binning: 2,
                reverse: false,
            },
            NDDimension {
                size: 4,
                offset: 0,
                binning: 2,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::Float32).unwrap();
        assert_eq!(out.data.data_type(), NDDataType::Float32);
        assert_eq!(out.dims[0].size, 2);
        assert_eq!(out.dims[1].size, 2);
        if let NDDataBuffer::F32(ref v) = out.data {
            assert_eq!(v[0], 10.0); // 0+1+4+5
            assert_eq!(v[1], 18.0); // 2+3+6+7
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_cumulative_offset_and_binning() {
        // Source with existing offset=10, binning=2
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        src.dims[0].offset = 10;
        src.dims[0].binning = 2;
        src.dims[1].offset = 20;
        src.dims[1].binning = 3;
        if let NDDataBuffer::U8(ref mut v) = src.data {
            for i in 0..16 {
                v[i] = i as u8;
            }
        }

        let dims_out = vec![
            NDDimension {
                size: 2,
                offset: 1,
                binning: 2,
                reverse: false,
            },
            NDDimension {
                size: 2,
                offset: 1,
                binning: 2,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        // Cumulative offset: src.offset + dims_out.offset
        assert_eq!(out.dims[0].offset, 10 + 1);
        assert_eq!(out.dims[1].offset, 20 + 1);
        // Cumulative binning: src.binning * dims_out.binning
        assert_eq!(out.dims[0].binning, 2 * 2);
        assert_eq!(out.dims[1].binning, 3 * 2);
    }

    #[test]
    fn test_convert_1d() {
        // 1D: 8 elements, offset=2, size=4, binning=2 -> 2 output elements
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(vec![NDDimension::new(8)], NDDataType::UInt16);
        if let NDDataBuffer::U16(ref mut v) = src.data {
            for i in 0..8 {
                v[i] = (i * 10) as u16;
            }
            // [0, 10, 20, 30, 40, 50, 60, 70]
        }

        let dims_out = vec![NDDimension {
            size: 4,
            offset: 2,
            binning: 2,
            reverse: false,
        }];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt16).unwrap();
        assert_eq!(out.dims.len(), 1);
        assert_eq!(out.dims[0].size, 2);
        if let NDDataBuffer::U16(ref v) = out.data {
            // offset=2: src[2]=20, src[3]=30 -> sum=50
            // next: src[4]=40, src[5]=50 -> sum=90
            assert_eq!(v[0], 50);
            assert_eq!(v[1], 90);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_3d() {
        // 3D: 2x2x2 with identity dims -> should copy exactly
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(
            vec![
                NDDimension::new(2),
                NDDimension::new(2),
                NDDimension::new(2),
            ],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = src.data {
            for i in 0..8 {
                v[i] = (i + 1) as u8;
            }
        }

        let dims_out = vec![
            NDDimension {
                size: 2,
                offset: 0,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 2,
                offset: 0,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 2,
                offset: 0,
                binning: 1,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        if let NDDataBuffer::U8(ref v) = out.data {
            for i in 0..8 {
                assert_eq!(v[i], (i + 1) as u8);
            }
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_convert_dim_mismatch_error() {
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        // Wrong number of dims_out
        let dims_out = vec![NDDimension {
            size: 4,
            offset: 0,
            binning: 1,
            reverse: false,
        }];

        let result = pool.convert(&src, &dims_out, NDDataType::UInt8);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_offset_out_of_bounds_error() {
        let pool = NDArrayPool::new(1_000_000);
        let src = make_4x4_u8();
        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 2,
                binning: 1,
                reverse: false,
            }, // 2+4 > 4
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: false,
            },
        ];

        let result = pool.convert(&src, &dims_out, NDDataType::UInt8);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_preserves_metadata() {
        let pool = NDArrayPool::new(1_000_000);
        let mut src = make_4x4_u8();
        src.time_stamp = 12345.678;

        let dims_out = vec![
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: false,
            },
            NDDimension {
                size: 4,
                offset: 0,
                binning: 1,
                reverse: false,
            },
        ];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt8).unwrap();
        assert_eq!(out.time_stamp, 12345.678);
    }

    #[test]
    fn test_convert_binning_and_reverse_combined() {
        // 4x1, binning=2, reverse=true
        let pool = NDArrayPool::new(1_000_000);
        let mut src = NDArray::new(vec![NDDimension::new(4)], NDDataType::UInt16);
        if let NDDataBuffer::U16(ref mut v) = src.data {
            v[0] = 1;
            v[1] = 2;
            v[2] = 3;
            v[3] = 4;
        }

        let dims_out = vec![NDDimension {
            size: 4,
            offset: 0,
            binning: 2,
            reverse: true,
        }];

        let out = pool.convert(&src, &dims_out, NDDataType::UInt16).unwrap();
        assert_eq!(out.dims[0].size, 2);
        if let NDDataBuffer::U16(ref v) = out.data {
            // Without reverse: [1+2, 3+4] = [3, 7]
            // With reverse: output[0] reads from high end, output[1] from low end
            // eff_coords[0] for out_coord=0 with reverse => size-1-0 = 1 -> src[2..3] = 3+4 = 7
            // eff_coords[0] for out_coord=1 with reverse => size-1-1 = 0 -> src[0..1] = 1+2 = 3
            assert_eq!(v[0], 7);
            assert_eq!(v[1], 3);
        } else {
            panic!("wrong type");
        }
    }
}
