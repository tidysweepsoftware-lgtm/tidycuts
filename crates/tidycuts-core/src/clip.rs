//! Clips and everything that can be set on one: transform, crop, color,
//! effects, transitions, animations, speed and audio.

use crate::keyframe::Animatable;
use crate::layout::Cell;
use crate::looks::{ColorLut, Curves, Hsl};
use crate::project::{MediaId, MediaItem, MediaKind};
use crate::speed::SpeedRamp;
use crate::time::Time;
use crate::visualizer::VisualizerSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub type ClipId = Uuid;

/// Straight-alpha sRGB color, each channel 0..=1.
pub type Color = [f32; 4];

pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
pub const BLACK: Color = [0.0, 0.0, 0.0, 1.0];

fn yes() -> bool {
    true
}

fn one() -> f64 {
    1.0
}

fn hundred() -> Animatable {
    Animatable::constant(100.0)
}

fn zero() -> Animatable {
    Animatable::constant(0.0)
}

/// Position, scale and rotation of a visual clip, all keyframable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// Percent, 100 = media fitted to the frame (titles: natural size).
    pub scale_x: Animatable,
    pub scale_y: Animatable,
    /// Offset from the frame centre in project pixels.
    pub position_x: Animatable,
    pub position_y: Animatable,
    /// Degrees, clockwise.
    pub rotation: Animatable,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    /// Percent, 0..=100.
    pub opacity: Animatable,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            scale_x: hundred(),
            scale_y: hundred(),
            position_x: zero(),
            position_y: zero(),
            rotation: zero(),
            flip_horizontal: false,
            flip_vertical: false,
            opacity: hundred(),
        }
    }
}

/// How much is cut off each side of the picture, as fractions of its size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Crop {
    pub fn is_none(&self) -> bool {
        self.left == 0.0 && self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0
    }

    /// Keeps at least 5% of the picture on each axis.
    pub fn clamped(self) -> Crop {
        let c = |v: f64| v.clamp(0.0, 0.95);
        let mut out = Crop {
            left: c(self.left),
            top: c(self.top),
            right: c(self.right),
            bottom: c(self.bottom),
        };
        if out.left + out.right > 0.95 {
            out.right = 0.95 - out.left;
        }
        if out.top + out.bottom > 0.95 {
            out.bottom = 0.95 - out.top;
        }
        out
    }
}

/// Compositing blend modes, as in a Compositing section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    Lighten,
    Screen,
    ColorDodge,
    Add,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Subtract,
}

impl BlendMode {
    pub const ALL: [BlendMode; 15] = [
        BlendMode::Normal,
        BlendMode::Darken,
        BlendMode::Multiply,
        BlendMode::ColorBurn,
        BlendMode::LinearBurn,
        BlendMode::Lighten,
        BlendMode::Screen,
        BlendMode::ColorDodge,
        BlendMode::Add,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Subtract,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Darken => "Darken",
            BlendMode::Multiply => "Multiply",
            BlendMode::ColorBurn => "Color Burn",
            BlendMode::LinearBurn => "Linear Burn",
            BlendMode::Lighten => "Lighten",
            BlendMode::Screen => "Screen",
            BlendMode::ColorDodge => "Color Dodge",
            BlendMode::Add => "Linear Dodge (Add)",
            BlendMode::Overlay => "Overlay",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::HardLight => "Hard Light",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Subtract => "Subtract",
        }
    }

    pub fn index(self) -> u32 {
        BlendMode::ALL.iter().position(|b| *b == self).unwrap_or(0) as u32
    }
}

/// Color correction (Color tab). Every basic value is -100..=100
/// with 0 meaning unchanged, except `hue` which is in degrees; HSL, curves
/// and the LUT are in `looks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorAdjust {
    pub exposure: Animatable,
    pub brightness: Animatable,
    pub contrast: Animatable,
    pub highlights: Animatable,
    pub shadows: Animatable,
    pub whites: Animatable,
    pub blacks: Animatable,
    pub saturation: Animatable,
    pub vibrance: Animatable,
    pub temperature: Animatable,
    pub tint: Animatable,
    pub hue: Animatable,
    pub vignette: Animatable,
    #[serde(skip_serializing_if = "Hsl::is_neutral")]
    pub hsl: Hsl,
    #[serde(skip_serializing_if = "Curves::is_identity")]
    pub curves: Curves,
    #[serde(skip_serializing_if = "Wheels::is_neutral")]
    pub wheels: Wheels,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lut: Option<ColorLut>,
}

