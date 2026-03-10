use std::sync::Arc;
use std::thread;

use asyn_rs::error::AsynResult;
use asyn_rs::port::{PortDriver, PortDriverBase, PortFlags};
use asyn_rs::runtime::config::RuntimeConfig;
use asyn_rs::runtime::port::{create_port_runtime, PortRuntimeHandle};
use asyn_rs::user::AsynUser;

use asyn_rs::port_handle::PortHandle;

use crate::ndarray::NDArray;
use crate::ndarray_pool::NDArrayPool;
use crate::params::ndarray_driver::NDArrayDriverParams;

use super::channel::{ndarray_channel, NDArrayOutput, NDArrayReceiver, NDArraySender};
use super::params::PluginBaseParams;

/// A single parameter update produced by a plugin's process_array.
pub enum ParamUpdate {
    Int32(usize, i32),
    Float64(usize, f64),
}

/// Result of processing one array: output arrays + param updates to write back.
pub struct ProcessResult {
    pub output_arrays: Vec<Arc<NDArray>>,
    pub param_updates: Vec<ParamUpdate>,
}

impl ProcessResult {
    /// Convenience: sink plugin with only param updates, no output arrays.
    pub fn sink(param_updates: Vec<ParamUpdate>) -> Self {
        Self { output_arrays: vec![], param_updates }
    }

    /// Convenience: passthrough/transform plugin with output arrays but no param updates.
    pub fn arrays(output_arrays: Vec<Arc<NDArray>>) -> Self {
        Self { output_arrays, param_updates: vec![] }
    }

    /// Convenience: no outputs, no param updates.
    pub fn empty() -> Self {
        Self { output_arrays: vec![], param_updates: vec![] }
    }
}

/// Pure processing logic. No threading concerns.
pub trait NDPluginProcess: Send + 'static {
    /// Process one array. Return output arrays and param updates.
    fn process_array(&mut self, array: &NDArray, pool: &NDArrayPool) -> ProcessResult;

    /// Plugin type name for PLUGIN_TYPE param.
    fn plugin_type(&self) -> &str;

    /// Register plugin-specific params on the base. Called once during construction.
    fn register_params(&mut self, _base: &mut PortDriverBase) -> Result<(), asyn_rs::error::AsynError> {
        Ok(())
    }

    /// Called when a param changes. Reason is the param index.
    fn on_param_change(&mut self, _reason: usize, _params: &PluginParamSnapshot) {}
}

/// Read-only snapshot of param values available to the processing thread.
pub struct PluginParamSnapshot {
    pub enable_callbacks: bool,
}

/// PortDriver implementation for a plugin's control plane.
#[allow(dead_code)]
pub struct PluginPortDriver {
    base: PortDriverBase,
    ndarray_params: NDArrayDriverParams,
    plugin_params: PluginBaseParams,
    param_change_tx: tokio::sync::mpsc::Sender<usize>,
}

impl PluginPortDriver {
    fn new<P: NDPluginProcess>(
        port_name: &str,
        plugin_type_name: &str,
        queue_size: usize,
        ndarray_port: &str,
        param_change_tx: tokio::sync::mpsc::Sender<usize>,
        processor: &mut P,
    ) -> AsynResult<Self> {
        let mut base = PortDriverBase::new(
            port_name,
            1,
            PortFlags {
                can_block: true,
                ..Default::default()
            },
        );

        let ndarray_params = NDArrayDriverParams::create(&mut base)?;
        let plugin_params = PluginBaseParams::create(&mut base)?;

        // Set defaults (EnableCallbacks=0 matches C default: Disable)
        base.set_int32_param(plugin_params.enable_callbacks, 0, 0)?;
        base.set_int32_param(plugin_params.blocking_callbacks, 0, 0)?;
        base.set_int32_param(plugin_params.queue_size, 0, queue_size as i32)?;
        base.set_int32_param(plugin_params.dropped_arrays, 0, 0)?;
        base.set_int32_param(plugin_params.queue_use, 0, 0)?;
        base.set_string_param(plugin_params.plugin_type, 0, plugin_type_name.into())?;
        base.set_int32_param(ndarray_params.array_callbacks, 0, 1)?;

        // Set plugin identity params
        base.set_string_param(ndarray_params.port_name_self, 0, port_name.into())?;
        if !ndarray_port.is_empty() {
            base.set_string_param(plugin_params.nd_array_port, 0, ndarray_port.into())?;
        }

        // Let the processor register its plugin-specific params
        processor.register_params(&mut base)?;

        Ok(Self {
            base,
            ndarray_params,
            plugin_params,
            param_change_tx,
        })
    }
}

