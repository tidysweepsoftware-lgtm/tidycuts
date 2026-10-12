//! Video effects: shader passes from `effects.wgsl` and motion effects.

use super::animations::Motion;
use super::{ColorDef, ParamDef, Sample};
use crate::motion::Spring;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectKind {
    /// `passes` runs of shader `id` in `effects.wgsl` (the pass index is
    /// handed to the shader in `u.flags`).
    Shader { id: u32, passes: u32 },
    /// Moves the clip instead of changing its pixels; see [`motion`].
    Motion,
}

#[derive(Debug, Clone, Copy)]
pub struct EffectDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub kind: EffectKind,
    pub params: &'static [ParamDef],
    pub colors: &'static [ColorDef],
    pub sample: Sample,
}

/// Shader id of "Lens Distortion" (`distort.lens`): p0.x is the amount in
/// -1..1, and negative values straighten barrel distortion, so the clip's
/// lens correction can reuse it.
pub const LENS_SHADER: u32 = 23;

const fn shader(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    shader: u32,
    params: &'static [ParamDef],
    colors: &'static [ColorDef],
    sample: Sample,
) -> EffectDef {
    EffectDef {
        id,
        name,
        category,
        kind: EffectKind::Shader {
            id: shader,
            passes: 1,
        },
        params,
        colors,
        sample,
    }
}

/// A separable effect run twice (horizontal, then vertical).
const fn two_pass(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    shader: u32,
    params: &'static [ParamDef],
    sample: Sample,
) -> EffectDef {
    EffectDef {
        id,
        name,
        category,
        kind: EffectKind::Shader {
            id: shader,
            passes: 2,
        },
        params,
        colors: &[],
        sample,
    }
}

const fn moving(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    params: &'static [ParamDef],
) -> EffectDef {
    EffectDef {
        id,
        name,
        category,
        kind: EffectKind::Motion,
        params,
        colors: &[],
        sample: Sample::Landscape,
    }
}

const fn color(key: &'static str, label: &'static str, r: f32, g: f32, b: f32) -> ColorDef {
    ColorDef {
        key,
        label,
        default: [r, g, b, 1.0],
    }
}

const fn pct(key: &'static str, label: &'static str, default: f32) -> ParamDef {
    ParamDef::percent(key, label, default)
}

/// The styles of the flicker shader (shader 92), in order.
const FLICKER_STYLES: &[&str] = &[
    "Negative",
    "Colour flash",
    "RGB stroke",
    "Neon swing",
    "Hallucination",
    "Exposure jitter",
    "Hue cycle",
    "Strobe",
    "Colour slice",
    "Pulse flash",
];

/// The styles of the glass shader (shader 93).
const GLASS_STYLES: &[&str] = &["Shatter", "Cracked", "Prism shards"];
/// The styles of the grunge shader (shader 96).
const GRUNGE_STYLES: &[&str] = &["Dust and scratches", "Grime", "Crumpled paper"];
/// The styles of the neon frame shader (shader 97).
const NEON_STYLES: &[&str] = &["Steady", "Chase", "Pulse", "Two colours"];
/// The styles of the 3D layout shader (shader 99).
const LAYOUT_STYLES: &[&str] = &["Fan of three", "Stairs", "Floating card"];
/// The styles of the beat shader (shader 100), in order.
const BEAT_STYLES: &[&str] = &[
    "Zoom pump",
    "Shake kick",
    "Colour pop",
    "Blur thump",
    "Strobe",
    "Hue kick",
    "Squash and bounce",
    "Vignette pulse",
];
/// The sound-reactive shader.
pub const AUDIO_SHADER: u32 = 101;
/// What a sound-reactive effect listens to, in order.
const REACT_BANDS: &[&str] = &["Everything", "Bass", "Mid", "Treble"];
/// The looks of the sound-reactive shader, in order.
const REACT_STYLES: &[&str] = &[
    "Zoom",
    "Shake",
    "Colour pop",
    "Flash",
    "Colour split",
    "Blur",
    "Glow",
    "Vignette",
    "Hue shift",
    "Tear",
];
const AMOUNT: ParamDef = ParamDef::percent("amount", "Amount", 0.5);
const SPEED: ParamDef = ParamDef::percent("speed", "Speed", 0.5);
const CENTER_X: ParamDef = ParamDef::percent("center_x", "Center X", 0.5);
const CENTER_Y: ParamDef = ParamDef::percent("center_y", "Center Y", 0.5);
const BPM: ParamDef = ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 100.0, " bpm");
const ANGLE: ParamDef = ParamDef::value("angle", "Angle", -180.0, 180.0, 0.0, "°");
const GAP: ParamDef = ParamDef::percent("gap", "Gap", 0.3);
const DURATION: ParamDef = ParamDef::value("duration", "Duration", 1.0, 30.0, 6.0, "s");

/// Amount, speed and size for particle effects.
const PARTICLES: &[ParamDef] = &[
    pct("amount", "Amount", 0.5),
    pct("speed", "Speed", 0.4),
    pct("size", "Size", 0.5),
];

const BLACK: ColorDef = color("color", "Color", 0.0, 0.0, 0.0);
const WHITE: ColorDef = color("color", "Color", 1.0, 1.0, 1.0);

use Sample::{Landscape, Meadow, Nap, Night, Pet, Portrait, Sunset, Wildlife};