/// One colour wheel (Lift, Gamma, Gain or Offset): where the puck sits on
/// the disc, each way from the middle (-1..=1, up and right positive), and
/// the master slider beside it that brightens or darkens everything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wheel {
    pub x: f64,
    pub y: f64,
    pub master: f64,
}

impl Wheel {
    pub fn is_neutral(&self) -> bool {
        *self == Wheel::default()
    }
}

/// The four colour wheels of the Color tab: Lift
/// moves the shadows, Gamma the middle tones, Gain the highlights and
/// Offset everything at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wheels {
    pub lift: Wheel,
    pub gamma: Wheel,
    pub gain: Wheel,
    pub offset: Wheel,
}

impl Wheels {
    pub fn is_neutral(&self) -> bool {
        *self == Wheels::default()
    }
}

impl Default for ColorAdjust {
    fn default() -> Self {
        ColorAdjust {
            exposure: zero(),
            brightness: zero(),
            contrast: zero(),
            highlights: zero(),
            shadows: zero(),
            whites: zero(),
            blacks: zero(),
            saturation: zero(),
            vibrance: zero(),
            temperature: zero(),
            tint: zero(),
            hue: zero(),
            vignette: zero(),
            hsl: Hsl::default(),
            curves: Curves::default(),
            wheels: Wheels::default(),
            lut: None,
        }
    }
}

impl ColorAdjust {
    pub fn is_neutral(&self) -> bool {
        *self == ColorAdjust::default()
    }

    /// Every keyframable value with its name (basic sliders, HSL and the
    /// LUT's strength).
    pub fn fields_mut(&mut self) -> Vec<(&'static str, &mut Animatable)> {
        let mut out = vec![
            ("exposure", &mut self.exposure),
            ("brightness", &mut self.brightness),
            ("contrast", &mut self.contrast),
            ("highlights", &mut self.highlights),
            ("shadows", &mut self.shadows),
            ("whites", &mut self.whites),
            ("blacks", &mut self.blacks),
            ("saturation", &mut self.saturation),
            ("vibrance", &mut self.vibrance),
            ("temperature", &mut self.temperature),
            ("tint", &mut self.tint),
            ("hue", &mut self.hue),
            ("vignette", &mut self.vignette),
        ];
        out.extend(self.hsl.fields_mut());
        if let Some(l) = &mut self.lut {
            out.push(("lut_strength", &mut l.strength));
        }
        out
    }
}

/// How a picture is enlarged when it's shown bigger than its own pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Upscale {
    /// Plain bilinear filtering.
    #[default]
    Standard,
    /// Sharper bicubic filtering.
    Sharp,
    /// Edge-aware upscaling followed by contrast-adaptive sharpening.
    Enhanced,
}

impl Upscale {
    pub const ALL: [Upscale; 3] = [Upscale::Standard, Upscale::Sharp, Upscale::Enhanced];

    pub fn label(self) -> &'static str {
        match self {
            Upscale::Standard => "Standard",
            Upscale::Sharp => "Sharp",
            Upscale::Enhanced => "Enhanced (edge-aware)",
        }
    }
}

/// Picture clean-up for grainy, blocky or low-resolution footage.
/// Strengths are 0..=100.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Enhance {
    pub denoise: f64,
    pub deblock: f64,
    pub sharpen: f64,
    pub upscale: Upscale,
}

impl Enhance {
    pub fn is_off(&self) -> bool {
        self.denoise <= 0.0
            && self.deblock <= 0.0
            && self.sharpen <= 0.0
            && self.upscale == Upscale::Standard
    }
}

/// A video effect or filter applied to a clip. `kind` names an entry in the
/// built-in catalog (for example `"blur.gaussian"` or `"filter.teal_orange"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub id: Uuid,
    pub kind: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Parameters that differ from the catalog defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Animatable>,
}

