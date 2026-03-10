//! NDPluginROIStat: computes basic statistics for multiple ROI regions on each array.
//!
//! Each ROI is a rectangular sub-region of a 2D image. For each enabled ROI,
//! the plugin computes min, max, mean, total, and net (background-subtracted total).
//! Optionally accumulates time series data in circular buffers.

use ad_core::ndarray::{NDArray, NDDataBuffer};
use ad_core::ndarray_pool::NDArrayPool;
use ad_core::plugin::runtime::{NDPluginProcess, ProcessResult};

/// Configuration for a single ROI region.
#[derive(Debug, Clone)]
pub struct ROIStatROI {
    pub enabled: bool,
    /// Offset in pixels: [x, y].
    pub offset: [usize; 2],
    /// Size in pixels: [x, y].
    pub size: [usize; 2],
    /// Width of the background border (pixels). 0 = no background subtraction.
    pub bgd_width: usize,
}

impl Default for ROIStatROI {
    fn default() -> Self {
        Self {
            enabled: true,
            offset: [0, 0],
            size: [0, 0],
            bgd_width: 0,
        }
    }
}

/// Statistics computed for a single ROI.
#[derive(Debug, Clone, Default)]
pub struct ROIStatResult {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub total: f64,
    /// Net = total - background_average * roi_elements. Zero if bgd_width is 0.
    pub net: f64,
}

/// Time-series acquisition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TSMode {
    Idle,
    Acquiring,
}

/// Number of statistics tracked per ROI (min, max, mean, total, net).
const NUM_STATS: usize = 5;

/// Processor that computes ROI statistics on 2D arrays.
pub struct ROIStatProcessor {
    rois: Vec<ROIStatROI>,
    results: Vec<ROIStatResult>,
    /// Time series buffers: [roi_index][stat_index][time_point].
    ts_mode: TSMode,
    ts_buffers: Vec<Vec<Vec<f64>>>,
    ts_num_points: usize,
    ts_current: usize,
}

impl ROIStatProcessor {
    /// Create a new processor with the given ROI definitions.
    pub fn new(rois: Vec<ROIStatROI>, ts_num_points: usize) -> Self {
        let n = rois.len();
        let results = vec![ROIStatResult::default(); n];
        let ts_buffers = vec![vec![Vec::new(); NUM_STATS]; n];
        Self {
            rois,
            results,
            ts_mode: TSMode::Idle,
            ts_buffers,
            ts_num_points,
            ts_current: 0,
        }
    }

    /// Get the current results for all ROIs.
    pub fn results(&self) -> &[ROIStatResult] {
        &self.results
    }

    /// Get the ROI definitions.
    pub fn rois(&self) -> &[ROIStatROI] {
        &self.rois
    }

    /// Mutable access to ROI definitions.
    pub fn rois_mut(&mut self) -> &mut Vec<ROIStatROI> {
        &mut self.rois
    }

    /// Set the time series mode.
    pub fn set_ts_mode(&mut self, mode: TSMode) {
        if mode == TSMode::Acquiring && self.ts_mode != TSMode::Acquiring {
            // Reset time series on start
            for roi_bufs in &mut self.ts_buffers {
                for stat_buf in roi_bufs.iter_mut() {
                    stat_buf.clear();
                }
            }
            self.ts_current = 0;
        }
        self.ts_mode = mode;
    }

    /// Get time series buffer for a specific ROI and stat index.
    /// stat_index: 0=min, 1=max, 2=mean, 3=total, 4=net
    pub fn ts_buffer(&self, roi_index: usize, stat_index: usize) -> &[f64] {
        if roi_index < self.ts_buffers.len() && stat_index < NUM_STATS {
            &self.ts_buffers[roi_index][stat_index]
        } else {
            &[]
        }
    }

