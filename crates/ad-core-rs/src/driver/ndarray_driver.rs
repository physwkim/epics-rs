use std::path::Path;
use std::sync::Arc;

use asyn_rs::error::AsynResult;
use asyn_rs::port::{PortDriverBase, PortFlags};

use crate::ndarray::NDArray;
use crate::ndarray_pool::NDArrayPool;
use crate::params::ndarray_driver::NDArrayDriverParams;
use crate::plugin::channel::{NDArrayOutput, NDArraySender, QueuedArrayCounter};

/// Parse a C printf-style template with two `%s` and one `%d`-like specifier.
///
/// Handles format specifiers like `%s`, `%d`, `%3.3d`, `%04d`, `%06d`, etc.
/// The C++ original does: `epicsSnprintf(buf, max, template, path, name, number)`.
fn sprintf_template(template: &str, path: &str, name: &str, number: i32) -> String {
    let mut result = String::with_capacity(template.len() + path.len() + name.len() + 16);
    let mut chars = template.chars().peekable();
    let mut string_arg_idx = 0; // 0 = path, 1 = name

    while let Some(ch) = chars.next() {
        if ch == '%' {
            // Collect the format specifier
            let mut spec = String::new();
            // Collect flags, width, precision
            while let Some(&c) = chars.peek() {
                if c == 's' || c == 'd' || c == 'i' || c == 'o' || c == 'x' || c == 'X' {
                    break;
                }
                if c == '%' {
                    break;
                }
                spec.push(c);
                chars.next();
            }
            match chars.next() {
                Some('s') => {
                    let s = if string_arg_idx == 0 { path } else { name };
                    string_arg_idx += 1;
                    result.push_str(s);
                }
                Some('d') | Some('i') => {
                    // Parse width and precision from spec like "3.3", "04", "06"
                    let formatted = format_int_spec(&spec, number);
                    result.push_str(&formatted);
                }
                Some('%') => {
                    result.push('%');
                }
                Some(c) => {
                    result.push('%');
                    result.push_str(&spec);
                    result.push(c);
                }
                None => {
                    result.push('%');
                    result.push_str(&spec);
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Format an integer with a printf-style width/precision spec.
/// Handles specs like "", "3.3", "04", "06", "3", ".3", etc.
fn format_int_spec(spec: &str, value: i32) -> String {
    if spec.is_empty() {
        return value.to_string();
    }

    let zero_pad = spec.starts_with('0');
    let spec_clean = spec.trim_start_matches('0');

    // Split on '.' for width.precision
    let (width_str, prec_str) = if let Some(dot_pos) = spec_clean.find('.') {
        (&spec_clean[..dot_pos], Some(&spec_clean[dot_pos + 1..]))
    } else {
        (spec_clean, None)
    };

    let width: usize = width_str.parse().unwrap_or(0);
    let precision: usize = prec_str.and_then(|s| s.parse().ok()).unwrap_or(0);

    let min_digits = width.max(precision);
    if min_digits == 0 {
        return value.to_string();
    }

    if zero_pad || precision > 0 {
        format!("{:0>width$}", value, width = min_digits)
    } else {
        format!("{:>width$}", value, width = min_digits)
    }
}

/// Base state for asynNDArrayDriver (file handling, attribute mgmt, pool).
pub struct NDArrayDriverBase {
    pub port_base: PortDriverBase,
    pub params: NDArrayDriverParams,
    pub pool: Arc<NDArrayPool>,
    pub array_output: NDArrayOutput,
    pub queued_counter: Arc<QueuedArrayCounter>,
}

impl NDArrayDriverBase {
    pub fn new(port_name: &str, max_memory: usize) -> AsynResult<Self> {
        let mut port_base = PortDriverBase::new(
            port_name,
            1,
            PortFlags {
                can_block: true,
                ..Default::default()
            },
        );

        let params = NDArrayDriverParams::create(&mut port_base)?;

        port_base.set_int32_param(params.array_callbacks, 0, 1)?;
        port_base.set_float64_param(params.pool_max_memory, 0, max_memory as f64 / 1_048_576.0)?;

        let pool = Arc::new(NDArrayPool::new(max_memory));

        Ok(Self {
            port_base,
            params,
            pool,
            array_output: NDArrayOutput::new(),
            queued_counter: Arc::new(QueuedArrayCounter::new()),
        })
    }

    /// Connect a downstream channel-based receiver.
    pub fn connect_downstream(&mut self, mut sender: NDArraySender) {
        sender.set_queued_counter(self.queued_counter.clone());
        self.array_output.add(sender);
    }

    /// Number of connected downstream channels.
    pub fn num_plugins(&self) -> usize {
        self.array_output.num_senders()
    }

    /// Publish an array: update counters, push to plugins and channel outputs.
    pub fn publish_array(&mut self, array: Arc<NDArray>) -> AsynResult<()> {
        let counter = self
            .port_base
            .get_int32_param(self.params.array_counter, 0)?
            + 1;
        self.port_base
            .set_int32_param(self.params.array_counter, 0, counter)?;

        let info = array.info();
        self.port_base
            .set_int32_param(self.params.array_size_x, 0, info.x_size as i32)?;
        self.port_base
            .set_int32_param(self.params.array_size_y, 0, info.y_size as i32)?;
        self.port_base
            .set_int32_param(self.params.array_size_z, 0, info.color_size as i32)?;
        self.port_base
            .set_int32_param(self.params.array_size, 0, info.total_bytes as i32)?;
        self.port_base
            .set_int32_param(self.params.unique_id, 0, array.unique_id)?;

        // Update pool stats
        self.port_base.set_float64_param(
            self.params.pool_used_memory,
            0,
            self.pool.allocated_bytes() as f64 / 1_048_576.0,
        )?;
        self.port_base.set_int32_param(
            self.params.pool_free_buffers,
            0,
            self.pool.num_free_buffers() as i32,
        )?;
        self.port_base.set_int32_param(
            self.params.pool_alloc_buffers,
            0,
            self.pool.num_alloc_buffers() as i32,
        )?;

        let callbacks_enabled = self
            .port_base
            .get_int32_param(self.params.array_callbacks, 0)?
            != 0;

        if callbacks_enabled {
            self.port_base.set_generic_pointer_param(
                self.params.ndarray_data,
                0,
                array.clone() as Arc<dyn std::any::Any + Send + Sync>,
            )?;

            self.array_output.publish(array);
        }

        self.port_base.call_param_callbacks(0)?;

        Ok(())
    }

    /// Construct a file path from template, path, name, and number.
    ///
    /// Matches C++ `asynNDArrayDriver::createFileName` which uses
    /// `epicsSnprintf(fullFileName, maxChars, fileTemplate, filePath, fileName, fileNumber)`.
    /// The template is a C printf format string, e.g., `"%s%s_%3.3d.dat"`.
    pub fn create_file_name(&mut self) -> AsynResult<String> {
        let path = self.port_base.get_string_param(self.params.file_path, 0)?;
        let name = self.port_base.get_string_param(self.params.file_name, 0)?;
        let number = self.port_base.get_int32_param(self.params.file_number, 0)?;
        let template = self
            .port_base
            .get_string_param(self.params.file_template, 0)?;
        let auto_increment = self
            .port_base
            .get_int32_param(self.params.auto_increment, 0)
            .unwrap_or(0);

        let full = if template.is_empty() {
            format!("{}{}{:04}", path, name, number)
        } else {
            // Parse C printf-style template: two %s args (path, name) and one %d-like arg (number)
            sprintf_template(template, path, name, number)
        };

        self.port_base
            .set_string_param(self.params.full_file_name, 0, full.clone())?;

        // C++: auto-increment file number after creating filename
        if auto_increment != 0 {
            self.port_base
                .set_int32_param(self.params.file_number, 0, number + 1)?;
        }

        Ok(full)
    }

    /// Check if the file path directory exists.
    /// Normalizes the path to ensure it has a trailing '/'.
    pub fn check_path(&mut self) -> AsynResult<bool> {
        let path_ref = self.port_base.get_string_param(self.params.file_path, 0)?;
        let mut path = path_ref.to_string();
        // Ensure trailing separator (C++ checkPath does this)
        if !path.is_empty() && !path.ends_with('/') && !path.ends_with(std::path::MAIN_SEPARATOR) {
            path.push('/');
            self.port_base
                .set_string_param(self.params.file_path, 0, path.clone())?;
        }
        let exists = Path::new(&path).is_dir();
        self.port_base
            .set_int32_param(self.params.file_path_exists, 0, exists as i32)?;
        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::channel::ndarray_channel;

    #[test]
    fn test_new_sets_callbacks_enabled() {
        let drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        assert_eq!(
            drv.port_base
                .get_int32_param(drv.params.array_callbacks, 0)
                .unwrap(),
            1,
        );
    }

    #[test]
    fn test_publish_array() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        let arr = drv
            .pool
            .alloc(
                vec![
                    crate::ndarray::NDDimension::new(64),
                    crate::ndarray::NDDimension::new(64),
                ],
                crate::ndarray::NDDataType::UInt8,
            )
            .unwrap();
        drv.publish_array(Arc::new(arr)).unwrap();
        assert_eq!(
            drv.port_base
                .get_int32_param(drv.params.array_counter, 0)
                .unwrap(),
            1,
        );
    }

    #[test]
    fn test_publish_updates_size_info() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        let arr = drv
            .pool
            .alloc(
                vec![
                    crate::ndarray::NDDimension::new(320),
                    crate::ndarray::NDDimension::new(240),
                ],
                crate::ndarray::NDDataType::UInt16,
            )
            .unwrap();
        drv.publish_array(Arc::new(arr)).unwrap();
        assert_eq!(
            drv.port_base
                .get_int32_param(drv.params.array_size_x, 0)
                .unwrap(),
            320,
        );
        assert_eq!(
            drv.port_base
                .get_int32_param(drv.params.array_size_y, 0)
                .unwrap(),
            240,
        );
    }

    #[test]
    fn test_create_file_name_default() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        drv.port_base
            .set_string_param(drv.params.file_path, 0, "/tmp/".into())
            .unwrap();
        drv.port_base
            .set_string_param(drv.params.file_name, 0, "test_".into())
            .unwrap();
        drv.port_base
            .set_int32_param(drv.params.file_number, 0, 42)
            .unwrap();
        drv.port_base
            .set_string_param(drv.params.file_template, 0, "".into())
            .unwrap();

        let name = drv.create_file_name().unwrap();
        assert_eq!(name, "/tmp/test_0042");
    }

    #[test]
    fn test_check_path_exists() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        drv.port_base
            .set_string_param(drv.params.file_path, 0, "/tmp".into())
            .unwrap();
        assert!(drv.check_path().unwrap());
    }

    #[test]
    fn test_check_path_not_exists() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        drv.port_base
            .set_string_param(drv.params.file_path, 0, "/nonexistent_path_xyz".into())
            .unwrap();
        assert!(!drv.check_path().unwrap());
    }

    #[test]
    fn test_connect_downstream() {
        let mut drv = NDArrayDriverBase::new("TEST", 1_000_000).unwrap();
        let (sender, mut receiver) = ndarray_channel("DOWNSTREAM", 10);
        drv.connect_downstream(sender);
        assert_eq!(drv.num_plugins(), 1);

        let arr = drv
            .pool
            .alloc(
                vec![crate::ndarray::NDDimension::new(8)],
                crate::ndarray::NDDataType::UInt8,
            )
            .unwrap();
        let id = arr.unique_id;
        drv.publish_array(Arc::new(arr)).unwrap();

        let received = receiver.blocking_recv().unwrap();
        assert_eq!(received.unique_id, id);
    }
}