impl Effect {
    pub fn new(kind: impl Into<String>) -> Self {
        Effect {
            id: Uuid::new_v4(),
            kind: kind.into(),
            enabled: true,
            params: BTreeMap::new(),
        }
    }
}

/// A transition at the start of a clip, blending from the clip before it on
/// the same track (or from nothing when there's a gap). It is centred on the
/// cut and lasts `duration`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: String,
    pub duration: Time,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f64>,
}

impl Transition {
    pub fn new(kind: impl Into<String>, duration: Time) -> Self {
        Transition {
            kind: kind.into(),
            duration,
            params: BTreeMap::new(),
        }
    }
}

/// An in or out animation preset (Animation tab), e.g. `"fade"`,
/// `"zoom_in"`, `"slide_left"` or, for titles, `"typewriter"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipAnimation {
    pub kind: String,
    pub duration: Time,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// A text title. Sizes are in project pixels for a 1080-pixel-high frame
/// and scale with the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TitleSpec {
    /// Decoration style from the title catalog (bars, boxes, underline…).
    pub style: String,
    pub text: String,
    pub font: String,
    pub size: f32,
    pub color: Color,
    /// Second color used by the style's decorations.
    pub accent: Color,
    pub align: TextAlign,
    pub bold: bool,
    pub italic: bool,
    pub letter_spacing: f32,
    pub line_spacing: f32,
    /// Outline width in pixels, 0 = none.
    pub outline: f32,
    pub outline_color: Color,
    /// Shadow strength 0..=100, 0 = none.
    pub shadow: f32,
    pub shadow_color: Color,
    /// Background box behind the text.
    pub background: bool,
    pub background_color: Color,
}

impl Default for TitleSpec {
    fn default() -> Self {
        TitleSpec {
            style: "plain".into(),
            text: "Your Title Here".into(),
            font: "Poppins Bold".into(),
            size: 96.0,
            color: WHITE,
            accent: [0.18, 0.85, 0.69, 1.0],
            align: TextAlign::Center,
            bold: false,
            italic: false,
            letter_spacing: 0.0,
            line_spacing: 1.15,
            outline: 0.0,
            outline_color: BLACK,
            shadow: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.8],
            background: false,
            background_color: [0.0, 0.0, 0.0, 0.6],
        }
    }
}

/// A built-in vector sticker. `motion` is its looping animation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickerSpec {
    pub kind: String,
    pub color: Color,
    pub color2: Color,
    #[serde(default)]
    pub motion: String,
}

/// Content that TidyCuts draws itself instead of reading from a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Generator {
    Title(TitleSpec),
    Sticker(StickerSpec),
    /// A full-frame solid color (a "color matte").
    Solid(Color),
    /// A full-frame two-color gradient; angle in degrees.
    Gradient {
        a: Color,
        b: Color,
        angle: f32,
    },
    /// Applies the clip's color, filters and effects to every track below
    /// it for as long as it lasts (filter/effect clips).
    Adjustment,
    /// An empty split-screen cell waiting for media to be dropped on it.
    Placeholder,
    /// Bars, lines or rings that move with the sound.
    Visualizer(VisualizerSpec),
}

impl Generator {
    pub fn label(&self) -> String {
        match self {
            Generator::Title(t) => {
                let first = t.text.lines().next().unwrap_or("").trim();
                if first.is_empty() {
                    "Title".into()
                } else {
                    first.to_string()
                }
            }
            Generator::Sticker(s) => s.kind.clone(),
            Generator::Solid(_) => "Color".into(),
            Generator::Gradient { .. } => "Gradient".into(),
            Generator::Adjustment => "Adjustment".into(),
            Generator::Placeholder => "Empty cell".into(),
            Generator::Visualizer(v) => format!("{} visualizer", v.style_label()),
        }
    }
}

