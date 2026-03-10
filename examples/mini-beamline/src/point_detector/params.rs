use asyn_rs::error::AsynResult;
use asyn_rs::param::ParamType;
use asyn_rs::port::PortDriverBase;

/// Parameter indices for a PointDetector port driver.
#[derive(Clone, Copy)]
pub struct PointDetectorParams {
    pub motor_pos: usize,
    pub beam_current: usize,
    pub exposure_time: usize,
    pub det_value: usize,
    pub det_mode: usize,
    pub det_sigma: usize,
    pub det_center: usize,
}

impl PointDetectorParams {
    pub fn create(base: &mut PortDriverBase) -> AsynResult<Self> {
        Ok(Self {
            motor_pos: base.create_param("MOTOR_POS", ParamType::Float64)?,
            beam_current: base.create_param("BEAM_CURRENT", ParamType::Float64)?,
            exposure_time: base.create_param("EXPOSURE_TIME", ParamType::Float64)?,
            det_value: base.create_param("DET_VALUE", ParamType::Float64)?,
            det_mode: base.create_param("DET_MODE", ParamType::Int32)?,
            det_sigma: base.create_param("DET_SIGMA", ParamType::Float64)?,
            det_center: base.create_param("DET_CENTER", ParamType::Float64)?,
        })
    }
}