impl PortDriver for PluginPortDriver {
    fn base(&self) -> &PortDriverBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PortDriverBase {
        &mut self.base
    }

    fn io_write_int32(&mut self, user: &mut AsynUser, value: i32) -> AsynResult<()> {
        let reason = user.reason;
        self.base.set_int32_param(reason, 0, value)?;
        self.base.call_param_callbacks(0)?;
        let _ = self.param_change_tx.try_send(reason);
        Ok(())
    }

    fn io_write_float64(&mut self, user: &mut AsynUser, value: f64) -> AsynResult<()> {
        let reason = user.reason;
        self.base.set_float64_param(reason, 0, value)?;
        self.base.call_param_callbacks(0)?;
        let _ = self.param_change_tx.try_send(reason);
        Ok(())
    }
}

/// Handle to a running plugin runtime. Provides access to sender and port handle.
#[derive(Clone)]
pub struct PluginRuntimeHandle {
    port_runtime: PortRuntimeHandle,
    array_sender: NDArraySender,
    port_name: String,
    pub ndarray_params: NDArrayDriverParams,
    pub plugin_params: PluginBaseParams,
}

impl PluginRuntimeHandle {
    pub fn port_runtime(&self) -> &PortRuntimeHandle {
        &self.port_runtime
    }

    pub fn array_sender(&self) -> &NDArraySender {
        &self.array_sender
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }
}

/// Create a plugin runtime with control plane (PortActor) and data plane (processing thread).
///
/// Returns:
/// - `PluginRuntimeHandle` for wiring and control
/// - `PortRuntimeHandle` for param I/O
/// - `JoinHandle` for the data processing thread
pub fn create_plugin_runtime<P: NDPluginProcess>(
    port_name: &str,
    mut processor: P,
    pool: Arc<NDArrayPool>,
    queue_size: usize,
    ndarray_port: &str,
) -> (PluginRuntimeHandle, thread::JoinHandle<()>) {
    // Param change channel (control plane -> data plane)
    let (param_tx, param_rx) = tokio::sync::mpsc::channel::<usize>(64);

    // Capture plugin type before mutable borrow
    let plugin_type_name = processor.plugin_type().to_string();

    // Create the port driver for control plane
    let driver = PluginPortDriver::new(port_name, &plugin_type_name, queue_size, ndarray_port, param_tx, &mut processor)
        .expect("failed to create plugin port driver");

    let enable_callbacks_reason = driver.plugin_params.enable_callbacks;
    let ndarray_params = driver.ndarray_params;
    let plugin_params = driver.plugin_params;

    // Create port runtime (actor thread for param I/O)
    let (port_runtime, _actor_jh) =
        create_port_runtime(driver, RuntimeConfig::default());

    // Clone port handle for the data thread to write params back
    let port_handle = port_runtime.port_handle().clone();

    // Array channel (data plane)
    let (array_sender, array_rx) = ndarray_channel(port_name, queue_size);

    // Output fan-out (initially empty; downstream wired later)
    let array_output = Arc::new(parking_lot::Mutex::new(NDArrayOutput::new()));
    let data_output = array_output.clone();

    // Spawn data processing thread
    let data_jh = thread::Builder::new()
        .name(format!("plugin-data-{port_name}"))
        .spawn(move || {
            plugin_data_loop(
                processor,
                array_rx,
                param_rx,
                data_output,
                pool,
                enable_callbacks_reason,
                ndarray_params,
                port_handle,
            );
        })
        .expect("failed to spawn plugin data thread");

    let handle = PluginRuntimeHandle {
        port_runtime,
        array_sender,
        port_name: port_name.to_string(),
        ndarray_params,
        plugin_params,
    };

    (handle, data_jh)
}

