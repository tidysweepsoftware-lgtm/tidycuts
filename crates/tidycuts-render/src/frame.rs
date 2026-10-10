//! What a frame is made of, as handed to the compositor. Everything here is
//! already resolved for one moment in time (keyframes evaluated, catalog
//! entries looked up).

use std::sync::Arc;

/// An RGBA8 picture, rows packed with no padding.
#[derive(Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Whether colors are already multiplied by alpha.
    pub premultiplied: bool,
}

impl std::fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RgbaImage({}x{})", self.width, self.height)
    }
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        RgbaImage {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            premultiplied: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YuvMatrix {
    Bt601,
    #[default]
    Bt709,
    Bt2020,
}

impl YuvMatrix {
    /// Picks the matrix from ffprobe's `color_space`, falling back on the
    /// picture height like players do.
    pub fn guess(name: Option<&str>, height: u32) -> YuvMatrix {
        match name.unwrap_or("") {
            "bt709" => YuvMatrix::Bt709,
            "bt2020nc" | "bt2020c" | "bt2020" => YuvMatrix::Bt2020,
            "bt470bg" | "smpte170m" | "bt601" | "fcc" => YuvMatrix::Bt601,
            _ if height < 720 => YuvMatrix::Bt601,
            _ => YuvMatrix::Bt709,
        }
    }
}

/// A planar YUV 4:2:0 picture (Y plane, then U, then V), as ffmpeg's
/// `yuv420p` rawvideo output.
#[derive(Clone)]
pub struct YuvImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub matrix: YuvMatrix,
    pub full_range: bool,
}

impl YuvImage {
    pub fn byte_len(width: u32, height: u32) -> usize {
        let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
        (width * height + 2 * cw * ch) as usize
    }
}

impl std::fmt::Debug for YuvImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YuvImage({}x{})", self.width, self.height)
    }
}

#[derive(Debug, Clone)]
pub enum Pixels {
    Rgba(Arc<RgbaImage>),
    Yuv420(Arc<YuvImage>),
}

impl Pixels {
    pub fn size(&self) -> (u32, u32) {
        match self {
            Pixels::Rgba(i) => (i.width, i.height),
            Pixels::Yuv420(i) => (i.width, i.height),
        }
    }
}

/// Where a layer's picture comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// A decoded video frame, uploaded this frame.
    Pixels(Pixels),
    /// A picture stored earlier with `Compositor::store_static`.
    Cached(u64),
    Solid([f32; 4]),
    /// Two colors and an angle in degrees.
    Gradient([f32; 4], [f32; 4], f32),
    /// Sound levels drawn as an audio visualizer.
    Visualizer(VisualizerParams),
}

/// An audio visualizer's look and its levels at one moment. See
/// `visualizer.wgsl`.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualizerParams {
    /// Index into `tidycuts_core::VISUALIZER_STYLES`.
    pub style: u32,
    /// Bar heights 0..1, one per bar (1 to 64 of them).
    pub levels: Vec<f32>,
    /// Bottom (or start) and top (or end) colors, straight alpha.
    pub colors: [[f32; 4]; 2],
    pub mirror: bool,
    /// Bar width as a fraction of each bar's space, or the line width.
    pub thickness: f32,
}

/// Color correction values; 0 means unchanged. See `grade.wgsl`.
#[derive(Debug, Clone, PartialEq)]
pub struct Grade {
    pub exposure: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub temperature: f32,
    pub tint: f32,
    pub vibrance: f32,
    /// Degrees.
    pub hue: f32,
    pub vignette: f32,
    pub fade: f32,
    pub grain: f32,
    pub gamma: f32,
    pub lift: [f32; 3],
    pub gain: [f32; 3],
    /// rgb + amount.
    pub shadow_tint: [f32; 4],
    pub highlight_tint: [f32; 4],
    pub mono: bool,
    /// 0..1 mix with the original.
    pub strength: f32,
    /// HSL, curves and a LUT (the Color tab's), if any.
    pub looks: Option<Arc<crate::looks::Looks>>,
}

impl Default for Grade {
    fn default() -> Self {
        Grade {
            exposure: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            temperature: 0.0,
            tint: 0.0,
            vibrance: 0.0,
            hue: 0.0,
            vignette: 0.0,
            fade: 0.0,
            grain: 0.0,
            gamma: 0.0,
            lift: [0.0; 3],
            gain: [0.0; 3],
            shadow_tint: [0.0; 4],
            highlight_tint: [0.0; 4],
            mono: false,
            strength: 1.0,
            looks: None,
        }
    }
}

impl Grade {
    pub fn is_identity(&self) -> bool {
        if self.strength <= 0.0 {
            return true;
        }
        let mut g = self.clone();
        g.strength = 1.0;
        g == Grade::default()
    }
}

/// One shader effect pass (or a few, for blurs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fx {
    /// Effect id in `effects.wgsl`.
    pub shader: u32,
    pub passes: u32,
    pub params: [[f32; 4]; 4],
    pub colors: [[f32; 4]; 2],
    /// Seconds, for animated effects.
    pub time: f32,
}

