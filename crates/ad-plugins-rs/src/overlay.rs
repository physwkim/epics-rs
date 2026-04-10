use std::sync::Arc;

use ad_core_rs::ndarray::{NDArray, NDDataBuffer};
use ad_core_rs::ndarray_pool::NDArrayPool;
use ad_core_rs::plugin::runtime::{NDPluginProcess, ProcessResult};

/// Shape to draw.
#[derive(Debug, Clone)]
pub enum OverlayShape {
    Cross {
        center_x: usize,
        center_y: usize,
        size: usize,
    },
    Rectangle {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    },
    Ellipse {
        center_x: usize,
        center_y: usize,
        rx: usize,
        ry: usize,
    },
    Text {
        x: usize,
        y: usize,
        text: String,
        font_size: usize,
    },
}

/// Draw mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawMode {
    Set,
    XOR,
}

/// A single overlay definition.
#[derive(Debug, Clone)]
pub struct OverlayDef {
    pub shape: OverlayShape,
    pub draw_mode: DrawMode,
    pub color: [u8; 3], // RGB color; for Mono, color[1] (green) is used
    pub width_x: usize, // line thickness in X direction (0 or 1 = 1px)
    pub width_y: usize, // line thickness in Y direction (0 or 1 = 1px)
}

// ---------------------------------------------------------------------------
// Minimal 5x7 bitmap font
// ---------------------------------------------------------------------------

const FONT_WIDTH: usize = 5;
const FONT_HEIGHT: usize = 7;

