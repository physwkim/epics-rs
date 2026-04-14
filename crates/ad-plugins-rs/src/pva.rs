//! NDPluginPva — serves the latest NDArray as an NTNDArray over pvAccess.
//!
//! Corresponds to C++ areaDetector's NDPluginPva.
//! Captures each incoming NDArray, converts it to `NtPayload::NdArray`,
//! and publishes it as a PV through the spvirit PVA server.

use std::sync::Arc;

use ad_core_rs::ndarray::{NDArray, NDDataBuffer};
use ad_core_rs::ndarray_pool::NDArrayPool;
use ad_core_rs::plugin::runtime::{NDPluginProcess, ProcessResult};
use parking_lot::Mutex;
use spvirit_types::{
    NdCodec, NdDimension, NtAlarm, NtAttribute, NtNdArray, NtPayload, NtTimeStamp,
    ScalarArrayValue, ScalarValue,
};
use tokio::sync::mpsc;

/// PVA plugin processor: captures the latest NDArray and converts to NtPayload.
pub struct PvaProcessor {
    latest: Arc<Mutex<Option<NtPayload>>>,
    subscribers: Arc<Mutex<Vec<mpsc::Sender<NtPayload>>>>,
}

impl PvaProcessor {
    pub fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Handle for accessing the latest NtPayload snapshot.
    pub fn latest_handle(&self) -> Arc<Mutex<Option<NtPayload>>> {
        self.latest.clone()
    }

    /// Handle for subscribing to updates.
    pub fn subscribers_handle(&self) -> Arc<Mutex<Vec<mpsc::Sender<NtPayload>>>> {
        self.subscribers.clone()
    }
}

impl Default for PvaProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl NDPluginProcess for PvaProcessor {
    fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
        let payload = ndarray_to_nt_payload(array);
        *self.latest.lock() = Some(payload.clone());

        // Notify subscribers, remove dead ones
        let mut subs = self.subscribers.lock();
        subs.retain(|tx| tx.try_send(payload.clone()).is_ok());

        // Pass through to downstream plugins
        ProcessResult::arrays(vec![Arc::new(array.clone())])
    }

    fn plugin_type(&self) -> &str {
        "NDPluginPva"
    }

    fn array_data_handle(&self) -> Option<Arc<Mutex<Option<Arc<NDArray>>>>> {
        None // We serve NtPayload, not raw NDArray
    }
}

// ---------------------------------------------------------------------------
// NDArray → NtPayload conversion
// ---------------------------------------------------------------------------

fn ndarray_to_nt_payload(array: &NDArray) -> NtPayload {
    let value = ndbuffer_to_scalar_array(&array.data);
    let element_size = array.data.data_type().element_size() as i64;
    let num_elements = array.data.len() as i64;
    let uncompressed_size = num_elements * element_size;

    let (compressed_size, codec) = match &array.codec {
        Some(c) => (
            c.compressed_size as i64,
            NdCodec {
                name: codec_name_to_string(c.name),
                parameters: Default::default(),
            },
        ),
        None => (
            uncompressed_size,
            NdCodec {
                name: String::new(),
                parameters: Default::default(),
            },
        ),
    };

    let dimension: Vec<NdDimension> = array
        .dims
        .iter()
        .map(|d| NdDimension {
            size: d.size as i32,
            offset: d.offset as i32,
            full_size: d.size as i32,
            binning: d.binning.max(1) as i32,
            reverse: d.reverse,
        })
        .collect();

    let data_time_stamp = epics_ts_to_nt(&array.timestamp);

    let attribute: Vec<NtAttribute> = array
        .attributes
        .iter()
        .map(|a| NtAttribute {
            name: a.name.clone(),
            value: attribute_value_to_scalar(&a.value),
            descriptor: a.description.clone(),
            source_type: ndattr_source_type(&a.source),
            source: ndattr_source_string(&a.source),
        })
        .collect();

    NtPayload::NdArray(NtNdArray {
        value,
        codec,
        compressed_size,
        uncompressed_size,
        dimension,
        unique_id: array.unique_id,
        data_time_stamp,
        attribute,
        descriptor: None,
        alarm: Some(NtAlarm {
            severity: 0,
            status: 0,
            message: "NO_ALARM".into(),
        }),
        time_stamp: Some(epics_ts_to_nt(&array.timestamp)),
        display: None,
    })
}

fn ndbuffer_to_scalar_array(buf: &NDDataBuffer) -> ScalarArrayValue {
    match buf {
        NDDataBuffer::I8(v) => ScalarArrayValue::I8(v.clone()),
        NDDataBuffer::U8(v) => ScalarArrayValue::U8(v.clone()),
        NDDataBuffer::I16(v) => ScalarArrayValue::I16(v.clone()),
        NDDataBuffer::U16(v) => ScalarArrayValue::U16(v.clone()),
        NDDataBuffer::I32(v) => ScalarArrayValue::I32(v.clone()),
        NDDataBuffer::U32(v) => ScalarArrayValue::U32(v.clone()),
        NDDataBuffer::I64(v) => ScalarArrayValue::I64(v.clone()),
        NDDataBuffer::U64(v) => ScalarArrayValue::U64(v.clone()),
        NDDataBuffer::F32(v) => ScalarArrayValue::F32(v.clone()),
        NDDataBuffer::F64(v) => ScalarArrayValue::F64(v.clone()),
    }
}

