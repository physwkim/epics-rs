#![allow(
    clippy::approx_constant,
    clippy::collapsible_if,
    clippy::manual_is_multiple_of,
    clippy::needless_range_loop,
    clippy::new_without_default,
    clippy::too_many_arguments
)]

pub mod error;
pub mod timestamp;
pub mod ndarray;
pub mod attributes;
pub mod ndarray_handle;
pub mod ndarray_pool;
pub mod codec;
pub mod color;
pub mod params;
pub mod driver;
pub mod plugin;
pub mod pixel_cast;
pub mod color_layout;
pub mod roi;

#[cfg(feature = "ioc")]
pub mod ioc;