    /// Compute statistics for a single ROI on a 2D data buffer.
    pub fn compute_roi_stats(
        data: &NDDataBuffer,
        x_size: usize,
        y_size: usize,
        roi: &ROIStatROI,
    ) -> ROIStatResult {
        let roi_x = roi.offset[0];
        let roi_y = roi.offset[1];
        let roi_w = roi.size[0];
        let roi_h = roi.size[1];

        // Clamp ROI to image bounds
        if roi_x >= x_size || roi_y >= y_size || roi_w == 0 || roi_h == 0 {
            return ROIStatResult::default();
        }
        let roi_w = roi_w.min(x_size - roi_x);
        let roi_h = roi_h.min(y_size - roi_y);

        let mut min = f64::MAX;
        let mut max = f64::MIN;
        let mut total = 0.0f64;
        let mut count = 0usize;

        for iy in roi_y..(roi_y + roi_h) {
            for ix in roi_x..(roi_x + roi_w) {
                let idx = iy * x_size + ix;
                if let Some(val) = data.get_as_f64(idx) {
                    if val < min { min = val; }
                    if val > max { max = val; }
                    total += val;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return ROIStatResult::default();
        }

        let mean = total / count as f64;

        // Background subtraction
        let net = if roi.bgd_width > 0 {
            let bgd = Self::compute_background(data, x_size, y_size, roi);
            total - bgd * count as f64
        } else {
            total
        };

        ROIStatResult { min, max, mean, total, net }
    }

    /// Compute average background from the border of the ROI.
    fn compute_background(
        data: &NDDataBuffer,
        x_size: usize,
        y_size: usize,
        roi: &ROIStatROI,
    ) -> f64 {
        let roi_x = roi.offset[0];
        let roi_y = roi.offset[1];
        let roi_w = roi.size[0].min(x_size.saturating_sub(roi_x));
        let roi_h = roi.size[1].min(y_size.saturating_sub(roi_y));
        let bw = roi.bgd_width;

        if bw == 0 || roi_w == 0 || roi_h == 0 {
            return 0.0;
        }

        let mut bgd_total = 0.0f64;
        let mut bgd_count = 0usize;

        for iy in roi_y..(roi_y + roi_h) {
            for ix in roi_x..(roi_x + roi_w) {
                // Check if this pixel is in the border region
                let dx_from_left = ix - roi_x;
                let dx_from_right = (roi_x + roi_w - 1) - ix;
                let dy_from_top = iy - roi_y;
                let dy_from_bottom = (roi_y + roi_h - 1) - iy;

                let in_border = dx_from_left < bw
                    || dx_from_right < bw
                    || dy_from_top < bw
                    || dy_from_bottom < bw;

                if in_border {
                    let idx = iy * x_size + ix;
                    if let Some(val) = data.get_as_f64(idx) {
                        bgd_total += val;
                        bgd_count += 1;
                    }
                }
            }
        }

        if bgd_count == 0 { 0.0 } else { bgd_total / bgd_count as f64 }
    }
}

impl NDPluginProcess for ROIStatProcessor {
    fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
        let info = array.info();
        let x_size = info.x_size;
        let y_size = info.y_size;

        // Ensure results vec matches rois
        self.results.resize(self.rois.len(), ROIStatResult::default());

        for (i, roi) in self.rois.iter().enumerate() {
            if !roi.enabled {
                self.results[i] = ROIStatResult::default();
                continue;
            }
            self.results[i] = Self::compute_roi_stats(&array.data, x_size, y_size, roi);
        }

        // Accumulate time series
        if self.ts_mode == TSMode::Acquiring {
            // Ensure ts_buffers match roi count
            while self.ts_buffers.len() < self.rois.len() {
                self.ts_buffers.push(vec![Vec::new(); NUM_STATS]);
            }

            for (i, result) in self.results.iter().enumerate() {
                if i >= self.ts_buffers.len() { break; }
                let stats = [result.min, result.max, result.mean, result.total, result.net];
                for (s, &val) in stats.iter().enumerate() {
                    let buf = &mut self.ts_buffers[i][s];
                    if buf.len() >= self.ts_num_points && self.ts_num_points > 0 {
                        // Circular: overwrite oldest
                        let idx = self.ts_current % self.ts_num_points;
                        if idx < buf.len() {
                            buf[idx] = val;
                        }
                    } else {
                        buf.push(val);
                    }
                }
            }
            self.ts_current += 1;
        }

        ProcessResult::sink(vec![])
    }

    fn plugin_type(&self) -> &str {
        "NDPluginROIStat"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ad_core::ndarray::{NDDataType, NDDimension};

    fn make_2d_array(x: usize, y: usize, fill: impl Fn(usize, usize) -> f64) -> NDArray {
        let mut arr = NDArray::new(
            vec![NDDimension::new(x), NDDimension::new(y)],
            NDDataType::Float64,
        );
        if let NDDataBuffer::F64(ref mut v) = arr.data {
            for iy in 0..y {
                for ix in 0..x {
                    v[iy * x + ix] = fill(ix, iy);
                }
            }
        }
        arr
    }

    #[test]
    fn test_single_roi_full_image() {
        let arr = make_2d_array(4, 4, |_x, _y| 10.0);
        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [0, 0],
            size: [4, 4],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        assert!((r.min - 10.0).abs() < 1e-10);
        assert!((r.max - 10.0).abs() < 1e-10);
        assert!((r.mean - 10.0).abs() < 1e-10);
        assert!((r.total - 160.0).abs() < 1e-10);
    }

    #[test]
    fn test_single_roi_subregion() {
        // 8x8 image, values = x + y * 8
        let arr = make_2d_array(8, 8, |x, y| (x + y * 8) as f64);

        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [2, 2],
            size: [3, 3],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        // ROI pixels: (2,2)=18, (3,2)=19, (4,2)=20, (2,3)=26, (3,3)=27, (4,3)=28, (2,4)=34, (3,4)=35, (4,4)=36
        assert!((r.min - 18.0).abs() < 1e-10);
        assert!((r.max - 36.0).abs() < 1e-10);
        let expected_total = 18.0 + 19.0 + 20.0 + 26.0 + 27.0 + 28.0 + 34.0 + 35.0 + 36.0;
        assert!((r.total - expected_total).abs() < 1e-10);
        assert!((r.mean - expected_total / 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_multiple_rois() {
        let arr = make_2d_array(8, 8, |x, _y| x as f64);

        let rois = vec![
            ROIStatROI {
                enabled: true,
                offset: [0, 0],
                size: [4, 4],
                bgd_width: 0,
            },
            ROIStatROI {
                enabled: true,
                offset: [4, 0],
                size: [4, 4],
                bgd_width: 0,
            },
        ];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r0 = &proc.results()[0];
        assert!((r0.min - 0.0).abs() < 1e-10);
        assert!((r0.max - 3.0).abs() < 1e-10);

        let r1 = &proc.results()[1];
        assert!((r1.min - 4.0).abs() < 1e-10);
        assert!((r1.max - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_bgd_width() {
        // 6x6 image, center 2x2 has value 100, border has value 10
        let arr = make_2d_array(6, 6, |x, y| {
            if x >= 2 && x < 4 && y >= 2 && y < 4 {
                100.0
            } else {
                10.0
            }
        });

        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [1, 1],
            size: [4, 4],
            bgd_width: 1,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        // ROI is 4x4 at (1,1): border pixels = 12 (all with value 10), center = 4 (value 100)
        // bgd average = (12*10 + ... well, border includes some 100s)
        // Actually border pixels at bgd_width=1: the outer ring of the 4x4 ROI
        // That outer ring occupies 12 of 16 pixels
        assert!(r.net < r.total, "net should be less than total with bgd subtraction");
    }

    #[test]
    fn test_empty_roi() {
        let arr = make_2d_array(4, 4, |_, _| 10.0);
        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [0, 0],
            size: [0, 0],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        assert!((r.total - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_disabled_roi() {
        let arr = make_2d_array(4, 4, |_, _| 10.0);
        let rois = vec![ROIStatROI {
            enabled: false,
            offset: [0, 0],
            size: [4, 4],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        assert!((r.total - 0.0).abs() < 1e-10, "disabled ROI should have zero stats");
    }

    #[test]
    fn test_roi_out_of_bounds() {
        let arr = make_2d_array(4, 4, |_, _| 10.0);
        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [10, 10],
            size: [4, 4],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        assert!((r.total - 0.0).abs() < 1e-10, "out-of-bounds ROI should produce zero stats");
    }

    #[test]
    fn test_roi_partially_out_of_bounds() {
        let arr = make_2d_array(4, 4, |_, _| 5.0);
        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [2, 2],
            size: [10, 10],  // extends beyond image
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        // Should be clamped to 2x2 region
        assert!((r.total - 20.0).abs() < 1e-10);
        assert!((r.mean - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_time_series() {
        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [0, 0],
            size: [4, 4],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 100);
        let pool = NDArrayPool::new(1_000_000);
        proc.set_ts_mode(TSMode::Acquiring);

        for i in 0..5 {
            let arr = make_2d_array(4, 4, |_, _| (i + 1) as f64);
            proc.process_array(&arr, &pool);
        }

        // Check mean time series (stat index 2)
        let ts = proc.ts_buffer(0, 2);
        assert_eq!(ts.len(), 5);
        assert!((ts[0] - 1.0).abs() < 1e-10);
        assert!((ts[4] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_u8_data() {
        let mut arr = NDArray::new(
            vec![NDDimension::new(4), NDDimension::new(4)],
            NDDataType::UInt8,
        );
        if let NDDataBuffer::U8(ref mut v) = arr.data {
            for (i, val) in v.iter_mut().enumerate() {
                *val = (i + 1) as u8;
            }
        }

        let rois = vec![ROIStatROI {
            enabled: true,
            offset: [0, 0],
            size: [4, 4],
            bgd_width: 0,
        }];

        let mut proc = ROIStatProcessor::new(rois, 0);
        let pool = NDArrayPool::new(1_000_000);
        proc.process_array(&arr, &pool);

        let r = &proc.results()[0];
        assert!((r.min - 1.0).abs() < 1e-10);
        assert!((r.max - 16.0).abs() < 1e-10);
    }
}
