//! Vector stickers drawn with tiny-skia at whatever size they're shown.
//!
//! Every drawing works in units relative to the sticker height (see
//! `kit`), uses `color` as its main color and `color2` as the secondary
//! one so users can recolor it, and leaves a transparent margin for its
//! soft shadow.

mod arrows;
mod badges;
mod bubbles;
mod callouts;
mod emoji;
mod kit;
mod lettering;
mod lines;
mod nature;
mod shapes;
mod social;
mod sparkles;
mod symbols;

use crate::RgbaImage;
use tidycuts_core::StickerSpec;
use tiny_skia::Pixmap;

/// What a sticker drawing gets.
pub struct DrawCtx {
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub color2: [f32; 4],
    /// Seconds, for animated stickers.
    pub t: f32,
}

#[derive(Clone, Copy)]
pub struct StickerDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub color: [f32; 4],
    pub color2: [f32; 4],
    /// Default looping motion (see `animations::sticker_motion`).
    pub motion: &'static str,
    /// Width / height.
    pub aspect: f32,
    /// Redrawn every frame (particles, sparkles…) rather than once.
    pub animated: bool,
    pub draw: fn(&mut Pixmap, &DrawCtx),
}

/// Natural sticker height in project pixels for a 1080p frame.
pub const STICKER_HEIGHT: f32 = 320.0;

const W: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const INK: [f32; 4] = [0.12, 0.12, 0.16, 1.0];

const fn c(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}

#[allow(clippy::too_many_arguments)]
const fn s(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    color: [f32; 4],
    color2: [f32; 4],
    motion: &'static str,
    aspect: f32,
    draw: fn(&mut Pixmap, &DrawCtx),
) -> StickerDef {
    StickerDef {
        id,
        name,
        category,
        color,
        color2,
        motion,
        aspect,
        animated: false,
        draw,
    }
}

const fn animated(mut d: StickerDef) -> StickerDef {
    d.animated = true;
    d
}

const ARROWS: &str = "Arrows";
const SHAPES: &str = "Shapes";
const EMOJI: &str = "Emoji";
const SOCIAL: &str = "Social Media";
const BUBBLES: &str = "Speech Bubbles";
const LINES: &str = "Lines";
const BADGES: &str = "Badges";
const SPARKLES: &str = "Sparkles";
const SYMBOLS: &str = "Symbols";
const NATURE: &str = "Nature";