pub static EFFECTS: &[EffectDef] = &[
    // ---- Basic ----
    shader(
        "basic.chroma_key",
        "Chroma Key",
        "Basic",
        16,
        &[
            pct("tolerance", "Tolerance", 0.4),
            pct("softness", "Softness", 0.3),
            pct("spill", "Spill Removal", 0.5),
        ],
        &[color("key", "Key Color", 0.0, 0.8, 0.2)],
        Landscape,
    ),
    shader("basic.invert", "Invert", "Basic", 6, &[], &[], Sunset),
    shader(
        "basic.mosaic",
        "Mosaic",
        "Basic",
        5,
        &[pct("size", "Size", 0.3)],
        &[],
        Pet,
    ),
    shader(
        "basic.mirror",
        "Mirror",
        "Basic",
        8,
        &[ParamDef::choice(
            "mode",
            "Mode",
            &["Left", "Right", "Top", "Bottom", "Quad"],
            0,
        )],
        &[],
        Sunset,
    ),
    shader(
        "basic.posterize",
        "Posterize",
        "Basic",
        35,
        &[ParamDef::value("levels", "Levels", 2.0, 12.0, 4.0, "")],
        &[],
        Sunset,
    ),
    shader(
        "basic.color_splash",
        "Color Splash",
        "Basic",
        36,
        &[
            pct("tolerance", "Tolerance", 0.45),
            pct("softness", "Softness", 0.4),
        ],
        &[color("key", "Keep Color", 0.9, 0.2, 0.25)],
        Sunset,
    ),
    shader(
        "basic.duotone",
        "Duotone",
        "Basic",
        37,
        &[pct("contrast", "Contrast", 0.3)],
        &[
            color("shadows", "Shadows", 0.12, 0.1, 0.42),
            color("highlights", "Highlights", 1.0, 0.62, 0.45),
        ],
        Portrait,
    ),
    shader(
        "basic.sketch",
        "Sketch",
        "Basic",
        38,
        &[pct("strength", "Strength", 0.6), pct("color", "Color", 0.0)],
        &[],
        Wildlife,
    ),
    shader(
        "basic.comic",
        "Comic",
        "Basic",
        39,
        &[
            pct("outlines", "Outlines", 0.5),
            pct("levels", "Color Levels", 0.4),
        ],
        &[],
        Nap,
    ),
    shader(
        "basic.emboss",
        "Emboss",
        "Basic",
        40,
        &[
            pct("strength", "Strength", 0.5),
            ParamDef::value("angle", "Light Angle", -180.0, 180.0, 135.0, "°"),
            pct("color", "Color", 0.0),
        ],
        &[],
        Meadow,
    ),
    shader(
        "basic.thermal",
        "Thermal",
        "Basic",
        41,
        &[pct("amount", "Amount", 1.0)],
        &[],
        Portrait,
    ),
    shader(
        "basic.night_vision",
        "Night Vision",
        "Basic",
        42,
        &[
            pct("brightness", "Brightness", 0.5),
            pct("noise", "Noise", 0.4),
            ParamDef::choice("mask", "Mask", &["None", "Scope", "Binoculars"], 0),
        ],
        &[],
        Landscape,
    ),
    // ---- Cinematic ----
    shader(
        "cinematic.halation",
        "Halation",
        "Cinematic",
        74,
        &[
            pct("amount", "Amount", 0.6),
            pct("radius", "Radius", 0.5),
            pct("threshold", "Threshold", 0.5),
        ],
        &[color("color", "Glow", 1.0, 0.38, 0.14)],
        Sunset,
    ),
    shader(
        "cinematic.pro_mist",
        "Pro-Mist",
        "Cinematic",
        75,
        &[
            pct("amount", "Amount", 0.5),
            pct("spread", "Spread", 0.5),
            pct("highlights", "Highlights", 0.6),
        ],
        &[],
        Portrait,
    ),
    shader(
        "cinematic.dream_glow",
        "Dream Glow",
        "Cinematic",
        76,
        &[
            pct("amount", "Amount", 0.6),
            pct("softness", "Softness", 0.6),
        ],
        &[color("color", "Tint", 1.0, 0.82, 0.92)],
        Meadow,
    ),
    shader(
        "cinematic.anamorphic",
        "Anamorphic Streaks",
        "Cinematic",
        77,
        &[
            pct("amount", "Amount", 0.6),
            pct("length", "Length", 0.6),
            pct("threshold", "Threshold", 0.65),
        ],
        &[color("color", "Streak", 0.4, 0.65, 1.0)],
        Night,
    ),
    shader(
        "cinematic.light_sweep",
        "Light Sweep",
        "Cinematic",
        78,
        &[
            pct("speed", "Speed", 0.4),
            pct("width", "Width", 0.3),
            ParamDef::value("angle", "Angle", -90.0, 90.0, 20.0, "°"),
            pct("intensity", "Intensity", 0.7),
        ],
        &[WHITE],
        Portrait,
    ),
    shader(
        "cinematic.spotlight",
        "Spotlight",
        "Cinematic",
        79,
        &[
            pct("size", "Size", 0.35),
            pct("softness", "Softness", 0.45),
            pct("darkness", "Darkness", 0.75),
            CENTER_X,
            pct("center_y", "Center Y", 0.42),
        ],
        &[],
        Portrait,
    ),
    // ---- Trendy ----
    shader(
        "trendy.beat_flash",
        "Beat Flash",
        "Trendy",
        80,
        &[
            BPM,
            pct("intensity", "Intensity", 0.6),
            pct("decay", "Decay", 0.5),
        ],
        &[WHITE],
        Pet,
    ),
    shader(
        "trendy.rgb_pulse",
        "RGB Pulse",
        "Trendy",
        81,
        &[BPM, pct("amount", "Amount", 0.6)],
        &[],
        Portrait,
    ),
    shader(
        "trendy.speed_lines",
        "Speed Lines",
        "Trendy",
        82,
        &[
            pct("density", "Density", 0.6),
            pct("clear", "Clear Area", 0.4),
            pct("speed", "Speed", 0.5),
            pct("opacity", "Opacity", 0.85),
        ],
        &[WHITE],
        Wildlife,
    ),
    shader(
        "trendy.echo",
        "Echo",
        "Trendy",
        83,
        &[
            pct("amount", "Amount", 0.8),
            pct("distance", "Distance", 0.6),
            ParamDef::value("angle", "Angle", -180.0, 180.0, 0.0, "°"),
            pct("speed", "Speed", 0.4),
        ],
        &[],
        Pet,
    ),
    shader(
        "trendy.camcorder",
        "Camcorder",
        "Trendy",
        84,
        &[pct("size", "Size", 0.5), pct("opacity", "Opacity", 1.0)],
        &[],
        Nap,
    ),
    shader(
        "trendy.fluid_colour",
        "Fluid Colour",
        "Trendy",
        85,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.35),
            pct("scale", "Scale", 0.5),
        ],
        &[
            color("color", "Colour 1", 1.0, 0.25, 0.65),
            color("color2", "Colour 2", 0.2, 0.55, 1.0),
        ],
        Portrait,
    ),
    shader(
        "trendy.prism",
        "Prism",
        "Trendy",
        86,
        &[
            ParamDef::choice("mode", "Mode", &["Radial", "Linear"], 0),
            pct("amount", "Amount", 0.5),
            ParamDef::value("angle", "Angle", -180.0, 180.0, 0.0, "°"),
            pct("intensity", "Intensity", 0.7),
        ],
        &[],
        Sunset,
    ),
    shader(
        "trendy.liquid",
        "Liquid Warp",
        "Trendy",
        87,
        &[
            pct("amount", "Amount", 0.7),
            pct("scale", "Scale", 0.4),
            pct("speed", "Speed", 0.4),
        ],
        &[],
        Landscape,
    ),
    shader(
        "trendy.shockwave",
        "Shockwave",
        "Trendy",
        91,
        &[
            pct("strength", "Strength", 0.6),
            pct("speed", "Speed", 0.5),
            pct("x", "Centre X", 0.5),
            pct("y", "Centre Y", 0.5),
        ],
        &[],
        Landscape,
    ),
    moving(
        "trendy.punch_zoom",
        "Punch Zoom",
        "Trendy",
        &[BPM, pct("amount", "Amount", 0.6)],
    ),
    moving(
        "trendy.impact",
        "Impact Hit",
        "Trendy",
        &[pct("amount", "Amount", 0.6), pct("speed", "Speed", 0.4)],
    ),
    // ---- Blur ----
    two_pass(
        "blur.gaussian",
        "Blur",
        "Blur",
        1,
        &[pct("radius", "Radius", 0.4)],
        Pet,
    ),
    two_pass(
        "blur.area",
        "Blur Area",
        "Blur",
        72,
        &[
            ParamDef::choice("shape", "Shape", &["Ellipse", "Rectangle", "Rounded"], 0),
            pct("blur", "Blur", 0.8),
            CENTER_X,
            pct("center_y", "Center Y", 0.4),
            pct("width", "Width", 0.17),
            pct("height", "Height", 0.4),
            pct("feather", "Feather", 0.3),
            ParamDef::toggle("outside", "Blur Outside", false),
        ],
        Portrait,
    ),
    shader(
        "blur.pixelate_area",
        "Pixelate Area",
        "Blur",
        73,
        &[
            ParamDef::choice("shape", "Shape", &["Ellipse", "Rectangle", "Rounded"], 0),
            pct("size", "Block Size", 0.45),
            CENTER_X,
            pct("center_y", "Center Y", 0.4),
            pct("width", "Width", 0.17),
            pct("height", "Height", 0.4),
            pct("feather", "Feather", 0.0),
            ParamDef::toggle("outside", "Pixelate Outside", false),
        ],
        &[],
        Portrait,
    ),
    shader(
        "blur.motion",
        "Motion Blur",
        "Blur",
        4,
        &[pct("amount", "Amount", 0.6), ANGLE],
        &[],
        Wildlife,
    ),
    shader(
        "blur.zoom",
        "Zoom Blur",
        "Blur",
        3,
        &[pct("amount", "Amount", 0.5), CENTER_X, CENTER_Y],
        &[],
        Sunset,
    ),
    shader(
        "blur.spin",
        "Spin Blur",
        "Blur",
        2,
        &[pct("amount", "Amount", 0.4)],
        &[],
        Sunset,
    ),
    two_pass(
        "blur.tilt_shift",
        "Tilt-Shift",
        "Blur",
        31,
        &[
            pct("blur", "Blur", 0.6),
            pct("position", "Focus Position", 0.62),
            pct("focus", "Focus Size", 0.25),
            pct("saturation", "Saturation", 0.4),
        ],
        Sunset,
    ),
    two_pass(
        "blur.focus",
        "Focus Blur",
        "Blur",
        32,
        &[
            pct("blur", "Blur", 0.6),
            pct("focus", "Focus Size", 0.35),
            CENTER_X,
            CENTER_Y,
        ],
        Portrait,
    ),
    shader(
        "blur.lens",
        "Lens Blur",
        "Blur",
        33,
        &[
            pct("radius", "Radius", 0.5),
            pct("highlights", "Bokeh Highlights", 0.6),
        ],
        &[],
        Sunset,
    ),
    shader(
        "blur.frosted",
        "Frosted Glass",
        "Blur",
        34,
        &[pct("amount", "Amount", 0.5), pct("grain", "Grain", 0.3)],
        &[],
        Nap,
    ),
    // ---- Shake ----
    moving("shake.camera", "Camera Shake", "Shake", &[AMOUNT, SPEED]),
    moving(
        "shake.earthquake",
        "Earthquake",
        "Shake",
        &[pct("amount", "Amount", 0.6), SPEED],
    ),
    moving(
        "shake.handheld",
        "Handheld",
        "Shake",
        &[AMOUNT, pct("speed", "Speed", 0.4)],
    ),
    moving(
        "shake.jitter",
        "Jitter",
        "Shake",
        &[pct("amount", "Amount", 0.4), SPEED],
    ),
    moving(
        "shake.sway",
        "Sway",
        "Shake",
        &[AMOUNT, pct("speed", "Speed", 0.4)],
    ),
    moving("shake.bounce", "Bounce", "Shake", &[AMOUNT, SPEED]),
    // ---- Zoom ----
    moving(
        "zoom.pulse",
        "Zoom Pulse",
        "Zoom",
        &[pct("amount", "Amount", 0.4), pct("speed", "Speed", 0.4)],
    ),
    moving(
        "zoom.heartbeat",
        "Heartbeat",
        "Zoom",
        &[
            AMOUNT,
            ParamDef::value("bpm", "Beats per Minute", 40.0, 180.0, 72.0, " bpm"),
        ],
    ),
    moving(
        "zoom.ken_burns_in",
        "Ken Burns In",
        "Zoom",
        &[pct("zoom", "Zoom", 0.25), DURATION],
    ),
    moving(
        "zoom.ken_burns_out",
        "Ken Burns Out",
        "Zoom",
        &[pct("zoom", "Zoom", 0.25), DURATION],
    ),
    moving(
        "zoom.pan_left",
        "Pan Left",
        "Zoom",
        &[pct("zoom", "Zoom", 0.2), DURATION],
    ),
    moving(
        "zoom.pan_right",
        "Pan Right",
        "Zoom",
        &[pct("zoom", "Zoom", 0.2), DURATION],
    ),
    moving(
        "zoom.spin",
        "Spin",
        "Zoom",
        &[
            ParamDef::value("speed", "Speed", -360.0, 360.0, 45.0, "°/s"),
            ParamDef::toggle("fill", "Fill Frame", false),
        ],
    ),
    // ---- Distortion ----
    shader(
        "distort.lens",
        "Lens Distortion",
        "Distortion",
        LENS_SHADER,
        &[ParamDef::value("amount", "Amount", -1.0, 1.0, 0.3, "")],
        &[],
        Meadow,
    ),
    shader(
        "distort.fisheye",
        "Fisheye",
        "Distortion",
        24,
        &[
            pct("strength", "Strength", 0.6),
            ParamDef::toggle("circle", "Circular", false),
        ],
        &[],
        Sunset,
    ),
    shader(
        "distort.bulge",
        "Bulge",
        "Distortion",
        25,
        &[
            pct("amount", "Amount", 0.7),
            pct("radius", "Radius", 0.6),
            CENTER_X,
            CENTER_Y,
        ],
        &[],
        Landscape,
    ),
    shader(
        "distort.pinch",
        "Pinch",
        "Distortion",
        26,
        &[
            pct("amount", "Amount", 0.7),
            pct("radius", "Radius", 0.6),
            CENTER_X,
            CENTER_Y,
        ],
        &[],
        Portrait,
    ),
    shader(
        "distort.twirl",
        "Twirl",
        "Distortion",
        27,
        &[
            ParamDef::value("angle", "Angle", -720.0, 720.0, 200.0, "°"),
            pct("radius", "Radius", 0.6),
            pct("speed", "Speed", 0.0),
        ],
        &[],
        Sunset,
    ),
    shader(
        "distort.ripple",
        "Ripple",
        "Distortion",
        28,
        &[
            pct("amount", "Amount", 0.5),
            pct("wavelength", "Wavelength", 0.4),
            pct("speed", "Speed", 0.4),
            CENTER_X,
            CENTER_Y,
        ],
        &[],
        Pet,
    ),
    shader(
        "distort.wave",
        "Wave",
        "Distortion",
        15,
        &[
            pct("amplitude", "Amplitude", 0.4),
            pct("frequency", "Frequency", 0.4),
            SPEED,
        ],
        &[],
        Sunset,
    ),
    shader(
        "distort.heat",
        "Heat Haze",
        "Distortion",
        29,
        &[AMOUNT, SPEED, pct("scale", "Scale", 0.5)],
        &[],
        Wildlife,
    ),
    shader(
        "distort.kaleidoscope",
        "Kaleidoscope",
        "Distortion",
        9,
        &[
            pct("segments", "Segments", 0.4),
            pct("speed", "Rotation", 0.2),
        ],
        &[],
        Sunset,
    ),
    shader(
        "distort.glass_tiles",
        "Glass Tiles",
        "Distortion",
        30,
        &[
            pct("size", "Tile Size", 0.4),
            pct("refraction", "Refraction", 0.5),
        ],
        &[],
        Nap,
    ),
    // ---- Glitch ----
    shader(
        "glitch.digital",
        "Digital Glitch",
        "Glitch",
        11,
        &[AMOUNT, SPEED],
        &[],
        Sunset,
    ),
    shader(
        "glitch.rgb_split",
        "RGB Split",
        "Glitch",
        10,
        &[AMOUNT, SPEED],
        &[],
        Meadow,
    ),
    shader(
        "glitch.chromatic",
        "Chromatic Aberration",
        "Glitch",
        46,
        &[AMOUNT],
        &[],
        Landscape,
    ),
    shader(
        "glitch.slice",
        "Slice",
        "Glitch",
        47,
        &[pct("amount", "Amount", 0.6), SPEED],
        &[],
        Portrait,
    ),
    shader(
        "glitch.bad_signal",
        "Bad Signal",
        "Glitch",
        48,
        &[AMOUNT, SPEED],
        &[],
        Pet,
    ),
    shader(
        "glitch.datamosh",
        "Datamosh",
        "Glitch",
        49,
        &[AMOUNT, SPEED, pct("size", "Block Size", 0.4)],
        &[],
        Sunset,
    ),
    // ---- Retro ----
    shader(
        "retro.vhs",
        "VHS",
        "Retro",
        12,
        &[pct("amount", "Amount", 0.75)],
        &[],
        Wildlife,
    ),
    shader(
        "retro.old_film",
        "Old Film",
        "Retro",
        13,
        &[pct("amount", "Amount", 0.7)],
        &[],
        Portrait,
    ),
    shader(
        "retro.grain",
        "Film Grain",
        "Retro",
        14,
        &[pct("amount", "Amount", 0.4), pct("size", "Size", 0.3)],
        &[],
        Nap,
    ),
    shader(
        "retro.halftone",
        "Halftone",
        "Retro",
        7,
        &[pct("size", "Dot Size", 0.35)],
        &[],
        Portrait,
    ),
    shader(
        "retro.crt",
        "CRT Screen",
        "Retro",
        43,
        &[
            pct("scanlines", "Scanlines", 0.6),
            pct("curvature", "Curvature", 0.5),
        ],
        &[],
        Sunset,
    ),
    shader(
        "retro.led",
        "LED Screen",
        "Retro",
        44,
        &[pct("size", "Dot Size", 0.3), pct("glow", "Glow", 0.5)],
        &[],
        Sunset,
    ),
    shader(
        "retro.pixel_art",
        "Pixel Art",
        "Retro",
        45,
        &[pct("size", "Pixel Size", 0.5), pct("colors", "Colors", 0.6)],
        &[],
        Meadow,
    ),
    // ---- Light ----
    shader(
        "light.leak",
        "Light Leak",
        "Light",
        17,
        &[
            pct("intensity", "Intensity", 0.6),
            pct("speed", "Speed", 0.4),
        ],
        &[color("color", "Color", 1.0, 0.55, 0.2)],
        Landscape,
    ),
    shader(
        "light.bloom",
        "Bloom",
        "Light",
        50,
        &[
            pct("intensity", "Intensity", 0.7),
            pct("threshold", "Threshold", 0.4),
            pct("radius", "Radius", 0.5),
        ],
        &[],
        Sunset,
    ),
    shader(
        "light.soft_glow",
        "Soft Glow",
        "Light",
        51,
        &[AMOUNT, pct("radius", "Radius", 0.5)],
        &[],
        Portrait,
    ),
    shader(
        "light.rays",
        "Light Rays",
        "Light",
        52,
        &[
            pct("intensity", "Intensity", 0.7),
            pct("x", "Source X", 0.72),
            pct("y", "Source Y", 0.3),
            pct("length", "Length", 0.6),
        ],
        &[color("color", "Color", 1.0, 0.9, 0.7)],
        Pet,
    ),
    shader(
        "light.lens_flare",
        "Lens Flare",
        "Light",
        53,
        &[
            pct("intensity", "Intensity", 0.8),
            pct("x", "Source X", 0.72),
            pct("y", "Source Y", 0.3),
        ],
        &[color("color", "Color", 1.0, 0.85, 0.6)],
        Wildlife,
    ),
    shader(
        "light.neon_edges",
        "Neon Edges",
        "Light",
        54,
        &[
            pct("glow", "Glow", 0.6),
            pct("background", "Background", 0.15),
            pct("speed", "Color Speed", 0.3),
        ],
        &[
            color("color", "Color 1", 0.1, 0.9, 1.0),
            color("color2", "Color 2", 1.0, 0.2, 0.75),
        ],
        Sunset,
    ),
    shader(
        "light.film_burn",
        "Film Burn",
        "Light",
        55,
        &[AMOUNT, SPEED],
        &[color("color", "Color", 1.0, 0.4, 0.08)],
        Portrait,
    ),
    // ---- Particles ----
    shader(
        "particles.snow",
        "Snow",
        "Particles",
        18,
        &[
            pct("amount", "Amount", 0.5),
            pct("speed", "Speed", 0.4),
            pct("size", "Size", 0.4),
        ],
        &[],
        Nap,
    ),
    shader(
        "particles.rain",
        "Rain",
        "Particles",
        56,
        &[
            pct("amount", "Amount", 0.6),
            SPEED,
            pct("wind", "Wind", 0.6),
        ],
        &[],
        Meadow,
    ),
    shader(
        "particles.bubbles",
        "Bubbles",
        "Particles",
        57,
        PARTICLES,
        &[],
        Landscape,
    ),
    shader(
        "particles.confetti",
        "Confetti",
        "Particles",
        58,
        &[
            pct("amount", "Amount", 0.6),
            SPEED,
            pct("size", "Size", 0.5),
        ],
        &[],
        Sunset,
    ),
    shader(
        "particles.fireflies",
        "Fireflies",
        "Particles",
        59,
        &[
            AMOUNT,
            pct("speed", "Speed", 0.4),
            pct("size", "Size", 0.65),
        ],
        &[color("color", "Color", 1.0, 0.85, 0.35)],
        Sunset,
    ),
    shader(
        "particles.hearts",
        "Hearts",
        "Particles",
        60,
        PARTICLES,
        &[color("color", "Color", 1.0, 0.25, 0.45)],
        Portrait,
    ),
    shader(
        "particles.sparkles",
        "Sparkles",
        "Particles",
        61,
        &[AMOUNT, SPEED, pct("size", "Size", 0.65)],
        &[color("color", "Color", 1.0, 0.95, 0.8)],
        Sunset,
    ),
    shader(
        "particles.dust",
        "Dust Motes",
        "Particles",
        88,
        PARTICLES,
        &[color("color", "Color", 1.0, 0.92, 0.75)],
        Sunset,
    ),
    shader(
        "particles.embers",
        "Embers",
        "Particles",
        89,
        PARTICLES,
        &[color("color", "Color", 1.0, 0.5, 0.15)],
        Night,
    ),
    shader(
        "particles.mist",
        "Mist",
        "Particles",
        90,
        &[
            pct("amount", "Amount", 0.85),
            pct("speed", "Speed", 0.35),
            pct("height", "Height", 0.6),
        ],
        &[color("color", "Color", 0.86, 0.9, 1.0)],
        Landscape,
    ),
    // ---- Overlay ----
    shader(
        "overlay.vignette",
        "Vignette",
        "Overlay",
        62,
        &[
            pct("amount", "Amount", 0.7),
            pct("size", "Size", 0.45),
            pct("softness", "Softness", 0.5),
            pct("roundness", "Roundness", 0.3),
        ],
        &[BLACK],
        Pet,
    ),
    shader(
        "overlay.bokeh",
        "Bokeh Lights",
        "Overlay",
        63,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.4),
            pct("size", "Size", 0.5),
        ],
        &[
            color("color", "Color 1", 1.0, 0.75, 0.35),
            color("color2", "Color 2", 1.0, 0.4, 0.6),
        ],
        Portrait,
    ),
    shader(
        "overlay.viewfinder",
        "Viewfinder",
        "Overlay",
        64,
        &[pct("opacity", "Opacity", 0.9)],
        &[WHITE],
        Wildlife,
    ),
    // ---- Frame ----
    shader(
        "frame.rounded",
        "Rounded Frame",
        "Frame",
        19,
        &[
            pct("radius", "Corner Radius", 0.3),
            pct("border", "Border", 0.3),
        ],
        &[WHITE],
        Nap,
    ),
    shader(
        "frame.letterbox",
        "Letterbox",
        "Frame",
        20,
        &[ParamDef::value(
            "ratio",
            "Aspect Ratio",
            1.5,
            2.8,
            2.35,
            ":1",
        )],
        &[BLACK],
        Meadow,
    ),
    shader(
        "frame.instant_photo",
        "Instant Photo",
        "Frame",
        65,
        &[pct("border", "Border", 0.4), pct("vintage", "Vintage", 0.4)],
        &[color("color", "Paper", 0.97, 0.96, 0.93)],
        Portrait,
    ),
    shader(
        "frame.film_strip",
        "Film Strip",
        "Frame",
        66,
        &[pct("border", "Border", 0.4)],
        &[color("color", "Film", 0.07, 0.06, 0.05)],
        Sunset,
    ),
    shader(
        "frame.tv",
        "Retro TV",
        "Frame",
        67,
        &[pct("curvature", "Curvature", 0.5)],
        &[color("color", "Cabinet", 0.45, 0.28, 0.16)],
        Landscape,
    ),
    shader(
        "frame.blur_border",
        "Blur Border",
        "Frame",
        68,
        &[
            pct("size", "Size", 0.75),
            pct("blur", "Blur", 0.6),
            pct("radius", "Corner Radius", 0.3),
        ],
        &[],
        Portrait,
    ),
    // ---- Split Screen ----
    shader(
        "split.side_by_side",
        "Side by Side",
        "Split Screen",
        69,
        &[GAP],
        &[BLACK],
        Pet,
    ),
    shader(
        "split.top_bottom",
        "Top & Bottom",
        "Split Screen",
        70,
        &[GAP],
        &[BLACK],
        Sunset,
    ),
    shader(
        "split.three",
        "Three Panels",
        "Split Screen",
        71,
        &[GAP],
        &[BLACK],
        Wildlife,
    ),
    shader(
        "split.grid_2x2",
        "2x2 Grid",
        "Split Screen",
        21,
        &[],
        &[],
        Sunset,
    ),
    shader(
        "split.grid_3x3",
        "3x3 Grid",
        "Split Screen",
        22,
        &[],
        &[],
        Nap,
    ),
    // ---- More camera motion: shakes and zooms ----
    moving("shake.sideways", "Sideways", "Shake", &[AMOUNT, SPEED]),
    moving("shake.up_down", "Up-Down", "Shake", &[AMOUNT, SPEED]),
    moving("shake.chaos", "Chaos", "Shake", &[AMOUNT, SPEED]),
    moving("shake.mini_wave", "Mini Wave", "Shake", &[AMOUNT, SPEED]),
    moving("shake.pendulum", "Pendulum", "Shake", &[AMOUNT, SPEED]),
    moving("shake.vibration", "Vibration", "Shake", &[AMOUNT, SPEED]),
    moving(
        "shake.rebound",
        "Rotational Rebound",
        "Shake",
        &[AMOUNT, SPEED],
    ),
    moving("shake.crash_in", "Crash In", "Shake", &[AMOUNT, SPEED]),
    moving("shake.wobble", "Slight Wobble", "Shake", &[AMOUNT, SPEED]),
    moving(
        "shake.focus_zoom",
        "Focus Zoom Shake",
        "Shake",
        &[AMOUNT, SPEED],
    ),
    moving(
        "shake.bounce_cam",
        "Bounce Camera",
        "Shake",
        &[AMOUNT, SPEED],
    ),
    moving(
        "shake.heart_shake",
        "Heartbeat Shake",
        "Shake",
        &[AMOUNT, BPM],
    ),
    moving("shake.silky", "Silky Camera", "Shake", &[AMOUNT, SPEED]),
    moving("zoom.expand", "Expand", "Zoom", &[AMOUNT, SPEED]),
    moving(
        "shake.flash_black",
        "Flash Black",
        "Shake",
        &[AMOUNT, SPEED],
    ),
    moving("zoom.wobble", "Wobble Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.rotary", "Rotary Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.slider", "Slider Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.s_curve", "S-Curve Camera", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.throb", "Throb Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.mini", "Mini Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.exposure", "Exposure Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving("zoom.sequence", "Sequence Zoom", "Zoom", &[AMOUNT, SPEED]),
    moving(
        "zoom.freeze_focus",
        "Freeze Focus Zoom",
        "Zoom",
        &[AMOUNT, SPEED],
    ),
    moving(
        "zoom.flash_lens",
        "Flash Lens Zoom",
        "Zoom",
        &[AMOUNT, SPEED],
    ),
    // ---- Flicker: colour flashes and strobes (one shader, ten styles) ----
    shader(
        "flicker.negative",
        "Negative Flicker",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 0),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "flicker.colour_flash",
        "Colour Flash",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 1),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "flicker.rgb_stroke",
        "RGB Stroke",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 2),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Landscape,
    ),
    shader(
        "flicker.neon_swing",
        "Neon Swing",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 3),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Sunset,
    ),
    shader(
        "flicker.hallucination",
        "Hallucination",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 4),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "flicker.jitter",
        "Flicker Jitter",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 5),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Pet,
    ),
    shader(
        "flicker.hue_cycle",
        "Hue Cycle",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 6),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Landscape,
    ),
    shader(
        "flicker.strobe",
        "Black & White Strobe",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 7),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "flicker.colour_slice",
        "Colour Slice",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 8),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Landscape,
    ),
    shader(
        "flicker.pulse_flash",
        "Pulse Flash",
        "Flicker",
        92,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", FLICKER_STYLES, 9),
            pct("variation", "Variation", 0.3),
        ],
        &[],
        Sunset,
    ),
    // ---- Glass, sparkle, grunge, neon frames, celebration, 3D layouts ----
    shader(
        "glass.shatter",
        "Shatter",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GLASS_STYLES, 0),
            pct("size", "Size", 0.45),
        ],
        &[],
        Portrait,
    ),
    shader(
        "glass.shatter_fine",
        "Shatter Fine",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GLASS_STYLES, 0),
            pct("size", "Size", 0.8),
        ],
        &[],
        Landscape,
    ),
    shader(
        "glass.shatter_big",
        "Shatter Big",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.8),
            pct("speed", "Speed", 0.45),
            ParamDef::choice("style", "Style", GLASS_STYLES, 0),
            pct("size", "Size", 0.1),
        ],
        &[],
        Pet,
    ),
    shader(
        "glass.cracked",
        "Cracked Glass",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GLASS_STYLES, 1),
            pct("size", "Size", 0.45),
        ],
        &[],
        Portrait,
    ),
    shader(
        "glass.cracked_heavy",
        "Cracked Heavy",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 1.0),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GLASS_STYLES, 1),
            pct("size", "Size", 0.75),
        ],
        &[],
        Landscape,
    ),
    shader(
        "glass.prism",
        "Prism Shards",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GLASS_STYLES, 2),
            pct("size", "Size", 0.45),
        ],
        &[],
        Sunset,
    ),
    shader(
        "glass.prism_fine",
        "Prism Fine",
        "Glass",
        93,
        &[
            pct("amount", "Amount", 0.8),
            pct("speed", "Speed", 0.4),
            ParamDef::choice("style", "Style", GLASS_STYLES, 2),
            pct("size", "Size", 0.8),
        ],
        &[],
        Portrait,
    ),
    shader(
        "sparkle.glitter_gold",
        "Gold Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.5),
            pct("spread", "Spread", 0.5),
        ],
        &[color("color", "Tint", 1.0, 0.82, 0.4)],
        Portrait,
    ),
    shader(
        "sparkle.glitter_silver",
        "Silver Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.5),
            pct("spread", "Spread", 0.5),
        ],
        &[color("color", "Tint", 0.9, 0.95, 1.0)],
        Portrait,
    ),
    shader(
        "sparkle.glitter_rose",
        "Rose Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.5),
            pct("spread", "Spread", 0.5),
        ],
        &[color("color", "Tint", 1.0, 0.62, 0.78)],
        Pet,
    ),
    shader(
        "sparkle.glitter_blue",
        "Ice Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.5),
            pct("spread", "Spread", 0.5),
        ],
        &[color("color", "Tint", 0.55, 0.8, 1.0)],
        Landscape,
    ),
    shader(
        "sparkle.glitter_fine",
        "Fine Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.9),
            pct("speed", "Speed", 0.6),
            pct("size", "Size", 0.1),
            pct("spread", "Spread", 0.4),
        ],
        &[color("color", "Tint", 1.0, 0.9, 0.7)],
        Sunset,
    ),
    shader(
        "sparkle.glitter_chunky",
        "Chunky Glitter",
        "Sparkle",
        94,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.4),
            pct("size", "Size", 0.85),
            pct("spread", "Spread", 0.5),
        ],
        &[color("color", "Tint", 1.0, 1.0, 1.0)],
        Portrait,
    ),
    shader(
        "sparkle.starburst_white",
        "Starburst",
        "Sparkle",
        95,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.5),
            pct("points", "Points", 0.3),
        ],
        &[color("color", "Tint", 1.0, 1.0, 1.0)],
        Portrait,
    ),
    shader(
        "sparkle.starburst_warm",
        "Warm Starburst",
        "Sparkle",
        95,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.55),
            pct("points", "Points", 0.55),
        ],
        &[color("color", "Tint", 1.0, 0.8, 0.5)],
        Sunset,
    ),
    shader(
        "sparkle.starburst_cool",
        "Cool Starburst",
        "Sparkle",
        95,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.55),
            pct("points", "Points", 0.8),
        ],
        &[color("color", "Tint", 0.6, 0.8, 1.0)],
        Landscape,
    ),
    shader(
        "sparkle.starburst_gentle",
        "Gentle Starburst",
        "Sparkle",
        95,
        &[
            pct("amount", "Amount", 0.3),
            pct("speed", "Speed", 0.35),
            pct("size", "Size", 0.4),
            pct("points", "Points", 0.3),
        ],
        &[color("color", "Tint", 1.0, 0.97, 0.9)],
        Pet,
    ),
    shader(
        "grunge.dust_scratches",
        "Dust & Scratches",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 0),
            pct("size", "Size", 0.5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "grunge.fine_dust",
        "Fine Dust",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 0),
            pct("size", "Size", 0.1),
        ],
        &[],
        Portrait,
    ),
    shader(
        "grunge.heavy_dust",
        "Heavy Dust",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 1.0),
            pct("speed", "Speed", 0.6),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 0),
            pct("size", "Size", 0.8),
        ],
        &[],
        Pet,
    ),
    shader(
        "grunge.grime",
        "Grime",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.3),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 1),
            pct("size", "Size", 0.5),
        ],
        &[],
        Portrait,
    ),
    shader(
        "grunge.grime_heavy",
        "Heavy Grime",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 1.0),
            pct("speed", "Speed", 0.3),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 1),
            pct("size", "Size", 0.6),
        ],
        &[],
        Landscape,
    ),
    shader(
        "grunge.paper",
        "Crumpled Paper",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 0.6),
            pct("speed", "Speed", 0.3),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 2),
            pct("size", "Size", 0.5),
        ],
        &[],
        Sunset,
    ),
    shader(
        "grunge.paper_soft",
        "Soft Paper",
        "Grunge",
        96,
        &[
            pct("amount", "Amount", 0.35),
            pct("speed", "Speed", 0.3),
            ParamDef::choice("style", "Style", GRUNGE_STYLES, 2),
            pct("size", "Size", 0.3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "frame.neon_steady",
        "Neon Frame",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.5),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", NEON_STYLES, 0),
            pct("inset", "Inset", 0.3),
        ],
        &[
            color("color", "Colour 1", 0.1, 0.9, 1.0),
            color("color2", "Colour 2", 0.1, 0.9, 1.0),
        ],
        Landscape,
    ),
    shader(
        "frame.neon_chase",
        "Neon Chase",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.5),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", NEON_STYLES, 1),
            pct("inset", "Inset", 0.3),
        ],
        &[
            color("color", "Colour 1", 1.0, 0.2, 0.7),
            color("color2", "Colour 2", 0.2, 0.8, 1.0),
        ],
        Portrait,
    ),
    shader(
        "frame.neon_pulse",
        "Neon Pulse",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.6),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", NEON_STYLES, 2),
            pct("inset", "Inset", 0.3),
        ],
        &[
            color("color", "Colour 1", 1.0, 0.3, 0.4),
            color("color2", "Colour 2", 1.0, 0.3, 0.4),
        ],
        Sunset,
    ),
    shader(
        "frame.neon_duo",
        "Neon Duo",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.5),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", NEON_STYLES, 3),
            pct("inset", "Inset", 0.3),
        ],
        &[
            color("color", "Colour 1", 0.4, 0.3, 1.0),
            color("color2", "Colour 2", 0.1, 1.0, 0.7),
        ],
        Pet,
    ),
    shader(
        "frame.neon_amber",
        "Amber Neon",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.6),
            pct("speed", "Speed", 0.5),
            ParamDef::choice("style", "Style", NEON_STYLES, 0),
            pct("inset", "Inset", 0.45),
        ],
        &[
            color("color", "Colour 1", 1.0, 0.7, 0.2),
            color("color2", "Colour 2", 1.0, 0.7, 0.2),
        ],
        Landscape,
    ),
    shader(
        "frame.neon_thin_chase",
        "Thin Neon Chase",
        "Frame",
        97,
        &[
            pct("glow", "Glow", 0.3),
            pct("speed", "Speed", 0.7),
            ParamDef::choice("style", "Style", NEON_STYLES, 1),
            pct("inset", "Inset", 0.1),
        ],
        &[
            color("color", "Colour 1", 0.7, 1.0, 0.5),
            color("color2", "Colour 2", 0.2, 0.6, 1.0),
        ],
        Portrait,
    ),
    shader(
        "celebration.fireworks",
        "Fireworks",
        "Celebration",
        98,
        &[
            pct("amount", "Amount", 0.8),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.55),
            pct("trails", "Trails", 0.5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "celebration.fireworks_bold",
        "Bold Fireworks",
        "Celebration",
        98,
        &[
            pct("amount", "Amount", 1.0),
            pct("speed", "Speed", 0.5),
            pct("size", "Size", 0.85),
            pct("trails", "Trails", 0.7),
        ],
        &[],
        Sunset,
    ),
    shader(
        "celebration.fireworks_tiny",
        "Tiny Fireworks",
        "Celebration",
        98,
        &[
            pct("amount", "Amount", 0.9),
            pct("speed", "Speed", 0.6),
            pct("size", "Size", 0.25),
            pct("trails", "Trails", 0.4),
        ],
        &[],
        Landscape,
    ),
    shader(
        "celebration.fireworks_slow",
        "Slow Fireworks",
        "Celebration",
        98,
        &[
            pct("amount", "Amount", 0.8),
            pct("speed", "Speed", 0.15),
            pct("size", "Size", 0.6),
            pct("trails", "Trails", 0.6),
        ],
        &[],
        Portrait,
    ),
    shader(
        "celebration.fireworks_rapid",
        "Rapid Fireworks",
        "Celebration",
        98,
        &[
            pct("amount", "Amount", 0.7),
            pct("speed", "Speed", 0.9),
            pct("size", "Size", 0.5),
            pct("trails", "Trails", 0.4),
        ],
        &[],
        Pet,
    ),
    shader(
        "layout.fan",
        "Triple Fan",
        "Layout 3D",
        99,
        &[
            pct("tilt", "Tilt", 0.5),
            pct("sway", "Sway", 0.4),
            ParamDef::choice("style", "Style", LAYOUT_STYLES, 0),
            pct("gap", "Gap", 0.5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "layout.fan_wide",
        "Wide Fan",
        "Layout 3D",
        99,
        &[
            pct("tilt", "Tilt", 0.85),
            pct("sway", "Sway", 0.4),
            ParamDef::choice("style", "Style", LAYOUT_STYLES, 0),
            pct("gap", "Gap", 0.9),
        ],
        &[],
        Portrait,
    ),
    shader(
        "layout.stairs",
        "Stairs",
        "Layout 3D",
        99,
        &[
            pct("tilt", "Tilt", 0.5),
            pct("sway", "Sway", 0.4),
            ParamDef::choice("style", "Style", LAYOUT_STYLES, 1),
            pct("gap", "Gap", 0.5),
        ],
        &[],
        Sunset,
    ),
    shader(
        "layout.card",
        "Floating Card",
        "Layout 3D",
        99,
        &[
            pct("tilt", "Tilt", 0.5),
            pct("sway", "Sway", 0.4),
            ParamDef::choice("style", "Style", LAYOUT_STYLES, 2),
            pct("gap", "Gap", 0.5),
        ],
        &[],
        Pet,
    ),
    shader(
        "layout.card_tilted",
        "Tilted Card",
        "Layout 3D",
        99,
        &[
            pct("tilt", "Tilt", 0.95),
            pct("sway", "Sway", 0.5),
            ParamDef::choice("style", "Style", LAYOUT_STYLES, 2),
            pct("gap", "Gap", 0.5),
        ],
        &[],
        Landscape,
    ),
    // ---- Beat: the picture follows the tempo ----
    shader(
        "beat.zoom_pump",
        "Zoom Pump",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 0),
        ],
        &[],
        Portrait,
    ),
    shader(
        "beat.zoom_pump_fast",
        "Zoom Pump Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 0),
        ],
        &[],
        Portrait,
    ),
    shader(
        "beat.shake_kick",
        "Shake Kick",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 1),
        ],
        &[],
        Landscape,
    ),
    shader(
        "beat.shake_kick_fast",
        "Shake Kick Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 1),
        ],
        &[],
        Landscape,
    ),
    shader(
        "beat.colour_pop",
        "Colour Pop",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 2),
        ],
        &[],
        Sunset,
    ),
    shader(
        "beat.colour_pop_fast",
        "Colour Pop Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 2),
        ],
        &[],
        Sunset,
    ),
    shader(
        "beat.blur_thump",
        "Blur Thump",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 3),
        ],
        &[],
        Pet,
    ),
    shader(
        "beat.blur_thump_fast",
        "Blur Thump Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 3),
        ],
        &[],
        Pet,
    ),
    shader(
        "beat.strobe",
        "Beat Strobe",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 4),
        ],
        &[],
        Portrait,
    ),
    shader(
        "beat.strobe_fast",
        "Beat Strobe Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 4),
        ],
        &[],
        Portrait,
    ),
    shader(
        "beat.hue_kick",
        "Hue Kick",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "beat.hue_kick_fast",
        "Hue Kick Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "beat.squash_bounce",
        "Squash Bounce",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 6),
        ],
        &[],
        Pet,
    ),
    shader(
        "beat.squash_bounce_fast",
        "Squash Bounce Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 6),
        ],
        &[],
        Pet,
    ),
    shader(
        "beat.vignette_pulse",
        "Vignette Pulse",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 96.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 7),
        ],
        &[],
        Sunset,
    ),
    shader(
        "beat.vignette_pulse_fast",
        "Vignette Pulse Fast",
        "Beat",
        100,
        &[
            ParamDef::value("bpm", "Beats per Minute", 40.0, 200.0, 128.0, " bpm"),
            pct("amount", "Amount", 0.7),
            pct("decay", "Decay", 0.5),
            ParamDef::choice("style", "Style", BEAT_STYLES, 7),
        ],
        &[],
        Sunset,
    ),
    // ---- Sound reactive: the picture moves with the clip's own sound ----
    shader(
        "react.zoom_bass",
        "Bass Zoom",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 1),
            ParamDef::choice("style", "Style", REACT_STYLES, 0),
        ],
        &[],
        Landscape,
    ),
    shader(
        "react.zoom_all",
        "Sound Zoom",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 0),
        ],
        &[],
        Portrait,
    ),
    shader(
        "react.shake_bass",
        "Bass Shake",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 1),
            ParamDef::choice("style", "Style", REACT_STYLES, 1),
        ],
        &[],
        Sunset,
    ),
    shader(
        "react.shake_all",
        "Sound Shake",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 1),
        ],
        &[],
        Pet,
    ),
    shader(
        "react.colour_pop",
        "Sound Colour Pop",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 2),
        ],
        &[],
        Landscape,
    ),
    shader(
        "react.flash_bass",
        "Bass Flash",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 1),
            ParamDef::choice("style", "Style", REACT_STYLES, 3),
        ],
        &[],
        Portrait,
    ),
    shader(
        "react.flash_treble",
        "Treble Flash",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 3),
            ParamDef::choice("style", "Style", REACT_STYLES, 3),
        ],
        &[],
        Night,
    ),
    shader(
        "react.split_all",
        "Sound Colour Split",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 4),
        ],
        &[],
        Portrait,
    ),
    shader(
        "react.split_treble",
        "Treble Colour Split",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 3),
            ParamDef::choice("style", "Style", REACT_STYLES, 4),
        ],
        &[],
        Sunset,
    ),
    shader(
        "react.blur_bass",
        "Bass Blur",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 1),
            ParamDef::choice("style", "Style", REACT_STYLES, 5),
        ],
        &[],
        Landscape,
    ),
    shader(
        "react.glow_mid",
        "Mid Glow",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 2),
            ParamDef::choice("style", "Style", REACT_STYLES, 6),
        ],
        &[],
        Night,
    ),
    shader(
        "react.glow_all",
        "Sound Glow",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 6),
        ],
        &[],
        Sunset,
    ),
    shader(
        "react.vignette_bass",
        "Bass Vignette",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 1),
            ParamDef::choice("style", "Style", REACT_STYLES, 7),
        ],
        &[],
        Pet,
    ),
    shader(
        "react.hue_mid",
        "Mid Hue Shift",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 2),
            ParamDef::choice("style", "Style", REACT_STYLES, 8),
        ],
        &[],
        Landscape,
    ),
    shader(
        "react.hue_all",
        "Sound Hue Shift",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 8),
        ],
        &[],
        Portrait,
    ),
    shader(
        "react.tear_treble",
        "Treble Tear",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 3),
            ParamDef::choice("style", "Style", REACT_STYLES, 9),
        ],
        &[],
        Night,
    ),
    shader(
        "react.tear_all",
        "Sound Tear",
        "Sound Reactive",
        AUDIO_SHADER,
        &[
            pct("sensitivity", "Sensitivity", 0.6),
            pct("amount", "Amount", 0.7),
            ParamDef::choice("band", "Reacts to", REACT_BANDS, 0),
            ParamDef::choice("style", "Style", REACT_STYLES, 9),
        ],
        &[],
        Landscape,
    ),
];

