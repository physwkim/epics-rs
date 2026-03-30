use std::path::{Path, PathBuf};
use std::sync::Arc;

use ad_core_rs::error::{ADError, ADResult};
use ad_core_rs::ndarray::{NDArray, NDDataBuffer, NDDataType, NDDimension};
use ad_core_rs::ndarray_pool::NDArrayPool;
use ad_core_rs::plugin::file_base::{NDFileMode, NDFileWriter, NDPluginFileBase};
use ad_core_rs::plugin::runtime::{NDPluginProcess, ProcessResult};

use tiff::encoder::colortype;
use tiff::encoder::TiffEncoder;

/// TIFF file writer using the `tiff` crate for proper encoding/decoding.
pub struct TiffWriter {
    current_path: Option<PathBuf>,
}

impl TiffWriter {
    pub fn new() -> Self {
        Self { current_path: None }
    }
}

impl NDFileWriter for TiffWriter {
    fn open_file(&mut self, path: &Path, _mode: NDFileMode, _array: &NDArray) -> ADResult<()> {
        self.current_path = Some(path.to_path_buf());
        Ok(())
    }

    fn write_file(&mut self, array: &NDArray) -> ADResult<()> {
        let path = self.current_path.as_ref()
            .ok_or_else(|| ADError::UnsupportedConversion("no file open".into()))?;

        let info = array.info();
        let width = info.x_size as u32;
        let height = info.y_size as u32;

        let file = std::fs::File::create(path)?;
        let mut encoder = TiffEncoder::new(file)
            .map_err(|e| ADError::UnsupportedConversion(format!("TIFF encoder error: {}", e)))?;

        match &array.data {
            NDDataBuffer::U8(v) => {
                if info.color_size == 3 {
                    encoder.write_image::<colortype::RGB8>(width, height, v)
                } else {
                    encoder.write_image::<colortype::Gray8>(width, height, v)
                }
            }
            NDDataBuffer::U16(v) => {
                if info.color_size == 3 {
                    encoder.write_image::<colortype::RGB16>(width, height, v)
                } else {
                    encoder.write_image::<colortype::Gray16>(width, height, v)
                }
            }
            NDDataBuffer::I32(v) => {
                // No signed Gray32 in tiff crate; reinterpret as u32
                let u32_data: Vec<u32> = v.iter().map(|&x| x as u32).collect();
                encoder.write_image::<colortype::Gray32>(width, height, &u32_data)
            }
            NDDataBuffer::U32(v) => {
                encoder.write_image::<colortype::Gray32>(width, height, v)
            }
            NDDataBuffer::F32(_) => {
                // Write as raw gray bytes via U8 fallback
                let raw = array.data.as_u8_slice();
                let raw_width = (info.x_size * 4) as u32;
                encoder.write_image::<colortype::Gray8>(raw_width, height, raw)
            }
            NDDataBuffer::F64(_) => {
                let raw = array.data.as_u8_slice();
                let raw_width = (info.x_size * 8) as u32;
                encoder.write_image::<colortype::Gray8>(raw_width, height, raw)
            }
            _ => {
                // Fallback: write as raw bytes with Gray8
                let raw = array.data.as_u8_slice();
                encoder.write_image::<colortype::Gray8>(
                    raw.len() as u32 / height.max(1),
                    height,
                    raw,
                )
            }
        }
        .map_err(|e| ADError::UnsupportedConversion(format!("TIFF write error: {}", e)))?;

        Ok(())
    }

    fn read_file(&mut self) -> ADResult<NDArray> {
        use tiff::decoder::Decoder;

        let path = self.current_path.as_ref()
            .ok_or_else(|| ADError::UnsupportedConversion("no file open".into()))?;

        let file = std::fs::File::open(path)?;
        let mut decoder = Decoder::new(file)
            .map_err(|e| ADError::UnsupportedConversion(format!("TIFF decode error: {}", e)))?;

        let (width, height) = decoder.dimensions()
            .map_err(|e| ADError::UnsupportedConversion(format!("TIFF dimensions error: {}", e)))?;

        let result = decoder.read_image()
            .map_err(|e| ADError::UnsupportedConversion(format!("TIFF read error: {}", e)))?;

        match result {
            tiff::decoder::DecodingResult::U8(data) => {
                let dims = vec![
                    NDDimension::new(width as usize),
                    NDDimension::new(height as usize),
                ];
                let mut arr = NDArray::new(dims, NDDataType::UInt8);
                arr.data = NDDataBuffer::U8(data);
                Ok(arr)
            }
            tiff::decoder::DecodingResult::U16(data) => {
                let dims = vec![
                    NDDimension::new(width as usize),
                    NDDimension::new(height as usize),
                ];
                let mut arr = NDArray::new(dims, NDDataType::UInt16);
                arr.data = NDDataBuffer::U16(data);
                Ok(arr)
            }
            tiff::decoder::DecodingResult::U32(data) => {
                let dims = vec![
                    NDDimension::new(width as usize),
                    NDDimension::new(height as usize),
                ];
                let mut arr = NDArray::new(dims, NDDataType::UInt32);
                arr.data = NDDataBuffer::U32(data);
                Ok(arr)
            }
            _ => Err(ADError::UnsupportedConversion(
                "unsupported TIFF pixel format".into(),
            )),
        }
    }