/// Clean-up applied before color and effects.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EnhanceParams {
    pub denoise: f32,
    pub deblock: f32,
    pub sharpen: f32,
    /// 0 standard, 1 sharp (bicubic), 2 enhanced (Lanczos + adaptive sharpen).
    pub upscale: u32,
    /// Size of the compression blocks in texels (8 at the source resolution).
    pub block_size: f32,
}

/// One picture placed on the canvas.
#[derive(Debug, Clone)]
pub struct Layer {
    pub source: Source,
    /// Size in project pixels at 100% scale.
    pub base_size: [f32; 2],
    /// Visible part of the source in uv (x0, y0, x1, y1).
    pub crop: [f32; 4],
    /// Centre in project pixels (0,0 = top-left of the frame).
    pub center: [f32; 2],
    /// 1 = 100%. Negative values mirror.
    pub scale: [f32; 2],
    /// Degrees clockwise.
    pub rotation: f32,
    pub flip: [bool; 2],
    pub opacity: f32,
    /// Index into `BlendMode::ALL`.
    pub blend: u32,
    /// Rounded corners in project pixels.
    pub corner_radius: f32,
    pub enhance: EnhanceParams,
    pub grade: Option<Grade>,
    /// Filter looks applied after `grade`, in order.
    pub filters: Vec<Grade>,
    pub effects: Vec<Fx>,
    pub mask: Option<MaskParams>,
    pub shadow: Option<ShadowParams>,
    /// Seconds since the clip started, for animated effects.
    pub time: f32,
    /// The split-screen cell the layer is shown in.
    pub cell: Option<CellParams>,
}

/// A split-screen cell: the layer shows only inside `inner` (with its
/// corners rounded by `radius`), and the rest of `outer` is the border.
/// Corners are in project pixels, going round the cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellParams {
    pub outer: [[f32; 2]; 4],
    pub inner: [[f32; 2]; 4],
    pub radius: f32,
    /// Straight alpha; alpha 0 leaves the border see-through.
    pub border: [f32; 4],
}

/// A shape limiting which part of a layer shows. Coordinates are
/// layer-local: 0..1 across the visible (cropped) picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskParams {
    /// 0 single line, 1 double line, 2 rectangle, 3 ellipse, 4 star, 5 heart
    /// (the order of `tidycuts_core::MaskShape::ALL`).
    pub shape: u32,
    pub center: [f32; 2],
    pub size: [f32; 2],
    /// Degrees clockwise.
    pub rotation: f32,
    /// Rectangle corner rounding 0..1.
    pub roundness: f32,
    /// Edge softness as a fraction of the layer's smaller side.
    pub feather: f32,
    pub invert: bool,
}

/// A drop shadow cast by a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowParams {
    /// Straight-alpha color; alpha is the shadow's opacity.
    pub color: [f32; 4],
    /// Offset in project pixels.
    pub offset: [f32; 2],
    /// Blur radius in project pixels.
    pub blur: f32,
}

/// A layer shown only where another layer (the matte) is.
#[derive(Debug, Clone)]
pub struct MatteItem {
    pub layer: Layer,
    pub matte: Layer,
    /// 1 alpha, 2 alpha inverted, 3 luma, 4 luma inverted.
    pub mode: u32,
}

impl Layer {
    /// A layer filling `size` (project pixels) at the frame centre.
    pub fn full(source: Source, size: [f32; 2]) -> Self {
        Layer {
            source,
            base_size: size,
            crop: [0.0, 0.0, 1.0, 1.0],
            center: [size[0] * 0.5, size[1] * 0.5],
            scale: [1.0, 1.0],
            rotation: 0.0,
            flip: [false, false],
            opacity: 1.0,
            blend: 0,
            corner_radius: 0.0,
            enhance: EnhanceParams::default(),
            grade: None,
            filters: Vec::new(),
            effects: Vec::new(),
            mask: None,
            shadow: None,
            time: 0.0,
            cell: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransitionItem {
    pub from: Option<Layer>,
    pub to: Option<Layer>,
    /// Transition id in `transitions.wgsl`.
    pub kind: u32,
    pub progress: f32,
    pub params: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub time: f32,
}

/// Color and effects applied to everything below (an adjustment clip).
#[derive(Debug, Clone)]
pub struct AdjustItem {
    pub grade: Option<Grade>,
    pub effects: Vec<Fx>,
    pub opacity: f32,
    pub time: f32,
}

#[derive(Debug, Clone)]
pub enum Item {
    Layer(Layer),
    Transition(TransitionItem),
    Adjust(AdjustItem),
    Matte(MatteItem),
}

/// A whole frame: the canvas size in project pixels, a background and the
/// items from bottom to top.
#[derive(Debug, Clone)]
pub struct Frame {
    pub canvas: [f32; 2],
    pub background: [f32; 4],
    pub items: Vec<Item>,
}