/// Return a 5x7 bitmap for printable ASCII characters.
/// Each row is encoded as a u8 with the 5 MSBs representing pixels.
fn get_char_bitmap(ch: char) -> [[bool; FONT_WIDTH]; FONT_HEIGHT] {
    let pattern: [u8; 7] = match ch {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '"' => [0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '$' => [0x04, 0x1E, 0x05, 0x0E, 0x14, 0x0F, 0x04],
        '%' => [0x03, 0x13, 0x08, 0x04, 0x02, 0x19, 0x18],
        '&' => [0x06, 0x09, 0x05, 0x02, 0x15, 0x09, 0x16],
        '\'' => [0x04, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        ')' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        '*' => [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x02],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04],
        '/' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        '0' => [0x0E, 0x11, 0x19, 0x15, 0x13, 0x11, 0x0E],
        '1' => [0x04, 0x06, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x10, 0x08, 0x04, 0x02, 0x1F],
        '3' => [0x0E, 0x11, 0x10, 0x0C, 0x10, 0x11, 0x0E],
        '4' => [0x08, 0x0C, 0x0A, 0x09, 0x1F, 0x08, 0x08],
        '5' => [0x1F, 0x01, 0x0F, 0x10, 0x10, 0x11, 0x0E],
        '6' => [0x0C, 0x02, 0x01, 0x0F, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x10, 0x08, 0x04, 0x02, 0x02, 0x02],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x1E, 0x10, 0x08, 0x06],
        ':' => [0x00, 0x00, 0x04, 0x00, 0x04, 0x00, 0x00],
        ';' => [0x00, 0x00, 0x04, 0x00, 0x04, 0x04, 0x02],
        '<' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '>' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '?' => [0x0E, 0x11, 0x10, 0x08, 0x04, 0x00, 0x04],
        '@' => [0x0E, 0x11, 0x15, 0x1D, 0x05, 0x01, 0x0E],
        'A' | 'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' | 'b' => [0x0F, 0x11, 0x11, 0x0F, 0x11, 0x11, 0x0F],
        'C' | 'c' => [0x0E, 0x11, 0x01, 0x01, 0x01, 0x11, 0x0E],
        'D' | 'd' => [0x07, 0x09, 0x11, 0x11, 0x11, 0x09, 0x07],
        'E' | 'e' => [0x1F, 0x01, 0x01, 0x0F, 0x01, 0x01, 0x1F],
        'F' | 'f' => [0x1F, 0x01, 0x01, 0x0F, 0x01, 0x01, 0x01],
        'G' | 'g' => [0x0E, 0x11, 0x01, 0x1D, 0x11, 0x11, 0x0E],
        'H' | 'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' | 'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' | 'j' => [0x1C, 0x08, 0x08, 0x08, 0x08, 0x09, 0x06],
        'K' | 'k' => [0x11, 0x09, 0x05, 0x03, 0x05, 0x09, 0x11],
        'L' | 'l' => [0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x1F],
        'M' | 'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' | 'n' => [0x11, 0x13, 0x15, 0x15, 0x19, 0x11, 0x11],
        'O' | 'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' | 'p' => [0x0F, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x01],
        'Q' | 'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x09, 0x16],
        'R' | 'r' => [0x0F, 0x11, 0x11, 0x0F, 0x05, 0x09, 0x11],
        'S' | 's' => [0x0E, 0x11, 0x01, 0x0E, 0x10, 0x11, 0x0E],
        'T' | 't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' | 'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' | 'v' => [0x11, 0x11, 0x11, 0x11, 0x0A, 0x0A, 0x04],
        'W' | 'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' | 'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' | 'y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' | 'z' => [0x1F, 0x10, 0x08, 0x04, 0x02, 0x01, 0x1F],
        '[' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '\\' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        ']' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        '^' => [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '`' => [0x02, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '{' => [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '}' => [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
        '~' => [0x00, 0x00, 0x02, 0x15, 0x08, 0x00, 0x00],
        _ => [0x00; 7], // blank for unknown chars
    };

    let mut bitmap = [[false; FONT_WIDTH]; FONT_HEIGHT];
    for row in 0..FONT_HEIGHT {
        let byte = pattern[row];
        for col in 0..FONT_WIDTH {
            bitmap[row][col] = (byte >> col) & 1 != 0;
        }
    }
    bitmap
}

// ---------------------------------------------------------------------------
// Per-type drawing via macro
// ---------------------------------------------------------------------------

macro_rules! draw_on_typed_buffer {
    ($data:expr, $T:ty, $overlays:expr, $w:expr, $h:expr, xor) => {{
        draw_on_typed_buffer!(@inner $data, $T, $overlays, $w, $h, |data: &mut [$T], idx: usize, mode: DrawMode, value: $T| {
            match mode {
                DrawMode::Set => data[idx] = value,
                DrawMode::XOR => data[idx] ^= value,
            }
        });
    }};
    ($data:expr, $T:ty, $overlays:expr, $w:expr, $h:expr, set_only) => {{
        draw_on_typed_buffer!(@inner $data, $T, $overlays, $w, $h, |data: &mut [$T], idx: usize, _mode: DrawMode, value: $T| {
            data[idx] = value;
        });
    }};
    (@inner $data:expr, $T:ty, $overlays:expr, $w:expr, $h:expr, $set_fn:expr) => {{
        let data: &mut [$T] = $data;
        let w: usize = $w;
        let h: usize = $h;
        let set_fn = $set_fn;

        for overlay in $overlays.iter() {
            // C++ uses pOverlay->green for mono overlays
            let value: $T = overlay.color[1] as $T;
            let wx = overlay.width_x.max(1);
            let wy = overlay.width_y.max(1);

            // Closure to set a single pixel
            let mut set_pixel = |x: usize, y: usize| {
                if x < w && y < h {
                    let idx = y * w + x;
                    set_fn(data, idx, overlay.draw_mode, value);
                }
            };

            match &overlay.shape {
                OverlayShape::Cross { center_x, center_y, size } => {
                    let cx = *center_x;
                    let cy = *center_y;
                    let half = *size / 2;
                    // Horizontal arm: WidthY thickness (centered vertically)
                    let wy_half = wy / 2;
                    for dx in 0..=half.min(w) {
                        for t in 0..wy {
                            let ty = (cy + t).wrapping_sub(wy_half);
                            if cx + dx < w { set_pixel(cx + dx, ty); }
                            if dx <= cx { set_pixel(cx - dx, ty); }
                        }
                    }
                    // Vertical arm: WidthX thickness (centered horizontally)
                    let wx_half = wx / 2;
                    for dy in 0..=half.min(h) {
                        for t in 0..wx {
                            let tx = (cx + t).wrapping_sub(wx_half);
                            if cy + dy < h { set_pixel(tx, cy + dy); }
                            if dy <= cy { set_pixel(tx, cy - dy); }
                        }
                    }
                }
                OverlayShape::Rectangle { x, y, width, height } => {
                    // Border thickness grows inward
                    let bx = wx.min(*width);
                    let by = wy.min(*height);
                    // Top edge
                    for dy in 0..by {
                        for dx in 0..*width {
                            set_pixel(x + dx, y + dy);
                        }
                    }
                    // Bottom edge
                    for dy in 0..by {
                        if *height > dy {
                            for dx in 0..*width {
                                set_pixel(x + dx, y + height - 1 - dy);
                            }
                        }
                    }
                    // Left edge (between top and bottom borders)
                    let inner_start = by;
                    let inner_end = height.saturating_sub(by);
                    for dy in inner_start..inner_end {
                        for t in 0..bx {
                            set_pixel(x + t, y + dy);
                        }
                    }
                    // Right edge (between top and bottom borders)
                    for dy in inner_start..inner_end {
                        for t in 0..bx {
                            if *width > t {
                                set_pixel(x + width - 1 - t, y + dy);
                            }
                        }
                    }
                }
                OverlayShape::Ellipse { center_x, center_y, rx, ry } => {
                    let cx = *center_x as f64;
                    let cy = *center_y as f64;
                    let rxf = *rx as f64;
                    let ryf = *ry as f64;
                    // Draw concentric ellipses from outer radius to inner radius
                    let wx_thickness = (wx as f64).min(rxf);
                    let wy_thickness = (wy as f64).min(ryf);
                    let steps = ((rxf + ryf) * 4.0).max(64.0) as usize;
                    for layer in 0..wx.max(wy) {
                        let frac = layer as f64;
                        let rx_cur = (rxf - frac * wx_thickness / wx.max(1) as f64).max(0.0);
                        let ry_cur = (ryf - frac * wy_thickness / wy.max(1) as f64).max(0.0);
                        if rx_cur <= 0.0 && ry_cur <= 0.0 {
                            break;
                        }
                        for i in 0..steps {
                            let angle = 2.0 * std::f64::consts::PI * i as f64 / steps as f64;
                            let px = (cx + rx_cur * angle.cos()).round() as usize;
                            let py = (cy + ry_cur * angle.sin()).round() as usize;
                            set_pixel(px, py);
                        }
                    }
                }
                OverlayShape::Text { x, y, text, font_size } => {
                    let scale = (*font_size).max(1) / FONT_HEIGHT.max(1);
                    let scale = scale.max(1);
                    let mut cursor_x = *x;
                    for ch in text.chars() {
                        let bitmap = get_char_bitmap(ch);
                        for row in 0..FONT_HEIGHT {
                            for col in 0..FONT_WIDTH {
                                if bitmap[row][col] {
                                    for sy in 0..scale {
                                        for sx in 0..scale {
                                            set_pixel(
                                                cursor_x + col * scale + sx,
                                                *y + row * scale + sy,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        cursor_x += (FONT_WIDTH + 1) * scale;
                    }
                }
            }
        }
    }};
}

/// Draw overlays on a 2D array. Supports I8, U8, I16, U16, I32, U32, I64, U64, F32, F64.
pub fn draw_overlays(src: &NDArray, overlays: &[OverlayDef]) -> NDArray {
    let mut arr = src.clone();
    if arr.dims.len() < 2 {
        return arr;
    }
    let w = arr.dims[0].size;
    let h = arr.dims[1].size;

    match &mut arr.data {
        NDDataBuffer::U8(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), u8, overlays, w, h, xor);
        }
        NDDataBuffer::U16(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), u16, overlays, w, h, xor);
        }
        NDDataBuffer::I16(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), i16, overlays, w, h, xor);
        }
        NDDataBuffer::I32(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), i32, overlays, w, h, xor);
        }
        NDDataBuffer::U32(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), u32, overlays, w, h, xor);
        }
        NDDataBuffer::F32(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), f32, overlays, w, h, set_only);
        }
        NDDataBuffer::F64(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), f64, overlays, w, h, set_only);
        }
        NDDataBuffer::I8(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), i8, overlays, w, h, xor);
        }
        NDDataBuffer::I64(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), i64, overlays, w, h, xor);
        }
        NDDataBuffer::U64(data) => {
            draw_on_typed_buffer!(data.as_mut_slice(), u64, overlays, w, h, xor);
        }
    }

    arr
}