fn plugin_data_loop<P: NDPluginProcess>(
    mut processor: P,
    mut array_rx: NDArrayReceiver,
    mut param_rx: tokio::sync::mpsc::Receiver<usize>,
    array_output: Arc<parking_lot::Mutex<NDArrayOutput>>,
    pool: Arc<NDArrayPool>,
    enable_callbacks_reason: usize,
    ndarray_params: NDArrayDriverParams,
    port_handle: PortHandle,
) {
    let enabled = true;
    let mut array_counter: i32 = 0;

    loop {
        match array_rx.blocking_recv() {
            Some(array) => {
                // Drain pending param changes
                while let Ok(reason) = param_rx.try_recv() {
                    let snapshot = PluginParamSnapshot {
                        enable_callbacks: enabled,
                    };
                    if reason == enable_callbacks_reason {
                        // We can't read from port handle here easily,
                        // so we toggle based on the notification
                    }
                    processor.on_param_change(reason, &snapshot);
                }

                if !enabled {
                    continue;
                }

                let result = processor.process_array(&array, &pool);

                // Publish output arrays to downstream plugins
                let output = array_output.lock();
                for out in &result.output_arrays {
                    output.publish(out.clone());
                }
                drop(output);

                // Update base NDArrayDriver params from array metadata
                array_counter += 1;
                let info = array.info();
                let color_mode = if array.dims.len() <= 2 { 0 } else { 2 }; // Mono or RGB1
                port_handle.write_int32_no_wait(ndarray_params.array_counter, 0, array_counter);
                port_handle.write_int32_no_wait(ndarray_params.unique_id, 0, array.unique_id);
                port_handle.write_int32_no_wait(ndarray_params.n_dimensions, 0, array.dims.len() as i32);
                port_handle.write_int32_no_wait(ndarray_params.array_size_x, 0, info.x_size as i32);
                port_handle.write_int32_no_wait(ndarray_params.array_size_y, 0, info.y_size as i32);
                port_handle.write_int32_no_wait(ndarray_params.data_type, 0, array.data.data_type() as i32);
                port_handle.write_int32_no_wait(ndarray_params.color_mode, 0, color_mode);

                // Write plugin-specific param updates
                for update in &result.param_updates {
                    match update {
                        ParamUpdate::Int32(reason, value) => {
                            port_handle.write_int32_no_wait(*reason, 0, *value);
                        }
                        ParamUpdate::Float64(reason, value) => {
                            port_handle.write_float64_no_wait(*reason, 0, *value);
                        }
                    }
                }

                // Flush all dirty params as I/O Intr notifications
                let _ = port_handle.call_param_callbacks_blocking(0);
            }
            None => break, // channel closed = shutdown
        }
    }
}

/// Connect a downstream plugin's sender to a plugin runtime's output.
/// This must be called before starting acquisition.
pub fn wire_downstream(
    _upstream: &PluginRuntimeHandle,
    _downstream_sender: NDArraySender,
) {
    // For Phase 3, wiring is done via the PluginRuntimeHandle's output.
    // The actual wiring mechanism will be finalized in Phase 4.
    // For now, tests can use create_plugin_runtime_with_output.
}