/// Sound settings for a clip (Audio tab). Everything here is
/// plain signal processing done by ffmpeg filters; nothing needs AI models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub fade_in: Time,
    pub fade_out: Time,
    /// Silences a video clip's own sound (for example after detaching it).
    pub muted: bool,
    /// Left/right balance, -100 (left) ..= 100 (right).
    pub pan: f64,
    /// Semitones, -12 ..= 12.
    pub pitch: f64,
    /// Keeps the original pitch when the speed changes.
    pub keep_pitch: bool,
    /// Evens out the loudness to a standard level (-14 LUFS).
    pub normalize: bool,
    /// Steady-noise reduction (hiss, fans, hum-like rooms), 0 = off ..= 100.
    pub denoise: f64,
    /// Voice clean-up: a trained model separates the voice from everything
    /// else. 0 = off ..= 100 (how much of the cleaned sound is used).
    pub voice_clean: f64,
    /// Evens out loud and quiet parts so a voice stays at one level,
    /// 0 = off ..= 100 (how much quiet sound may be lifted).
    pub leveler: f64,
    /// Keeps peaks under -1 dB so a boosted clip cannot distort.
    pub limiter: bool,
    /// Cuts low rumble such as wind and handling noise.
    pub wind_removal: bool,
    /// Removes mains hum at 50 or 60 Hz and its harmonics; 0 = off.
    pub hum_removal: u32,
    /// Softens harsh "s" sounds in speech.
    pub de_ess: bool,
    /// Ten-band equalizer gains in dB (31 Hz .. 16 kHz), -12 ..= 12.
    pub eq: [f64; 10],
    /// Name of the equalizer preset the gains came from.
    pub eq_preset: String,
    /// Lowers every other clip's sound by this percent while this clip
    /// plays (ducking music under a voice-over); 0 = off.
    pub ducking: f64,
    /// Voice effect id ("" = none), e.g. "robot", "radio", "echo".
    pub voice: String,
    /// Which audio stream of the file to play, for files with several.
    pub stream: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        AudioSettings {
            fade_in: Time::ZERO,
            fade_out: Time::ZERO,
            muted: false,
            pan: 0.0,
            pitch: 0.0,
            keep_pitch: true,
            normalize: false,
            denoise: 0.0,
            voice_clean: 0.0,
            leveler: 0.0,
            limiter: false,
            wind_removal: false,
            hum_removal: 0,
            de_ess: false,
            eq: [0.0; 10],
            eq_preset: String::new(),
            ducking: 0.0,
            voice: String::new(),
            stream: 0,
        }
    }
}

impl AudioSettings {
    pub fn is_default(&self) -> bool {
        *self == AudioSettings::default()
    }

    /// Whether any processing beyond volume and fades is switched on.
    pub fn has_processing(&self) -> bool {
        self.pan != 0.0
            || self.pitch != 0.0
            || self.normalize
            || self.denoise > 0.0
            || self.voice_clean > 0.0
            || self.leveler > 0.0
            || self.limiter
            || self.wind_removal
            || self.hum_removal != 0
            || self.de_ess
            || self.eq.iter().any(|g| *g != 0.0)
            || !self.voice.is_empty()
    }
}

/// What fills the frame around a clip that doesn't cover it (for example a
/// vertical phone video in a 16:9 project).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundFill {
    #[default]
    None,
    /// A blurred, enlarged copy of the clip itself.
    Blur,
    /// A solid color.
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipBackground {
    pub fill: BackgroundFill,
    /// Blur strength 0..=100.
    pub blur: f64,
    pub color: Color,
}

impl Default for ClipBackground {
    fn default() -> Self {
        ClipBackground {
            fill: BackgroundFill::None,
            blur: 60.0,
            color: BLACK,
        }
    }
}

/// A drop shadow under the clip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shadow {
    pub enabled: bool,
    pub color: Color,
    /// 0..=100.
    pub opacity: f64,
    /// Offset in project pixels.
    pub distance: f64,
    /// Direction the shadow falls, degrees (135 = down-right).
    pub angle: f64,
    /// Softness 0..=100.
    pub blur: f64,
}