/// Maximum number of overlays.
const MAX_OVERLAYS: usize = 8;

/// Runtime overlay state — one per addr (0..7).
#[derive(Debug, Clone)]
struct OverlaySlot {
    use_overlay: bool,
    shape: i32,     // 0=Cross, 1=Rectangle, 2=Ellipse, 3=Text
    draw_mode: i32, // 0=Set, 1=XOR
    position_x: usize,
    position_y: usize,
    size_x: usize,
    size_y: usize,
    width_x: usize,
    width_y: usize,
    red: u8,
    green: u8,
    blue: u8,
    display_text: String,
    font: usize,
}

impl Default for OverlaySlot {
    fn default() -> Self {
        Self {
            use_overlay: false,
            shape: 1, // Rectangle
            draw_mode: 0,
            position_x: 0,
            position_y: 0,
            size_x: 0,
            size_y: 0,
            width_x: 1,
            width_y: 1,
            red: 255,
            green: 0,
            blue: 0,
            display_text: String::new(),
            font: 0,
        }
    }
}

impl OverlaySlot {
    fn to_overlay_def(&self) -> Option<OverlayDef> {
        if !self.use_overlay {
            return None;
        }
        let draw_mode = if self.draw_mode == 1 {
            DrawMode::XOR
        } else {
            DrawMode::Set
        };
        let color = [self.red, self.green, self.blue];
        let shape = match self.shape {
            0 => OverlayShape::Cross {
                center_x: self.position_x + self.size_x / 2,
                center_y: self.position_y + self.size_y / 2,
                size: self.size_x.max(self.size_y),
            },
            1 => OverlayShape::Rectangle {
                x: self.position_x,
                y: self.position_y,
                width: self.size_x,
                height: self.size_y,
            },
            2 => OverlayShape::Ellipse {
                center_x: self.position_x + self.size_x / 2,
                center_y: self.position_y + self.size_y / 2,
                rx: self.size_x / 2,
                ry: self.size_y / 2,
            },
            3 => OverlayShape::Text {
                x: self.position_x,
                y: self.position_y,
                text: self.display_text.clone(),
                font_size: (self.font + 1) * FONT_HEIGHT,
            },
            _ => OverlayShape::Rectangle {
                x: self.position_x,
                y: self.position_y,
                width: self.size_x,
                height: self.size_y,
            },
        };
        Some(OverlayDef {
            shape,
            draw_mode,
            color,
            width_x: self.width_x,
            width_y: self.width_y,
        })
    }
}

