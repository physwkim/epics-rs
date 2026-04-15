use std::sync::Arc;
use std::time::{Duration, Instant};

use asyn_rs::port_handle::PortHandle;

use ad_core_rs::driver::{ADStatus, ImageMode};
use ad_core_rs::ndarray::{NDArray, NDDataBuffer, NDDimension};
use ad_core_rs::params::ADBaseParams;
use ad_core_rs::plugin::channel::{NDArrayOutput, QueuedArrayCounter};

use crate::beamline_sim::{self, SimConfig};

use super::params::{XrtConfigSnapshot, XrtDetectorParams};
use super::types::DirtyFlags;

/// Commands sent from the driver to the acquisition task.
pub enum AcqCommand {
    Start,
    Stop,
}

/// Bundled state for the acquisition task thread.
pub(crate) struct AcquisitionContext {
    pub acq_rx: std::sync::mpsc::Receiver<AcqCommand>,
    pub port_handle: PortHandle,
    pub array_output: Arc<parking_lot::Mutex<NDArrayOutput>>,
    pub dirty: Arc<parking_lot::Mutex<DirtyFlags>>,
    pub ad: ADBaseParams,
    pub xrt: XrtDetectorParams,
    pub sim_config: SimConfig,
    pub queued_counter: Arc<QueuedArrayCounter>,
}

impl AcquisitionContext {
    fn end_acquisition(&self, wait_for_plugins: bool) {
        if wait_for_plugins {
            self.queued_counter.wait_until_zero(Duration::from_secs(5));
        }
        self.port_handle.set_params_and_notify(
            0,
            vec![
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: self.ad.acquire_busy,
                    addr: 0,
                    value: 0,
                },
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: self.ad.status,
                    addr: 0,
                    value: ADStatus::Idle as i32,
                },
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: self.ad.acquire,
                    addr: 0,
                    value: 0,
                },
            ],
        );
    }
}

fn wait_for_stop(acq_rx: &std::sync::mpsc::Receiver<AcqCommand>, duration: Duration) -> bool {
    if duration.as_millis() < 10 {
        let deadline = Instant::now() + duration;
        loop {
            match acq_rx.try_recv() {
                Ok(AcqCommand::Stop) => return true,
                Ok(AcqCommand::Start) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return true,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::hint::spin_loop();
        }
    } else {
        match acq_rx.recv_timeout(duration) {
            Ok(AcqCommand::Stop) => true,
            Ok(AcqCommand::Start) => false,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => false,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => true,
        }
    }
}

pub(crate) fn start_acquisition_task(ctx: AcquisitionContext) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("XrtSimTask".into())
        .spawn(move || acquisition_loop(ctx))
        .expect("failed to spawn XrtSimTask thread")
}

