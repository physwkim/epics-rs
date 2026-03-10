use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::SeedableRng;

use asyn_rs::port_handle::PortHandle;

use ad_core::driver::{ADStatus, ImageMode};
use ad_core::ndarray::{NDArray, NDDataBuffer, NDDimension};
use ad_core::params::ADBaseParams;
use ad_core::plugin::channel::NDArrayOutput;

use crate::physics::{self, MovingDotImageConfig};

use super::params::{MovingDotConfigSnapshot, MovingDotParams};
use super::types::DirtyFlags;

const MIN_DELAY_SECS: f64 = 1e-5;

/// Commands sent from the driver to the acquisition task.
pub enum AcqCommand {
    Start,
    Stop,
}

/// Check if a Stop command has been received within the given duration.
fn wait_for_stop(acq_rx: &std::sync::mpsc::Receiver<AcqCommand>, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;

    if duration.as_millis() < 10 {
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

/// Start the acquisition task thread.
pub fn start_acquisition_task(
    acq_rx: std::sync::mpsc::Receiver<AcqCommand>,
    port_handle: PortHandle,
    array_output: Arc<parking_lot::Mutex<NDArrayOutput>>,
    dirty: Arc<parking_lot::Mutex<DirtyFlags>>,
    ad_params: ADBaseParams,
    dot_params: MovingDotParams,
    image_config: MovingDotImageConfig,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("MovingDotTask".into())
        .spawn(move || {
            acquisition_loop(acq_rx, port_handle, array_output, dirty, ad_params, dot_params, image_config);
        })
        .expect("failed to spawn MovingDotTask thread")
}

fn acquisition_loop(
    acq_rx: std::sync::mpsc::Receiver<AcqCommand>,
    port_handle: PortHandle,
    array_output: Arc<parking_lot::Mutex<NDArrayOutput>>,
    dirty: Arc<parking_lot::Mutex<DirtyFlags>>,
    ad: ADBaseParams,
    dot: MovingDotParams,
    image_config: MovingDotImageConfig,
) {
    let mut rng = StdRng::from_entropy();

    loop {
        // Wait for Start command
        match acq_rx.recv() {
            Ok(AcqCommand::Start) => {}
            Ok(AcqCommand::Stop) => continue,
            Err(_) => break,
        }

        // Initialize counters
        let _ = port_handle.write_int32_blocking(ad.num_images_counter, 0, 0);
        let _ = port_handle.write_int32_blocking(ad.status, 0, ADStatus::Acquire as i32);
        let _ = port_handle.write_int32_blocking(ad.acquire_busy, 0, 1);

        let mut num_counter = 0;
        let mut array_counter = port_handle.read_int32_blocking(ad.base.array_counter, 0).unwrap_or(0);

        // Read initial config
        let mut config = match MovingDotConfigSnapshot::read_via_handle(&port_handle, &ad, &dot) {
            Ok(cfg) => cfg,
            Err(_) => continue,
        };

        loop {
            let start_time = Instant::now();

            // Take dirty flags
            let dirty_flags = dirty.lock().take();
            let reset = dirty_flags.any;

            if reset {
                config = match MovingDotConfigSnapshot::read_via_handle(&port_handle, &ad, &dot) {
                    Ok(cfg) => cfg,
                    Err(_) => break,
                };
            }

            // Generate the image
            let width = config.size_x;
            let height = config.size_y;
            let img_data = physics::moving_dot_image_with_config(
                width,
                height,
                config.motor_x,
                config.motor_y,
                config.beam_current,
                config.acquire_time,
                config.shutter_open,
                &mut rng,
                &image_config,
            );

            // Build NDArray from f64 data
            let dims = vec![
                NDDimension::new(height),
                NDDimension::new(width),
            ];
            let mut frame = NDArray {
                unique_id: 0,
                timestamp: ad_core::timestamp::EpicsTimestamp::default(),
                dims,
                data: NDDataBuffer::F64(img_data),
                attributes: ad_core::attributes::NDAttributeList::new(),
                codec: None,
            };

            // Exposure time delay with stop interruption
            let elapsed = start_time.elapsed().as_secs_f64();
            let delay = (config.acquire_time - elapsed).max(MIN_DELAY_SECS);
            if wait_for_stop(&acq_rx, Duration::from_secs_f64(delay)) {
                let _ = port_handle.write_int32_blocking(ad.acquire_busy, 0, 0);
                let _ = port_handle.write_int32_blocking(ad.status, 0, ADStatus::Idle as i32);
                let _ = port_handle.write_int32_blocking(ad.acquire, 0, 0);
                let _ = port_handle.call_param_callbacks_blocking(0);
                break;
            }

            // Update counters
            num_counter += 1;
            array_counter += 1;

            frame.unique_id = array_counter;
            frame.timestamp = ad_core::timestamp::EpicsTimestamp::now();

            port_handle.write_int32_no_wait(ad.base.array_counter, 0, array_counter);
            port_handle.write_int32_no_wait(ad.num_images_counter, 0, num_counter);
            port_handle.write_float64_no_wait(ad.base.timestamp_rbv, 0, frame.timestamp.as_f64());
            port_handle.write_int32_no_wait(ad.base.epics_ts_sec, 0, frame.timestamp.sec as i32);
            port_handle.write_int32_no_wait(ad.base.epics_ts_nsec, 0, frame.timestamp.nsec as i32);
            let _ = port_handle.call_param_callbacks_blocking(0);

            if config.array_callbacks {
                let output = array_output.lock();
                if config.wait_for_plugins {
                    output.publish_and_wait(Arc::new(frame));
                } else {
                    output.publish(Arc::new(frame));
                }
            }

            // Check stop conditions
            if config.image_mode == ImageMode::Single
                || (config.image_mode == ImageMode::Multiple && num_counter >= config.num_images)
            {
                let _ = port_handle.write_int32_blocking(ad.acquire_busy, 0, 0);
                let _ = port_handle.write_int32_blocking(ad.status, 0, ADStatus::Idle as i32);
                let _ = port_handle.write_int32_blocking(ad.acquire, 0, 0);
                let _ = port_handle.call_param_callbacks_blocking(0);
                break;
            }

            // Period delay
            let total_elapsed = start_time.elapsed().as_secs_f64();
            let period_delay = config.acquire_period - total_elapsed;
            if period_delay > 0.0 {
                if wait_for_stop(&acq_rx, Duration::from_secs_f64(period_delay)) {
                    let _ = port_handle.write_int32_blocking(ad.acquire_busy, 0, 0);
                    let _ = port_handle.write_int32_blocking(ad.status, 0, ADStatus::Idle as i32);
                    let _ = port_handle.write_int32_blocking(ad.acquire, 0, 0);
                    let _ = port_handle.call_param_callbacks_blocking(0);
                    break;
                }
            }
        }
    }
}