impl Default for Shadow {
    fn default() -> Self {
        Shadow {
            enabled: false,
            color: BLACK,
            opacity: 60.0,
            distance: 20.0,
            angle: 135.0,
            blur: 40.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskShape {
    /// Everything on one side of a line.
    Line,
    /// A band between two parallel lines.
    DoubleLine,
    #[default]
    Rectangle,
    Ellipse,
    Star,
    Heart,
}

impl MaskShape {
    pub const ALL: [MaskShape; 6] = [
        MaskShape::Line,
        MaskShape::DoubleLine,
        MaskShape::Rectangle,
        MaskShape::Ellipse,
        MaskShape::Star,
        MaskShape::Heart,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MaskShape::Line => "Single Line",
            MaskShape::DoubleLine => "Double Line",
            MaskShape::Rectangle => "Rectangle",
            MaskShape::Ellipse => "Circle",
            MaskShape::Star => "Star",
            MaskShape::Heart => "Heart",
        }
    }

    pub fn index(self) -> u32 {
        MaskShape::ALL.iter().position(|m| *m == self).unwrap_or(0) as u32
    }
}

/// Shows only part of a clip. Position and size are fractions of the clip's
/// picture (0.5 = middle); everything is keyframable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub shape: MaskShape,
    pub x: Animatable,
    pub y: Animatable,
    pub width: Animatable,
    pub height: Animatable,
    /// Degrees.
    pub rotation: Animatable,
    /// Rectangle corner rounding, 0..=100.
    #[serde(default = "zero")]
    pub roundness: Animatable,
    /// Edge softness, 0..=100.
    pub feather: Animatable,
    #[serde(default)]
    pub invert: bool,
}

impl Mask {
    pub fn new(shape: MaskShape) -> Self {
        let (w, h) = match shape {
            MaskShape::Line => (1.0, 0.5),
            MaskShape::DoubleLine => (1.0, 0.3),
            _ => (0.5, 0.5),
        };
        Mask {
            shape,
            x: Animatable::constant(0.5),
            y: Animatable::constant(0.5),
            width: Animatable::constant(w),
            height: Animatable::constant(h),
            rotation: zero(),
            roundness: zero(),
            feather: Animatable::constant(2.0),
            invert: false,
        }
    }
}

/// Uses the clip on the track directly above as a stencil for this clip.
/// The matte clip itself is then hidden.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackMatte {
    #[default]
    None,
    Alpha,
    AlphaInverted,
    Luma,
    LumaInverted,
}

impl TrackMatte {
    pub const ALL: [TrackMatte; 5] = [
        TrackMatte::None,
        TrackMatte::Alpha,
        TrackMatte::AlphaInverted,
        TrackMatte::Luma,
        TrackMatte::LumaInverted,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrackMatte::None => "None",
            TrackMatte::Alpha => "Alpha Matte",
            TrackMatte::AlphaInverted => "Alpha Inverted Matte",
            TrackMatte::Luma => "Luma Matte",
            TrackMatte::LumaInverted => "Luma Inverted Matte",
        }
    }
}

/// The rest of the Video tab: anchor, corners, background, shadow,
/// mask, matte, motion blur, stabilization, flicker removal, lens
/// correction and auto enhance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VideoSettings {
    /// Point the clip rotates and scales around, as an offset from the
    /// picture's centre in project pixels.
    pub anchor: [f64; 2],
    /// Rounded corners, in project pixels.
    pub corner_radius: f64,
    /// Blur along the clip's own movement, 0 = off ..= 100.
    pub motion_blur: f64,
    pub background: ClipBackground,
    pub shadow: Shadow,
    pub mask: Option<Mask>,
    pub track_matte: TrackMatte,
    /// Smooths out camera shake, 0 = off ..= 100 (needs a one-off analysis).
    pub stabilize: f64,
    /// Removes brightness flicker from lights and time-lapses.
    pub deflicker: bool,
    /// Straightens wide-angle (fisheye) bending, -100 ..= 100, 0 = off.
    pub lens_correction: f64,
    /// Automatic levels and white balance, 0 = off ..= 100.
    pub auto_enhance: f64,
    /// The split-screen cell the clip is shown in, if any.
    pub cell: Option<Cell>,
}

impl VideoSettings {
    pub fn is_default(&self) -> bool {
        *self == VideoSettings::default()
    }
}

