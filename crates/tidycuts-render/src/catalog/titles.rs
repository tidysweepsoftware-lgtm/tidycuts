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
const CARDS: &str = "Cards";
const CLEAN: &str = "Clean";
const GLOSSY: &str = "Bold & Glossy";
const NEON: &str = "Neon & Glass";
const SCRIPT: &str = "Script & Elegant";
const TECH: &str = "Tech & Gaming";
const SPORT: &str = "Sport & Fitness";
const BIZ: &str = "Business & Events";

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
    // ----- New set: cards, clean, glossy, neon, script, tech, sport, business -----
    preset(
        "card.name_blue",
        "Name Card Blue",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Alex Morgan\nProduct Designer",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x2f80ff,
            )
        },
    ),
    preset(
        "card.name_pink",
        "Name Card Pink",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Sam Rivera\nHead of Growth",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0xe8417f,
            )
        },
    ),
    preset(
        "card.name_green",
        "Name Card Green",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Jordan Lee\nTrail Guide",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x1fb87a,
            )
        },
    ),
    preset(
        "card.name_orange",
        "Name Card Orange",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Taylor Kim\nHead Chef",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0xff7a29,
            )
        },
    ),
    preset(
        "card.name_violet",
        "Name Card Violet",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Robin Patel\nMusic Producer",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x7b4dff,
            )
        },
    ),
    preset(
        "card.name_dark",
        "Name Card Dark",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Casey Brooks\nFounder & CEO",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x23262e,
            )
        },
    ),
    preset(
        "card.name_light",
        "Name Card Light",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_gradient",
                "Morgan Ellis\nReal Estate Agent",
                "Poppins SemiBold",
                60.0,
                0x1b2230,
                0xf1f3f7,
            )
        },
    ),
    preset(
        "card.news_red",
        "News Bar Red",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "card_bar",
                "BREAKING NEWS",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0xeb2f35,
            )
        },
    ),
    preset(
        "card.news_blue",
        "News Bar Blue",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "card_bar",
                "LIVE UPDATE",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0x1f6bd6,
            )
        },
    ),
    preset(
        "card.location_teal",
        "Location Teal",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_bar",
                "Lisbon, Portugal",
                "Poppins",
                60.0,
                0xffffff,
                0x1fc2ad,
            )
        },
    ),
    preset(
        "card.location_amber",
        "Location Amber",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_bar",
                "Kyoto, Japan",
                "Poppins",
                60.0,
                0xffffff,
                0xffb81f,
            )
        },
    ),
    preset(
        "card.date_slate",
        "Date Slate",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "card_bar",
                "12 June 2026",
                "Poppins",
                56.0,
                0xffffff,
                0x3a4252,
            )
        },
    ),
    preset(
        "card.listing_gold",
        "Listing Gold",
        CARDS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.06,
            ..spec(
                "card_bar",
                "FOR SALE  ·  3 BED",
                "Poppins SemiBold",
                52.0,
                0xffffff,
                0xd8b25a,
            )
        },
    ),
    preset(
        "card.subscribe_red",
        "Subscribe Red",
        CARDS,
        [0.0, 360.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Subscribe",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0xf2503f,
            )
        },
    ),
    preset(
        "card.follow_violet",
        "Follow Violet",
        CARDS,
        [0.0, 360.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Follow",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x7b4dff,
            )
        },
    ),
    preset(
        "card.like_pink",
        "Like Pink",
        CARDS,
        [0.0, 360.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Like this",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0xec4899,
            )
        },
    ),
    preset(
        "card.new_video",
        "New Video Blue",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "New video",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x2f80ff,
            )
        },
    ),
    preset(
        "card.review_gold",
        "Review Gold",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Honest review",
                "Poppins SemiBold",
                60.0,
                0x241b05,
                0xffc233,
            )
        },
    ),
    preset(
        "card.sponsor_green",
        "Sponsored Green",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "card_pill",
                "Sponsored",
                "Poppins SemiBold",
                52.0,
                0xffffff,
                0x20b36b,
            )
        },
    ),
    preset(
        "card.price_orange",
        "Price Orange",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Only $49",
                "Poppins SemiBold",
                64.0,
                0xffffff,
                0xff6a2a,
            )
        },
    ),
    preset(
        "card.rating",
        "Rating",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "4.8 out of 5",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0x18a999,
            )
        },
    ),
    preset(
        "card.chapter",
        "Chapter Card",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "card_pill",
                "Chapter 1",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0x3a4252,
            )
        },
    ),
    preset(
        "card.step",
        "Step Card",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "card_pill",
                "Step 1",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0xff4d6d,
            )
        },
    ),
    preset(
        "card.tip_mint",
        "Pro Tip",
        CARDS,
        [0.0, -330.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Pro tip",
                "Poppins SemiBold",
                56.0,
                0x0e2b22,
                0x6ee7b7,
            )
        },
    ),
    preset(
        "card.pill_dark",
        "Dark Pill",
        CARDS,
        [0.0, 360.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Link in description",
                "Poppins",
                52.0,
                0xffffff,
                0x20232b,
            )
        },
    ),
    preset(
        "clean.title_white",
        "Clean White",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: -0.02,
            ..spec(
                "clean_soft",
                "Simply better.",
                "Poppins SemiBold",
                108.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.title_light",
        "Clean Light",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.01,
            ..spec(
                "clean_soft",
                "Less, but better.",
                "Montserrat Light",
                100.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.title_bold",
        "Clean Bold",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: -0.01,
            ..spec(
                "clean_soft",
                "Built for builders",
                "Outfit",
                112.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.title_wide",
        "Clean Wide",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.16,
            ..spec("clean_soft", "OUR STORY", "Sora", 88.0, 0xffffff, 0xffffff)
        },
    ),
    preset(
        "clean.title_warm",
        "Clean Warm",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "Honest review",
                "Poppins",
                96.0,
                0xf5e6c8,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.title_mono",
        "Clean Mono",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: -0.01,
            ..spec(
                "clean_soft",
                "v2.0 is here",
                "Space Grotesk",
                100.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.title_round",
        "Clean Round",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "Hello, world",
                "Bricolage Grotesque",
                100.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.grad_sky",
        "Gradient Sky",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: -0.02,
            ..spec(
                "clean_gradient",
                "Made to move.",
                "Poppins SemiBold",
                108.0,
                0xffffff,
                0x9ea9ff,
            )
        },
    ),
    preset(
        "clean.grad_peach",
        "Gradient Peach",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "Golden hour",
                "Outfit",
                116.0,
                0xfff1e0,
                0xff9f7a,
            )
        },
    ),
    preset(
        "clean.grad_mint",
        "Gradient Mint",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "Fresh start",
                "Montserrat",
                104.0,
                0xf2fff9,
                0x59e0b2,
            )
        },
    ),
    preset(
        "clean.grad_rose",
        "Gradient Rose",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "Date night",
                "Sora",
                100.0,
                0xfff0f6,
                0xff6fa5,
            )
        },
    ),
    preset(
        "clean.grad_gold",
        "Gradient Gold",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.08,
            ..spec(
                "clean_gradient",
                "OPEN HOUSE",
                "Montserrat",
                92.0,
                0xfff6dd,
                0xe6b84f,
            )
        },
    ),
    preset(
        "clean.glow_soon",
        "Glow Soon",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.14,
            ..spec(
                "clean_glow",
                "COMING SOON",
                "Poppins",
                92.0,
                0xffffff,
                0x80b3ff,
            )
        },
    ),
    preset(
        "clean.glow_new",
        "Glow New",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.12,
            ..spec(
                "clean_glow",
                "NEW COLLECTION",
                "Montserrat Light",
                84.0,
                0xffffff,
                0xffb3d1,
            )
        },
    ),
    preset(
        "clean.glow_open",
        "Glow Open",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.18,
            ..spec(
                "clean_glow",
                "NOW OPEN",
                "Outfit Light",
                96.0,
                0xffffff,
                0x9ff5d0,
            )
        },
    ),
    preset(
        "clean.glow_day",
        "Glow Day One",
        CLEAN,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "clean_glow",
                "DAY ONE",
                "Space Grotesk",
                100.0,
                0xffffff,
                0xffd18a,
            )
        },
    ),
    preset(
        "clean.sub_line",
        "Clean Subtitle",
        CLEAN,
        SUBTITLE_POS,
        Some(("fade", 0.3)),
        Some(("fade", 0.3)),
        3.0,
        || TitleSpec {
            ..spec(
                "subtitle",
                "Plain, readable captions",
                "Poppins SemiBold",
                44.0,
                0xffffff,
                0x000000,
            )
        },
    ),
    preset(
        "clean.sub_soft",
        "Soft Subtitle",
        CLEAN,
        SUBTITLE_POS,
        Some(("words_soft", 0.7)),
        Some(("fade", 0.3)),
        3.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "Soft captions that glide in",
                "Poppins",
                46.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "clean.quote_soft",
        "Quiet Quote",
        CLEAN,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        6.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "Stay curious.",
                "Gloock",
                112.0,
                0xf4efe6,
                0xffffff,
            )
        },
    ),
    preset(
        "gloss.sale",
        "Sale Pop",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "pop_3d",
                "SALE 50%",
                "Poppins Black",
                120.0,
                0xff6fb5,
                0x7a2bd1,
            )
        },
    ),
    preset(
        "gloss.sale_yellow",
        "Sale Yellow",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "pop_3d",
                "MEGA SALE",
                "Titan One",
                110.0,
                0xffe14a,
                0xff6a00,
            )
        },
    ),
    preset(
        "gloss.new_arrival",
        "New Arrival",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "pop_3d",
                "NEW ARRIVAL",
                "Lilita One",
                112.0,
                0x4af0d0,
                0x0b7bd1,
            )
        },
    ),
    preset(
        "gloss.level_up",
        "Level Up",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec("pop_3d", "LEVEL UP!", "Chango", 96.0, 0xa6ff5c, 0x1f8a3a)
        },
    ),
    preset(
        "gloss.winner",
        "Winner",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "pop_3d",
                "WINNER",
                "Archivo Black",
                120.0,
                0xffd24a,
                0xe8590c,
            )
        },
    ),
    preset(
        "gloss.big_news",
        "Big News",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "pop_3d",
                "BIG NEWS",
                "Passion One",
                130.0,
                0xff5a5a,
                0x8a1030,
            )
        },
    ),
    preset(
        "gloss.gold_vip",
        "Gold VIP",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "gold_bevel",
                "GO LIVE",
                "Poppins Black",
                110.0,
                0xffd24d,
                0x8a4b08,
            )
        },
    ),
    preset(
        "gloss.gold_gala",
        "Gold Gala",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_rise", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "gold_bevel",
                "GRAND GALA",
                "Cinzel",
                96.0,
                0xffd966,
                0x7a4a0a,
            )
        },
    ),
    preset(
        "gloss.gold_champ",
        "Gold Champion",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "gold_bevel",
                "CHAMPION",
                "Archivo Black",
                104.0,
                0xffe08a,
                0x7a4a0a,
            )
        },
    ),
    preset(
        "gloss.gold_year",
        "Gold New Year",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_rise", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "gold_bevel",
                "HAPPY 2027",
                "Montserrat",
                100.0,
                0xffd966,
                0x6b3d06,
            )
        },
    ),
    preset(
        "gloss.silver_launch",
        "Silver Launch",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "gold_bevel",
                "LAUNCH DAY",
                "Poppins Black",
                100.0,
                0xe8eef7,
                0x4a5568,
            )
        },
    ),
    preset(
        "gloss.rose_gold",
        "Rose Gold",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_rise", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "gold_bevel",
                "ROSE & GOLD",
                "Playball",
                110.0,
                0xffc9b8,
                0x8a4b4b,
            )
        },
    ),
    preset(
        "gloss.bubble",
        "Bubble Pop",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec("pop_3d", "HELLO!", "Bowlby One", 120.0, 0x8be9ff, 0x2c5be0)
        },
    ),
    preset(
        "gloss.candy",
        "Candy",
        GLOSSY,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "pop_3d",
                "SWEET DEAL",
                "Lilita One",
                112.0,
                0xffa6e0,
                0xc2185b,
            )
        },
    ),
    preset(
        "neon.new_drop",
        "Neon New Drop",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "neon_tube",
                "NEW DROP",
                "Poppins Black",
                104.0,
                0x66f2ff,
                0x1a8cff,
            )
        },
    ),
    preset(
        "neon.open",
        "Neon Open",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "neon_tube",
                "OPEN LATE",
                "Audiowide",
                96.0,
                0xff7ad9,
                0xd11fa8,
            )
        },
    ),
    preset(
        "neon.live",
        "Neon Live",
        NEON,
        [0.0, -330.0],
        Some(("letters_rise", 1.0)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "neon_tube",
                "LIVE",
                "Poppins Black",
                110.0,
                0xff5b6e,
                0xd11a33,
            )
        },
    ),
    preset(
        "neon.party",
        "Neon Party",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "neon_tube",
                "AFTER PARTY",
                "Krona One",
                80.0,
                0xb794ff,
                0x6a2cff,
            )
        },
    ),
    preset(
        "neon.green",
        "Neon Green",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "neon_tube",
                "PLAY LOUD",
                "Archivo Black",
                100.0,
                0x9dff6b,
                0x2fcf3c,
            )
        },
    ),
    preset(
        "neon.music",
        "Neon Music",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec("neon_tube", "DJ NIGHT", "Bungee", 110.0, 0x7af0ff, 0x2aa7ff)
        },
    ),
    preset(
        "neon.tour",
        "Neon Tour",
        NEON,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.08,
            ..spec(
                "neon_tube",
                "WORLD TOUR",
                "Michroma",
                72.0,
                0xffd36b,
                0xff8a1f,
            )
        },
    ),
    preset(
        "glass.summer",
        "Glass Summer",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "Summer in Lisbon",
                "Poppins SemiBold",
                96.0,
                0xd9ecff,
                0x80b3ff,
            )
        },
    ),
    preset(
        "glass.travel",
        "Glass Travel",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "Wander more",
                "Outfit",
                112.0,
                0xe9f6ff,
                0x9ad0ff,
            )
        },
    ),
    preset(
        "glass.vlog",
        "Glass Vlog",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "A day with me",
                "Sora",
                92.0,
                0xf1ecff,
                0xb9a6ff,
            )
        },
    ),
    preset(
        "glass.food",
        "Glass Food",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "Fresh & tasty",
                "Poppins SemiBold",
                96.0,
                0xfff1d9,
                0xffc878,
            )
        },
    ),
    preset(
        "glass.fit",
        "Glass Fitness",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "Stronger today",
                "Montserrat",
                92.0,
                0xe0fff2,
                0x7af0b8,
            )
        },
    ),
    preset(
        "glass.home",
        "Glass Home Tour",
        NEON,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "glass_soft",
                "Home tour",
                "Space Grotesk",
                112.0,
                0xecf2ff,
                0x9bb8ff,
            )
        },
    ),
    preset(
        "script.birthday",
        "Birthday Script",
        SCRIPT,
        [0.0, 0.0],
        Some(("letters_rise", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.01,
            ..spec(
                "gold",
                "Happy Birthday",
                "Playball",
                140.0,
                0xffd98a,
                0xb3761b,
            )
        },
    ),
    preset(
        "script.save_date",
        "Save The Date",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "plain",
                "Save the Date",
                "Allura",
                150.0,
                0xfff4e6,
                0xe6c389,
            )
        },
    ),
    preset(
        "script.wedding",
        "Wedding Day",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "plain",
                "Our Wedding Day",
                "Parisienne",
                130.0,
                0xffffff,
                0xf2c4d4,
            )
        },
    ),
    preset(
        "script.mr_mrs",
        "Mr & Mrs",
        SCRIPT,
        [0.0, 0.0],
        Some(("letters_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec("plain", "Mr & Mrs", "Sacramento", 170.0, 0xfff6ee, 0xf2c4d4)
        },
    ),
    preset(
        "script.thanks",
        "Thank You",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec("plain", "Thank you", "Norican", 150.0, 0xffffff, 0xffffff)
        },
    ),
    preset(
        "script.bon_appetit",
        "Bon Appetit",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            shadow: 40.0,
            ..spec("plain", "Bon Appetit", "Damion", 150.0, 0xffe9c9, 0xf0a35e)
        },
    ),
    preset(
        "script.merry",
        "Merry and Bright",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec(
                "gold",
                "Merry & Bright",
                "Kaushan Script",
                130.0,
                0xffe08a,
                0xb3361b,
            )
        },
    ),
    preset(
        "script.engraved",
        "Engraved",
        SCRIPT,
        [0.0, 0.0],
        Some(("letters_rise", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "double_line",
                "THE GRAND HOTEL",
                "Cinzel",
                84.0,
                0xf4ead2,
                0xc8a45a,
            )
        },
    ),
    preset(
        "script.editorial",
        "Editorial Serif",
        SCRIPT,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec(
                "editorial",
                "The Quiet Hours\nA film in four parts",
                "Gloock",
                110.0,
                0xf3eee4,
                0xc9b488,
            )
        },
    ),
    preset(
        "script.noir",
        "Noir",
        SCRIPT,
        [0.0, 0.0],
        Some(("letters_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.06,
            ..spec("plain", "MIDNIGHT", "Limelight", 120.0, 0xf5f1e8, 0xf5f1e8)
        },
    ),
    preset(
        "script.handwritten",
        "Handwritten",
        SCRIPT,
        [0.0, 0.0],
        Some(("typewriter", 1.6)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "plain",
                "Dear diary...",
                "Caveat",
                130.0,
                0xfff7ea,
                0xffd9a8,
            )
        },
    ),
    preset(
        "script.brush",
        "Brush Script",
        SCRIPT,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "plain",
                "Adventure awaits",
                "Nanum Brush Script",
                150.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "tech.game_over",
        "Game Over",
        TECH,
        [0.0, 0.0],
        Some(("letters_pop", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "glitch",
                "GAME OVER",
                "Press Start 2P",
                72.0,
                0xff4d6d,
                0x2fe0ff,
            )
        },
    ),
    preset(
        "tech.level_up",
        "Level Up Arcade",
        TECH,
        [0.0, 0.0],
        Some(("letters_pop", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec("retro_3d", "LEVEL UP", "Bungee", 110.0, 0xffe14a, 0xe8590c)
        },
    ),
    preset(
        "tech.player_one",
        "Player One",
        TECH,
        [0.0, 0.0],
        Some(("letters_rise", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec("neon", "PLAYER 1", "Audiowide", 100.0, 0x7af0ff, 0xff4fd8)
        },
    ),
    preset(
        "tech.victory",
        "Victory",
        TECH,
        [0.0, 0.0],
        Some(("slam", 0.7)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "retro_3d",
                "VICTORY",
                "Black Ops One",
                130.0,
                0xffd24a,
                0xb51f1f,
            )
        },
    ),
    preset(
        "tech.high_score",
        "High Score",
        TECH,
        [0.0, 0.0],
        Some(("letters_pop", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "glitch",
                "NEW HIGH SCORE",
                "Chakra Petch Bold",
                80.0,
                0x9dff6b,
                0xff4fd8,
            )
        },
    ),
    preset(
        "tech.glitch_title",
        "Glitch Title",
        TECH,
        [0.0, 0.0],
        Some(("flicker", 0.7)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "glitch",
                "SYSTEM ONLINE",
                "Chakra Petch Bold",
                84.0,
                0xffffff,
                0xff3d71,
            )
        },
    ),
    preset(
        "tech.setup_tour",
        "Setup Tour",
        TECH,
        [0.0, 0.0],
        Some(("letters_rise", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "neon_tube",
                "SETUP TOUR",
                "Quantico Bold",
                100.0,
                0x8ff0ff,
                0x2f7bff,
            )
        },
    ),
    preset(
        "tech.gear_review",
        "Gear Review",
        TECH,
        [0.0, 0.0],
        Some(("letters_rise", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "clean_glow",
                "GEAR REVIEW",
                "Rajdhani Bold",
                110.0,
                0xffffff,
                0x66e0ff,
            )
        },
    ),
    preset(
        "tech.stream_live",
        "Stream Live",
        TECH,
        [0.0, -330.0],
        Some(("slide_down", 0.5)),
        Some(("slide_up", 0.4)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.06,
            ..spec(
                "box",
                "LIVE NOW",
                "Chakra Petch Bold",
                64.0,
                0xffffff,
                0xef3b4a,
            )
        },
    ),
    preset(
        "tech.speedrun",
        "Speedrun",
        TECH,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec("split", "SPEEDRUN", "Faster One", 100.0, 0xffe14a, 0xff4d00)
        },
    ),
    preset(
        "tech.boss_fight",
        "Boss Fight",
        TECH,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "stroke_only",
                "BOSS FIGHT",
                "Bungee Inline",
                110.0,
                0xffffff,
                0xff3b3b,
            )
        },
    ),
    preset(
        "tech.terminal",
        "Terminal",
        TECH,
        [0.0, 0.0],
        Some(("typewriter", 1.4)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "plain",
                "> npm run build_",
                "JetBrains Mono",
                72.0,
                0x7dff9b,
                0x7dff9b,
            )
        },
    ),
    preset(
        "tech.code_lesson",
        "Code Lesson",
        TECH,
        [0.0, 0.0],
        Some(("typewriter", 1.4)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            ..spec(
                "plain",
                "Lesson 1: Variables",
                "Space Mono Bold",
                72.0,
                0xe6edf3,
                0x7dff9b,
            )
        },
    ),
    preset(
        "tech.retro_wave",
        "Retro Wave",
        TECH,
        [0.0, 0.0],
        Some(("letters_rise", 1.1)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec("neon", "OUTRUN", "Stalinist One", 80.0, 0xff7ad9, 0x2fe0ff)
        },
    ),
    preset(
        "sport.workout",
        "Workout",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "long_shadow",
                "WORKOUT",
                "Big Shoulders Display",
                170.0,
                0xffffff,
                0xff4d1a,
            )
        },
    ),
    preset(
        "sport.no_pain",
        "No Pain No Gain",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "stack",
                "NO PAIN\nNO GAIN",
                "Staatliches",
                130.0,
                0xffffff,
                0xffcf1a,
            )
        },
    ),
    preset(
        "sport.day_one",
        "Day One",
        SPORT,
        [0.0, 0.0],
        Some(("letters_pop", 0.8)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "long_shadow",
                "DAY 1",
                "Big Shoulders Display",
                260.0,
                0xffffff,
                0x2f80ff,
            )
        },
    ),
    preset(
        "sport.round_two",
        "Round Two",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec("box", "ROUND 2", "Oswald", 120.0, 0x141414, 0xffd60a)
        },
    ),
    preset(
        "sport.final_score",
        "Final Score",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "split",
                "FINAL SCORE",
                "Bebas Neue",
                160.0,
                0xffffff,
                0xff3b3b,
            )
        },
    ),
    preset(
        "sport.match_day",
        "Match Day",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec("stack", "MATCH\nDAY", "Anton", 150.0, 0xffffff, 0x1fc26b)
        },
    ),
    preset(
        "sport.run_club",
        "Run Club",
        SPORT,
        [0.0, 0.0],
        Some(("slide_left", 0.6)),
        Some(("slide_right", 0.5)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "long_shadow",
                "RUN CLUB",
                "Londrina Solid",
                170.0,
                0xffffff,
                0xff6a2a,
            )
        },
    ),
    preset(
        "sport.leg_day",
        "Leg Day",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            shadow: 60.0,
            ..spec(
                "plain",
                "LEG DAY",
                "Big Shoulders Display",
                200.0,
                0xffffff,
                0xff4d1a,
            )
        },
    ),
    preset(
        "sport.sunday",
        "Sunday Football",
        SPORT,
        [0.0, 0.0],
        Some(("letters_rise", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "underline",
                "SUNDAY LEAGUE",
                "Teko",
                150.0,
                0xffffff,
                0x2fd17f,
            )
        },
    ),
    preset(
        "sport.reps",
        "Reps and Sets",
        SPORT,
        [0.0, 0.0],
        Some(("words_pop", 0.8)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.03,
            ..spec("tag", "3 SETS x 12", "Oswald", 110.0, 0xffffff, 0xffd60a)
        },
    ),
    preset(
        "sport.pr",
        "Personal Record",
        SPORT,
        [0.0, 0.0],
        Some(("slam", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "retro_3d",
                "NEW PR!",
                "Titan One",
                140.0,
                0xffd24a,
                0xd1330d,
            )
        },
    ),
    preset(
        "sport.cardio",
        "Cardio",
        SPORT,
        [0.0, 0.0],
        Some(("slide_up", 0.6)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.03,
            ..spec(
                "highlight",
                "CARDIO BLAST",
                "Staatliches",
                130.0,
                0xffffff,
                0xff3b6b,
            )
        },
    ),
    preset(
        "biz.quarter",
        "Quarterly Results",
        BIZ,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "Q3 results",
                "Montserrat",
                112.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "biz.agenda",
        "Agenda",
        BIZ,
        LOWER_LEFT,
        Some(("slide_left", 0.6)),
        Some(("slide_left", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "lower_bar",
                "Agenda\nThree things today",
                "Poppins SemiBold",
                72.0,
                0xffffff,
                0x2f80ff,
            )
        },
    ),
    preset(
        "biz.speaker",
        "Speaker",
        BIZ,
        LOWER_LEFT,
        Some(("slide_left", 0.6)),
        Some(("slide_left", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "lower_slant",
                "DR. AVA CHEN\nKeynote speaker",
                "Poppins SemiBold",
                68.0,
                0xffffff,
                0x7b4dff,
            )
        },
    ),
    preset(
        "biz.webinar",
        "Webinar",
        BIZ,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "Live webinar",
                "Outfit",
                112.0,
                0xffffff,
                0x7da8ff,
            )
        },
    ),
    preset(
        "biz.pitch",
        "Pitch Deck",
        BIZ,
        [0.0, 0.0],
        Some(("words_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_soft",
                "The big idea",
                "Sora",
                108.0,
                0xffffff,
                0xffffff,
            )
        },
    ),
    preset(
        "biz.growth",
        "Growth",
        BIZ,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "+248% growth",
                "Poppins SemiBold",
                112.0,
                0xf2fff9,
                0x59e0b2,
            )
        },
    ),
    preset(
        "biz.open_house",
        "Open House",
        BIZ,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "double_line",
                "OPEN HOUSE",
                "Cinzel",
                96.0,
                0xffffff,
                0xd8b25a,
            )
        },
    ),
    preset(
        "biz.just_listed",
        "Just Listed",
        BIZ,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.08,
            ..spec(
                "clean_gradient",
                "JUST LISTED",
                "Montserrat",
                96.0,
                0xffffff,
                0xe6b84f,
            )
        },
    ),
    preset(
        "biz.sold",
        "Sold",
        BIZ,
        [0.0, 0.0],
        Some(("letters_pop", 0.9)),
        Some(("fade", 0.4)),
        4.0,
        || TitleSpec {
            letter_spacing: 0.06,
            ..spec("box", "SOLD", "Archivo Black", 140.0, 0xffffff, 0xe5333f)
        },
    ),
    preset(
        "biz.venue",
        "Venue",
        BIZ,
        [-420.0, 340.0],
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            ..spec(
                "card_bar",
                "The Glass House, Downtown",
                "Poppins",
                52.0,
                0xffffff,
                0x3a4252,
            )
        },
    ),
    preset(
        "event.invite",
        "You Are Invited",
        BIZ,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec(
                "refined",
                "You are invited",
                "DM Serif Display",
                100.0,
                0xfff4e6,
                0xe6c389,
            )
        },
    ),
    preset(
        "event.festival",
        "Festival",
        BIZ,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "retro_3d",
                "SUMMER FEST",
                "Bungee",
                100.0,
                0xffe14a,
                0xff4d4d,
            )
        },
    ),
    preset(
        "event.doors",
        "Doors Open",
        BIZ,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "clean_glow",
                "DOORS OPEN 8 PM",
                "Poppins",
                72.0,
                0xffffff,
                0xffd18a,
            )
        },
    ),
    preset(
        "event.countdown",
        "Countdown Label",
        BIZ,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.6)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.2,
            ..spec(
                "clean_glow",
                "STARTS IN",
                "Montserrat Light",
                80.0,
                0xffffff,
                0x9ad0ff,
            )
        },
    ),
    preset(
        "captions.pop_yellow",
        "Caption Pop Yellow",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            outline: 9.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "THIS CHANGES EVERYTHING",
                "Anton",
                96.0,
                0xffffff,
                0xffe033,
            )
        },
    ),
    preset(
        "captions.pop_green",
        "Caption Pop Green",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "WATCH THIS UNTIL THE END",
                "Archivo Black",
                80.0,
                0xffffff,
                0x3dff7a,
            )
        },
    ),
    preset(
        "captions.pop_pink",
        "Caption Pop Pink",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.01,
            outline: 9.0,
            outline_color: hex(0x2a0a3d),
            shadow: 45.0,
            ..spec(
                "plain",
                "YOU WON'T BELIEVE THIS",
                "Lilita One",
                100.0,
                0xffffff,
                0xff5fb3,
            )
        },
    ),
    preset(
        "captions.pop_cyan",
        "Caption Pop Cyan",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "LET'S GET STRONGER TODAY",
                "Bebas Neue",
                130.0,
                0xffffff,
                0x3de3ff,
            )
        },
    ),
    preset(
        "captions.pop_orange",
        "Caption Pop Orange",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x3a1500),
            shadow: 45.0,
            ..spec(
                "plain",
                "TODAY WE ARE COOKING PASTA",
                "Titan One",
                82.0,
                0xffffff,
                0xff8a1f,
            )
        },
    ),
    preset(
        "captions.pop_red",
        "Caption Pop Red",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "NEW RECORD IN ROUND TWO",
                "Staatliches",
                120.0,
                0xffffff,
                0xff3b3b,
            )
        },
    ),
    preset(
        "captions.bold_white",
        "Caption Bold White",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "FIVE TIPS FOR BETTER SLEEP",
                "Montserrat",
                78.0,
                0xffffff,
                0xffd23f,
            )
        },
    ),
    preset(
        "captions.bold_mint",
        "Caption Bold Mint",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "WELCOME TO THE TOUR",
                "Outfit",
                100.0,
                0xffffff,
                0x6ee7b7,
            )
        },
    ),
    preset(
        "captions.bold_violet",
        "Caption Bold Violet",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "SUBSCRIBE FOR MORE",
                "Sora",
                84.0,
                0xffffff,
                0xb794ff,
            )
        },
    ),
    preset(
        "captions.bold_blue",
        "Caption Bold Blue",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 40.0,
            ..spec(
                "plain",
                "LIVE FROM THE STUDIO",
                "Bricolage Grotesque",
                96.0,
                0xffffff,
                0x5ec1ff,
            )
        },
    ),
    preset(
        "captions.sport_gold",
        "Caption Sport Gold",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            outline: 8.0,
            outline_color: hex(0x000000),
            shadow: 45.0,
            ..spec(
                "plain",
                "NO PAIN NO GAIN",
                "Big Shoulders Display",
                150.0,
                0xffffff,
                0xffcf1a,
            )
        },
    ),
    preset(
        "captions.news_slab",
        "Caption News Slab",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "subtitle",
                "THE FULL STORY IN ONE MINUTE",
                "Oswald",
                88.0,
                0xffffff,
                0xffd23f,
            )
        },
    ),
    preset(
        "captions.box_black",
        "Caption Box Black",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.4)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            ..spec(
                "subtitle",
                "HOW TO MAKE IT AT HOME",
                "Poppins Black",
                72.0,
                0xffffff,
                0x3dff7a,
            )
        },
    ),
    preset(
        "captions.soft_light",
        "Caption Soft Light",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.2)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "plain",
                "Slow down and enjoy the view",
                "Montserrat Light",
                72.0,
                0xffffff,
                0xffd18a,
            )
        },
    ),
    preset(
        "captions.soft_serif",
        "Caption Soft Serif",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.2)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            shadow: 35.0,
            ..spec(
                "plain",
                "Where the road ends, we begin",
                "Gloock",
                72.0,
                0xf5efe3,
                0xe6c389,
            )
        },
    ),
    preset(
        "captions.mono_code",
        "Caption Mono Code",
        CAPTIONS,
        [0.0, 330.0],
        Some(("karaoke", 2.2)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            shadow: 30.0,
            ..spec(
                "plain",
                "const answer = 42;",
                "JetBrains Mono",
                72.0,
                0xe6edf3,
                0x7dff9b,
            )
        },
    ),
    preset(
        "endcredits.thanks",
        "Thanks For Watching",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            ..spec(
                "editorial",
                "Thanks for watching\nSee you in the next one",
                "Gloock",
                104.0,
                0xf3eee4,
                0xc9b488,
            )
        },
    ),
    preset(
        "endcredits.thanks_clean",
        "Thanks Clean",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "Thanks for watching",
                "Outfit",
                112.0,
                0xffffff,
                0x9ea9ff,
            )
        },
    ),
    preset(
        "endcredits.the_end",
        "The End",
        CREDITS,
        [0.0, 0.0],
        Some(("letters_soft", 1.4)),
        Some(("fade", 0.8)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.2,
            ..spec(
                "clean_glow",
                "THE END",
                "Montserrat Light",
                100.0,
                0xffffff,
                0x9ad0ff,
            )
        },
    ),
    preset(
        "endcredits.see_you",
        "See You Soon",
        CREDITS,
        [0.0, 0.0],
        Some(("letters_soft", 1.2)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            ..spec(
                "clean_gradient",
                "See you soon!",
                "Poppins SemiBold",
                112.0,
                0xfff1e0,
                0xff9f7a,
            )
        },
    ),
    preset(
        "endcredits.directed",
        "Directed By",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "refined",
                "DIRECTED BY\nAlex Morgan",
                "Poppins",
                60.0,
                0xf4ead2,
                0xc8a45a,
            )
        },
    ),
    preset(
        "endcredits.music",
        "Music By",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "refined",
                "MUSIC BY\nYour Name",
                "Poppins",
                60.0,
                0xffffff,
                0x9ad0ff,
            )
        },
    ),
    preset(
        "endcredits.cast",
        "Starring",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            ..spec(
                "editorial",
                "Starring\nSam, Jordan and Taylor",
                "DM Serif Display",
                96.0,
                0xffffff,
                0xe6c389,
            )
        },
    ),
    preset(
        "endcredits.sponsor",
        "Sponsored By",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "refined",
                "SPONSORED BY\nYour Brand",
                "Montserrat",
                60.0,
                0xffffff,
                0xffd18a,
            )
        },
    ),
    preset(
        "endcredits.filmed_in",
        "Filmed In",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            ..spec(
                "editorial",
                "Filmed in\nLisbon, Portugal",
                "Playball",
                110.0,
                0xfff4e6,
                0xe6c389,
            )
        },
    ),
    preset(
        "endcredits.special",
        "Special Thanks",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            ..spec(
                "editorial",
                "Special thanks\nto everyone who helped",
                "Gloock",
                92.0,
                0xf3eee4,
                0xc9b488,
            )
        },
    ),
    preset(
        "endcredits.continued",
        "To Be Continued",
        CREDITS,
        [0.0, 0.0],
        Some(("letters_rise", 1.2)),
        Some(("fade", 0.7)),
        6.0,
        || TitleSpec {
            shadow: 40.0,
            ..spec(
                "plain",
                "To be continued...",
                "Playball",
                130.0,
                0xffe9c9,
                0xf0a35e,
            )
        },
    ),
    preset(
        "endcredits.follow",
        "Follow For More",
        CREDITS,
        [0.0, 360.0],
        Some(("letters_soft", 1.3)),
        Some(("fade", 0.4)),
        6.0,
        || TitleSpec {
            ..spec(
                "card_pill",
                "Follow for more",
                "Poppins SemiBold",
                60.0,
                0xffffff,
                0x7b4dff,
            )
        },
    ),
    preset(
        "endcredits.watch_next",
        "Watch Next",
        CREDITS,
        LOWER_LEFT,
        Some(("letters_soft", 1.5)),
        Some(("fade", 0.5)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "card_bar",
                "WATCH NEXT",
                "Poppins SemiBold",
                56.0,
                0xffffff,
                0xeb2f35,
            )
        },
    ),
    preset(
        "endcredits.credits_roll",
        "Credits Line",
        CREDITS,
        [0.0, 0.0],
        Some(("words_soft", 1.4)),
        Some(("fade", 0.8)),
        7.0,
        || TitleSpec {
            letter_spacing: 0.1,
            ..spec(
                "double_line",
                "CAST AND CREW",
                "Cinzel",
                84.0,
                0xf4ead2,
                0xc8a45a,
            )
        },
    ),
    preset(
        "endcredits.game_over",
        "Game Over Credits",
        CREDITS,
        [0.0, 0.0],
        Some(("letters_pop", 1.0)),
        Some(("fade", 0.6)),
        6.0,
        || TitleSpec {
            letter_spacing: 0.04,
            ..spec(
                "glitch",
                "THANKS FOR PLAYING",
                "Chakra Petch Bold",
                72.0,
                0xffffff,
                0x2fe0ff,
            )
        },
    ),
    preset(
        "endcredits.sport",
        "Full Time",
        CREDITS,
        [0.0, 0.0],
        Some(("slam", 0.7)),
        Some(("fade", 0.5)),
        5.0,
        || TitleSpec {
            letter_spacing: 0.02,
            ..spec(
                "long_shadow",
                "FULL TIME",
                "Big Shoulders Display",
                180.0,
                0xffffff,
                0xff4d1a,
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
            [
                BASIC, MODERN, CAPTIONS, LOWER, CALLOUTS, OPENERS, CREDITS, SUBTITLES, SOCIAL,
                CARDS, CLEAN, GLOSSY, NEON, SCRIPT, TECH, SPORT, BIZ,
            ]
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