pub static STICKERS: &[StickerDef] = &[
    // Arrows
    s(
        "arrow.straight",
        "Straight Arrow",
        ARROWS,
        c(1.0, 0.33, 0.27),
        W,
        "bounce",
        2.0,
        arrows::straight,
    ),
    s(
        "arrow.curved",
        "Curved Arrow",
        ARROWS,
        c(1.0, 0.76, 0.1),
        W,
        "bounce",
        1.35,
        arrows::curved,
    ),
    s(
        "arrow.hand_drawn",
        "Hand-drawn Arrow",
        ARROWS,
        W,
        INK,
        "bounce",
        1.8,
        arrows::hand_drawn,
    ),
    s(
        "arrow.circular",
        "Circular Arrow",
        ARROWS,
        c(0.2, 0.55, 1.0),
        W,
        "spin",
        1.0,
        arrows::circular,
    ),
    s(
        "arrow.pointing_hand",
        "Pointing Hand",
        ARROWS,
        c(1.0, 0.8, 0.3),
        c(0.96, 0.97, 1.0),
        "wiggle",
        1.3,
        arrows::pointing_hand,
    ),
    animated(s(
        "arrow.chevrons",
        "Chevrons",
        ARROWS,
        c(0.2, 0.86, 0.5),
        W,
        "none",
        1.6,
        arrows::chevrons,
    )),
    s(
        "arrow.loop",
        "Loop Arrow",
        ARROWS,
        c(1.0, 0.32, 0.6),
        W,
        "float",
        1.7,
        arrows::doodle_loop,
    ),
    // Shapes
    s(
        "shape.circle",
        "Circle",
        SHAPES,
        c(0.15, 0.72, 0.98),
        W,
        "pulse",
        1.0,
        shapes::circle_disc,
    ),
    s(
        "shape.ring",
        "Ring",
        SHAPES,
        c(1.0, 0.25, 0.3),
        W,
        "pulse",
        1.0,
        shapes::ring,
    ),
    s(
        "shape.star",
        "Star",
        SHAPES,
        c(1.0, 0.8, 0.15),
        W,
        "pulse",
        1.0,
        shapes::star_shape,
    ),
    s(
        "shape.heart",
        "Heart",
        SHAPES,
        c(1.0, 0.25, 0.42),
        W,
        "heartbeat",
        1.1,
        shapes::heart_shape,
    ),
    s(
        "shape.burst",
        "Burst",
        SHAPES,
        c(1.0, 0.45, 0.1),
        c(1.0, 0.85, 0.25),
        "spin",
        1.0,
        shapes::burst,
    ),
    s(
        "shape.blob",
        "Blob",
        SHAPES,
        c(0.55, 0.32, 1.0),
        c(1.0, 0.38, 0.65),
        "float",
        1.15,
        shapes::blob,
    ),
    s(
        "shape.hexagon",
        "Hexagon",
        SHAPES,
        c(0.2, 0.82, 0.55),
        W,
        "none",
        1.0,
        shapes::hexagon,
    ),
    s(
        "shape.triangle",
        "Triangle",
        SHAPES,
        c(0.3, 0.72, 1.0),
        c(1.0, 0.4, 0.7),
        "none",
        1.1,
        shapes::triangle,
    ),
    // Emoji
    s(
        "emoji.smile",
        "Smile",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(1.0, 0.45, 0.5),
        "bounce",
        1.0,
        emoji::smile,
    ),
    s(
        "emoji.laugh",
        "Tears of Joy",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(0.4, 0.75, 1.0),
        "wiggle",
        1.0,
        emoji::laugh,
    ),
    s(
        "emoji.heart_eyes",
        "Heart Eyes",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(1.0, 0.2, 0.35),
        "pulse",
        1.0,
        emoji::heart_eyes,
    ),
    s(
        "emoji.wow",
        "Wow",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(1.0, 0.42, 0.48),
        "shake",
        1.0,
        emoji::wow,
    ),
    s(
        "emoji.sad",
        "Sad",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(0.4, 0.72, 1.0),
        "float",
        1.0,
        emoji::sad,
    ),
    s(
        "emoji.cool",
        "Cool",
        EMOJI,
        c(1.0, 0.8, 0.2),
        c(0.15, 0.15, 0.24),
        "bounce",
        1.0,
        emoji::cool,
    ),
    s(
        "emoji.thumbs_up",
        "Thumbs Up",
        EMOJI,
        c(1.0, 0.8, 0.28),
        c(0.25, 0.55, 1.0),
        "bounce",
        0.95,
        emoji::thumbs_up,
    ),
    animated(s(
        "emoji.fire",
        "Fire",
        EMOJI,
        c(1.0, 0.42, 0.1),
        c(1.0, 0.85, 0.2),
        "none",
        0.8,
        emoji::fire,
    )),
    s(
        "emoji.hundred",
        "100",
        EMOJI,
        c(0.95, 0.15, 0.2),
        W,
        "pulse",
        1.3,
        emoji::hundred,
    ),
    s(
        "emoji.party",
        "Party Popper",
        EMOJI,
        c(1.0, 0.75, 0.2),
        c(0.95, 0.25, 0.6),
        "wiggle",
        1.0,
        emoji::party,
    ),
    animated(s(
        "emoji.eyes",
        "Eyes",
        EMOJI,
        W,
        c(0.2, 0.13, 0.1),
        "none",
        1.3,
        emoji::eyes,
    )),
    s(
        "emoji.wink",
        "Wink",
        EMOJI,
        c(1.0, 0.82, 0.24),
        c(1.0, 0.5, 0.5),
        "wiggle",
        1.0,
        emoji::wink,
    ),
    s(
        "emoji.angry",
        "Angry",
        EMOJI,
        c(0.96, 0.36, 0.26),
        c(0.92, 0.94, 0.98),
        "shake",
        1.0,
        emoji::angry,
    ),
    s(
        "emoji.sob",
        "Crying",
        EMOJI,
        c(1.0, 0.82, 0.24),
        c(0.3, 0.65, 1.0),
        "bounce",
        1.0,
        emoji::sob,
    ),
    s(
        "emoji.thinking",
        "Thinking",
        EMOJI,
        c(1.0, 0.82, 0.24),
        c(0.85, 0.9, 1.0),
        "float",
        1.0,
        emoji::thinking,
    ),
    animated(s(
        "emoji.sleepy",
        "Sleepy",
        EMOJI,
        c(1.0, 0.82, 0.24),
        c(0.4, 0.6, 1.0),
        "float",
        1.0,
        emoji::sleepy,
    )),
    // Symbols
    s(
        "symbol.check",
        "Check Mark",
        SYMBOLS,
        c(0.18, 0.78, 0.38),
        W,
        "pulse",
        1.0,
        symbols::check,
    ),
    s(
        "symbol.cross",
        "Cross Mark",
        SYMBOLS,
        c(0.96, 0.26, 0.28),
        W,
        "shake",
        1.0,
        symbols::cross,
    ),
    s(
        "symbol.question",
        "Question Mark",
        SYMBOLS,
        c(0.35, 0.55, 1.0),
        W,
        "wiggle",
        0.8,
        symbols::question,
    ),
    s(
        "symbol.exclamation",
        "Exclamation Mark",
        SYMBOLS,
        c(1.0, 0.78, 0.1),
        W,
        "shake",
        0.6,
        symbols::exclamation,
    ),
    s(
        "symbol.bulb",
        "Light Bulb",
        SYMBOLS,
        c(1.0, 0.86, 0.25),
        c(0.62, 0.64, 0.7),
        "pulse",
        1.0,
        symbols::bulb,
    ),
    s(
        "symbol.bolt",
        "Lightning Bolt",
        SYMBOLS,
        c(1.0, 0.82, 0.12),
        W,
        "shake",
        1.0,
        symbols::bolt,
    ),
    s(
        "symbol.note",
        "Music Note",
        SYMBOLS,
        c(0.55, 0.4, 1.0),
        W,
        "bounce",
        1.0,
        symbols::music_note,
    ),
    s(
        "symbol.crown",
        "Crown",
        SYMBOLS,
        c(1.0, 0.76, 0.15),
        c(0.9, 0.2, 0.4),
        "float",
        1.0,
        symbols::crown,
    ),
    s(
        "symbol.trophy",
        "Trophy",
        SYMBOLS,
        c(1.0, 0.76, 0.15),
        c(0.5, 0.32, 0.18),
        "bounce",
        1.0,
        symbols::trophy,
    ),
    s(
        "symbol.gift",
        "Gift Box",
        SYMBOLS,
        c(0.92, 0.2, 0.3),
        c(1.0, 0.84, 0.25),
        "wiggle",
        1.0,
        symbols::gift,
    ),
    animated(s(
        "symbol.rocket",
        "Rocket",
        SYMBOLS,
        c(0.9, 0.92, 0.97),
        c(0.95, 0.28, 0.22),
        "float",
        1.0,
        symbols::rocket,
    )),
    s(
        "symbol.pin",
        "Map Pin",
        SYMBOLS,
        c(0.95, 0.25, 0.3),
        W,
        "bounce",
        1.0,
        symbols::pin,
    ),
    s(
        "symbol.play",
        "Play Button",
        SYMBOLS,
        c(0.2, 0.55, 1.0),
        W,
        "pulse",
        1.0,
        symbols::play,
    ),
    s(
        "symbol.camera",
        "Camera",
        SYMBOLS,
        c(0.25, 0.27, 0.33),
        c(0.7, 0.74, 0.8),
        "none",
        1.2,
        symbols::camera,
    ),
    s(
        "symbol.microphone",
        "Microphone",
        SYMBOLS,
        c(0.6, 0.64, 0.72),
        c(0.25, 0.27, 0.33),
        "none",
        0.8,
        symbols::microphone,
    ),
    s(
        "symbol.clapper",
        "Clapperboard",
        SYMBOLS,
        c(0.17, 0.18, 0.22),
        W,
        "wiggle",
        1.2,
        symbols::clapper,
    ),
    // Social media
    s(
        "social.like",
        "Like Button",
        SOCIAL,
        c(1.0, 0.27, 0.45),
        W,
        "pulse",
        2.2,
        social::like,
    ),
    s(
        "social.subscribe",
        "Subscribe",
        SOCIAL,
        c(0.93, 0.15, 0.2),
        W,
        "pulse",
        3.2,
        social::subscribe,
    ),
    s(
        "social.bell",
        "Bell",
        SOCIAL,
        c(1.0, 0.78, 0.15),
        c(1.0, 0.22, 0.3),
        "swing",
        1.0,
        social::bell_alert,
    ),
    s(
        "social.share",
        "Share",
        SOCIAL,
        c(0.2, 0.6, 1.0),
        W,
        "bounce",
        1.0,
        social::share,
    ),
    s(
        "social.comment",
        "Comment",
        SOCIAL,
        c(0.45, 0.42, 1.0),
        c(1.0, 0.25, 0.35),
        "bounce",
        1.18,
        social::comment,
    ),
    s(
        "social.follow",
        "Follow",
        SOCIAL,
        c(0.15, 0.55, 1.0),
        W,
        "pulse",
        2.6,
        social::follow,
    ),
    animated(s(
        "social.live",
        "Live",
        SOCIAL,
        c(0.95, 0.15, 0.25),
        W,
        "none",
        1.8,
        social::live,
    )),
    animated(s(
        "social.cursor_click",
        "Cursor Click",
        SOCIAL,
        W,
        c(1.0, 0.82, 0.2),
        "none",
        1.0,
        social::cursor_click,
    )),
    s(
        "social.hashtag",
        "Hashtag",
        SOCIAL,
        c(0.35, 0.4, 1.0),
        W,
        "wiggle",
        1.0,
        social::hashtag,
    ),
    s(
        "social.mention",
        "Mention",
        SOCIAL,
        c(0.2, 0.75, 0.95),
        W,
        "wiggle",
        1.0,
        social::mention,
    ),
    s(
        "social.verified",
        "Verified",
        SOCIAL,
        c(0.2, 0.55, 1.0),
        W,
        "pulse",
        1.0,
        social::verified,
    ),
    s(
        "social.views",
        "Views",
        SOCIAL,
        c(0.18, 0.2, 0.28),
        W,
        "none",
        2.4,
        social::views,
    ),
    s(
        "social.link_in_bio",
        "Link in Bio",
        SOCIAL,
        c(0.55, 0.35, 1.0),
        W,
        "bounce",
        3.0,
        callouts::link_in_bio,
    ),
    s(
        "social.swipe_up",
        "Swipe Up",
        SOCIAL,
        c(1.0, 0.4, 0.3),
        W,
        "bounce",
        2.6,
        callouts::swipe_up,
    ),
    s(
        "social.watch_now",
        "Watch Now",
        SOCIAL,
        c(1.0, 0.3, 0.35),
        W,
        "pulse",
        2.7,
        callouts::watch_now,
    ),
    s(
        "social.new_video",
        "New Video",
        SOCIAL,
        c(0.25, 0.55, 1.0),
        W,
        "pulse",
        2.9,
        callouts::new_video,
    ),
    s(
        "social.save",
        "Save",
        SOCIAL,
        c(1.0, 0.7, 0.2),
        W,
        "wiggle",
        2.0,
        callouts::save,
    ),
    s(
        "social.repost",
        "Repost",
        SOCIAL,
        c(0.2, 0.75, 0.5),
        W,
        "wiggle",
        2.4,
        callouts::repost_button,
    ),
    s(
        "social.like_count",
        "Like Count",
        SOCIAL,
        c(1.0, 0.27, 0.45),
        W,
        "heartbeat",
        2.4,
        callouts::like_count,
    ),
    s(
        "social.subscribers",
        "Subscribers",
        SOCIAL,
        c(0.3, 0.4, 0.9),
        W,
        "pulse",
        2.7,
        callouts::subscribers,
    ),
    s(
        "social.comment_below",
        "Comment Below",
        SOCIAL,
        c(0.2, 0.7, 0.85),
        W,
        "bounce",
        3.3,
        callouts::comment_below,
    ),
    s(
        "social.thanks",
        "Thanks for Watching",
        SOCIAL,
        c(1.0, 0.45, 0.6),
        W,
        "float",
        4.4,
        callouts::thanks,
    ),
    s(
        "social.giveaway",
        "Giveaway",
        SOCIAL,
        c(0.65, 0.3, 0.9),
        W,
        "wiggle",
        2.7,
        callouts::giveaway,
    ),
    s(
        "social.trending",
        "Trending",
        SOCIAL,
        c(1.0, 0.5, 0.15),
        W,
        "pulse",
        2.6,
        callouts::trending,
    ),
    animated(s(
        "social.now_playing",
        "Now Playing",
        SOCIAL,
        c(0.12, 0.14, 0.2),
        W,
        "none",
        3.0,
        callouts::now_playing,
    )),
    s(
        "social.notify_on",
        "Notifications On",
        SOCIAL,
        c(0.2, 0.8, 0.4),
        W,
        "pulse",
        1.7,
        callouts::notify_on,
    ),
    s(
        "social.rating",
        "Five Stars",
        SOCIAL,
        c(1.0, 0.78, 0.15),
        W,
        "pulse",
        3.2,
        callouts::rating,
    ),
    // Speech bubbles
    s(
        "bubble.speech",
        "Speech",
        BUBBLES,
        W,
        INK,
        "float",
        1.35,
        bubbles::speech,
    ),
    s(
        "bubble.thought",
        "Thought",
        BUBBLES,
        W,
        INK,
        "float",
        1.35,
        bubbles::thought,
    ),
    s(
        "bubble.shout",
        "Shout",
        BUBBLES,
        c(1.0, 0.88, 0.25),
        INK,
        "shake",
        1.35,
        bubbles::shout,
    ),
    s(
        "bubble.rounded",
        "Chat",
        BUBBLES,
        c(0.25, 0.6, 1.0),
        W,
        "float",
        1.5,
        bubbles::rounded,
    ),
    animated(s(
        "bubble.typing",
        "Typing",
        BUBBLES,
        c(0.95, 0.95, 0.97),
        c(0.45, 0.47, 0.55),
        "none",
        1.6,
        bubbles::typing,
    )),
    // Lines
    s(
        "line.swoosh",
        "Underline Swoosh",
        LINES,
        c(1.0, 0.75, 0.12),
        c(1.0, 0.4, 0.3),
        "none",
        3.0,
        lines::swoosh,
    ),
    s(
        "line.scribble_circle",
        "Scribble Circle",
        LINES,
        c(0.95, 0.18, 0.25),
        W,
        "none",
        1.4,
        lines::scribble_circle,
    ),
    s(
        "line.brush",
        "Brush Stroke",
        LINES,
        c(0.25, 0.62, 1.0),
        W,
        "none",
        3.2,
        lines::brush,
    ),
    s(
        "line.divider",
        "Divider",
        LINES,
        c(1.0, 0.78, 0.3),
        W,
        "none",
        4.0,
        lines::divider,
    ),
    s(
        "line.squiggle",
        "Squiggle",
        LINES,
        c(0.25, 0.85, 0.75),
        W,
        "none",
        3.0,
        lines::squiggle,
    ),
    s(
        "line.highlight",
        "Highlighter",
        LINES,
        c(1.0, 0.92, 0.15),
        W,
        "none",
        3.0,
        lines::highlight,
    ),
    s(
        "line.zigzag",
        "Zigzag",
        LINES,
        c(1.0, 0.5, 0.6),
        W,
        "none",
        2.4,
        lines::zigzag,
    ),
    s(
        "line.spiral",
        "Spiral",
        LINES,
        c(0.4, 0.5, 1.0),
        W,
        "spin",
        1.0,
        lines::spiral,
    ),
    s(
        "line.dotted",
        "Dotted Wave",
        LINES,
        c(1.0, 0.6, 0.25),
        c(0.3, 0.8, 0.9),
        "none",
        2.4,
        lines::dotted,
    ),
    // Badges
    s(
        "badge.new",
        "New",
        BADGES,
        c(0.18, 0.78, 0.4),
        W,
        "pulse",
        1.0,
        badges::new_badge,
    ),
    s(
        "badge.sale",
        "Sale",
        BADGES,
        c(0.95, 0.2, 0.25),
        W,
        "pulse",
        1.8,
        badges::sale,
    ),
    s(
        "badge.wow",
        "WOW",
        BADGES,
        c(1.0, 0.85, 0.15),
        c(0.95, 0.2, 0.2),
        "shake",
        1.3,
        badges::wow,
    ),
    s(
        "badge.omg",
        "OMG",
        BADGES,
        c(1.0, 0.35, 0.65),
        c(0.45, 0.25, 0.9),
        "wiggle",
        1.6,
        badges::omg,
    ),
    s(
        "badge.lol",
        "LOL",
        BADGES,
        c(0.2, 0.78, 0.95),
        W,
        "bounce",
        1.4,
        badges::lol,
    ),
    s(
        "badge.ribbon",
        "Ribbon Banner",
        BADGES,
        c(0.9, 0.18, 0.28),
        c(1.0, 0.85, 0.4),
        "none",
        2.8,
        badges::ribbon_banner,
    ),
    s(
        "badge.seal",
        "Award Seal",
        BADGES,
        c(1.0, 0.75, 0.15),
        c(0.2, 0.45, 0.95),
        "float",
        0.85,
        badges::seal,
    ),
    s(
        "badge.price_tag",
        "Price Tag",
        BADGES,
        c(1.0, 0.55, 0.15),
        W,
        "swing",
        1.25,
        badges::price_tag,
    ),
    s(
        "badge.hot",
        "Hot",
        BADGES,
        c(1.0, 0.4, 0.15),
        W,
        "pulse",
        1.0,
        badges::hot,
    ),
    s(
        "badge.free",
        "Free",
        BADGES,
        c(0.2, 0.75, 0.45),
        W,
        "wiggle",
        2.4,
        badges::free,
    ),
    s(
        "badge.best",
        "Best",
        BADGES,
        c(0.3, 0.4, 0.95),
        c(1.0, 0.85, 0.3),
        "pulse",
        1.0,
        badges::best,
    ),
    s(
        "badge.discount",
        "50% Off",
        BADGES,
        c(0.95, 0.2, 0.35),
        W,
        "wiggle",
        1.0,
        badges::discount,
    ),
    // Nature
    s(
        "nature.sun",
        "Sun",
        NATURE,
        c(1.0, 0.82, 0.2),
        c(1.0, 0.58, 0.12),
        "spin",
        1.0,
        nature::sun,
    ),
    s(
        "nature.cloud",
        "Cloud",
        NATURE,
        c(0.88, 0.94, 1.0),
        c(0.6, 0.72, 0.92),
        "float",
        1.4,
        nature::cloud,
    ),
    s(
        "nature.rainbow",
        "Rainbow",
        NATURE,
        c(0.97, 0.98, 1.0),
        c(0.62, 0.74, 0.95),
        "pulse",
        1.5,
        nature::rainbow,
    ),
    s(
        "nature.moon",
        "Moon",
        NATURE,
        c(1.0, 0.9, 0.5),
        c(1.0, 0.95, 0.7),
        "float",
        1.0,
        nature::moon,
    ),
    s(
        "nature.snowflake",
        "Snowflake",
        NATURE,
        c(0.6, 0.85, 1.0),
        W,
        "spin",
        1.0,
        nature::snowflake,
    ),
    s(
        "nature.flower",
        "Flower",
        NATURE,
        c(1.0, 0.5, 0.72),
        c(1.0, 0.8, 0.2),
        "wiggle",
        1.0,
        nature::flower,
    ),
    s(
        "nature.leaf",
        "Leaf",
        NATURE,
        c(0.3, 0.74, 0.34),
        c(0.78, 0.96, 0.62),
        "swing",
        1.0,
        nature::leaf,
    ),
    s(
        "nature.raindrop",
        "Raindrop",
        NATURE,
        c(0.25, 0.6, 1.0),
        W,
        "bounce",
        1.0,
        nature::raindrop,
    ),
    // Sparkles
    animated(s(
        "sparkle.twinkle",
        "Twinkle",
        SPARKLES,
        c(1.0, 0.95, 0.7),
        c(1.0, 0.75, 0.3),
        "none",
        1.0,
        sparkles::twinkle_star,
    )),
    animated(s(
        "sparkle.glitter",
        "Glitter",
        SPARKLES,
        c(1.0, 0.82, 0.3),
        c(1.0, 0.6, 0.85),
        "none",
        1.4,
        sparkles::glitter,
    )),
    animated(s(
        "sparkle.confetti",
        "Confetti",
        SPARKLES,
        c(1.0, 0.3, 0.45),
        c(0.25, 0.65, 1.0),
        "none",
        1.2,
        sparkles::confetti,
    )),
    animated(s(
        "sparkle.stars",
        "Twinkling Stars",
        SPARKLES,
        c(1.0, 0.82, 0.2),
        c(0.6, 0.85, 1.0),
        "none",
        1.4,
        sparkles::stars,
    )),
    animated(s(
        "sparkle.hearts",
        "Floating Hearts",
        SPARKLES,
        c(1.0, 0.3, 0.45),
        c(1.0, 0.6, 0.75),
        "none",
        0.9,
        sparkles::hearts,
    )),
    animated(s(
        "sparkle.fireworks",
        "Fireworks",
        SPARKLES,
        c(1.0, 0.4, 0.6),
        c(0.4, 0.8, 1.0),
        "none",
        1.0,
        sparkles::fireworks,
    )),
    animated(s(
        "sparkle.shooting_star",
        "Shooting Star",
        SPARKLES,
        c(1.0, 0.85, 0.3),
        c(1.0, 0.6, 0.3),
        "none",
        1.4,
        sparkles::shooting_star,
    )),
    animated(s(
        "sparkle.snowfall",
        "Snowfall",
        SPARKLES,
        W,
        c(0.6, 0.8, 1.0),
        "none",
        1.0,
        sparkles::snowfall,
    )),
    animated(s(
        "sparkle.bubbles",
        "Bubbles",
        SPARKLES,
        c(0.5, 0.85, 1.0),
        c(1.0, 0.6, 0.9),
        "none",
        1.0,
        sparkles::bubbles,
    )),
];