/// A piece of media (or generated content) placed on a track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    /// The media file this clip plays; `None` for generated clips.
    #[serde(default)]
    pub media_id: Option<MediaId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<Generator>,
    /// Where the clip begins on the timeline.
    pub start: Time,
    /// The part of the source that plays: `source_in..source_out`.
    /// Generated clips use `0..length`.
    pub source_in: Time,
    pub source_out: Time,
    /// Playback speed, 1 = normal. The clip lasts `source length / speed`.
    /// Ignored while `ramp` is set.
    #[serde(default = "one")]
    pub speed: f64,
    /// A speed that changes over the clip (speed ramping).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<SpeedRamp>,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Crop::is_none")]
    pub crop: Crop,
    #[serde(default)]
    pub blend: BlendMode,
    #[serde(default, skip_serializing_if = "ColorAdjust::is_neutral")]
    pub color: ColorAdjust,
    #[serde(default, skip_serializing_if = "Enhance::is_off")]
    pub enhance: Enhance,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_in: Option<Transition>,
    /// Transition to nothing at the end (only used when no clip follows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_out: Option<Transition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_in: Option<ClipAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim_out: Option<ClipAnimation>,
    /// Percent, 100 = unchanged.
    #[serde(default = "hundred")]
    pub volume: Animatable,
    #[serde(default, skip_serializing_if = "AudioSettings::is_default")]
    pub audio: AudioSettings,
    #[serde(default, skip_serializing_if = "VideoSettings::is_default")]
    pub video: VideoSettings,
    /// Skipped in playback and export but kept on the timeline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Name shown on the timeline instead of the file name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Color label (index into the timeline's mark colors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<u8>,
    /// Clips with the same link (a video clip and the audio detached from
    /// it) are cut and deleted together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Uuid>,
}

impl Clip {
    fn blank(start: Time, len: Time) -> Self {
        Clip {
            id: Uuid::new_v4(),
            media_id: None,
            generator: None,
            start,
            source_in: Time::ZERO,
            source_out: len,
            speed: 1.0,
            ramp: None,
            reverse: false,
            transform: Transform::default(),
            crop: Crop::default(),
            blend: BlendMode::Normal,
            color: ColorAdjust::default(),
            enhance: Enhance::default(),
            effects: Vec::new(),
            transition_in: None,
            transition_out: None,
            anim_in: None,
            anim_out: None,
            volume: hundred(),
            audio: AudioSettings::default(),
            video: VideoSettings::default(),
            disabled: false,
            name: None,
            mark: None,
            link: None,
        }
    }

    pub fn new(media: &MediaItem, start: Time) -> Self {
        let mut c = Clip::blank(start, media.duration);
        c.media_id = Some(media.id);
        c
    }

    pub fn generated(generator: Generator, start: Time, len: Time) -> Self {
        let mut c = Clip::blank(start, len);
        c.generator = Some(generator);
        c
    }

    pub fn is_generated(&self) -> bool {
        self.generator.is_some()
    }

    pub fn is_adjustment(&self) -> bool {
        matches!(self.generator, Some(Generator::Adjustment))
    }

    /// An empty split-screen cell.
    pub fn is_placeholder(&self) -> bool {
        matches!(self.generator, Some(Generator::Placeholder))
    }

    pub fn title(&self) -> Option<&TitleSpec> {
        match &self.generator {
            Some(Generator::Title(t)) => Some(t),
            _ => None,
        }
    }

    pub fn title_mut(&mut self) -> Option<&mut TitleSpec> {
        match &mut self.generator {
            Some(Generator::Title(t)) => Some(t),
            _ => None,
        }
    }

    /// Length of source material used, before speed.
    pub fn source_len(&self) -> Time {
        self.source_out - self.source_in
    }

    /// Length on the timeline.
    pub fn duration(&self) -> Time {
        self.play_time(self.source_in, self.source_out)
    }

    /// How long the source from `a` to `b` lasts on the timeline.
    pub fn play_time(&self, a: Time, b: Time) -> Time {
        match &self.ramp {
            Some(r) => Time::from_secs_f64(r.play_secs(a.as_secs_f64(), b.as_secs_f64())),
            None => (b - a).scale(1.0 / self.speed.max(1e-6)),
        }
    }

