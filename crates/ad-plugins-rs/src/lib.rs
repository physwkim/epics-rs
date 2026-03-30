#![allow(
    clippy::approx_constant,
    clippy::collapsible_if,
    clippy::erasing_op,
    clippy::identity_op,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::needless_range_loop,
    clippy::new_without_default,
    clippy::op_ref,
    clippy::type_complexity,
    clippy::too_many_arguments
)]

pub mod par_util;
pub mod std_arrays;
pub mod stats;
pub mod roi;
pub mod process;
pub mod transform;
pub mod color_convert;
pub mod overlay;
pub mod fft;
pub mod time_series;
pub mod circular_buff;
pub mod codec;
pub mod gather;
pub mod scatter;
pub mod file_tiff;
pub mod file_jpeg;
pub mod file_hdf5;
pub mod file_netcdf;
pub mod passthrough;
pub mod attribute;
pub mod roi_stat;
pub mod bad_pixel;
pub mod attr_plot;
pub mod pos_plugin;

#[cfg(feature = "ioc")]
pub mod ioc;
