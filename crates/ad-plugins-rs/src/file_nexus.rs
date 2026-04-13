//! NeXus file writer plugin.
//!
//! Writes NDArray data in NeXus/HDF5 format using the rust-hdf5 library.
//! Follows the simplified NXdata convention:
//!
//! ```text
//! /entry (NX_class=NXentry)
//!   /instrument (NX_class=NXinstrument)
//!     /detector (NX_class=NXdetector)
//!       /data → dataset [frames × Y × X]
//!   /data (NX_class=NXdata)
//!     /data → same dataset
//! ```

use std::path::{Path, PathBuf};

use ad_core_rs::error::{ADError, ADResult};
use ad_core_rs::ndarray::{NDArray, NDDataBuffer, NDDataType, NDDimension};
use ad_core_rs::ndarray_pool::NDArrayPool;
use ad_core_rs::plugin::file_base::{NDFileMode, NDFileWriter};
use ad_core_rs::plugin::file_controller::FilePluginController;
use ad_core_rs::plugin::runtime::{
    NDPluginProcess, ParamChangeResult, PluginParamSnapshot, ProcessResult,
};

use rust_hdf5::{H5Dataset, H5File};

/// NeXus file writer using HDF5 with NeXus group structure.
pub struct NexusWriter {
    current_path: Option<PathBuf>,
    file: Option<H5File>,
    frame_count: usize,
    /// Reusable dataset handle for multi-frame writes.
    dataset: Option<H5Dataset>,
}