/// Param indices for per-overlay params.
#[derive(Default)]
struct OverlayParamIndices {
    use_overlay: Option<usize>,
    position_x: Option<usize>,
    position_y: Option<usize>,
    center_x: Option<usize>,
    center_y: Option<usize>,
    size_x: Option<usize>,
    size_y: Option<usize>,
    width_x: Option<usize>,
    width_y: Option<usize>,
    shape: Option<usize>,
    draw_mode: Option<usize>,
    red: Option<usize>,
    green: Option<usize>,
    blue: Option<usize>,
    display_text: Option<usize>,
    font: Option<usize>,
}

/// Pure overlay processing logic with runtime-configurable overlays.
pub struct OverlayProcessor {
    slots: [OverlaySlot; MAX_OVERLAYS],
    params: OverlayParamIndices,
}

impl OverlayProcessor {
    pub fn new(overlays: Vec<OverlayDef>) -> Self {
        let mut slots: [OverlaySlot; MAX_OVERLAYS] = Default::default();
        for (i, o) in overlays.into_iter().enumerate().take(MAX_OVERLAYS) {
            let slot = &mut slots[i];
            slot.use_overlay = true;
            slot.draw_mode = if o.draw_mode == DrawMode::XOR { 1 } else { 0 };
            slot.red = o.color[0];
            slot.green = o.color[1];
            slot.blue = o.color[2];
            slot.width_x = o.width_x;
            slot.width_y = o.width_y;
            match o.shape {
                OverlayShape::Cross {
                    center_x,
                    center_y,
                    size,
                } => {
                    slot.shape = 0;
                    slot.position_x = center_x.saturating_sub(size / 2);
                    slot.position_y = center_y.saturating_sub(size / 2);
                    slot.size_x = size;
                    slot.size_y = size;
                }
                OverlayShape::Rectangle {
                    x,
                    y,
                    width,
                    height,
                } => {
                    slot.shape = 1;
                    slot.position_x = x;
                    slot.position_y = y;
                    slot.size_x = width;
                    slot.size_y = height;
                }
                OverlayShape::Ellipse {
                    center_x,
                    center_y,
                    rx,
                    ry,
                } => {
                    slot.shape = 2;
                    slot.position_x = center_x.saturating_sub(rx);
                    slot.position_y = center_y.saturating_sub(ry);
                    slot.size_x = rx * 2;
                    slot.size_y = ry * 2;
                }
                OverlayShape::Text {
                    x,
                    y,
                    text,
                    font_size,
                } => {
                    slot.shape = 3;
                    slot.position_x = x;
                    slot.position_y = y;
                    slot.display_text = text;
                    slot.font = font_size / FONT_HEIGHT.max(1);
                }
            }
        }
        Self {
            slots,
            params: OverlayParamIndices::default(),
        }
    }