pub fn get(id: &str) -> Option<&'static EffectDef> {
    EFFECTS.iter().find(|e| e.id == id)
}

/// Packs parameter values (looked up by key, already including defaults)
/// into the shader's p0..p3 and c0..c1.
pub fn pack(def: &EffectDef, value: impl Fn(&str) -> f32) -> ([[f32; 4]; 4], [[f32; 4]; 2]) {
    let mut p = [[0.0f32; 4]; 4];
    for (i, param) in def.params.iter().take(16).enumerate() {
        p[i / 4][i % 4] = value(param.key);
    }
    let mut c = [[0.0f32, 0.0, 0.0, 1.0]; 2];
    for (i, color) in def.colors.iter().take(2).enumerate() {
        c[i] = [
            value(&format!("{}_r", color.key)),
            value(&format!("{}_g", color.key)),
            value(&format!("{}_b", color.key)),
            1.0,
        ];
    }
    (p, c)
}

/// Default value for a parameter key, including color channels.
pub fn default_value(def: &EffectDef, key: &str) -> f32 {
    if let Some(p) = def.params.iter().find(|p| p.key == key) {
        return p.default;
    }
    for c in def.colors {
        for (i, suffix) in ["_r", "_g", "_b"].iter().enumerate() {
            if key.strip_suffix(suffix) == Some(c.key) {
                return c.default[i];
            }
        }
    }
    0.0
}

