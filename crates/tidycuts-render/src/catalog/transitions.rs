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
        let start = src
            .find("fn fs_transition")
            .expect("fs_transition in the WGSL");
        let body = &src[start..];
        let end = body.find("\n}\n").unwrap_or(body.len());
        body[..end].to_string()
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
            [FADE, WIPE, SLIDE, ZOOM, D3, BLUR, GLITCH, SPLIT, WARP, LIGHT, SHAPE]
        );
    }
}