    fn build_active_overlays(&self) -> Vec<OverlayDef> {
        self.slots
            .iter()
            .filter_map(|s| s.to_overlay_def())
            .collect()
    }
}

impl NDPluginProcess for OverlayProcessor {
    fn process_array(&mut self, array: &NDArray, _pool: &NDArrayPool) -> ProcessResult {
        let active = self.build_active_overlays();
        let out = draw_overlays(array, &active);
        ProcessResult::arrays(vec![Arc::new(out)])
    }

    fn plugin_type(&self) -> &str {
        "NDPluginOverlay"
    }

    fn register_params(
        &mut self,
        base: &mut asyn_rs::port::PortDriverBase,
    ) -> asyn_rs::error::AsynResult<()> {
        use asyn_rs::param::ParamType;
        base.create_param("MAX_SIZE_X", ParamType::Int32)?;
        base.create_param("MAX_SIZE_Y", ParamType::Int32)?;
        base.create_param("NAME", ParamType::Octet)?;
        base.create_param("USE", ParamType::Int32)?;
        base.create_param("OVERLAY_POSITION_X", ParamType::Int32)?;
        base.create_param("OVERLAY_POSITION_Y", ParamType::Int32)?;
        base.create_param("OVERLAY_CENTER_X", ParamType::Int32)?;
        base.create_param("OVERLAY_CENTER_Y", ParamType::Int32)?;
        base.create_param("OVERLAY_SIZE_X", ParamType::Int32)?;
        base.create_param("OVERLAY_SIZE_Y", ParamType::Int32)?;
        base.create_param("OVERLAY_WIDTH_X", ParamType::Int32)?;
        base.create_param("OVERLAY_WIDTH_Y", ParamType::Int32)?;
        base.create_param("OVERLAY_SHAPE", ParamType::Int32)?;
        base.create_param("OVERLAY_DRAW_MODE", ParamType::Int32)?;
        base.create_param("OVERLAY_RED", ParamType::Int32)?;
        base.create_param("OVERLAY_GREEN", ParamType::Int32)?;
        base.create_param("OVERLAY_BLUE", ParamType::Int32)?;
        base.create_param("OVERLAY_DISPLAY_TEXT", ParamType::Octet)?;
        base.create_param("OVERLAY_TIMESTAMP_FORMAT", ParamType::Octet)?;
        base.create_param("OVERLAY_FONT", ParamType::Int32)?;

        self.params.use_overlay = base.find_param("USE");
        self.params.position_x = base.find_param("OVERLAY_POSITION_X");
        self.params.position_y = base.find_param("OVERLAY_POSITION_Y");
        self.params.center_x = base.find_param("OVERLAY_CENTER_X");
        self.params.center_y = base.find_param("OVERLAY_CENTER_Y");
        self.params.size_x = base.find_param("OVERLAY_SIZE_X");
        self.params.size_y = base.find_param("OVERLAY_SIZE_Y");
        self.params.width_x = base.find_param("OVERLAY_WIDTH_X");
        self.params.width_y = base.find_param("OVERLAY_WIDTH_Y");
        self.params.shape = base.find_param("OVERLAY_SHAPE");
        self.params.draw_mode = base.find_param("OVERLAY_DRAW_MODE");
        self.params.red = base.find_param("OVERLAY_RED");
        self.params.green = base.find_param("OVERLAY_GREEN");
        self.params.blue = base.find_param("OVERLAY_BLUE");
        self.params.display_text = base.find_param("OVERLAY_DISPLAY_TEXT");
        self.params.font = base.find_param("OVERLAY_FONT");
        Ok(())
    }

