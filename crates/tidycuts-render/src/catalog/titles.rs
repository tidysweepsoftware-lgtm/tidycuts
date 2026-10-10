//! Title presets: a text style plus default position and animations.

use tidycuts_core::{TextAlign, TitleSpec};

#[derive(Clone, Copy)]
pub struct TitlePreset {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub make: fn() -> TitleSpec,
    /// Offset of the title's centre from the frame centre, in 1920x1080 pixels.
    pub position: [f32; 2],
    /// In/out animation ids with durations in seconds.
    pub anim_in: Option<(&'static str, f32)>,
    pub anim_out: Option<(&'static str, f32)>,
    /// Default length in seconds.
    pub duration: f32,
}

/// An opaque color from 0xRRGGBB.
fn hex(rgb: u32) -> [f32; 4] {
    hexa(rgb, 1.0)
}

fn hexa(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        a,
    ]
}

/// The common part of every preset.
fn spec(style: &str, text: &str, font: &str, size: f32, color: u32, accent: u32) -> TitleSpec {
    TitleSpec {
        style: style.into(),
        text: text.into(),
        font: font.into(),
        size,
        color: hex(color),
        accent: hex(accent),
        ..TitleSpec::default()
    }
}

const BASIC: &str = "Basic";
const MODERN: &str = "Modern";
const CAPTIONS: &str = "Animated Captions";
const LOWER: &str = "Lower Thirds";
const CALLOUTS: &str = "Callouts";
const OPENERS: &str = "Openers";
const CREDITS: &str = "End Credits";
const SUBTITLES: &str = "Subtitles";
const SOCIAL: &str = "Social";

/// Where lower thirds sit: bottom-left, inside the title-safe area.
const LOWER_LEFT: [f32; 2] = [-560.0, 340.0];
const SUBTITLE_POS: [f32; 2] = [0.0, 400.0];

#[allow(clippy::too_many_arguments)]
const fn preset(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    position: [f32; 2],
    anim_in: Option<(&'static str, f32)>,
    anim_out: Option<(&'static str, f32)>,
    duration: f32,
    make: fn() -> TitleSpec,
) -> TitlePreset {
    TitlePreset {
        id,
        name,
        category,
        make,
        position,
        anim_in,
        anim_out,
        duration,
    }
}

pub static TITLES: &[TitlePreset] = &[
    // ----- Basic -----
    TitlePreset {
        id: "basic.title",
        name: "Basic Title",
        category: BASIC,
        make: TitleSpec::default,
        position: [0.0, 0.0],
        anim_in: Some(("fade", 0.5)),
        anim_out: Some(("fade", 0.5)),
        duration: 5.0,
    },
    preset(
        "basic.bold_shadow",
        "Bold Shadow",
        BASIC,
        [0.0, 0.0],
        Some(("pop", 0.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            shadow: 70.0,
            ..spec(
                "plain",
                "BIG BOLD TITLE",
                "Anton",
                150.0,
                0xffffff,
                0x2ed8b0,
            )
        },
    ),
    preset(
        "basic.elegant",
        "Elegant Serif",
        BASIC,
        [0.0, 0.0],
        Some(("fade", 1.0)),
        Some(("fade", 1.0)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            shadow: 35.0,
            ..spec(
                "double_line",
                "Elegant Title",
                "DM Serif Display",
                116.0,
                0xfff6e5,
                0xe8c27a,
            )
        },
    ),
    preset(
        "basic.hollow",
        "Hollow",
        BASIC,
        [0.0, 0.0],
        Some(("zoom_out", 0.8)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.08,
            ..spec(
                "stroke_only",
                "OUTLINED",
                "Bebas Neue",
                190.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "basic.box",
        "Color Box",
        BASIC,
        [0.0, 0.0],
        Some(("wipe_right", 0.6)),
        Some(("wipe_left", 0.5)),
        5.0,
        || {
            spec(
                "box",
                "Your Title Here",
                "Poppins Black",
                88.0,
                0x14141a,
                0xffd23f,
            )
        },
    ),
    preset(
        "basic.script",
        "Script",
        BASIC,
        [0.0, 0.0],
        Some(("blur", 1.0)),
        Some(("fade", 0.8)),
        6.0,
        || TitleSpec {
            shadow: 45.0,
            ..spec(
                "plain",
                "Beautiful Moments",
                "Great Vibes",
                150.0,
                0xffffff,
                0xffd6e0,
            )
        },
    ),
    preset(
        "basic.underline",
        "Underline",
        BASIC,
        [0.0, 0.0],
        Some(("rise", 0.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 30.0,
            ..spec(
                "underline",
                "Simple & Clean",
                "Poppins SemiBold",
                90.0,
                0xffffff,
                0x2ed8b0,
            )
        },
    ),
    preset(
        "basic.gradient",
        "Gradient",
        BASIC,
        [0.0, 0.0],
        Some(("zoom_in", 0.6)),
        Some(("zoom_out", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 40.0,
            ..spec(
                "gradient",
                "Gradient Title",
                "Poppins Black",
                120.0,
                0xffe259,
                0xff5f6d,
            )
        },
    ),
    // ----- Modern -----
    preset(
        "modern.refined",
        "Refined",
        MODERN,
        [0.0, 0.0],
        Some(("tracking", 1.4)),
        Some(("fade", 0.6)),
        6.0,
        || {
            spec(
                "refined",
                "SIMPLE ELEGANCE",
                "Poppins",
                84.0,
                0xffffff,
                0xe8c27a,
            )
        },
    ),
    preset(
        "modern.editorial",
        "Editorial",
        MODERN,
        [0.0, 0.0],
        Some(("fade", 0.9)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            shadow: 25.0,
            ..spec(
                "editorial",
                "The Morning Edition\nA short story about quiet places",
                "DM Serif Display",
                104.0,
                0xfff8ec,
                0xfff8ec,
            )
        },
    ),
    preset(
        "modern.stack",
        "Poster Stack",
        MODERN,
        [0.0, 0.0],
        Some(("letters_rise", 1.0)),
        Some(("fade", 0.6)),
        6.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "stack",
                "SUMMER\nTHE BEST TRIP EVER",
                "Anton",
                200.0,
                0xffffff,
                0xffd23f,
            )
        },
    ),
    preset(
        "modern.glass",
        "Glass Card",
        MODERN,
        [0.0, 0.0],
        Some(("pop", 0.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 30.0,
            ..spec(
                "glass",
                "Good Vibes Only",
                "Poppins SemiBold",
                92.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "modern.message",
        "Message Card",
        MODERN,
        [60.0, -200.0],
        Some(("slide_down", 0.6)),
        Some(("slide_up", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            background_color: [0.1, 0.11, 0.14, 0.94],
            ..spec(
                "notification",
                "New message\nHey! Are we still on for tonight?",
                "Poppins SemiBold",
                62.0,
                0xffffff,
                0x34c759,
            )
        },
    ),
    preset(
        "modern.gold",
        "Gold Foil",
        MODERN,
        [0.0, 0.0],
        Some(("zoom_in", 1.0)),
        Some(("fade", 0.6)),
        6.0,
        || {
            spec(
                "gold",
                "GOLDEN HOUR",
                "Abril Fatface",
                150.0,
                0xfff2b0,
                0xc9922a,
            )
        },
    ),
    preset(
        "modern.quote",
        "Quote",
        MODERN,
        [0.0, 0.0],
        Some(("fade", 1.0)),
        Some(("fade", 0.8)),
        7.0,
        || {
            spec(
                "quote",
                "\u{201c}\nStay hungry, stay curious.\n\u{2014} Someone Wise",
                "DM Serif Display",
                84.0,
                0xffffff,
                0xffc857,
            )
        },
    ),
    preset(
        "modern.slam",
        "Slam",
        MODERN,
        [0.0, 0.0],
        Some(("slam", 0.55)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            shadow: 60.0,
            letter_spacing: 0.02,
            ..spec("plain", "BIGGER", "Anton", 230.0, 0xffffff, 0xffffff)
        },
    ),
    preset(
        "modern.soft",
        "Soft Caption",
        MODERN,
        [0.0, 0.0],
        Some(("fade", 1.2)),
        Some(("fade", 1.2)),
        6.0,
        || TitleSpec {
            shadow: 55.0,
            letter_spacing: 0.04,
            ..spec(
                "plain",
                "slow down and breathe",
                "Poppins",
                74.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "modern.glass_lower",
        "Glass Name",
        MODERN,
        [-480.0, 330.0],
        Some(("slide_right", 0.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 30.0,
            ..spec(
                "glass",
                "Alex Morgan\nCreative Director",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    // ----- Animated captions -----
    preset(
        "captions.karaoke",
        "Karaoke",
        CAPTIONS,
        [0.0, 380.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            background_color: [0.0, 0.0, 0.0, 0.62],
            ..spec(
                "subtitle",
                "SING ALONG WITH ME",
                "Poppins Black",
                84.0,
                0xffffff,
                0xffd23f,
            )
        },
    ),
    preset(
        "captions.hype",
        "Hype Caption",
        CAPTIONS,
        [0.0, 300.0],
        Some(("karaoke", 2.5)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 9.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "NO WAY THAT JUST HAPPENED",
                "Poppins Black",
                84.0,
                0xffffff,
                0xffe033,
            )
        },
    ),
    preset(
        "captions.word_pop",
        "Word Pop",
        CAPTIONS,
        [0.0, 300.0],
        Some(("words_pop", 1.4)),
        Some(("fade", 0.4)),
        5.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "THIS IS THE BEST PART",
                "Poppins Black",
                92.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "captions.letters_rise",
        "Letters Rise",
        CAPTIONS,
        [0.0, 0.0],
        Some(("letters_rise", 1.3)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 40.0,
            ..spec(
                "plain",
                "Rise and shine",
                "Poppins Bold",
                120.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "captions.letters_pop",
        "Letters Pop",
        CAPTIONS,
        [0.0, 0.0],
        Some(("letters_pop", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "long_shadow",
                "POP POP POP",
                "Bangers",
                170.0,
                0xffffff,
                0xff4f9a,
            )
        },
    ),
    preset(
        "captions.tracking",
        "Tracking In",
        CAPTIONS,
        [0.0, 0.0],
        Some(("tracking", 1.8)),
        Some(("fade", 0.6)),
        6.0,
        || TitleSpec {
            shadow: 30.0,
            ..spec(
                "plain",
                "CINEMATIC",
                "Poppins SemiBold",
                130.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "captions.flicker",
        "Flicker On",
        CAPTIONS,
        [0.0, 0.0],
        Some(("flicker", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 50.0,
            ..spec(
                "plain",
                "AFTER DARK",
                "Bebas Neue",
                190.0,
                0xffffff,
                0xff2e97,
            )
        },
    ),
    preset(
        "captions.slam_stack",
        "Slam Stack",
        CAPTIONS,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            shadow: 50.0,
            ..spec(
                "stack",
                "FINAL\nTHE LAST CHAPTER",
                "Anton",
                190.0,
                0xffffff,
                0xff5a36,
            )
        },
    ),
    // ----- Lower thirds -----
    preset(
        "lower.bar",
        "Accent Bar",
        LOWER,
        LOWER_LEFT,
        Some(("wipe_right", 0.7)),
        Some(("wipe_left", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            background_color: hexa(0x0d1117, 0.82),
            ..spec(
                "lower_bar",
                "Name Surname\nJob Title",
                "Poppins Bold",
                74.0,
                0xffffff,
                0x2ed8b0,
            )
        },
    ),
    preset(
        "lower.slant",
        "Slanted Panels",
        LOWER,
        [-560.0, 330.0],
        Some(("slide_right", 0.7)),
        Some(("slide_left", 0.6)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            background_color: hexa(0x16161f, 0.92),
            ..spec(
                "lower_slant",
                "NAME SURNAME\nJob Title",
                "Poppins Black",
                68.0,
                0xffffff,
                0xff3d5a,
            )
        },
    ),
    preset(
        "lower.tag",
        "Label Tag",
        LOWER,
        LOWER_LEFT,
        Some(("rise", 0.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 40.0,
            ..spec(
                "tag",
                "INTERVIEW\nName Surname",
                "Poppins Bold",
                76.0,
                0xffffff,
                0xffc93c,
            )
        },
    ),
    preset(
        "lower.serif",
        "Serif Underline",
        LOWER,
        LOWER_LEFT,
        Some(("fade", 0.8)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 45.0,
            ..spec(
                "underline",
                "Name Surname",
                "DM Serif Display",
                90.0,
                0xffffff,
                0xe8c27a,
            )
        },
    ),
    preset(
        "lower.minimal",
        "Minimal",
        LOWER,
        [-460.0, 300.0],
        Some(("slide_right", 0.6)),
        Some(("fade", 0.4)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 55.0,
            line_spacing: 1.0,
            ..spec(
                "callout",
                "Name Surname\nLocation or Role",
                "Poppins SemiBold",
                72.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "lower.news",
        "News Banner",
        LOWER,
        [-480.0, 360.0],
        Some(("wipe_right", 0.5)),
        Some(("wipe_left", 0.4)),
        6.0,
        || TitleSpec {
            align: TextAlign::Left,
            background: true,
            background_color: hexa(0x0b0f19, 0.85),
            ..spec(
                "tag",
                "BREAKING NEWS\nYour headline goes here",
                "Poppins Bold",
                64.0,
                0xffffff,
                0xe11d2e,
            )
        },
    ),
    preset(
        "lower.stacked",
        "Stacked Box",
        LOWER,
        [-620.0, 330.0],
        Some(("wipe_down", 0.5)),
        Some(("wipe_up", 0.4)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            letter_spacing: 0.03,
            line_spacing: 0.9,
            ..spec(
                "box",
                "NAME SURNAME\nCREATIVE DIRECTOR",
                "Bebas Neue",
                80.0,
                0x111111,
                0xffffff,
            )
        },
    ),
    preset(
        "lower.terminal",
        "Terminal",
        LOWER,
        LOWER_LEFT,
        Some(("typewriter", 1.5)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            align: TextAlign::Left,
            background_color: hexa(0x050805, 0.85),
            line_spacing: 1.0,
            ..spec(
                "subtitle",
                "> name_surname\n> job title",
                "VT323",
                78.0,
                0x39ff7a,
                0x39ff7a,
            )
        },
    ),
    preset(
        "lower.blocks",
        "Color Blocks",
        LOWER,
        LOWER_LEFT,
        Some(("slide_up", 0.6)),
        Some(("slide_down", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            background_color: hexa(0x7b61ff, 0.95),
            ..spec(
                "subtitle",
                "Name Surname\nPhotographer",
                "Poppins Bold",
                64.0,
                0xffffff,
                0x7b61ff,
            )
        },
    ),
    // ----- Callouts -----
    preset(
        "callout.pointer",
        "Pointer",
        CALLOUTS,
        [300.0, -140.0],
        Some(("pop", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 50.0,
            ..spec(
                "callout",
                "Look Here!\nSomething interesting",
                "Poppins Bold",
                72.0,
                0xffffff,
                0xffd23f,
            )
        },
    ),
    preset(
        "callout.pointer_right",
        "Pointer Right",
        CALLOUTS,
        [-300.0, -120.0],
        Some(("slide_left", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            align: TextAlign::Right,
            shadow: 50.0,
            ..spec(
                "callout",
                "Point of Interest\nA short description",
                "Poppins SemiBold",
                68.0,
                0xffffff,
                0x4cc9f0,
            )
        },
    ),
    preset(
        "callout.bubble",
        "Bubble",
        CALLOUTS,
        [360.0, -220.0],
        Some(("pop", 0.5)),
        Some(("pop", 0.4)),
        3.0,
        || TitleSpec {
            letter_spacing: 0.03,
            ..spec("pill", "WOW!", "Bangers", 110.0, 0x111111, 0xffffff)
        },
    ),
    preset(
        "callout.marker",
        "Marker",
        CALLOUTS,
        [0.0, -200.0],
        Some(("wipe_right", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || {
            spec(
                "highlight",
                "Don't miss this!",
                "Permanent Marker",
                84.0,
                0x16161a,
                0xffe14d,
            )
        },
    ),
    preset(
        "callout.frame",
        "Frame",
        CALLOUTS,
        [280.0, -160.0],
        Some(("zoom_in", 0.5)),
        Some(("zoom_out", 0.4)),
        4.0,
        || TitleSpec {
            shadow: 40.0,
            letter_spacing: 0.04,
            ..spec(
                "outline_box",
                "NOTICE THIS",
                "Poppins SemiBold",
                72.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    // ----- Openers -----
    preset(
        "opener.neon",
        "Neon Sign",
        OPENERS,
        [0.0, 0.0],
        Some(("fade", 1.2)),
        Some(("fade", 0.8)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.08,
            ..spec("neon", "NEON NIGHTS", "Monoton", 120.0, 0xffe6f4, 0xff2e97)
        },
    ),
    preset(
        "opener.cinematic",
        "Cinematic",
        OPENERS,
        [0.0, 0.0],
        Some(("blur", 1.5)),
        Some(("fade", 1.2)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.35,
            shadow: 40.0,
            ..spec(
                "plain",
                "THE JOURNEY",
                "Bebas Neue",
                160.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "opener.retro",
        "Retro 3D",
        OPENERS,
        [0.0, 0.0],
        Some(("zoom_out", 0.8)),
        Some(("zoom_in", 0.6)),
        5.0,
        || TitleSpec {
            outline: 4.0,
            outline_color: hex(0x2b1055),
            ..spec(
                "retro_3d",
                "RETRO WAVE",
                "Righteous",
                140.0,
                0xffe66d,
                0xff4f9a,
            )
        },
    ),
    preset(
        "opener.glitch",
        "Glitch",
        OPENERS,
        [0.0, 0.0],
        Some(("squeeze", 0.5)),
        Some(("stretch", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "glitch",
                "SYSTEM ERROR",
                "Black Ops One",
                120.0,
                0xffffff,
                0x00f0ff,
            )
        },
    ),
    preset(
        "opener.long_shadow",
        "Long Shadow",
        OPENERS,
        [0.0, 0.0],
        Some(("rise", 0.8)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.03,
            ..spec(
                "long_shadow",
                "BIG IDEA",
                "Anton",
                170.0,
                0xffffff,
                0x3a0ca3,
            )
        },
    ),
    preset(
        "opener.typewriter",
        "Typewriter",
        OPENERS,
        [0.0, 0.0],
        Some(("typewriter", 2.0)),
        Some(("fade", 0.6)),
        6.0,
        || TitleSpec {
            shadow: 75.0,
            ..spec(
                "plain",
                "Chapter One\nThe Beginning",
                "Special Elite",
                96.0,
                0xf5ecd7,
                0xf5ecd7,
            )
        },
    ),
    preset(
        "opener.split",
        "Split",
        OPENERS,
        [0.0, 0.0],
        Some(("slide_up", 0.7)),
        Some(("slide_down", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "split",
                "SPLIT SECOND",
                "Poppins Black",
                124.0,
                0xffffff,
                0x00f5d4,
            )
        },
    ),
    preset(
        "opener.summer",
        "Summer",
        OPENERS,
        [0.0, 0.0],
        Some(("bounce", 1.0)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            shadow: 50.0,
            ..spec(
                "gradient",
                "Summer Vibes",
                "Lobster",
                150.0,
                0xfff27a,
                0xff7a3d,
            )
        },
    ),
    preset(
        "opener.arcade",
        "Arcade",
        OPENERS,
        [0.0, 0.0],
        Some(("pop", 0.5)),
        Some(("squeeze", 0.4)),
        4.0,
        || TitleSpec {
            outline: 6.0,
            outline_color: hex(0xe8175d),
            line_spacing: 1.6,
            ..spec(
                "plain",
                "GAME START\nPRESS ANY KEY",
                "Press Start 2P",
                64.0,
                0xfff35c,
                0xe8175d,
            )
        },
    ),
    // ----- End credits -----
    preset(
        "credits.thanks",
        "Thanks for Watching",
        CREDITS,
        [0.0, 0.0],
        Some(("fade", 1.0)),
        Some(("fade", 1.0)),
        5.0,
        || TitleSpec {
            shadow: 45.0,
            ..spec(
                "plain",
                "Thanks for Watching",
                "Pacifico",
                110.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "credits.the_end",
        "The End",
        CREDITS,
        [0.0, 0.0],
        Some(("fade", 1.5)),
        Some(("fade", 1.5)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.06,
            shadow: 30.0,
            ..spec(
                "double_line",
                "The End",
                "Abril Fatface",
                150.0,
                0xfff6e5,
                0xfff6e5,
            )
        },
    ),
    preset(
        "credits.directed_by",
        "Directed By",
        CREDITS,
        [0.0, 0.0],
        Some(("rise", 1.0)),
        Some(("fade", 0.8)),
        5.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "tag",
                "DIRECTED BY\nName Surname",
                "DM Serif Display",
                104.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "credits.starring",
        "Starring",
        CREDITS,
        [0.0, 0.0],
        Some(("slide_up", 1.2)),
        Some(("slide_up", 1.2)),
        8.0,
        || TitleSpec {
            line_spacing: 1.1,
            shadow: 40.0,
            ..spec(
                "plain",
                "STARRING\n\nName Surname\nName Surname\nName Surname",
                "Poppins SemiBold",
                64.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "credits.special_thanks",
        "Special Thanks",
        CREDITS,
        [0.0, 0.0],
        Some(("zoom_out", 1.0)),
        Some(("fade", 0.8)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.18,
            ..spec(
                "brackets",
                "SPECIAL THANKS",
                "Bebas Neue",
                120.0,
                0xffffff,
                0xe8c27a,
            )
        },
    ),
    preset(
        "credits.see_you",
        "See You Soon",
        CREDITS,
        [0.0, 0.0],
        Some(("bounce", 1.0)),
        Some(("zoom_in", 0.5)),
        5.0,
        || TitleSpec {
            outline: 5.0,
            outline_color: hex(0x1b1b2f),
            shadow: 40.0,
            ..spec(
                "plain",
                "See You Soon!",
                "Lobster",
                130.0,
                0xffd23f,
                0xffd23f,
            )
        },
    ),
    // ----- Subtitles -----
    preset(
        "subtitle.classic",
        "Classic",
        SUBTITLES,
        SUBTITLE_POS,
        Some(("fade", 0.2)),
        Some(("fade", 0.2)),
        3.0,
        || TitleSpec {
            outline: 3.0,
            shadow: 35.0,
            ..spec(
                "plain",
                "Subtitle text goes here",
                "Poppins SemiBold",
                58.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "subtitle.boxed",
        "Boxed",
        SUBTITLES,
        [0.0, 380.0],
        Some(("fade", 0.2)),
        Some(("fade", 0.2)),
        3.0,
        || TitleSpec {
            background_color: hexa(0x000000, 0.65),
            ..spec(
                "subtitle",
                "Subtitle text goes here\nand continues on this line",
                "Inter",
                54.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "subtitle.yellow",
        "Yellow",
        SUBTITLES,
        SUBTITLE_POS,
        Some(("fade", 0.2)),
        Some(("fade", 0.2)),
        3.0,
        || TitleSpec {
            outline: 4.0,
            shadow: 30.0,
            ..spec(
                "plain",
                "Yellow subtitles for films",
                "Poppins Bold",
                58.0,
                0xffe14d,
                0xffe14d,
            )
        },
    ),
    preset(
        "subtitle.cinema",
        "Cinema",
        SUBTITLES,
        [0.0, 390.0],
        Some(("fade", 0.3)),
        Some(("fade", 0.3)),
        3.0,
        || TitleSpec {
            italic: true,
            shadow: 80.0,
            ..spec(
                "plain",
                "Where are we going?\nSomewhere new.",
                "Inter",
                54.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "subtitle.banner",
        "Banner",
        SUBTITLES,
        SUBTITLE_POS,
        Some(("fade", 0.2)),
        Some(("fade", 0.2)),
        3.0,
        || TitleSpec {
            background: true,
            background_color: hexa(0x101014, 0.8),
            ..spec(
                "plain",
                "A clean caption on a dark banner",
                "Poppins SemiBold",
                52.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    // ----- Social -----
    preset(
        "social.subscribe",
        "Subscribe",
        SOCIAL,
        [0.0, 320.0],
        Some(("pop", 0.5)),
        Some(("pop", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "pill",
                "SUBSCRIBE",
                "Poppins Black",
                72.0,
                0xffffff,
                0xff1e1e,
            )
        },
    ),
    preset(
        "social.handle",
        "Handle",
        SOCIAL,
        LOWER_LEFT,
        Some(("slide_right", 0.6)),
        Some(("slide_left", 0.5)),
        5.0,
        || TitleSpec {
            align: TextAlign::Left,
            shadow: 40.0,
            ..spec(
                "tag",
                "FOLLOW\n@yourhandle",
                "Poppins Bold",
                72.0,
                0xffffff,
                0xff3e8a,
            )
        },
    ),
    preset(
        "social.caption",
        "Bold Caption",
        SOCIAL,
        [0.0, 220.0],
        Some(("pop", 0.4)),
        Some(("fade", 0.3)),
        3.0,
        || TitleSpec {
            outline: 7.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "WAIT FOR IT...",
                "Poppins Black",
                92.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "social.highlight",
        "Highlight Caption",
        SOCIAL,
        [0.0, 220.0],
        Some(("pop", 0.4)),
        Some(("fade", 0.3)),
        3.0,
        || {
            spec(
                "highlight",
                "This changed everything",
                "Poppins Black",
                76.0,
                0xffffff,
                0x7b61ff,
            )
        },
    ),
    preset(
        "social.like_share",
        "Like & Share",
        SOCIAL,
        [0.0, -260.0],
        Some(("bounce", 0.8)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            italic: true,
            letter_spacing: 0.03,
            ..spec("box", "LIKE & SHARE", "Bangers", 100.0, 0x111111, 0xffd60a)
        },
    ),
    preset(
        "social.comic",
        "Comic Burst",
        SOCIAL,
        [0.0, 0.0],
        Some(("pop", 0.5)),
        Some(("zoom_in", 0.4)),
        3.0,
        || TitleSpec {
            outline: 7.0,
            outline_color: hex(0x111111),
            letter_spacing: 0.04,
            ..spec("retro_3d", "BOOM!", "Bangers", 200.0, 0xffe600, 0xff3d00)
        },
    ),
    preset(
        "social.new_video",
        "New Video",
        SOCIAL,
        [0.0, 0.0],
        Some(("zoom_in", 0.6)),
        Some(("fade", 0.5)),
        4.0,
        || TitleSpec {
            shadow: 50.0,
            ..spec(
                "tag",
                "NEW VIDEO\nWatch till the end",
                "Poppins Black",
                96.0,
                0xffffff,
                0x00e5ff,
            )
        },
    ),
    preset(
        "social.hashtag",
        "Hashtag",
        SOCIAL,
        [0.0, -280.0],
        Some(("slide_down", 0.6)),
        Some(("slide_up", 0.5)),
        4.0,
        || TitleSpec {
            shadow: 50.0,
            ..spec(
                "gradient",
                "#trending",
                "Righteous",
                110.0,
                0xffffff,
                0xff4fa3,
            )
        },
    ),
];

pub fn get(id: &str) -> Option<&'static TitlePreset> {
    TITLES.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::animations::ANIMATIONS;
    use crate::fonts::FONTS;
    use crate::text::{render_title, STYLES};

    #[test]
    fn ids_are_unique_and_dotted_by_category() {
        let mut seen = std::collections::HashSet::new();
        for t in TITLES {
            assert!(seen.insert(t.id), "duplicate id {}", t.id);
            assert!(
                t.id.contains('.') && t.id == t.id.to_lowercase(),
                "{}",
                t.id
            );
            assert!(t.duration > 0.0 && !t.name.is_empty());
        }
        assert!(get("basic.title").is_some());
        assert!(TITLES.len() >= 40);
    }

    #[test]
    fn categories_come_in_order() {
        let cats = crate::catalog::categories(TITLES, |t| t.category);
        assert_eq!(
            cats,
            [BASIC, MODERN, CAPTIONS, LOWER, CALLOUTS, OPENERS, CREDITS, SUBTITLES, SOCIAL]
        );
    }

    #[test]
    fn styles_fonts_and_animations_exist() {
        let mut ids = std::collections::HashSet::new();
        for (id, _) in STYLES {
            assert!(ids.insert(*id), "duplicate style {id}");
        }
        assert!(ids.contains("plain"));
        for t in TITLES {
            let s = (t.make)();
            assert!(
                ids.contains(s.style.as_str()),
                "{}: style {}",
                t.id,
                s.style
            );
            assert!(
                FONTS.iter().any(|f| f.name == s.font),
                "{}: font {}",
                t.id,
                s.font
            );
            for (a, d) in [t.anim_in, t.anim_out].into_iter().flatten() {
                assert!(
                    ANIMATIONS.iter().any(|x| x.id == a),
                    "{}: animation {a}",
                    t.id
                );
                assert!(d > 0.0 && d < t.duration * 0.5, "{}: {a} lasts {d}", t.id);
            }
        }
    }

    /// Every preset draws something, and its letters and decorations stay
    /// inside the 1920x1080 frame at the preset position.
    #[test]
    fn presets_render_inside_the_frame() {
        let scale = 0.25;
        for t in TITLES {
            let r = render_title(&(t.make)(), scale, 1.0, None);
            let (w, h) = (r.image.width as usize, r.image.height as usize);
            let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
            for y in 0..h {
                for x in 0..w {
                    if r.image.pixels[(y * w + x) * 4 + 3] > 24 {
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                        y0 = y0.min(y);
                        y1 = y1.max(y);
                    }
                }
            }
            assert!(x1 > x0 && y1 > y0, "{} is blank", t.id);
            let left = 960.0 + t.position[0] - r.size[0] * 0.5;
            let top = 540.0 + t.position[1] - r.size[1] * 0.5;
            let ink = [
                left + x0 as f32 / scale,
                top + y0 as f32 / scale,
                left + (x1 + 1) as f32 / scale,
                top + (y1 + 1) as f32 / scale,
            ];
            assert!(
                ink[0] >= 40.0 && ink[1] >= 30.0 && ink[2] <= 1880.0 && ink[3] <= 1050.0,
                "{} goes off-frame: {ink:?}",
                t.id
            );
        }
    }
}