fn epics_ts_to_nt(ts: &ad_core_rs::timestamp::EpicsTimestamp) -> NtTimeStamp {
    NtTimeStamp {
        seconds_past_epoch: ts.sec as i64,
        nanoseconds: ts.nsec as i32,
        user_tag: 0,
    }
}

fn codec_name_to_string(name: ad_core_rs::codec::CodecName) -> String {
    use ad_core_rs::codec::CodecName;
    match name {
        CodecName::None => String::new(),
        CodecName::JPEG => "jpeg".into(),
        CodecName::LZ4 => "lz4".into(),
        CodecName::Blosc => "blosc".into(),
        CodecName::BSLZ4 => "bslz4".into(),
    }
}

fn ndattr_source_type(src: &ad_core_rs::attributes::NDAttrSource) -> i32 {
    use ad_core_rs::attributes::NDAttrSource;
    match src {
        NDAttrSource::Driver => 0,
        NDAttrSource::Param { .. } => 1,
        NDAttrSource::EpicsPV => 2,
        NDAttrSource::Function => 3,
        NDAttrSource::Constant => 4,
        NDAttrSource::Undefined => -1,
    }
}

fn ndattr_source_string(src: &ad_core_rs::attributes::NDAttrSource) -> String {
    use ad_core_rs::attributes::NDAttrSource;
    match src {
        NDAttrSource::Driver => "driver".into(),
        NDAttrSource::Param {
            port_name,
            param_name,
        } => format!("{port_name}.{param_name}"),
        NDAttrSource::EpicsPV => "epics".into(),
        NDAttrSource::Function => "function".into(),
        NDAttrSource::Constant => "constant".into(),
        NDAttrSource::Undefined => String::new(),
    }
}

fn attribute_value_to_scalar(val: &ad_core_rs::attributes::NDAttrValue) -> ScalarValue {
    use ad_core_rs::attributes::NDAttrValue;
    match val {
        NDAttrValue::Int8(v) => ScalarValue::I8(*v),
        NDAttrValue::UInt8(v) => ScalarValue::U8(*v),
        NDAttrValue::Int16(v) => ScalarValue::I16(*v),
        NDAttrValue::UInt16(v) => ScalarValue::U16(*v),
        NDAttrValue::Int32(v) => ScalarValue::I32(*v),
        NDAttrValue::UInt32(v) => ScalarValue::U32(*v),
        NDAttrValue::Int64(v) => ScalarValue::I64(*v),
        NDAttrValue::UInt64(v) => ScalarValue::U64(*v),
        NDAttrValue::Float32(v) => ScalarValue::F32(*v),
        NDAttrValue::Float64(v) => ScalarValue::F64(*v),
        NDAttrValue::String(v) => ScalarValue::Str(v.clone()),
        NDAttrValue::Undefined => ScalarValue::I32(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ad_core_rs::ndarray::{NDDataType, NDDimension};

    #[test]
    fn convert_simple_array() {
        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        arr.unique_id = 42;
        if let NDDataBuffer::U8(ref mut buf) = arr.data {
            for (i, v) in buf.iter_mut().enumerate() {
                *v = i as u8;
            }
        }

        let payload = ndarray_to_nt_payload(&arr);
        match &payload {
            NtPayload::NdArray(nt) => {
                assert_eq!(nt.unique_id, 42);
                assert_eq!(nt.dimension.len(), 2);
                assert_eq!(nt.dimension[0].size, 4);
                assert_eq!(nt.dimension[1].size, 4);
                assert_eq!(nt.uncompressed_size, 16);
                if let ScalarArrayValue::U8(data) = &nt.value {
                    assert_eq!(data.len(), 16);
                    assert_eq!(data[0], 0);
                    assert_eq!(data[15], 15);
                } else {
                    panic!("expected U8 array");
                }
            }
            _ => panic!("expected NdArray payload"),
        }
    }

    #[test]
    fn processor_stores_latest() {
        let mut proc = PvaProcessor::new();
        let pool = NDArrayPool::new(1_000_000);
        let arr = NDArray::new(vec![NDDimension::new(8)], NDDataType::Float64);

        proc.process_array(&arr, &pool);

        let latest = proc.latest_handle().lock().clone();
        assert!(latest.is_some());
        if let Some(NtPayload::NdArray(nt)) = latest {
            assert_eq!(nt.dimension[0].size, 8);
        } else {
            panic!("expected NdArray");
        }
    }
}