pub fn get(id: &str) -> Option<&'static StickerDef> {
    STICKERS.iter().find(|s| s.id == id)
}

/// Draws a sticker `height_px` tall (premultiplied RGBA).
pub fn render(spec: &StickerSpec, height_px: u32, t: f32) -> RgbaImage {
    let Some(def) = get(&spec.kind) else {
        return RgbaImage::new(1, 1);
    };
    let h = height_px.clamp(4, 4096);
    let w = ((h as f32 * def.aspect).round() as u32).clamp(4, 8192);
    let mut pm = Pixmap::new(w, h).unwrap();
    (def.draw)(
        &mut pm,
        &DrawCtx {
            w: w as f32,
            h: h as f32,
            color: spec.color,
            color2: spec.color2,
            t,
        },
    );
    RgbaImage {
        width: w,
        height: h,
        pixels: pm.take(),
        premultiplied: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{animations::STICKER_MOTIONS, categories};

    fn spec(def: &StickerDef) -> StickerSpec {
        StickerSpec {
            kind: def.id.into(),
            color: def.color,
            color2: def.color2,
            motion: def.motion.into(),
        }
    }

    #[test]
    fn ids_unique_and_well_formed() {
        for (i, a) in STICKERS.iter().enumerate() {
            assert!(
                a.id.contains('.')
                    && a.id.chars().all(|c| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || c == '.'
                        || c == '_'),
                "bad id {}",
                a.id
            );
            assert!(!a.name.is_empty());
            assert!(a.aspect > 0.2 && a.aspect < 6.0, "{}", a.id);
            for b in &STICKERS[i + 1..] {
                assert_ne!(a.id, b.id);
            }
        }
        assert!(STICKERS.len() >= 45, "{}", STICKERS.len());
    }

    #[test]
    fn motions_valid_and_categories_ordered() {
        for d in STICKERS {
            assert!(
                STICKER_MOTIONS.iter().any(|(id, _)| *id == d.motion),
                "{} motion {}",
                d.id,
                d.motion
            );
        }
        assert_eq!(
            categories(STICKERS, |s| s.category),
            [
                "Arrows",
                "Shapes",
                "Emoji",
                "Symbols",
                "Social Media",
                "Speech Bubbles",
                "Lines",
                "Badges",
                "Nature",
                "Sparkles"
            ]
        );
        for d in STICKERS {
            let prefix = d.id.split('.').next().unwrap();
            let same: Vec<_> = STICKERS
                .iter()
                .filter(|o| o.id.starts_with(&format!("{prefix}.")))
                .collect();
            assert!(
                same.iter().all(|o| o.category == d.category),
                "{} prefix spans categories",
                d.id
            );
        }
    }

    #[test]
    fn unknown_id_renders_blank() {
        let mut sp = spec(&STICKERS[0]);
        sp.kind = "nope".into();
        let img = render(&sp, 64, 0.0);
        assert_eq!((img.width, img.height), (1, 1));
    }

    #[test]
    fn every_sticker_renders_tiny() {
        for d in STICKERS {
            let img = render(&spec(d), 8, 0.0);
            assert_eq!(img.height, 8);
            let max = img.pixels.chunks(4).map(|p| p[3]).max().unwrap();
            assert!(max > 64, "{} is blank at 8 px", d.id);
        }
    }

    /// Checks one big render: mostly-opaque ink somewhere, nothing touching
    /// the border.
    fn check_large(d: &StickerDef, t: f32) -> Result<(), String> {
        let img = render(&spec(d), 1024, t);
        let (w, h) = (img.width as usize, img.height as usize);
        let alpha = |x: usize, y: usize| img.pixels[(y * w + x) * 4 + 3];
        let solid = img.pixels.chunks(4).filter(|p| p[3] >= 128).count();
        if solid < w * h / 400 {
            return Err(format!("{} at t={t}: only {solid} solid pixels", d.id));
        }
        let m = h / 200 + 1;
        for y in 0..h {
            for x in 0..w {
                let edge = x < m || y < m || x >= w - m || y >= h - m;
                if edge && alpha(x, y) > 2 {
                    return Err(format!(
                        "{} at t={t}: ink at the border ({x}, {y}) of {w}x{h}",
                        d.id
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn every_sticker_renders_large_with_margin() {
        let jobs: Vec<(&StickerDef, f32)> = STICKERS
            .iter()
            .flat_map(|d| {
                let ts: &[f32] = if d.animated {
                    &[0.0, 0.37, 1.21, 2.9]
                } else {
                    &[0.0]
                };
                ts.iter().map(move |&t| (d, t))
            })
            .collect();
        let errors: Vec<String> = std::thread::scope(|sc| {
            let handles: Vec<_> = jobs
                .chunks(jobs.len().div_ceil(4))
                .map(|chunk| {
                    sc.spawn(move || {
                        chunk
                            .iter()
                            .filter_map(|(d, t)| check_large(d, *t).err())
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    #[test]
    fn animated_stickers_change_and_static_ones_do_not() {
        for d in STICKERS {
            let a = render(&spec(d), 48, 0.0).pixels;
            let b = render(&spec(d), 48, 0.5).pixels;
            assert_eq!(d.animated, a != b, "{}", d.id);
        }
    }

    #[test]
    fn recoloring_changes_the_drawing() {
        for d in STICKERS {
            let base = render(&spec(d), 48, 0.0).pixels;
            let mut sp = spec(d);
            sp.color = [0.5, 0.2, 0.9, 1.0];
            let main = render(&sp, 48, 0.0).pixels;
            assert_ne!(base, main, "{} ignores color", d.id);
        }
    }
}