    /// Source seconds played after `local` seconds of the clip (negative
    /// before its start), at full precision for audio.
    pub fn played_secs(&self, local: f64) -> f64 {
        let (src_in, src_out) = (self.source_in.as_secs_f64(), self.source_out.as_secs_f64());
        match &self.ramp {
            None => local * self.speed,
            Some(r) if self.reverse => src_out - r.advance(src_out, -local),
            Some(r) => r.advance(src_in, local) - src_in,
        }
    }

    /// The source time (seconds) showing `local` seconds into the clip.
    pub fn source_secs(&self, local: f64) -> f64 {
        let played = self.played_secs(local);
        if self.reverse {
            self.source_out.as_secs_f64() - played
        } else {
            self.source_in.as_secs_f64() + played
        }
    }

    /// Seconds into the clip at which source time `s` shows: the inverse
    /// of [`Clip::source_secs`].
    pub fn local_secs(&self, s: f64) -> f64 {
        let (src_in, src_out) = (self.source_in.as_secs_f64(), self.source_out.as_secs_f64());
        match &self.ramp {
            Some(r) if self.reverse => r.play_secs(s, src_out),
            Some(r) => r.play_secs(src_in, s),
            None if self.reverse => (src_out - s) / self.speed.max(1e-6),
            None => (s - src_in) / self.speed.max(1e-6),
        }
    }

    /// The source time reached by playing on for `by` from source time
    /// `from`, in the clip's direction of play (used to move its ends).
    pub fn source_step(&self, from: Time, by: Time) -> Time {
        match &self.ramp {
            Some(r) => {
                let secs = if self.reverse { -by } else { by }.as_secs_f64();
                Time::from_secs_f64(r.advance(from.as_secs_f64(), secs))
            }
            None if self.reverse => from - by.scale(self.speed),
            None => from + by.scale(self.speed),
        }
    }

    /// The speed `local` seconds into the clip.
    pub fn speed_at(&self, local: f64) -> f64 {
        match &self.ramp {
            Some(r) => r.speed_at(self.source_secs(local)),
            None => self.speed,
        }
    }

    pub fn end(&self) -> Time {
        self.start + self.duration()
    }

    pub fn contains(&self, t: Time) -> bool {
        t >= self.start && t < self.end()
    }

    /// Time relative to the clip's start, which keyframes are measured in.
    pub fn local(&self, t: Time) -> Time {
        t - self.start
    }

    /// Maps a timeline time to the matching time in the source file.
    /// Times outside the clip extend past its ends (used by transitions).
    pub fn source_time(&self, t: Time) -> Time {
        if self.ramp.is_some() {
            return Time::from_secs_f64(self.source_secs((t - self.start).as_secs_f64()));
        }
        let played = (t - self.start).scale(self.speed);
        if self.reverse {
            self.source_out - played
        } else {
            self.source_in + played
        }
    }

    /// Whether the source has natural limits (videos and audio do; images
    /// and generated clips can be stretched to any length).
    pub fn has_fixed_length(&self, media: Option<&MediaItem>) -> bool {
        self.generator.is_none() && media.is_some_and(|m| m.kind != MediaKind::Image)
    }

    /// Calls `f` on every keyframable property.
    pub fn for_each_animatable(&mut self, mut f: impl FnMut(&mut Animatable)) {
        let t = &mut self.transform;
        for a in [
            &mut t.scale_x,
            &mut t.scale_y,
            &mut t.position_x,
            &mut t.position_y,
            &mut t.rotation,
            &mut t.opacity,
        ] {
            f(a);
        }
        for (_, a) in self.color.fields_mut() {
            f(a);
        }
        f(&mut self.volume);
        if let Some(m) = &mut self.video.mask {
            for a in [
                &mut m.x,
                &mut m.y,
                &mut m.width,
                &mut m.height,
                &mut m.rotation,
                &mut m.roundness,
                &mut m.feather,
            ] {
                f(a);
            }
        }
        for e in &mut self.effects {
            for a in e.params.values_mut() {
                f(a);
            }
        }
    }

    /// Whether any property has keyframes.
    pub fn is_animated(&mut self) -> bool {
        let mut any = false;
        self.for_each_animatable(|a| any |= a.is_animated());
        any
    }
}