    fn close_file(&mut self) -> ADResult<()> {
        self.current_path = None;
        Ok(())
    }

    fn supports_multiple_arrays(&self) -> bool {
        false
    }
}

/// TIFF file processor wrapping NDPluginFileBase + TiffWriter.
pub struct TiffFileProcessor {
    file_base: NDPluginFileBase,
    writer: TiffWriter,
}

impl TiffFileProcessor {
    pub fn new() -> Self {
        Self {
            file_base: NDPluginFileBase::new(),
            writer: TiffWriter::new(),
        }
    }

    pub fn file_base_mut(&mut self) -> &mut NDPluginFileBase {
        &mut self.file_base
    }
}

impl Default for TiffFileProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl NDPluginProcess for TiffFileProcessor {
    fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
        let _ = self
            .file_base
            .process_array(Arc::new(array.clone()), &mut self.writer);
        ProcessResult::empty() // file plugins are sinks
    }

    fn plugin_type(&self) -> &str {
        "NDFileTIFF"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ad_core_rs::ndarray::NDDataBuffer;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_path(prefix: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("adcore_test_{}_{}.tif", prefix, n))
    }

    #[test]
    fn test_write_u8_mono() {
        let path = temp_path("tiff_u8");
        let mut writer = TiffWriter::new();

        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(v) = &mut arr.data {
            for i in 0..16 { v[i] = i as u8; }
        }

        writer.open_file(&path, NDFileMode::Single, &arr).unwrap();
        writer.write_file(&arr).unwrap();
        writer.close_file().unwrap();

        // Verify file exists and has content
        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 16); // header + data
        // Check TIFF magic (little-endian II or big-endian MM)
        assert!(
            &data[0..2] == &[0x49, 0x49] || &data[0..2] == &[0x4D, 0x4D],
            "Expected TIFF magic bytes"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_write_u16() {
        let path = temp_path("tiff_u16");
        let mut writer = TiffWriter::new();

        let arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt16,
        );

        writer.open_file(&path, NDFileMode::Single, &arr).unwrap();
        writer.write_file(&arr).unwrap();
        writer.close_file().unwrap();

        let data = std::fs::read(&path).unwrap();
        assert!(data.len() > 32); // 16 elements * 2 bytes + header

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_roundtrip_u8() {
        let path = temp_path("tiff_rt_u8");
        let mut writer = TiffWriter::new();

        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(v) = &mut arr.data {
            for i in 0..16 { v[i] = (i * 10) as u8; }
        }

        writer.open_file(&path, NDFileMode::Single, &arr).unwrap();
        writer.write_file(&arr).unwrap();

        // Read it back
        let read_back = writer.read_file().unwrap();
        if let (NDDataBuffer::U8(orig), NDDataBuffer::U8(read)) =
            (&arr.data, &read_back.data)
        {
            assert_eq!(orig, read);
        } else {
            panic!("data type mismatch on roundtrip");
        }

        writer.close_file().unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_roundtrip_u16() {
        let path = temp_path("tiff_rt_u16");
        let mut writer = TiffWriter::new();

        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt16,
        );
        if let NDDataBuffer::U16(v) = &mut arr.data {
            for i in 0..16 { v[i] = (i * 1000) as u16; }
        }

        writer.open_file(&path, NDFileMode::Single, &arr).unwrap();
        writer.write_file(&arr).unwrap();

        let read_back = writer.read_file().unwrap();
        if let (NDDataBuffer::U16(orig), NDDataBuffer::U16(read)) =
            (&arr.data, &read_back.data)
        {
            assert_eq!(orig, read);
        } else {
            panic!("data type mismatch on roundtrip");
        }

        writer.close_file().unwrap();
        std::fs::remove_file(&path).ok();
    }
}