fn acquisition_loop(ctx: AcquisitionContext) {
    loop {
        // Wait for Start command
        match ctx.acq_rx.recv() {
            Ok(AcqCommand::Start) => {}
            Ok(AcqCommand::Stop) => continue,
            Err(_) => break,
        }

        // Initialize counters
        ctx.port_handle.set_params_and_notify(
            0,
            vec![
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: ctx.ad.num_images_counter,
                    addr: 0,
                    value: 0,
                },
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: ctx.ad.status,
                    addr: 0,
                    value: ADStatus::Acquire as i32,
                },
                asyn_rs::request::ParamSetValue::Int32 {
                    reason: ctx.ad.acquire_busy,
                    addr: 0,
                    value: 1,
                },
            ],
        );

        let mut num_counter = 0;
        let mut array_counter = ctx
            .port_handle
            .read_int32_blocking(ctx.ad.base.array_counter, 0)
            .unwrap_or(0);

        // Read initial config
        let mut config =
            match XrtConfigSnapshot::read_via_handle(&ctx.port_handle, &ctx.ad, &ctx.xrt) {
                Ok(cfg) => cfg,
                Err(_) => continue,
            };

        loop {
            let start_time = Instant::now();

            // Take dirty flags
            let dirty_flags = ctx.dirty.lock().take();
            if dirty_flags.any {
                if let Ok(cfg) =
                    XrtConfigSnapshot::read_via_handle(&ctx.port_handle, &ctx.ad, &ctx.xrt)
                {
                    config = cfg;
                }
            }

            // Run XRT simulation
            let sim_result = beamline_sim::simulate(&ctx.sim_config, &config.motors);

            // Extract statistics
            let [cx, cz] = sim_result.capture.centroid();
            let [rms_x, rms_z] = sim_result.capture.rms_size();
            let fwhm_x = sim_result.capture.fwhm_x().unwrap_or(0.0);
            let fwhm_z = sim_result.capture.fwhm_z().unwrap_or(0.0);
            let efficiency = sim_result.beamline_output.efficiency();
            let flux = sim_result.capture.total_intensity();
            let n_captured = sim_result.capture.n_captured;

            // Convert screen intensity to u16 image
            // Screen stores [ix * nz + iz] (x=row, z=col)
            // AreaDetector expects [iz * nx + ix] (z=row, x=col) for correct display
            let nx = ctx.sim_config.screen_nx;
            let nz = ctx.sim_config.screen_nz;
            let intensity = &sim_result.capture.intensity;

            let max_val = intensity.iter().cloned().fold(0.0_f64, f64::max);
            let scale = if max_val > 0.0 { 65000.0 / max_val } else { 0.0 };

            // Transpose: [ix*nz+iz] → [iz*nx+ix]
            let mut u16_data = vec![0u16; nx * nz];
            for ix in 0..nx {
                for iz in 0..nz {
                    let v = intensity[ix * nz + iz] * scale;
                    u16_data[iz * nx + ix] = v.round().clamp(0.0, 65535.0) as u16;
                }
            }

            let dims = vec![NDDimension::new(nx), NDDimension::new(nz)];
            let mut frame = NDArray {
                unique_id: 0,
                timestamp: ad_core_rs::timestamp::EpicsTimestamp::default(),
                time_stamp: 0.0,
                dims,
                data: NDDataBuffer::U16(u16_data),
                attributes: ad_core_rs::attributes::NDAttributeList::new(),
                codec: None,
            };

            // Wait for acquire_time
            let elapsed = start_time.elapsed().as_secs_f64();
            let delay = (config.acquire_time - elapsed).max(1e-5);
            if wait_for_stop(&ctx.acq_rx, Duration::from_secs_f64(delay)) {
                ctx.end_acquisition(config.wait_for_plugins);
                break;
            }

            // Update counters
            num_counter += 1;
            array_counter += 1;

            frame.unique_id = array_counter;
            frame.timestamp = ad_core_rs::timestamp::EpicsTimestamp::now();

            // Update simulation readback PVs + frame counters
            ctx.port_handle.set_params_and_notify(
                0,
                vec![
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.ad.base.array_counter,
                        addr: 0,
                        value: array_counter,
                    },
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.ad.num_images_counter,
                        addr: 0,
                        value: num_counter,
                    },
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.ad.base.array_size_x,
                        addr: 0,
                        value: nx as i32,
                    },
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.ad.base.array_size_y,
                        addr: 0,
                        value: nz as i32,
                    },
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.ad.base.array_size,
                        addr: 0,
                        value: (nx * nz * 2) as i32,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.ad.base.timestamp_rbv,
                        addr: 0,
                        value: frame.timestamp.as_f64(),
                    },
                    // Simulation readbacks
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_source_energy,
                        addr: 0,
                        value: sim_result.source_energy,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_dcm_energy,
                        addr: 0,
                        value: sim_result.dcm_energy,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_efficiency,
                        addr: 0,
                        value: efficiency * 100.0,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_flux,
                        addr: 0,
                        value: flux,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_centroid_x,
                        addr: 0,
                        value: cx,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_centroid_z,
                        addr: 0,
                        value: cz,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_fwhm_x,
                        addr: 0,
                        value: fwhm_x,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_fwhm_z,
                        addr: 0,
                        value: fwhm_z,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_rms_x,
                        addr: 0,
                        value: rms_x,
                    },
                    asyn_rs::request::ParamSetValue::Float64 {
                        reason: ctx.xrt.sim_rms_z,
                        addr: 0,
                        value: rms_z,
                    },
                    asyn_rs::request::ParamSetValue::Int32 {
                        reason: ctx.xrt.sim_nrays,
                        addr: 0,
                        value: n_captured as i32,
                    },
                ],
            );

            if config.array_callbacks {
                ctx.array_output.lock().publish(Arc::new(frame));
            }

            // Check stop conditions
            if config.image_mode == ImageMode::Single
                || (config.image_mode == ImageMode::Multiple && num_counter >= config.num_images)
            {
                ctx.end_acquisition(config.wait_for_plugins);
                break;
            }

            // Period delay
            let total_elapsed = start_time.elapsed().as_secs_f64();
            let period_delay = config.acquire_period - total_elapsed;
            if period_delay > 0.0 {
                if wait_for_stop(&ctx.acq_rx, Duration::from_secs_f64(period_delay)) {
                    ctx.end_acquisition(config.wait_for_plugins);
                    break;
                }
            }
        }
    }
}