// ---- Motion effects --------------------------------------------------------

const FRAME_W: f32 = 1920.0;
const FRAME_H: f32 = 1080.0;

fn hash(n: f32) -> f32 {
    let x = (n * 12.9898 + 4.1414).sin() * 43758.547;
    x - x.floor()
}

/// Smooth value noise in -1..1.
fn noise(x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let w = f * f * (3.0 - 2.0 * f);
    let a = hash(i) * 2.0 - 1.0;
    let b = hash(i + 1.0) * 2.0 - 1.0;
    a + (b - a) * w
}

/// Layered noise in about -1..1; `seed` picks an independent curve.
fn wobble(x: f32, seed: f32) -> f32 {
    noise(x + seed * 31.7) * 0.65
        + noise(x * 2.13 + seed * 7.3 + 5.0) * 0.25
        + noise(x * 4.37 + seed * 3.1 + 9.0) * 0.1
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Scale that keeps the frame covered while the clip moves by up to `dx`,
/// `dy` project pixels and turns by up to `deg` degrees, so no edges show.
fn cover(dx: f32, dy: f32, deg: f32) -> f32 {
    let (s, c) = deg.abs().to_radians().sin_cos();
    c + (FRAME_W / FRAME_H) * s + 2.0 * (dx.abs() / FRAME_W).max(dy.abs() / FRAME_H)
}

/// The movement a motion effect adds at `t` seconds into the clip. Offsets
/// are in 1920x1080 project pixels; shakes zoom in just enough that the
/// frame edges never show.
pub fn motion(id: &str, t: f32, value: impl Fn(&str) -> f32) -> Motion {
    let mut m = Motion::NONE;
    let tau = std::f32::consts::TAU;
    let amount = value("amount");
    let speed = value("speed");
    match id {
        "shake.camera" => {
            let a = amount * 30.0;
            let f = 4.0 + speed * 20.0;
            let n = |k: f32| (t * f + k).sin() * 0.6 + (t * f * 2.3 + k * 1.7).sin() * 0.4;
            m.dx = n(0.0) * a;
            m.dy = n(3.1) * a;
            m.rotation = n(5.7) * a * 0.04;
            m.scale = cover(a, a, a * 0.04);
        }
        "shake.earthquake" => {
            let a = amount * 60.0;
            let f = 8.0 + speed * 25.0;
            m.dx = (noise(t * f) * 0.7 + noise(t * f * 2.7 + 11.0) * 0.3) * a * 0.7;
            m.dy = (noise(t * f * 1.1 + 23.0) * 0.7 + noise(t * f * 3.1 + 37.0) * 0.3) * a;
            m.rotation = noise(t * f * 0.5 + 51.0) * amount * 1.5;
            m.scale = cover(a * 0.7, a, amount * 1.5);
            m.blur = amount * 0.12;
        }
        "shake.handheld" => {
            let a = amount * 40.0;
            let f = 0.3 + speed * 1.2;
            m.dx = wobble(t * f, 1.0) * a;
            m.dy = wobble(t * f, 2.0) * a * 0.7;
            m.rotation = wobble(t * f * 0.8, 3.0) * amount * 1.2;
            m.scale = cover(a, a * 0.7, amount * 1.2);
        }
        "shake.jitter" => {
            let a = amount * 25.0;
            let k = (t * (6.0 + speed * 18.0)).floor();
            m.dx = (hash(k) * 2.0 - 1.0) * a;
            m.dy = (hash(k + 17.3) * 2.0 - 1.0) * a;
            m.rotation = (hash(k + 5.1) * 2.0 - 1.0) * amount * 0.6;
            m.scale = cover(a, a, amount * 0.6);
        }
        "shake.sway" => {
            let ph = tau * (0.15 + speed * 0.6) * t;
            m.rotation = ph.sin() * amount * 4.0;
            m.dx = (ph + 0.4).sin() * amount * 40.0;
            m.dy = (1.0 - (ph * 2.0).cos()) * amount * 6.0;
            m.scale = cover(amount * 40.0, amount * 12.0, amount * 4.0);
        }
        "shake.bounce" => {
            let ph = (t * (0.6 + speed * 1.6)).fract();
            let h = 4.0 * ph * (1.0 - ph);
            let a = amount * 90.0;
            m.dy = -h * a;
            let squash = (1.0 - h).powi(8) * amount * 0.07;
            m.stretch = [1.0 + squash, 1.0 - squash];
            m.scale = cover(0.0, a, 0.0) + squash;
        }
        "trendy.punch_zoom" => {
            let ph = (t * value("bpm").max(1.0) / 60.0).fract();
            let hit = (1.0 - ph).powi(3);
            m.scale = 1.0 + amount * 0.16 * hit;
            m.blur = amount * 0.1 * hit * hit;
        }
        "trendy.impact" => {
            // A hit every so often: the picture jolts, rings and settles.
            let period = 1.0 / (0.3 + speed * 1.4);
            let k = (t / period).fract();
            let n = (t / period).floor();
            let decay = (1.0 - k).powi(3);
            let a = amount * 45.0;
            let ring = (k * period * 38.0).cos();
            m.dx = (hash(n) * 2.0 - 1.0) * a * decay * ring;
            m.dy = (hash(n + 7.3) * 2.0 - 1.0) * a * decay * ring.signum() * 0.8;
            m.rotation = (hash(n + 2.9) * 2.0 - 1.0) * amount * 1.6 * decay * ring;
            m.scale = cover(a, a, amount * 1.6);
            m.blur = amount * 0.14 * decay;
        }
        "zoom.pulse" => {
            let f = 0.3 + speed * 1.7;
            m.scale = 1.0 + amount * 0.15 * (0.5 - 0.5 * (tau * f * t).cos());
        }
        "zoom.heartbeat" => {
            let ph = (t * value("bpm").max(1.0) / 60.0).fract();
            let beat =
                (-((ph - 0.08) * 28.0).powi(2)).exp() + 0.6 * (-((ph - 0.28) * 28.0).powi(2)).exp();
            m.scale = 1.0 + amount * 0.12 * beat;
        }
        "zoom.ken_burns_in" | "zoom.ken_burns_out" => {
            let mut e = smooth01(t / value("duration").max(0.1));
            if id == "zoom.ken_burns_out" {
                e = 1.0 - e;
            }
            m.scale = 1.0 + value("zoom") * e;
        }
        "zoom.pan_left" | "zoom.pan_right" => {
            // The view travels left (or right), so the picture slides the
            // other way across the zoomed-in frame.
            let s = 1.0 + value("zoom").max(0.02);
            let range = (s - 1.0) * FRAME_W * 0.5;
            let e = smooth01(t / value("duration").max(0.1)) * 2.0 - 1.0;
            m.scale = s;
            m.dx = if id == "zoom.pan_left" {
                e * range
            } else {
                -e * range
            };
        }
        "zoom.spin" => {
            m.rotation = (t * speed) % 360.0;
            if value("fill") > 0.5 {
                m.scale = (FRAME_W * FRAME_W + FRAME_H * FRAME_H).sqrt() / FRAME_H;
            }
        }
        "shake.sideways" => {
            let a = amount * 55.0;
            let w = 6.0 + speed * 24.0;
            m.dx = (t * w).sin() * a;
            m.scale = cover(a, 0.0, 0.0);
            m.blur = amount * 0.05;
        }
        "shake.up_down" => {
            let a = amount * 45.0;
            let w = 6.0 + speed * 24.0;
            m.dy = (t * w).sin() * a;
            m.scale = cover(0.0, a, 0.0);
            m.blur = amount * 0.05;
        }
        "shake.chaos" => {
            let a = amount * 70.0;
            let k = (t * (8.0 + speed * 20.0)).floor();
            m.dx = (hash(k) * 2.0 - 1.0) * a;
            m.dy = (hash(k + 9.7) * 2.0 - 1.0) * a;
            m.rotation = (hash(k + 4.3) * 2.0 - 1.0) * amount * 2.5;
            m.scale = cover(a, a, amount * 2.5);
            m.blur = amount * 0.1;
        }
        "shake.mini_wave" => {
            let w = 2.0 + speed * 8.0;
            let a = amount * 30.0;
            m.dx = (t * w).sin() * a;
            m.dy = (t * w * 1.3 + 1.0).sin() * a;
            m.rotation = (t * w * 0.7).sin() * amount * 0.8;
            m.scale = cover(a, a, amount * 0.8);
        }
        "shake.pendulum" => {
            let ph = tau * (0.3 + speed * 0.8) * t;
            m.rotation = ph.sin() * amount * 8.0;
            m.dx = ph.sin() * amount * 50.0;
            m.scale = cover(amount * 50.0, 0.0, amount * 8.0);
        }
        "shake.vibration" => {
            let w = 90.0 + speed * 120.0;
            let a = amount * 10.0;
            m.dx = (t * w).sin() * a;
            m.dy = (t * w * 1.17).cos() * a;
            m.scale = cover(a, a, 0.0);
        }
        "shake.rebound" => {
            // A swing that overshoots and settles, over and over.
            let ph = (t * (0.5 + speed)).fract();
            m.rotation = (1.0 - Spring::new(2.5, 0.3).at(ph)) * amount * 10.0;
            m.scale = cover(0.0, 0.0, amount * 12.0);
        }
        "shake.crash_in" => {
            let ph = (t * (0.5 + speed * 1.5)).fract();
            let hit = (1.0 - ph).powi(4);
            m.scale = 1.0 + amount * 0.3 * hit;
            m.dy = hit * amount * 30.0;
            m.blur = amount * 0.2 * hit;
        }
        "shake.wobble" => {
            let f = 0.4 + speed;
            let a = amount * 18.0;
            m.dx = wobble(t * f, 4.0) * a;
            m.dy = wobble(t * f, 8.0) * a * 0.7;
            m.rotation = wobble(t * f * 0.8, 12.0) * amount * 0.6;
            m.scale = cover(a, a * 0.7, amount * 0.6);
        }
        "shake.focus_zoom" => {
            let ph = (t * (0.4 + speed * 1.2)).fract();
            let s = 0.5 - 0.5 * (tau * ph).cos();
            m.scale = 1.0 + amount * 0.2 * s;
            m.blur = amount * 0.15 * 4.0 * s * (1.0 - s);
        }
        "shake.bounce_cam" => {
            let ph = (t * (0.7 + speed * 1.5)).fract();
            let h = (ph * std::f32::consts::PI).sin();
            let a = amount * 70.0;
            m.dy = -h * a * (1.0 - ph);
            m.scale = cover(0.0, a, 0.0);
        }
        "shake.heart_shake" => {
            let ph = (t * value("bpm").max(1.0) / 60.0).fract();
            let env = (-ph * 9.0).exp() + 0.6 * (-((ph - 0.3).abs()) * 16.0).exp();
            let a = amount * 35.0;
            m.dx = (t * 90.0).sin() * a * env;
            m.dy = (t * 77.0).cos() * a * env;
            m.scale = cover(a, a, 0.0);
        }
        "shake.silky" => {
            let w = 0.5 + speed * 1.5;
            m.dx = (t * w).sin() * amount * 70.0;
            m.dy = (t * w * 0.8 + 1.0).sin() * amount * 40.0;
            m.scale = cover(amount * 70.0, amount * 40.0, 0.0);
            m.blur = amount * 0.04;
        }
        "zoom.expand" => {
            let ph = (t * (0.4 + speed)).fract();
            let e = 1.0 - (1.0 - ph).powi(3);
            m.scale = 0.78 + 0.22 * e;
        }
        "shake.flash_black" => {
            let ph = (t * (0.8 + speed * 2.0)).fract();
            m.opacity = 1.0 - amount * (1.0 - ph * 6.0).max(0.0);
        }
        "zoom.wobble" => {
            let f = 0.6 + speed * 3.0;
            let s = 0.5 - 0.5 * (tau * f * t).cos();
            m.rotation = (tau * f * t * 0.5).sin() * amount * 2.0;
            m.scale = (1.0 + amount * 0.1 * s) * cover(0.0, 0.0, amount * 2.0);
        }
        "zoom.rotary" => {
            m.rotation = (tau * (0.1 + speed * 0.3) * t).sin() * amount * 8.0;
            m.scale = cover(0.0, 0.0, amount * 8.0) + amount * 0.1;
        }
        "zoom.slider" => {
            let e = smooth01((t * (0.2 + speed * 0.6)).fract());
            m.scale = 1.0 + amount * 0.3;
            m.dx = (e * 2.0 - 1.0) * amount * FRAME_W * 0.12;
        }
        "zoom.s_curve" => {
            let e = smooth01((t * (0.2 + speed * 0.6)).fract());
            m.dx = (e * tau).sin() * amount * 60.0;
            m.dy = (e - 0.5) * amount * 80.0;
            m.scale = cover(amount * 60.0, amount * 40.0, 0.0) + amount * 0.2 * e;
        }
        "zoom.throb" => {
            let ph = (t * (0.8 + speed * 2.0)).fract();
            let throb = (-ph * 8.0).exp();
            m.scale = 1.0 + amount * 0.25 * throb;
            m.blur = amount * 0.2 * throb;
        }
        "zoom.mini" => {
            let f = 0.8 + speed * 3.0;
            m.scale = 1.0 + amount * 0.05 * (0.5 - 0.5 * (tau * f * t).cos());
        }
        "zoom.exposure" => {
            let ph = (t * (0.3 + speed * 0.8)).fract();
            m.scale = 1.0 + amount * 0.3 * ph;
            m.blur = amount * 0.2 * ph * ph;
        }
        "zoom.sequence" => {
            let step = ((t * (0.5 + speed)).fract() * 4.0).floor();
            m.scale = 1.0 + amount * 0.08 * step;
        }
        "zoom.freeze_focus" => {
            let ph = (t * (0.4 + speed * 0.8)).fract();
            let k = (ph * 4.0).min(1.0);
            m.scale = 1.0 + amount * 0.3 * (1.0 - (1.0 - k).powi(3));
            m.blur = amount * 0.15 * (1.0 - k);
        }
        "zoom.flash_lens" => {
            let ph = (t * (0.5 + speed * 1.2)).fract();
            let s = smooth01(ph);
            m.scale = 1.0 + amount * 0.35 * s;
            m.blur = amount * 0.25 * 4.0 * s * (1.0 - s);
        }
        _ => {}
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_fit_the_shader_slots() {
        let mut ids = std::collections::HashSet::new();
        for e in EFFECTS {
            assert!(ids.insert(e.id), "duplicate id {}", e.id);
            assert!(e.params.len() <= 16, "{} has too many params", e.id);
            assert!(e.colors.len() <= 2, "{} has too many colors", e.id);
            assert!(
                e.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_'),
                "{} is not a lowercase dotted id",
                e.id
            );
            assert!(e.id.contains('.'), "{} has no category prefix", e.id);
            let mut keys = std::collections::HashSet::new();
            for p in e.params {
                assert!(keys.insert(p.key), "{} repeats param {}", e.id, p.key);
                assert!(
                    p.min <= p.default && p.default <= p.max,
                    "{}.{} default out of range",
                    e.id,
                    p.key
                );
            }
            for c in e.colors {
                assert!(keys.insert(c.key), "{} repeats key {}", e.id, c.key);
            }
        }
    }

    #[test]
    fn every_shader_has_a_case() {
        let src = crate::shader_source();
        // The cases are spread over three dispatchers before `fs_effect`.
        let start = src.find("fn fx_dispatch_a").expect("dispatchers in WGSL");
        let end = src.find("fn fs_effect").expect("fs_effect in WGSL");
        let body = &src[start..end];
        for e in EFFECTS {
            if let EffectKind::Shader { id, passes } = e.kind {
                assert!(passes >= 1, "{}", e.id);
                assert!(
                    body.contains(&format!("case {id}u:")),
                    "{} uses shader {id} with no case",
                    e.id
                );
            }
        }
    }

    #[test]
    fn categories_follow_the_library_order() {
        let order = [
            "Basic",
            "Cinematic",
            "Trendy",
            "Blur",
            "Shake",
            "Zoom",
            "Distortion",
            "Glitch",
            "Retro",
            "Light",
            "Particles",
            "Overlay",
            "Frame",
            "Split Screen",
            "Flicker",
            "Glass",
            "Sparkle",
            "Grunge",
            "Celebration",
            "Layout 3D",
            "Beat",
            "Sound Reactive",
        ];
        assert_eq!(super::super::categories(EFFECTS, |e| e.category), order);
    }

    #[test]
    fn seed_ids_are_kept() {
        for id in [
            "blur.gaussian",
            "glitch.digital",
            "shake.camera",
            "distort.lens",
        ] {
            assert!(get(id).is_some(), "{id}");
        }
        assert_eq!(
            get("distort.lens").unwrap().kind,
            EffectKind::Shader {
                id: LENS_SHADER,
                passes: 1
            }
        );
    }

    #[test]
    fn pack_uses_defaults() {
        let def = get("light.rays").unwrap();
        let (p, c) = pack(def, |k| default_value(def, k));
        assert_eq!(p[0], [0.7, 0.72, 0.3, 0.6]);
        assert_eq!(c[0], [1.0, 0.9, 0.7, 1.0]);
        assert_eq!(c[1], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn motions_move_and_cover_the_frame() {
        for e in EFFECTS.iter().filter(|e| e.kind == EffectKind::Motion) {
            let value = |k: &str| default_value(e, k);
            let moved = (0..40).any(|i| motion(e.id, i as f32 * 0.37, value) != Motion::NONE);
            assert!(moved, "{} never moves", e.id);
            for i in 0..200 {
                let m = motion(e.id, i as f32 * 0.05, value);
                assert!(
                    m.dx.is_finite() && m.dy.is_finite() && m.scale.is_finite(),
                    "{}",
                    e.id
                );
                if e.category == "Shake" || e.id.starts_with("zoom.pan") {
                    // Offsets never reveal the frame edges.
                    let s = m.scale * m.stretch[0].min(m.stretch[1]);
                    let slack_x = (s - 1.0) * FRAME_W * 0.5;
                    let slack_y = (s - 1.0) * FRAME_H * 0.5;
                    assert!(
                        m.dx.abs() <= slack_x + 0.5,
                        "{} dx {} slack {}",
                        e.id,
                        m.dx,
                        slack_x
                    );
                    assert!(
                        m.dy.abs() <= slack_y + 0.5,
                        "{} dy {} slack {}",
                        e.id,
                        m.dy,
                        slack_y
                    );
                }
            }
        }
    }
}