    fn on_param_change(
        &mut self,
        reason: usize,
        params: &ad_core_rs::plugin::runtime::PluginParamSnapshot,
    ) -> ad_core_rs::plugin::runtime::ParamChangeResult {
        use ad_core_rs::plugin::runtime::{ParamChangeResult, ParamChangeValue, ParamUpdate};

        let idx = params.addr as usize;
        if idx >= MAX_OVERLAYS {
            return ParamChangeResult::updates(vec![]);
        }
        let slot = &mut self.slots[idx];
        let mut updates = Vec::new();

        if Some(reason) == self.params.use_overlay {
            slot.use_overlay = params.value.as_i32() != 0;
        } else if Some(reason) == self.params.shape {
            slot.shape = params.value.as_i32();
        } else if Some(reason) == self.params.draw_mode {
            slot.draw_mode = params.value.as_i32();
        } else if Some(reason) == self.params.position_x {
            slot.position_x = params.value.as_i32().max(0) as usize;
            if let Some(ci) = self.params.center_x {
                updates.push(ParamUpdate::int32_addr(
                    ci,
                    idx as i32,
                    (slot.position_x + slot.size_x / 2) as i32,
                ));
            }
        } else if Some(reason) == self.params.position_y {
            slot.position_y = params.value.as_i32().max(0) as usize;
            if let Some(ci) = self.params.center_y {
                updates.push(ParamUpdate::int32_addr(
                    ci,
                    idx as i32,
                    (slot.position_y + slot.size_y / 2) as i32,
                ));
            }
        } else if Some(reason) == self.params.center_x {
            let cx = params.value.as_i32().max(0) as usize;
            slot.position_x = cx.saturating_sub(slot.size_x / 2);
            if let Some(pi) = self.params.position_x {
                updates.push(ParamUpdate::int32_addr(
                    pi,
                    idx as i32,
                    slot.position_x as i32,
                ));
            }
        } else if Some(reason) == self.params.center_y {
            let cy = params.value.as_i32().max(0) as usize;
            slot.position_y = cy.saturating_sub(slot.size_y / 2);
            if let Some(pi) = self.params.position_y {
                updates.push(ParamUpdate::int32_addr(
                    pi,
                    idx as i32,
                    slot.position_y as i32,
                ));
            }
        } else if Some(reason) == self.params.size_x {
            slot.size_x = params.value.as_i32().max(0) as usize;
            if let Some(ci) = self.params.center_x {
                updates.push(ParamUpdate::int32_addr(
                    ci,
                    idx as i32,
                    (slot.position_x + slot.size_x / 2) as i32,
                ));
            }
        } else if Some(reason) == self.params.size_y {
            slot.size_y = params.value.as_i32().max(0) as usize;
            if let Some(ci) = self.params.center_y {
                updates.push(ParamUpdate::int32_addr(
                    ci,
                    idx as i32,
                    (slot.position_y + slot.size_y / 2) as i32,
                ));
            }
        } else if Some(reason) == self.params.width_x {
            slot.width_x = params.value.as_i32().max(0) as usize;
        } else if Some(reason) == self.params.width_y {
            slot.width_y = params.value.as_i32().max(0) as usize;
        } else if Some(reason) == self.params.red {
            slot.red = params.value.as_i32().clamp(0, 255) as u8;
        } else if Some(reason) == self.params.green {
            slot.green = params.value.as_i32().clamp(0, 255) as u8;
        } else if Some(reason) == self.params.blue {
            slot.blue = params.value.as_i32().clamp(0, 255) as u8;
        } else if Some(reason) == self.params.display_text {
            if let ParamChangeValue::Octet(s) = &params.value {
                slot.display_text = s.clone();
            }
        } else if Some(reason) == self.params.font {
            slot.font = params.value.as_i32().max(0) as usize;
        }

        ParamChangeResult::updates(updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ad_core_rs::ndarray::{NDDataType, NDDimension};

    fn make_8x8() -> NDArray {
        NDArray::new(
            vec![NDDimension::new(8), NDDimension::new(8)],
            NDDataType::UInt8,
        )
    }

    #[test]
    fn test_rectangle() {
        let arr = make_8x8();
        let overlays = vec![OverlayDef {
            shape: OverlayShape::Rectangle {
                x: 1,
                y: 1,
                width: 4,
                height: 3,
            },
            draw_mode: DrawMode::Set,
            color: [0, 255, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::U8(ref v) = out.data {
            // Top edge of rectangle at y=1, x=1..4
            assert_eq!(v[1 * 8 + 1], 255);
            assert_eq!(v[1 * 8 + 2], 255);
            assert_eq!(v[1 * 8 + 3], 255);
            assert_eq!(v[1 * 8 + 4], 255);
            // Inside should still be 0
            assert_eq!(v[2 * 8 + 2], 0);
        }
    }

    #[test]
    fn test_xor_mode() {
        let mut arr = make_8x8();
        if let NDDataBuffer::U8(ref mut v) = arr.data {
            v[0] = 0xFF;
        }

        let overlays = vec![OverlayDef {
            shape: OverlayShape::Cross {
                center_x: 0,
                center_y: 0,
                size: 2,
            },
            draw_mode: DrawMode::XOR,
            color: [0, 0xFF, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::U8(ref v) = out.data {
            // Center pixel (0,0) is drawn twice (horiz + vert arms):
            // 0xFF ^ 0xFF ^ 0xFF = 0xFF
            assert_eq!(v[0], 0xFF);
            // Neighbor (1,0) drawn once: 0x00 ^ 0xFF = 0xFF
            assert_eq!(v[1], 0xFF);
            // Pixel (0,1) drawn once: 0x00 ^ 0xFF = 0xFF
            assert_eq!(v[1 * 8], 0xFF);
        }
    }

    #[test]
    fn test_cross() {
        let arr = make_8x8();
        let overlays = vec![OverlayDef {
            shape: OverlayShape::Cross {
                center_x: 4,
                center_y: 4,
                size: 4,
            },
            draw_mode: DrawMode::Set,
            color: [0, 200, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::U8(ref v) = out.data {
            assert_eq!(v[4 * 8 + 4], 200); // center
            assert_eq!(v[4 * 8 + 6], 200); // right arm
            assert_eq!(v[6 * 8 + 4], 200); // bottom arm
        }
    }

    #[test]
    fn test_text_rendering() {
        // Render "Hi" at position (0,0), font_size=7 (1x scale)
        let arr = NDArray::new(
            vec![NDDimension::new(20), NDDimension::new(10)],
            NDDataType::UInt8,
        );
        let overlays = vec![OverlayDef {
            shape: OverlayShape::Text {
                x: 0,
                y: 0,
                text: "Hi".to_string(),
                font_size: 7,
            },
            draw_mode: DrawMode::Set,
            color: [0, 255, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::U8(ref v) = out.data {
            let w = 20;
            // 'H' bitmap first row is 0x11 = bits 0 and 4 set
            // pixel (0,0) should be set (bit 0)
            assert_eq!(v[0 * w + 0], 255);
            // pixel (4,0) should be set (bit 4)
            assert_eq!(v[0 * w + 4], 255);
            // pixel (2,0) should NOT be set for 'H' row 0
            assert_eq!(v[0 * w + 2], 0);

            // 'I' starts at cursor_x = 6 (FONT_WIDTH=5 + 1 gap)
            // 'I' first row is 0x0E = bits 1,2,3 set
            assert_eq!(v[0 * w + 6 + 1], 255);
            assert_eq!(v[0 * w + 6 + 2], 255);
            assert_eq!(v[0 * w + 6 + 3], 255);
        }
    }

    #[test]
    fn test_u16_overlay() {
        let arr = NDArray::new(
            vec![NDDimension::new(8), NDDimension::new(8)],
            NDDataType::UInt16,
        );
        // Fill with zeros (already done by NDArray::new)
        let overlays = vec![OverlayDef {
            shape: OverlayShape::Rectangle {
                x: 1,
                y: 1,
                width: 4,
                height: 3,
            },
            draw_mode: DrawMode::Set,
            color: [0, 200, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::U16(ref v) = out.data {
            // Top edge at y=1, x=1
            assert_eq!(v[1 * 8 + 1], 200);
            assert_eq!(v[1 * 8 + 4], 200);
            // Inside should still be 0
            assert_eq!(v[2 * 8 + 2], 0);
        }
    }

    #[test]
    fn test_f32_overlay_ignores_xor() {
        let arr = NDArray::new(
            vec![NDDimension::new(8), NDDimension::new(8)],
            NDDataType::Float32,
        );
        let overlays = vec![OverlayDef {
            shape: OverlayShape::Cross {
                center_x: 4,
                center_y: 4,
                size: 2,
            },
            draw_mode: DrawMode::XOR, // should be treated as Set for floats
            color: [0, 100, 0],
            width_x: 1,
            width_y: 1,
        }];

        let out = draw_overlays(&arr, &overlays);
        if let NDDataBuffer::F32(ref v) = out.data {
            // Center pixel should be set (XOR falls back to Set for floats)
            assert_eq!(v[4 * 8 + 4], 100.0);
        }
    }
}
