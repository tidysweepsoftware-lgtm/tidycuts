//! Transitions from `transitions.wgsl`. Variants of one shader (wipe left,
//! wipe right…) are separate entries with different fixed parameters.

#[derive(Debug, Clone, Copy)]
pub struct TransitionDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    /// Shader id in `transitions.wgsl`.
    pub shader: u32,
    /// Fixed parameters handed to the shader as p0.
    pub params: [f32; 4],
    /// Color for transitions that pass through one (c0).
    pub color: [f32; 4],
    /// Default length in seconds.
    pub duration: f32,
}

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

const FADE: &str = "Fade & Dissolve";
const WIPE: &str = "Wipe";
const SLIDE: &str = "Slide";
const ZOOM: &str = "Zoom";
const D3: &str = "3D";
const BLUR: &str = "Blur";
const GLITCH: &str = "Glitch";
const SHAPE: &str = "Classic Shapes";
const SPLIT: &str = "Split";
const WARP: &str = "Warp";
const LIGHT: &str = "Light Leak";
const MG: &str = "Motion Graphics";
const BRUSH: &str = "Brush & Paper";
const DUST: &str = "Dust & Particles";
const CAMERA: &str = "Camera";
const LUXURY: &str = "Luxury";
const GLASS: &str = "Glass";
const CARDS: &str = "Cards";

/// Shader ids in `transitions.wgsl`.
mod shader {
    pub const DISSOLVE: u32 = 1;
    pub const DIP: u32 = 2;
    pub const WIPE: u32 = 3;
    pub const PUSH: u32 = 4;
    pub const COVER: u32 = 5;
    pub const IRIS: u32 = 6;
    pub const CLOCK: u32 = 7;
    pub const CROSS_ZOOM: u32 = 8;
    pub const ZOOM_IN: u32 = 9;
    pub const NOISE: u32 = 10;
    pub const BURN: u32 = 11;
    pub const PIXELATE: u32 = 12;
    pub const SPLIT: u32 = 13;
    pub const GLITCH: u32 = 14;
    pub const SWIRL: u32 = 15;
    pub const FLASH: u32 = 16;
    pub const BLUR: u32 = 17;
    pub const LUMA: u32 = 18;
    pub const ADDITIVE: u32 = 19;
    pub const BARN: u32 = 20;
    pub const BLINDS: u32 = 21;
    pub const CHECKER: u32 = 22;
    pub const UNCOVER: u32 = 23;
    pub const WHIP: u32 = 24;
    pub const ZOOM_OUT: u32 = 25;
    pub const SPIN_ZOOM: u32 = 26;
    pub const CUBE: u32 = 27;
    pub const CURL: u32 = 28;
    pub const FLIP: u32 = 29;
    pub const DOOR: u32 = 30;
    pub const FOLD: u32 = 31;
    pub const DIR_BLUR: u32 = 32;
    pub const DEFOCUS: u32 = 33;
    pub const RGB_SPLIT: u32 = 34;
    pub const DIGITAL: u32 = 35;
    pub const SHAPE: u32 = 36;
    pub const CORNERS: u32 = 37;
    pub const STRIPES: u32 = 38;
    pub const TILES: u32 = 39;
    pub const RIPPLE: u32 = 40;
    pub const LIQUID: u32 = 41;
    pub const DREAMY: u32 = 42;
    pub const LEAK: u32 = 43;
    pub const FLARE: u32 = 44;
    pub const LIGHT_WIPE: u32 = 45;
    pub const FILM_ROLL: u32 = 46;
    pub const ZOOM_PUNCH: u32 = 47;
    pub const SHADOW_FADE: u32 = 48;
    pub const MG_BARS: u32 = 49;
    pub const MG_CIRCLES: u32 = 50;
    pub const MG_RINGS: u32 = 51;
    pub const MG_WAVE: u32 = 52;
    pub const BRUSH: u32 = 53;
    pub const TEAR: u32 = 54;
    pub const DUST: u32 = 55;
    pub const SHUTTER: u32 = 56;
    pub const SPEED: u32 = 57;
    pub const GOLD: u32 = 58;
    pub const FREEZE: u32 = 59;
    pub const CARDPUSH: u32 = 60;
    pub const FROST: u32 = 61;
    pub const SHATTER: u32 = 62;
    pub const SLABS: u32 = 63;
    pub const TILEFLIP: u32 = 64;
    pub const TORUS: u32 = 65;
    pub const ICE: u32 = 66;
    pub const STEPS: u32 = 67;
}