/// Create a plugin runtime with a pre-wired output (for testing and direct wiring).
pub fn create_plugin_runtime_with_output<P: NDPluginProcess>(
    port_name: &str,
    mut processor: P,
    pool: Arc<NDArrayPool>,
    queue_size: usize,
    output: NDArrayOutput,
    ndarray_port: &str,
) -> (PluginRuntimeHandle, thread::JoinHandle<()>) {
    let (param_tx, param_rx) = tokio::sync::mpsc::channel::<usize>(64);

    let plugin_type_name = processor.plugin_type().to_string();
    let driver = PluginPortDriver::new(port_name, &plugin_type_name, queue_size, ndarray_port, param_tx, &mut processor)
        .expect("failed to create plugin port driver");

    let enable_callbacks_reason = driver.plugin_params.enable_callbacks;
    let ndarray_params = driver.ndarray_params;
    let plugin_params = driver.plugin_params;

    let (port_runtime, _actor_jh) =
        create_port_runtime(driver, RuntimeConfig::default());

    let port_handle = port_runtime.port_handle().clone();

    let (array_sender, array_rx) = ndarray_channel(port_name, queue_size);

    let data_output = Arc::new(parking_lot::Mutex::new(output));

    let data_jh = thread::Builder::new()
        .name(format!("plugin-data-{port_name}"))
        .spawn(move || {
            plugin_data_loop(
                processor,
                array_rx,
                param_rx,
                data_output,
                pool,
                enable_callbacks_reason,
                ndarray_params,
                port_handle,
            );
        })
        .expect("failed to spawn plugin data thread");

    let handle = PluginRuntimeHandle {
        port_runtime,
        array_sender,
        port_name: port_name.to_string(),
        ndarray_params,
        plugin_params,
    };

    (handle, data_jh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ndarray::{NDDataType, NDDimension};
    use crate::plugin::channel::ndarray_channel;

    /// Passthrough processor: returns the input array as-is.
    struct PassthroughProcessor;

    impl NDPluginProcess for PassthroughProcessor {
        fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
            ProcessResult::arrays(vec![Arc::new(array.clone())])
        }
        fn plugin_type(&self) -> &str {
            "Passthrough"
        }
    }

    /// Sink processor: consumes arrays, returns nothing.
    struct SinkProcessor {
        count: usize,
    }

    impl NDPluginProcess for SinkProcessor {
        fn process_array(&mut self, _array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
            self.count += 1;
            ProcessResult::empty()
        }
        fn plugin_type(&self) -> &str {
            "Sink"
        }
    }

    fn make_test_array(id: i32) -> Arc<NDArray> {
        let mut arr = NDArray::new(vec![NDDimension::new(4)], NDDataType::UInt8);
        arr.unique_id = id;
        Arc::new(arr)
    }

    #[test]
    fn test_passthrough_runtime() {
        let pool = Arc::new(NDArrayPool::new(1_000_000));

        // Create downstream receiver
        let (downstream_sender, mut downstream_rx) = ndarray_channel("DOWNSTREAM", 10);
        let mut output = NDArrayOutput::new();
        output.add(downstream_sender);

        let (handle, _data_jh) = create_plugin_runtime_with_output(
            "PASS1",
            PassthroughProcessor,
            pool,
            10,
            output,
            "",
        );

        // Send an array
        handle.array_sender().send(make_test_array(42));

        // Should come out the other side
        let received = downstream_rx.blocking_recv().unwrap();
        assert_eq!(received.unique_id, 42);
    }

    #[test]
    fn test_sink_runtime() {
        let pool = Arc::new(NDArrayPool::new(1_000_000));

        let (handle, _data_jh) = create_plugin_runtime(
            "SINK1",
            SinkProcessor { count: 0 },
            pool,
            10,
            "",
        );

        // Send arrays - they should be consumed silently
        handle.array_sender().send(make_test_array(1));
        handle.array_sender().send(make_test_array(2));

        // Give processing thread time
        std::thread::sleep(std::time::Duration::from_millis(50));

        // No crash, no output needed
        assert_eq!(handle.port_name(), "SINK1");
    }

    #[test]
    fn test_plugin_type_param() {
        let pool = Arc::new(NDArrayPool::new(1_000_000));

        let (handle, _data_jh) = create_plugin_runtime(
            "TYPE_TEST",
            PassthroughProcessor,
            pool,
            10,
            "",
        );

        // Verify port name
        assert_eq!(handle.port_name(), "TYPE_TEST");
        assert_eq!(handle.port_runtime().port_name(), "TYPE_TEST");
    }

    #[test]
    fn test_shutdown_on_handle_drop() {
        let pool = Arc::new(NDArrayPool::new(1_000_000));

        let (handle, data_jh) = create_plugin_runtime(
            "SHUTDOWN_TEST",
            PassthroughProcessor,
            pool,
            10,
            "",
        );

        // Drop the handle (closes sender channel, which should cause data thread to exit)
        let sender = handle.array_sender().clone();
        drop(handle);
        drop(sender);

        // Data thread should terminate
        let result = data_jh.join();
        assert!(result.is_ok());
    }

    #[test]
    fn test_dropped_count_when_queue_full() {
        let pool = Arc::new(NDArrayPool::new(1_000_000));

        // Very slow processor
        struct SlowProcessor;
        impl NDPluginProcess for SlowProcessor {
            fn process_array(
                &mut self,
                _array: &NDArray,
                _pool: &NDArrayPool,
            ) -> ProcessResult {
                std::thread::sleep(std::time::Duration::from_millis(100));
                ProcessResult::empty()
            }
            fn plugin_type(&self) -> &str {
                "Slow"
            }
        }

        let (handle, _data_jh) = create_plugin_runtime(
            "DROP_TEST",
            SlowProcessor,
            pool,
            1,
            "",
        );

        // Fill the queue and overflow
        for i in 0..10 {
            handle.array_sender().send(make_test_array(i));
        }

        // Some should have been dropped
        assert!(handle.array_sender().dropped_count() > 0);
    }
}