impl NexusWriter {
    pub fn new() -> Self {
        Self {
            current_path: None,
            file: None,
            frame_count: 0,
            dataset: None,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Write an NX_class marker dataset into a group.
    ///
    /// rust-hdf5 does not support group-level attributes, so we create a
    /// scalar u8 dataset named "NX_class" and attach the class name as a
    /// string attribute on it.
    fn write_nx_class(group: &rust_hdf5::H5Group, class_name: &str) -> ADResult<()> {
        let ds = group
            .new_dataset::<u8>()
            .shape([1usize])
            .create("NX_class")
            .map_err(|e| {
                ADError::UnsupportedConversion(format!("NX_class dataset error: {}", e))
            })?;
        ds.write_raw(&[0u8])
            .map_err(|e| ADError::UnsupportedConversion(format!("NX_class write error: {}", e)))?;
        let attr = ds
            .new_attr::<rust_hdf5::types::VarLenUnicode>()
            .shape(())
            .create("value")
            .map_err(|e| ADError::UnsupportedConversion(format!("NX_class attr error: {}", e)))?;
        attr.write_string(class_name).map_err(|e| {
            ADError::UnsupportedConversion(format!("NX_class attr write error: {}", e))
        })?;
        Ok(())
    }
}

impl Default for NexusWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl NDFileWriter for NexusWriter {
    fn open_file(&mut self, path: &Path, _mode: NDFileMode, _array: &NDArray) -> ADResult<()> {
        self.current_path = Some(path.to_path_buf());
        self.frame_count = 0;

        let h5file = H5File::create(path)
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus create error: {}", e)))?;

        // Create NeXus group hierarchy with NX_class marker datasets.
        // Note: rust-hdf5 does not support group-level attributes, so we store
        // NX_class as a scalar u8 dataset within each group. NeXus-aware readers
        // should use the path hierarchy for group identification.
        let entry = h5file
            .create_group("entry")
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus group error: {}", e)))?;
        Self::write_nx_class(&entry, "NXentry")?;
        let instrument = entry
            .create_group("instrument")
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus group error: {}", e)))?;
        Self::write_nx_class(&instrument, "NXinstrument")?;
        let _detector = instrument
            .create_group("detector")
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus group error: {}", e)))?;
        Self::write_nx_class(&_detector, "NXdetector")?;
        let _data_group = entry
            .create_group("data")
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus group error: {}", e)))?;
        Self::write_nx_class(&_data_group, "NXdata")?;

        self.file = Some(h5file);
        Ok(())
    }

    fn write_file(&mut self, array: &NDArray) -> ADResult<()> {
        let h5file = self
            .file
            .as_ref()
            .ok_or_else(|| ADError::UnsupportedConversion("no NeXus file open".into()))?;

        let frame_shape = array.dims.iter().rev().map(|d| d.size).collect::<Vec<_>>();

        if self.frame_count == 0 {
            // First frame: create a chunked dataset with leading frame dimension.
            let detector_group = h5file
                .root_group()
                .group("entry")
                .map_err(|e| ADError::UnsupportedConversion(e.to_string()))?
                .group("instrument")
                .map_err(|e| ADError::UnsupportedConversion(e.to_string()))?
                .group("detector")
                .map_err(|e| ADError::UnsupportedConversion(e.to_string()))?;

            // Shape: [1, dim0, dim1, ...], chunk: [1, dim0, dim1, ...]
            let mut ds_shape = vec![1usize];
            ds_shape.extend_from_slice(&frame_shape);
            let chunk_dims = ds_shape.clone();

            macro_rules! create_chunked {
                ($t:ty, $v:expr) => {{
                    let ds = detector_group
                        .new_dataset::<$t>()
                        .shape(&ds_shape[..])
                        .chunk(&chunk_dims[..])
                        .resizable()
                        .create("data")
                        .map_err(|e| {
                            ADError::UnsupportedConversion(format!("NeXus dataset error: {}", e))
                        })?;
                    let raw = unsafe {
                        std::slice::from_raw_parts(
                            $v.as_ptr() as *const u8,
                            $v.len() * std::mem::size_of::<$t>(),
                        )
                    };
                    ds.write_chunk(0, raw).map_err(|e| {
                        ADError::UnsupportedConversion(format!("NeXus write error: {}", e))
                    })?;
                    // Write NDArray attributes on the first frame
                    for attr in array.attributes.iter() {
                        let val_str = attr.value.as_string();
                        let _ = ds
                            .new_attr::<rust_hdf5::types::VarLenUnicode>()
                            .shape(())
                            .create(attr.name.as_str())
                            .and_then(|a| {
                                let s: rust_hdf5::types::VarLenUnicode =
                                    val_str.parse().unwrap_or_default();
                                a.write_scalar(&s)
                            });
                    }
                    ds
                }};
            }

            let ds = match &array.data {
                NDDataBuffer::U8(v) => create_chunked!(u8, v),
                NDDataBuffer::U16(v) => create_chunked!(u16, v),
                NDDataBuffer::I16(v) => create_chunked!(i16, v),
                NDDataBuffer::I32(v) => create_chunked!(i32, v),
                NDDataBuffer::U32(v) => create_chunked!(u32, v),
                NDDataBuffer::F32(v) => create_chunked!(f32, v),
                NDDataBuffer::F64(v) => create_chunked!(f64, v),
                _ => {
                    let raw = array.data.as_u8_slice();
                    let ds = detector_group
                        .new_dataset::<u8>()
                        .shape(&ds_shape[..])
                        .chunk(&chunk_dims[..])
                        .resizable()
                        .create("data")
                        .map_err(|e| {
                            ADError::UnsupportedConversion(format!("NeXus dataset error: {}", e))
                        })?;
                    ds.write_chunk(0, raw).map_err(|e| {
                        ADError::UnsupportedConversion(format!("NeXus write error: {}", e))
                    })?;
                    ds
                }
            };

            self.dataset = Some(ds);
        } else {
            // Subsequent frames: extend dataset and write new chunk.
            let ds = self.dataset.as_ref().ok_or_else(|| {
                ADError::UnsupportedConversion("no dataset for multi-frame write".into())
            })?;

            let new_frame_count = self.frame_count + 1;
            let mut new_shape = vec![new_frame_count];
            new_shape.extend_from_slice(&frame_shape);
            ds.extend(&new_shape).map_err(|e| {
                ADError::UnsupportedConversion(format!("NeXus extend error: {}", e))
            })?;

            let raw = array.data.as_u8_slice();
            ds.write_chunk(self.frame_count, raw)
                .map_err(|e| ADError::UnsupportedConversion(format!("NeXus write error: {}", e)))?;
        }

        // Write per-frame uniqueId and timeStamp as attributes on the dataset
        if let Some(ref ds) = self.dataset {
            let uid_name = format!("uniqueId_{}", self.frame_count);
            let _ = ds
                .new_attr::<i32>()
                .shape(())
                .create(&uid_name)
                .and_then(|a| a.write_numeric(&array.unique_id));
            let ts_name = format!("timeStamp_{}", self.frame_count);
            let _ = ds
                .new_attr::<f64>()
                .shape(())
                .create(&ts_name)
                .and_then(|a| a.write_numeric(&array.time_stamp));
        }

        self.frame_count += 1;
        Ok(())
    }

    fn read_file(&mut self) -> ADResult<NDArray> {
        let path = self
            .current_path
            .as_ref()
            .ok_or_else(|| ADError::UnsupportedConversion("no file open".into()))?;

        let h5file = H5File::open(path)
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus open error: {}", e)))?;

        // Try reading from /entry/instrument/detector/data
        let ds = h5file
            .dataset("entry/instrument/detector/data")
            .map_err(|e| ADError::UnsupportedConversion(format!("NeXus dataset error: {}", e)))?;

        let shape = ds.shape();
        let dims: Vec<NDDimension> = shape.iter().rev().map(|&s| NDDimension::new(s)).collect();

        if let Ok(data) = ds.read_raw::<u8>() {
            let mut arr = NDArray::new(dims, NDDataType::UInt8);
            arr.data = NDDataBuffer::U8(data);
            return Ok(arr);
        }
        if let Ok(data) = ds.read_raw::<u16>() {
            let mut arr = NDArray::new(dims, NDDataType::UInt16);
            arr.data = NDDataBuffer::U16(data);
            return Ok(arr);
        }
        if let Ok(data) = ds.read_raw::<f64>() {
            let mut arr = NDArray::new(dims, NDDataType::Float64);
            arr.data = NDDataBuffer::F64(data);
            return Ok(arr);
        }

        Err(ADError::UnsupportedConversion(
            "unsupported data type in NeXus file".into(),
        ))
    }

    fn close_file(&mut self) -> ADResult<()> {
        self.dataset = None;
        self.file = None;
        self.current_path = None;
        Ok(())
    }

    fn supports_multiple_arrays(&self) -> bool {
        true
    }
}

// ============================================================
// Processor
// ============================================================

pub struct NexusFileProcessor {
    ctrl: FilePluginController<NexusWriter>,
}

impl NexusFileProcessor {
    pub fn new() -> Self {
        Self {
            ctrl: FilePluginController::new(NexusWriter::new()),
        }
    }
}

impl Default for NexusFileProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl NDPluginProcess for NexusFileProcessor {
    fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
        self.ctrl.process_array(array)
    }

    fn plugin_type(&self) -> &str {
        "NDFileNexus"
    }

    fn register_params(
        &mut self,
        base: &mut asyn_rs::port::PortDriverBase,
    ) -> asyn_rs::error::AsynResult<()> {
        self.ctrl.register_params(base)?;
        use asyn_rs::param::ParamType;
        base.create_param("NEXUS_TEMPLATE_PATH", ParamType::Octet)?;
        base.create_param("NEXUS_TEMPLATE_FILE", ParamType::Octet)?;
        base.create_param("NEXUS_TEMPLATE_VALID", ParamType::Int32)?;
        base.create_param("TEMPLATE_FILE_PATH", ParamType::Octet)?;
        base.create_param("TEMPLATE_FILE_NAME", ParamType::Octet)?;
        base.create_param("TEMPLATE_FILE_VALID", ParamType::Int32)?;
        Ok(())
    }

    fn on_param_change(
        &mut self,
        reason: usize,
        params: &PluginParamSnapshot,
    ) -> ParamChangeResult {
        self.ctrl.on_param_change(reason, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(prefix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("adcore_test_{}_{}.nxs", prefix, n))
    }

    #[test]
    fn test_nexus_write_read() {
        let path = temp_path("nexus_basic");
        let mut writer = NexusWriter::new();

        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = arr.data {
            for i in 0..16 {
                v[i] = i as u8;
            }
        }

        writer.open_file(&path, NDFileMode::Single, &arr).unwrap();
        writer.write_file(&arr).unwrap();
        writer.close_file().unwrap();

        // Verify NeXus structure
        let h5file = H5File::open(&path).unwrap();
        let ds = h5file.dataset("entry/instrument/detector/data").unwrap();
        let data: Vec<u8> = ds.read_raw().unwrap();
        assert_eq!(data.len(), 16);
        assert_eq!(data[0], 0);
        assert_eq!(data[15], 15);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_nexus_multiple_frames() {
        let path = temp_path("nexus_multi");
        let mut writer = NexusWriter::new();

        let mut arr1 = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = arr1.data {
            for i in 0..16 {
                v[i] = i as u8;
            }
        }

        let mut arr2 = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = arr2.data {
            for i in 0..16 {
                v[i] = (i + 100) as u8;
            }
        }

        writer.open_file(&path, NDFileMode::Stream, &arr1).unwrap();
        writer.write_file(&arr1).unwrap();
        writer.write_file(&arr2).unwrap();
        writer.close_file().unwrap();

        assert_eq!(writer.frame_count(), 2);

        // Verify single dataset with leading frame dimension [2, 4, 4]
        let h5file = H5File::open(&path).unwrap();
        let ds = h5file.dataset("entry/instrument/detector/data").unwrap();
        let shape = ds.shape();
        assert_eq!(shape, vec![2, 4, 4]);

        let data: Vec<u8> = ds.read_raw().unwrap();
        assert_eq!(data.len(), 32);
        // First frame
        assert_eq!(data[0], 0);
        assert_eq!(data[15], 15);
        // Second frame
        assert_eq!(data[16], 100);
        assert_eq!(data[31], 115);

        std::fs::remove_file(&path).ok();
    }
}