/// Direction codes for push, slide, cube… (p0.x).
const LEFT: f32 = 0.0;
const RIGHT: f32 = 1.0;
const UP: f32 = 2.0;
const DOWN: f32 = 3.0;

const fn tr(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    shader: u32,
    params: [f32; 4],
    color: [f32; 4],
    duration: f32,
) -> TransitionDef {
    TransitionDef {
        id,
        name,
        category,
        shader,
        params,
        color,
        duration,
    }
}

/// A transition with one parameter and no color.
const fn tr1(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    shader: u32,
    p: f32,
    duration: f32,
) -> TransitionDef {
    tr(
        id,
        name,
        category,
        shader,
        [p, 0.0, 0.0, 0.0],
        BLACK,
        duration,
    )
}

pub static TRANSITIONS: &[TransitionDef] = &[
    // Fade & Dissolve
    tr1("dissolve", "Dissolve", FADE, shader::DISSOLVE, 0.0, 1.0),
    tr(
        "fade_black",
        "Fade to Black",
        FADE,
        shader::DIP,
        [0.0; 4],
        BLACK,
        1.0,
    ),
    tr(
        "fade_white",
        "Fade to White",
        FADE,
        shader::DIP,
        [0.0; 4],
        WHITE,
        1.0,
    ),
    tr(
        "dip_color",
        "Dip to Color",
        FADE,
        shader::DIP,
        [0.0; 4],
        [0.95, 0.38, 0.45, 1.0],
        1.0,
    ),
    tr(
        "luma_fade",
        "Luma Fade",
        FADE,
        shader::LUMA,
        [0.0, 0.12, 0.0, 0.0],
        BLACK,
        1.2,
    ),
    tr1(
        "additive_dissolve",
        "Additive Dissolve",
        FADE,
        shader::ADDITIVE,
        0.0,
        1.0,
    ),
    tr1(
        "noise_dissolve",
        "Noise Dissolve",
        FADE,
        shader::NOISE,
        0.0,
        1.0,
    ),
    tr1(
        "fade.shadow",
        "Shadow Fade",
        FADE,
        shader::SHADOW_FADE,
        0.0,
        1.0,
    ),
    // Wipe
    tr1("wipe.right", "Wipe Right", WIPE, shader::WIPE, 0.0, 1.0),
    tr1("wipe.left", "Wipe Left", WIPE, shader::WIPE, 1.0, 1.0),
    tr1("wipe.down", "Wipe Down", WIPE, shader::WIPE, 2.0, 1.0),
    tr1("wipe.up", "Wipe Up", WIPE, shader::WIPE, 3.0, 1.0),
    tr1(
        "wipe.diagonal",
        "Diagonal Wipe",
        WIPE,
        shader::WIPE,
        4.0,
        1.0,
    ),
    tr(
        "wipe.soft",
        "Soft Wipe",
        WIPE,
        shader::WIPE,
        [0.0, 0.35, 0.0, 0.0],
        BLACK,
        1.2,
    ),
    tr(
        "wipe.barn_doors",
        "Barn Doors",
        WIPE,
        shader::BARN,
        [0.0, 1.0, 0.0, 0.0],
        BLACK,
        1.0,
    ),
    tr(
        "wipe.blinds",
        "Venetian Blinds",
        WIPE,
        shader::BLINDS,
        [8.0, 0.0, 0.0, 0.0],
        BLACK,
        1.0,
    ),
    tr1(
        "wipe.checker",
        "Checkerboard",
        WIPE,
        shader::CHECKER,
        4.0,
        1.0,
    ),
    tr1("wipe.clock", "Clock Wipe", WIPE, shader::CLOCK, 0.0, 1.0),
    // Slide
    tr1("push.left", "Push Left", SLIDE, shader::PUSH, LEFT, 0.8),
    tr1("push.right", "Push Right", SLIDE, shader::PUSH, RIGHT, 0.8),
    tr1("push.up", "Push Up", SLIDE, shader::PUSH, UP, 0.8),
    tr1("push.down", "Push Down", SLIDE, shader::PUSH, DOWN, 0.8),
    tr1(
        "slide.left",
        "Slide In Left",
        SLIDE,
        shader::COVER,
        LEFT,
        0.8,
    ),
    tr1(
        "slide.right",
        "Slide In Right",
        SLIDE,
        shader::COVER,
        RIGHT,
        0.8,
    ),
    tr1(
        "uncover.left",
        "Uncover Left",
        SLIDE,
        shader::UNCOVER,
        LEFT,
        0.8,
    ),
    tr1("whip_pan", "Whip Pan", SLIDE, shader::WHIP, LEFT, 0.6),
    tr1("whip_right", "Whip Right", SLIDE, shader::WHIP, RIGHT, 0.6),
    tr1("whip_up", "Whip Up", SLIDE, shader::WHIP, UP, 0.6),
    tr1("whip_down", "Whip Down", SLIDE, shader::WHIP, DOWN, 0.6),
    tr1(
        "slide.film_up",
        "Film Roll Up",
        SLIDE,
        shader::FILM_ROLL,
        UP,
        0.9,
    ),
    tr1(
        "slide.film_down",
        "Film Roll Down",
        SLIDE,
        shader::FILM_ROLL,
        DOWN,
        0.9,
    ),
    // Zoom
    tr1(
        "zoom.punch",
        "Zoom Punch",
        ZOOM,
        shader::ZOOM_PUNCH,
        0.0,
        0.7,
    ),
    tr1("zoom.in", "Zoom In", ZOOM, shader::ZOOM_IN, 0.0, 1.0),
    tr1("zoom.out", "Zoom Out", ZOOM, shader::ZOOM_OUT, 0.0, 1.0),
    tr1(
        "zoom.cross",
        "Cross Zoom",
        ZOOM,
        shader::CROSS_ZOOM,
        0.0,
        0.8,
    ),
    tr1("zoom.spin", "Spin Zoom", ZOOM, shader::SPIN_ZOOM, 0.0, 1.0),
    // 3D
    tr1("cube.left", "Cube Left", D3, shader::CUBE, LEFT, 1.0),
    tr1("cube.right", "Cube Right", D3, shader::CUBE, RIGHT, 1.0),
    tr1("cube.up", "Cube Up", D3, shader::CUBE, UP, 1.0),
    tr1("cube.down", "Cube Down", D3, shader::CUBE, DOWN, 1.0),
    tr1("page_curl", "Page Curl", D3, shader::CURL, 0.0, 1.2),
    tr1("card_flip", "Card Flip", D3, shader::FLIP, 0.0, 1.0),
    tr1("door_open", "Door Open", D3, shader::DOOR, 0.0, 1.2),
    tr1("fold", "Fold", D3, shader::FOLD, 6.0, 1.0),
    // Blur
    tr1(
        "blur.dissolve",
        "Blur Dissolve",
        BLUR,
        shader::BLUR,
        0.0,
        1.0,
    ),
    tr1(
        "blur.directional",
        "Directional Blur",
        BLUR,
        shader::DIR_BLUR,
        0.0,
        1.0,
    ),
    tr1("blur.defocus", "Defocus", BLUR, shader::DEFOCUS, 0.0, 1.0),
    // Glitch
    tr1("glitch", "Glitch", GLITCH, shader::GLITCH, 0.0, 0.6),
    tr1(
        "rgb_split",
        "RGB Split",
        GLITCH,
        shader::RGB_SPLIT,
        0.0,
        0.8,
    ),
    tr1("pixelate", "Pixelate", GLITCH, shader::PIXELATE, 0.0, 1.0),
    tr1(
        "digital_noise",
        "Digital Noise",
        GLITCH,
        shader::DIGITAL,
        0.0,
        0.8,
    ),
    // Split
    tr1(
        "split.vertical",
        "Split Vertical",
        SPLIT,
        shader::SPLIT,
        0.0,
        1.0,
    ),
    tr1(
        "split.horizontal",
        "Split Horizontal",
        SPLIT,
        shader::SPLIT,
        1.0,
        1.0,
    ),
    tr1(
        "split.corners",
        "Four Corners",
        SPLIT,
        shader::CORNERS,
        0.0,
        1.0,
    ),
    tr(
        "split.stripes",
        "Stripes",
        SPLIT,
        shader::STRIPES,
        [8.0, 0.0, 0.0, 0.0],
        BLACK,
        1.0,
    ),
    tr1(
        "split.mosaic",
        "Mosaic Tiles",
        SPLIT,
        shader::TILES,
        5.0,
        1.2,
    ),
    // Warp
    tr1("ripple", "Ripple", WARP, shader::RIPPLE, 0.0, 1.2),
    tr1("swirl", "Swirl", WARP, shader::SWIRL, 0.0, 1.0),
    tr1("liquid", "Liquid", WARP, shader::LIQUID, 0.0, 1.2),
    tr1("dreamy", "Dreamy", WARP, shader::DREAMY, 0.0, 1.5),
    // Light Leak
    tr(
        "light.wipe",
        "Light Wipe",
        LIGHT,
        shader::LIGHT_WIPE,
        [RIGHT, 0.0, 0.0, 0.0],
        [1.0, 0.9, 0.7, 1.0],
        0.9,
    ),
    tr(
        "light.wipe_up",
        "Light Wipe Up",
        LIGHT,
        shader::LIGHT_WIPE,
        [UP, 0.0, 0.0, 0.0],
        [0.7, 0.85, 1.0, 1.0],
        0.9,
    ),
    tr(
        "leak.amber",
        "Amber Leak",
        LIGHT,
        shader::LEAK,
        [0.0; 4],
        [1.0, 0.5, 0.12, 1.0],
        1.2,
    ),
    tr(
        "leak.rose",
        "Rose Leak",
        LIGHT,
        shader::LEAK,
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.28, 0.45, 1.0],
        1.2,
    ),
    tr(
        "leak.azure",
        "Azure Leak",
        LIGHT,
        shader::LEAK,
        [2.0, 0.0, 0.0, 0.0],
        [0.25, 0.55, 1.0, 1.0],
        1.2,
    ),
    tr("flash", "Flash", LIGHT, shader::FLASH, [0.0; 4], WHITE, 0.8),
    tr1("film_burn", "Film Burn", LIGHT, shader::BURN, 0.0, 1.2),
    tr(
        "lens_flare",
        "Lens Flare",
        LIGHT,
        shader::FLARE,
        [0.0; 4],
        [0.45, 0.7, 1.0, 1.0],
        1.2,
    ),
    // Shape
    tr1("shape.circle", "Iris Circle", SHAPE, shader::IRIS, 0.0, 1.0),
    tr(
        "shape.diamond",
        "Diamond",
        SHAPE,
        shader::SHAPE,
        [0.0, 0.0, 0.0, 0.0],
        WHITE,
        1.0,
    ),
    tr(
        "shape.star",
        "Star",
        SHAPE,
        shader::SHAPE,
        [1.0, 0.2, 0.012, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "shape.heart",
        "Heart",
        SHAPE,
        shader::SHAPE,
        [2.0, 0.0, 0.012, 0.0],
        [1.0, 0.55, 0.7, 1.0],
        1.2,
    ),
    tr(
        "shape.hexagon",
        "Hexagon",
        SHAPE,
        shader::SHAPE,
        [3.0, 1.0 / 6.0, 0.0, 0.0],
        WHITE,
        1.0,
    ),
    // Motion graphics
    tr(
        "mg.bars_left",
        "Bars Left",
        MG,
        shader::MG_BARS,
        [LEFT, 6.0, 0.0, 0.0],
        [0.12, 0.82, 0.76, 1.0],
        0.9,
    ),
    tr(
        "mg.bars_right",
        "Bars Right",
        MG,
        shader::MG_BARS,
        [RIGHT, 6.0, 0.0, 0.0],
        [0.95, 0.3, 0.6, 1.0],
        0.9,
    ),
    tr(
        "mg.bars_up",
        "Bars Up",
        MG,
        shader::MG_BARS,
        [UP, 8.0, 0.0, 0.0],
        [1.0, 0.7, 0.15, 1.0],
        0.9,
    ),
    tr(
        "mg.bars_down",
        "Bars Down",
        MG,
        shader::MG_BARS,
        [DOWN, 8.0, 0.0, 0.0],
        [0.3, 0.45, 1.0, 1.0],
        0.9,
    ),
    tr(
        "mg.bars_dark",
        "Dark Bars",
        MG,
        shader::MG_BARS,
        [RIGHT, 4.0, 0.0, 0.0],
        [0.08, 0.09, 0.13, 1.0],
        0.9,
    ),
    // More motion graphics, brush, paper, dust, camera, luxury, glass, cards
    tr(
        "mg.circles_across",
        "Circles Across",
        MG,
        shader::MG_CIRCLES,
        [0.0, 8.0, 0.0, 0.0],
        [1.0, 0.62, 0.18, 1.0],
        1.0,
    ),
    tr(
        "mg.circles_radial",
        "Circles Burst",
        MG,
        shader::MG_CIRCLES,
        [1.0, 7.0, 0.0, 0.0],
        [0.28, 0.5, 1.0, 1.0],
        1.0,
    ),
    tr(
        "mg.circles_scatter",
        "Circles Scatter",
        MG,
        shader::MG_CIRCLES,
        [2.0, 10.0, 0.0, 0.0],
        [1.0, 0.32, 0.62, 1.0],
        1.0,
    ),
    tr(
        "mg.circles_diagonal",
        "Circles Diagonal",
        MG,
        shader::MG_CIRCLES,
        [3.0, 6.0, 0.0, 0.0],
        [0.2, 0.9, 0.7, 1.0],
        1.0,
    ),
    tr(
        "mg.dots_big",
        "Big Dots",
        MG,
        shader::MG_CIRCLES,
        [1.0, 4.0, 0.0, 0.0],
        [0.55, 0.35, 1.0, 1.0],
        1.0,
    ),
    tr(
        "mg.blobs_across",
        "Blobs Across",
        MG,
        shader::MG_CIRCLES,
        [0.0, 6.0, 1.0, 0.0],
        [1.0, 0.32, 0.62, 1.0],
        1.0,
    ),
    tr(
        "mg.blobs_scatter",
        "Blobs Scatter",
        MG,
        shader::MG_CIRCLES,
        [2.0, 8.0, 1.0, 0.0],
        [1.0, 0.62, 0.18, 1.0],
        1.0,
    ),
    tr(
        "mg.rings_out",
        "Rings Out",
        MG,
        shader::MG_RINGS,
        [1.0, 5.0, 0.0, 0.0],
        [0.28, 0.5, 1.0, 1.0],
        1.1,
    ),
    tr(
        "mg.rings_in",
        "Rings In",
        MG,
        shader::MG_RINGS,
        [0.0, 5.0, 0.0, 0.0],
        [1.0, 0.32, 0.62, 1.0],
        1.1,
    ),
    tr(
        "mg.rings_fine",
        "Fine Rings",
        MG,
        shader::MG_RINGS,
        [1.0, 9.0, 0.0, 0.0],
        [0.2, 0.9, 0.7, 1.0],
        1.1,
    ),
    tr(
        "mg.wave_left",
        "Colour Wave Left",
        MG,
        shader::MG_WAVE,
        [LEFT, 0.0, 0.0, 0.0],
        [0.55, 0.35, 1.0, 1.0],
        1.0,
    ),
    tr(
        "mg.wave_right",
        "Colour Wave Right",
        MG,
        shader::MG_WAVE,
        [RIGHT, 0.0, 0.0, 0.0],
        [1.0, 0.32, 0.62, 1.0],
        1.0,
    ),
    tr(
        "mg.wave_up",
        "Colour Wave Up",
        MG,
        shader::MG_WAVE,
        [UP, 0.0, 0.0, 0.0],
        [1.0, 0.62, 0.18, 1.0],
        1.0,
    ),
    tr(
        "mg.wave_down",
        "Colour Wave Down",
        MG,
        shader::MG_WAVE,
        [DOWN, 0.0, 0.0, 0.0],
        [0.28, 0.5, 1.0, 1.0],
        1.0,
    ),
    tr(
        "mg.wave_dark",
        "Dark Wave",
        MG,
        shader::MG_WAVE,
        [RIGHT, 0.0, 0.0, 0.0],
        [0.08, 0.09, 0.13, 1.0],
        1.0,
    ),
    tr(
        "brush.stroke_left",
        "Brush Left",
        BRUSH,
        shader::BRUSH,
        [LEFT, 0.5, 0.9, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.1,
    ),
    tr(
        "brush.stroke_right",
        "Brush Right",
        BRUSH,
        shader::BRUSH,
        [RIGHT, 0.5, 0.9, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.1,
    ),
    tr(
        "brush.stroke_up",
        "Brush Up",
        BRUSH,
        shader::BRUSH,
        [UP, 0.5, 0.9, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.1,
    ),
    tr(
        "brush.stroke_down",
        "Brush Down",
        BRUSH,
        shader::BRUSH,
        [DOWN, 0.5, 0.9, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.1,
    ),
    tr(
        "brush.bold",
        "Bold Brush",
        BRUSH,
        shader::BRUSH,
        [RIGHT, 0.15, 1.0, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.2,
    ),
    tr(
        "brush.fine",
        "Fine Brush",
        BRUSH,
        shader::BRUSH,
        [LEFT, 0.95, 0.7, 0.0],
        [0.05, 0.05, 0.07, 1.0],
        1.1,
    ),
    tr(
        "brush.gold",
        "Gold Brush",
        BRUSH,
        shader::BRUSH,
        [RIGHT, 0.6, 1.0, 0.0],
        [0.85, 0.62, 0.18, 1.0],
        1.1,
    ),
    tr(
        "paper.tear_sideways",
        "Paper Tear",
        BRUSH,
        shader::TEAR,
        [RIGHT, 0.5, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "paper.tear_vertical",
        "Paper Tear Vertical",
        BRUSH,
        shader::TEAR,
        [DOWN, 0.5, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "paper.tear_rough",
        "Rough Tear",
        BRUSH,
        shader::TEAR,
        [LEFT, 1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "paper.tear_clean",
        "Clean Tear",
        BRUSH,
        shader::TEAR,
        [UP, 0.1, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "dust.drift_left",
        "Dust Left",
        DUST,
        shader::DUST,
        [LEFT, 0.5, 0.3, 0.0],
        [1.0, 0.78, 0.35, 1.0],
        1.2,
    ),
    tr(
        "dust.drift_right",
        "Dust Right",
        DUST,
        shader::DUST,
        [RIGHT, 0.5, 0.3, 0.0],
        [1.0, 0.78, 0.35, 1.0],
        1.2,
    ),
    tr(
        "dust.drift_up",
        "Dust Up",
        DUST,
        shader::DUST,
        [UP, 0.5, 0.3, 0.0],
        [0.85, 0.9, 1.0, 1.0],
        1.2,
    ),
    tr(
        "dust.drift_down",
        "Dust Down",
        DUST,
        shader::DUST,
        [DOWN, 0.5, 0.3, 0.0],
        [0.85, 0.9, 1.0, 1.0],
        1.2,
    ),
    tr(
        "dust.fine",
        "Fine Dust",
        DUST,
        shader::DUST,
        [RIGHT, 0.8, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "dust.embers",
        "Embers",
        DUST,
        shader::DUST,
        [UP, 0.4, 0.6, 0.0],
        [1.0, 0.45, 0.1, 1.0],
        1.2,
    ),
    tr(
        "camera.shutter_6",
        "Shutter",
        CAMERA,
        shader::SHUTTER,
        [6.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "camera.shutter_8",
        "Shutter 8 Blades",
        CAMERA,
        shader::SHUTTER,
        [8.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "camera.shutter_5",
        "Shutter 5 Blades",
        CAMERA,
        shader::SHUTTER,
        [5.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "camera.speed_white",
        "Speed Rush",
        CAMERA,
        shader::SPEED,
        [0.0, 90.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "camera.speed_colour",
        "Speed Rush Colour",
        CAMERA,
        shader::SPEED,
        [1.0, 90.0, 0.0, 0.0],
        [1.0, 0.32, 0.62, 1.0],
        0.9,
    ),
    tr(
        "camera.speed_impact",
        "Impact Frame",
        CAMERA,
        shader::SPEED,
        [2.0, 70.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "camera.speed_dense",
        "Dense Lines",
        CAMERA,
        shader::SPEED,
        [0.0, 160.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "camera.freeze",
        "Photo Freeze",
        CAMERA,
        shader::FREEZE,
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.5,
    ),
    tr(
        "camera.freeze_instant",
        "Instant Photo",
        CAMERA,
        shader::FREEZE,
        [1.0, 1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.5,
    ),
    tr(
        "camera.freeze_dark",
        "Dark Frame Freeze",
        CAMERA,
        shader::FREEZE,
        [2.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.5,
    ),
    tr(
        "luxury.sweep_gold",
        "Gold Sweep",
        LUXURY,
        shader::GOLD,
        [RIGHT, 0.5, 0.0, 0.0],
        [1.0, 0.78, 0.35, 1.0],
        1.1,
    ),
    tr(
        "luxury.sweep_gold_left",
        "Gold Sweep Left",
        LUXURY,
        shader::GOLD,
        [LEFT, 0.5, 0.0, 0.0],
        [1.0, 0.78, 0.35, 1.0],
        1.1,
    ),
    tr(
        "luxury.sweep_silver",
        "Silver Sweep",
        LUXURY,
        shader::GOLD,
        [RIGHT, 0.5, 0.0, 0.0],
        [0.85, 0.9, 1.0, 1.0],
        1.1,
    ),
    tr(
        "luxury.sweep_rose",
        "Rose Sweep",
        LUXURY,
        shader::GOLD,
        [UP, 0.6, 0.0, 0.0],
        [1.0, 0.6, 0.7, 1.0],
        1.1,
    ),
    tr(
        "luxury.sweep_ice",
        "Ice Sweep",
        LUXURY,
        shader::GOLD,
        [DOWN, 0.6, 0.0, 0.0],
        [0.55, 0.85, 1.0, 1.0],
        1.1,
    ),
    tr(
        "glass.frost_left",
        "Frosted Pane Left",
        GLASS,
        shader::FROST,
        [LEFT, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "glass.frost_right",
        "Frosted Pane Right",
        GLASS,
        shader::FROST,
        [RIGHT, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "glass.frost_up",
        "Frosted Pane Up",
        GLASS,
        shader::FROST,
        [UP, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "glass.frost_down",
        "Frosted Pane Down",
        GLASS,
        shader::FROST,
        [DOWN, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "glass.frost_wide",
        "Wide Frosted Pane",
        GLASS,
        shader::FROST,
        [RIGHT, 1.0, 0.9, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.3,
    ),
    tr(
        "glass.balls_across",
        "Glass Balls Across",
        GLASS,
        shader::MG_CIRCLES,
        [0.0, 6.0, 2.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.balls_burst",
        "Glass Balls Burst",
        GLASS,
        shader::MG_CIRCLES,
        [1.0, 5.0, 2.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.balls_scatter",
        "Glass Balls Scatter",
        GLASS,
        shader::MG_CIRCLES,
        [2.0, 8.0, 2.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.balls_big",
        "Big Glass Balls",
        GLASS,
        shader::MG_CIRCLES,
        [1.0, 3.0, 2.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.shatter",
        "Shatter",
        GLASS,
        shader::SHATTER,
        [9.0, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.shatter_fine",
        "Fine Shatter",
        GLASS,
        shader::SHATTER,
        [16.0, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.shatter_big",
        "Big Shards",
        GLASS,
        shader::SHATTER,
        [5.0, 0.5, 0.5, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "glass.shatter_corner",
        "Corner Shatter",
        GLASS,
        shader::SHATTER,
        [9.0, 0.1, 0.1, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.2,
    ),
    tr(
        "cards.push_left",
        "Card Left",
        CARDS,
        shader::CARDPUSH,
        [LEFT, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "cards.push_right",
        "Card Right",
        CARDS,
        shader::CARDPUSH,
        [RIGHT, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "cards.push_up",
        "Card Up",
        CARDS,
        shader::CARDPUSH,
        [UP, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "cards.push_down",
        "Card Down",
        CARDS,
        shader::CARDPUSH,
        [DOWN, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        0.9,
    ),
    tr(
        "cards.rise",
        "Card Rise",
        CARDS,
        shader::CARDPUSH,
        [UP, 0.0, 1.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
    tr(
        "slabs.glass_across",
        "Glass Slabs",
        D3,
        shader::SLABS,
        [8.0, 0.0, 0.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_centre",
        "Glass Slabs Centre",
        D3,
        shader::SLABS,
        [10.0, 1.0, 0.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_scatter",
        "Glass Slabs Scatter",
        D3,
        shader::SLABS,
        [8.0, 2.0, 0.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_wide",
        "Wide Glass Slabs",
        D3,
        shader::SLABS,
        [4.0, 0.0, 0.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_rows",
        "Glass Rows",
        D3,
        shader::SLABS,
        [6.0, 0.0, 1.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_rows_scatter",
        "Glass Rows Scatter",
        D3,
        shader::SLABS,
        [9.0, 2.0, 1.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "slabs.glass_fine",
        "Fine Glass Slabs",
        D3,
        shader::SLABS,
        [16.0, 1.0, 0.0, 0.0],
        WHITE,
        1.2,
    ),
    tr(
        "tiles3d.flip_stairs",
        "Tile Flip Stairs",
        D3,
        shader::TILEFLIP,
        [8.0, 0.0, 0.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "tiles3d.flip_centre",
        "Tile Flip Centre",
        D3,
        shader::TILEFLIP,
        [7.0, 1.0, 0.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "tiles3d.flip_scatter",
        "Tile Flip Scatter",
        D3,
        shader::TILEFLIP,
        [10.0, 2.0, 0.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "tiles3d.flip_rows",
        "Tile Flip Rows",
        D3,
        shader::TILEFLIP,
        [8.0, 3.0, 1.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "tiles3d.flip_big",
        "Big Tile Flip",
        D3,
        shader::TILEFLIP,
        [4.0, 0.0, 1.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "tiles3d.flip_fine",
        "Fine Tile Flip",
        D3,
        shader::TILEFLIP,
        [16.0, 1.0, 0.0, 0.0],
        WHITE,
        1.3,
    ),
    tr(
        "torus.neon_cyan",
        "Neon Ring",
        D3,
        shader::TORUS,
        [1.0, 0.0, 1.0, 0.4],
        [0.25, 0.9, 1.0, 1.0],
        1.3,
    ),
    tr(
        "torus.neon_triple",
        "Neon Rings",
        D3,
        shader::TORUS,
        [3.0, 0.4, 1.0, 0.3],
        [1.0, 0.3, 0.8, 1.0],
        1.3,
    ),
    tr(
        "torus.glass_ring",
        "Glass Ring",
        D3,
        shader::TORUS,
        [1.0, 0.0, 2.0, 0.7],
        [0.8, 0.92, 1.0, 1.0],
        1.3,
    ),
    tr(
        "torus.glass_tilted",
        "Tilted Glass Ring",
        D3,
        shader::TORUS,
        [2.0, 0.6, 2.0, 0.6],
        [0.7, 0.85, 1.0, 1.0],
        1.3,
    ),
    tr(
        "ice.crack_centre",
        "Cracking Ice",
        GLASS,
        shader::ICE,
        [9.0, 0.5, 0.5, 0.0],
        [0.72, 0.88, 1.0, 1.0],
        1.4,
    ),
    tr(
        "ice.crack_fine",
        "Fine Ice",
        GLASS,
        shader::ICE,
        [15.0, 0.5, 0.5, 0.0],
        [0.9, 0.96, 1.0, 1.0],
        1.4,
    ),
    tr(
        "ice.crack_big",
        "Big Ice Plates",
        GLASS,
        shader::ICE,
        [5.0, 0.5, 0.5, 0.0],
        [0.72, 0.88, 1.0, 1.0],
        1.4,
    ),
    tr(
        "ice.crack_corner",
        "Corner Ice",
        GLASS,
        shader::ICE,
        [9.0, 0.15, 0.15, 0.0],
        [0.72, 0.88, 1.0, 1.0],
        1.4,
    ),
    tr(
        "ice.crack_deep",
        "Deep Blue Ice",
        GLASS,
        shader::ICE,
        [10.0, 0.5, 0.45, 0.0],
        [0.45, 0.7, 0.95, 1.0],
        1.4,
    ),
    tr(
        "steps.up",
        "Steps Up",
        MG,
        shader::STEPS,
        [UP, 8.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
    tr(
        "steps.down",
        "Steps Down",
        MG,
        shader::STEPS,
        [DOWN, 8.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
    tr(
        "steps.left",
        "Steps Left",
        MG,
        shader::STEPS,
        [LEFT, 8.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
    tr(
        "steps.right",
        "Steps Right",
        MG,
        shader::STEPS,
        [RIGHT, 8.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
    tr(
        "steps.pyramid",
        "Pyramid Steps",
        MG,
        shader::STEPS,
        [UP, 9.0, 1.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "steps.scatter",
        "Scattered Blocks",
        MG,
        shader::STEPS,
        [UP, 10.0, 2.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.1,
    ),
    tr(
        "steps.fine",
        "Fine Steps",
        MG,
        shader::STEPS,
        [RIGHT, 18.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        1.0,
    ),
];

pub fn get(id: &str) -> Option<&'static TransitionDef> {
    TRANSITIONS.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `case` lines of `fs_transition`.
    fn shader_cases() -> String {
        let src = crate::shader_source();
        // Both dispatchers: the second one (ids 49 and up) comes first.
        let second = src
            .find("fn tr_dispatch_b")
            .expect("tr_dispatch_b in the WGSL");
        let main = src
            .find("fn fs_transition")
            .expect("fs_transition in the WGSL");
        let tail = &src[main..];
        let end = main
            + tail
                .find(
                    "
}
",
                )
                .unwrap_or(tail.len());
        src[second..end].to_string()
    }

    #[test]
    fn ids_are_unique_and_every_shader_exists() {
        let cases = shader_cases();
        let mut seen = std::collections::HashSet::new();
        for t in TRANSITIONS {
            assert!(seen.insert(t.id), "duplicate transition id {}", t.id);
            assert!(
                t.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'),
                "id {} must be lowercase",
                t.id
            );
            assert!(
                cases.contains(&format!("case {}u:", t.shader)),
                "{} uses shader {} with no case in fs_transition",
                t.id,
                t.shader
            );
            assert!(
                (0.5..=1.5).contains(&t.duration),
                "{} duration {}",
                t.id,
                t.duration
            );
            assert!(!t.name.is_empty());
        }
        // Ids saved by earlier versions.
        for id in ["dissolve", "fade_black"] {
            assert!(get(id).is_some(), "{id} missing");
        }
    }

    #[test]
    fn categories_come_in_order() {
        let cats = crate::catalog::categories(TRANSITIONS, |t| t.category);
        assert_eq!(
            cats,
            [
                FADE, WIPE, SLIDE, ZOOM, D3, BLUR, GLITCH, SPLIT, WARP, LIGHT, SHAPE, MG, BRUSH,
                DUST, CAMERA, LUXURY, GLASS, CARDS,
            ]
        );
    }
}
