use asyn_rs::error::AsynResult;
use asyn_rs::param::ParamType;
use asyn_rs::port::PortDriverBase;
use asyn_rs::port_handle::PortHandle;

use ad_core_rs::driver::ImageMode;
use ad_core_rs::params::ADBaseParams;

/// MovingDot-specific parameter indices (beyond ADBase).
#[derive(Clone, Copy)]
pub struct MovingDotParams {
    pub motor_x_pos: usize,
    pub motor_y_pos: usize,
    pub beam_current: usize,
    pub shutter_open: usize,
}

impl MovingDotParams {
    pub fn create(base: &mut PortDriverBase) -> AsynResult<Self> {
        Ok(Self {
            motor_x_pos: base.create_param("DOT_MOTOR_X_POS", ParamType::Float64)?,
            motor_y_pos: base.create_param("DOT_MOTOR_Y_POS", ParamType::Float64)?,
            beam_current: base.create_param("DOT_BEAM_CURRENT", ParamType::Float64)?,
            shutter_open: base.create_param("DOT_SHUTTER_OPEN", ParamType::Int32)?,
        })
    }
}

/// Snapshot of all configuration needed for one frame computation.
pub struct MovingDotConfigSnapshot {
    pub motor_x: f64,
    pub motor_y: f64,
    pub beam_current: f64,
    pub shutter_open: bool,
    pub acquire_time: f64,
    pub acquire_period: f64,
    pub image_mode: ImageMode,
    pub num_images: i32,
    pub array_callbacks: bool,
    pub wait_for_plugins: bool,
    pub size_x: usize,
    pub size_y: usize,
}

impl MovingDotConfigSnapshot {
    /// Read config via PortHandle (async). For use from the acquisition task thread.
    pub async fn read_via_handle(
        handle: &PortHandle,
        ad: &ADBaseParams,
        dot: &MovingDotParams,
    ) -> AsynResult<Self> {
        Ok(Self {
            motor_x: handle.read_float64(dot.motor_x_pos, 0).await?,
            motor_y: handle.read_float64(dot.motor_y_pos, 0).await?,
            beam_current: handle.read_float64(dot.beam_current, 0).await?,
            shutter_open: handle.read_int32(dot.shutter_open, 0).await? != 0,
            acquire_time: handle.read_float64(ad.acquire_time, 0).await?,
            acquire_period: handle.read_float64(ad.acquire_period, 0).await?,
            image_mode: ImageMode::from_i32(handle.read_int32(ad.image_mode, 0).await?),
            num_images: handle.read_int32(ad.num_images, 0).await?,
            array_callbacks: handle.read_int32(ad.base.array_callbacks, 0).await? != 0,
            wait_for_plugins: handle
                .read_int32(ad.base.wait_for_plugins, 0)
                .await
                .unwrap_or(0)
                != 0,
            size_x: handle.read_int32(ad.size_x, 0).await? as usize,
            size_y: handle.read_int32(ad.size_y, 0).await? as usize,
        })
    }
}
