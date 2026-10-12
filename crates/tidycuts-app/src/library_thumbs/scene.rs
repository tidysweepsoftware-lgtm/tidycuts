//! What each library tile shows: a tiny project built from the catalog
//! entry and the procedural sample pictures, the moment used for the still
//! and the moments of the hover loop.

use crate::library::LibraryDrag;
use crate::templates;
use std::path::{Path, PathBuf};
use tidycuts_core::{
    Animatable, Clip, ClipAnimation, Effect, Generator, MediaItem, MediaKind, Project, StickerSpec,
    Time, TitleSpec, TrackId, TrackKind, Transition, VisualizerSpec,
};
use tidycuts_render::catalog::{effects, filters, stickers, titles, transitions, Sample};
use tidycuts_render::{sample, RgbaImage};

/// A catalog entry with a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Title(&'static str),
    Transition(&'static str),
    /// An effect or a filter.
    Effect(&'static str),
    Sticker(&'static str),
    Layout(&'static str),
    Visualizer(&'static str),
    Template(usize),
}

impl Key {
    pub fn of(drag: &LibraryDrag) -> Option<Key> {
        Some(match drag {
            LibraryDrag::Title(id) => Key::Title(id),
            LibraryDrag::Transition(id) => Key::Transition(id),
            LibraryDrag::Effect(id) => Key::Effect(id),
            LibraryDrag::Sticker(id) => Key::Sticker(id),
            LibraryDrag::Layout(id) => Key::Layout(id),
            LibraryDrag::Visualizer(id) => Key::Visualizer(id),
            LibraryDrag::Template(i) => Key::Template(*i),
            LibraryDrag::Media(_) | LibraryDrag::Sound(_) => return None,
        })
    }
}

/// Length used for plain background templates.
const STILL_SECS: f64 = 5.0;
/// Hover loops play at about this many frames per second.
const LOOP_FPS: f64 = 12.0;

/// The pictures tiles are made from, drawn on the fly at the size asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pic {
    Landscape,
    Sunset,
    Portrait,
    Wildlife,
    Meadow,
    Pet,
    Nap,
    Night,
    City,
    Food,
    GreenScreen,
    Checker,
}

const SCHEME: &str = "tidycuts-sample:";

impl Pic {
    fn name(self) -> &'static str {
        match self {
            Pic::Landscape => "landscape",
            Pic::Sunset => "sunset",
            Pic::Portrait => "portrait",
            Pic::Wildlife => "wildlife",
            Pic::Meadow => "meadow",
            Pic::Pet => "pet",
            Pic::Nap => "nap",
            Pic::Night => "night",
            Pic::City => "city",
            Pic::Food => "food",
            Pic::GreenScreen => "green-screen",
            Pic::Checker => "checker",
        }
    }

    pub fn from_path(p: &Path) -> Option<Pic> {
        let name = p.to_str()?.strip_prefix(SCHEME)?;
        [
            Pic::Landscape,
            Pic::Sunset,
            Pic::Portrait,
            Pic::Wildlife,
            Pic::Meadow,
            Pic::Pet,
            Pic::Nap,
            Pic::Night,
            Pic::City,
            Pic::Food,
            Pic::GreenScreen,
            Pic::Checker,
        ]
        .into_iter()
        .find(|p| p.name() == name)
    }

    fn of(s: Sample) -> Pic {
        match s {
            Sample::Landscape => Pic::Landscape,
            Sample::Sunset => Pic::Sunset,
            Sample::Portrait => Pic::Portrait,
            Sample::Wildlife => Pic::Wildlife,
            Sample::Meadow => Pic::Meadow,
            Sample::Pet => Pic::Pet,
            Sample::Nap => Pic::Nap,
            Sample::Night => Pic::Night,
            Sample::City => Pic::City,
            Sample::Food => Pic::Food,
        }
    }

    pub fn draw(self, w: u32, h: u32) -> RgbaImage {
        match self {
            Pic::Landscape => sample::landscape(w, h),
            Pic::Sunset => sample::sunset(w, h),
            Pic::Portrait => sample::portrait(w, h),
            Pic::Wildlife => sample::wildlife(w, h),
            Pic::Meadow => sample::meadow(w, h),
            Pic::Pet => sample::pet(w, h),
            Pic::Nap => sample::nap(w, h),
            Pic::Night => sample::night(w, h),
            Pic::City => sample::city(w, h),
            Pic::Food => sample::food(w, h),
            Pic::GreenScreen => sample::portrait_green(w, h),
            Pic::Checker => checker(w, h),
        }
    }

    /// A media item standing for this picture (never read from disk).
    fn media(self) -> MediaItem {
        let mut m = MediaItem::new(
            PathBuf::from(format!("{SCHEME}{}", self.name())),
            MediaKind::Image,
            Time::from_secs_f64(600.0),
        );
        m.width = 1920;
        m.height = 1080;
        m
    }
}

/// A dark checkerboard, the backdrop for stickers and transparent effects.
fn checker(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w.max(1), h.max(1));
    let cell = (h / 9).max(2);
    for y in 0..img.height {
        for x in 0..img.width {
            let v = if ((x / cell) + (y / cell)).is_multiple_of(2) {
                [52, 55, 64, 255]
            } else {
                [64, 67, 77, 255]
            };
            let i = ((y * img.width + x) * 4) as usize;
            img.pixels[i..i + 4].copy_from_slice(&v);
        }
    }
    img
}

/// A before/after comparison on a tile: the left side shows the picture as
/// it is, the right side with the effect.
pub struct Split {
    /// The same picture without the effect.
    pub plain: Project,
    /// Where the dividing line sits on the still (0..1 from the left);
    /// `None` shows the whole result.
    pub still: Option<f32>,
    /// Hovering sweeps the line across instead of playing the effect.
    pub sweep: bool,
}

/// What to render for one tile.
pub struct Scene {
    pub project: Project,
    /// The moment shown as the tile's still picture.
    pub still: Time,
    /// The moments of the hover loop (empty: no animation).
    pub frames: Vec<Time>,
    pub split: Option<Split>,
    /// More moments laid over the still (motion effects).
    pub blend: Vec<Time>,
}

/// Frames of a before/after sweep.
pub const SWEEP_FRAMES: usize = 24;

/// Where the dividing line is on frame `i` of `n` of a sweep: from the
/// left, to the right and back, easing at both ends.
pub fn sweep_position(i: usize, n: usize) -> f32 {
    let t = i as f32 / n.max(1) as f32;
    0.1 + 0.8 * (0.5 - 0.5 * (t * std::f32::consts::TAU).cos())
}

/// The hover loop: frame moments in seconds and seconds per frame.
pub struct Loop {
    pub times: Vec<f64>,
    pub interval: f32,
}

impl Loop {
    fn none() -> Loop {
        Loop {
            times: Vec::new(),
            interval: 0.1,
        }
    }

    /// A before/after sweep at `at` seconds.
    fn sweep(at: f64) -> Loop {
        Loop {
            times: vec![at; SWEEP_FRAMES],
            interval: 1.0 / LOOP_FPS as f32,
        }
    }

    /// `n` evenly spaced moments from `from` over `span` seconds.
    fn even(from: f64, span: f64, n: usize) -> Loop {
        let n = n.max(2);
        Loop {
            times: (0..n).map(|i| from + span * i as f64 / n as f64).collect(),
            interval: (span / n as f64) as f32,
        }
    }

    /// About [`LOOP_FPS`] frames over `span` seconds, between `lo` and `hi` frames.
    fn over(from: f64, span: f64, lo: usize, hi: usize) -> Loop {
        let n = ((span * LOOP_FPS).round() as usize).clamp(lo, hi);
        Loop::even(from, span, n)
    }
}

/// How much closer overlay templates are shown.
const OVERLAY_ZOOM: f64 = 1.6;

/// The moment of an effect shown on its still.
const EFFECT_AT: f64 = 1.3;

/// Seconds of the cut between the two pictures of a transition tile.
const CUT: f64 = 3.0;
/// Progress shown on a transition's still.
const TRANSITION_STILL: f64 = 0.42;
/// The same for dips through a color, which are all color in the middle.
const DIP_STILL: f64 = 0.27;

fn transition_secs(id: &str) -> f64 {
    transitions::get(id)
        .map(|d| d.duration as f64)
        .filter(|d| *d > 0.0)
        .unwrap_or(1.0)
}

fn transition_time(id: &str, progress: f64) -> f64 {
    let d = transition_secs(id);
    CUT - d * 0.5 + progress * d
}

/// How long a sticker's looping motion takes to come back to the start
/// (two periods for the quick ones).
fn sticker_loop_secs(def: &stickers::StickerDef) -> Option<f64> {
    let secs = match def.motion {
        "bounce" => 1.25,
        "pulse" | "heartbeat" => 5.0 / 3.0,
        "spin" => 3.0,
        "float" => 4.0,
        "wiggle" | "shake" => 1.0,
        "swing" => 2.5,
        _ if def.animated => 2.0,
        _ => return None,
    };
    Some(secs)
}

/// The hover loop of an entry. Cheap: no project is built.
pub fn hover_loop(key: Key) -> Loop {
    match key {
        Key::Transition(id) => {
            // Progress 0 → 1 with a short rest on both pictures.
            let n = 20;
            let times = (0..n)
                .map(|i| {
                    let p = (i as f64 / (n - 1) as f64 * 1.3 - 0.15).clamp(0.0, 1.0);
                    transition_time(id, p)
                })
                .collect();
            Loop {
                times,
                interval: 0.08,
            }
        }
        // Colour looks don't move: a before/after line sweeps across.
        Key::Effect(id) if filters::get(id).is_some() => Loop::sweep(EFFECT_AT),
        Key::Effect(_) => Loop::over(0.3, 2.0, SWEEP_FRAMES, SWEEP_FRAMES),
        Key::Title(id) => match titles::get(id).and_then(|p| p.anim_in) {
            Some((_, d)) => Loop::over(0.0, d as f64 + 0.8, 8, 24),
            None => Loop::none(),
        },
        Key::Sticker(id) => match stickers::get(id).and_then(sticker_loop_secs) {
            Some(secs) => Loop::over(0.0, secs, 12, 24),
            None => Loop::none(),
        },
        Key::Layout(_) => Loop::none(),
        Key::Visualizer(_) => Loop::over(0.0, 3.0, 12, 24),
        Key::Template(i) => match templates::get(i) {
            Some(t) => {
                let len = templates::length(t, STILL_SECS);
                Loop::over(0.0, len.min(3.6), 12, 30)
            }
            None => Loop::none(),
        },
    }
}

fn secs(s: f64) -> Time {
    Time::from_secs_f64(s)
}

/// An empty 1920x1080 project for a tile.
fn blank() -> Project {
    let mut p = Project::new("Library tile");
    p.settings.magnetic = false;
    p
}

/// The video track `lane` steps above the bottom one, added if needed.
fn lane(p: &mut Project, lane: usize) -> TrackId {
    while p.track_count(TrackKind::Video) <= lane {
        if p.add_track(TrackKind::Video).is_none() {
            break;
        }
    }
    let ids: Vec<TrackId> = p
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
        .map(|t| t.id)
        .collect();
    ids[ids.len().saturating_sub(1 + lane)]
}

fn put(p: &mut Project, lane_no: usize, clip: Clip) {
    let t = lane(p, lane_no);
    let at = clip.start;
    p.place_clip(clip, t, at);
}

/// A clip of a sample picture.
fn picture(p: &mut Project, pic: Pic, start: f64, len: f64) -> Clip {
    let m = pic.media();
    if p.media(m.id).is_none() {
        p.add_media(m.clone());
    }
    let mut c = Clip::new(&m, secs(start));
    c.source_out = c.source_in + secs(len);
    c
}

/// How much to enlarge a title's tile so the text reads at thumbnail size,
/// and the point (offset from the frame centre) to enlarge around.
fn title_zoom(spec: &TitleSpec, pos: [f64; 2]) -> (f64, [f64; 2]) {
    let (w, h) = (1920.0, 1080.0);
    let size = tidycuts_render::text::render_title(spec, 0.1, 1.0, None).size;
    let (tw, th) = (size[0] as f64, size[1] as f64);
    let z = (0.86 * w / tw.max(1.0))
        .min(0.62 * h / th.max(1.0))
        .clamp(1.0, 1.8);
    // Keep the title inside the frame after zooming, then keep the zoomed
    // background covering the frame.
    let fit = |p: f64, t: f64, full: f64| {
        let room = (0.48 * full - t * z * 0.5).max(0.0);
        (p * z).clamp(-room, room)
    };
    let q = [fit(pos[0], tw, w), fit(pos[1], th, h)];
    let lim = |full: f64| (z - 1.0) / z * full * 0.5;
    let f = [
        (pos[0] - q[0] / z).clamp(-lim(w), lim(w)),
        (pos[1] - q[1] / z).clamp(-lim(h), lim(h)),
    ];
    (z, f)
}

/// Half the width and height of a generated clip's picture in project
/// pixels (1920x1080), if it has a size of its own.
fn half_size(c: &Clip) -> Option<[f64; 2]> {
    let t = &c.transform;
    let s = [t.scale_x.value / 100.0, t.scale_y.value / 100.0];
    let size = match c.generator.as_ref()? {
        Generator::Solid(_) | Generator::Gradient { .. } => [1920.0, 1080.0],
        Generator::Title(spec) => {
            let r = tidycuts_render::text::render_title(spec, 0.1, 1.0, None).size;
            [r[0] as f64, r[1] as f64]
        }
        Generator::Sticker(spec) => {
            let aspect = stickers::get(&spec.kind).map(|d| d.aspect).unwrap_or(1.0) as f64;
            let h = stickers::STICKER_HEIGHT as f64;
            [h * aspect, h]
        }
        Generator::Adjustment | Generator::Placeholder | Generator::Visualizer(_) => return None,
    };
    Some([size[0] * s[0].abs() * 0.5, size[1] * s[1].abs() * 0.5])
}

/// A close-up of an overlay template: the most zoom (up to
/// [`OVERLAY_ZOOM`]) that keeps every piece narrower than the frame fully
/// in view, around the middle of the pieces.
fn overlay_zoom(p: &Project) -> (f64, [f64; 2]) {
    let (w, h) = (1920.0, 1080.0);
    let pieces: Vec<([f64; 2], [f64; 2])> = p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter_map(|c| {
            let half = half_size(c)?;
            let t = &c.transform;
            Some(([t.position_x.value, t.position_y.value], half))
        })
        .filter(|(_, half)| half[0] < w * 0.45)
        .collect();
    if pieces.is_empty() {
        return (1.0, [0.0, 0.0]);
    }
    let n = pieces.len() as f64;
    let mean = [
        pieces.iter().map(|(c, _)| c[0]).sum::<f64>() / n,
        pieces.iter().map(|(c, _)| c[1]).sum::<f64>() / n,
    ];
    let mut z = OVERLAY_ZOOM;
    while z > 1.0 {
        let lim = |full: f64| (z - 1.0) / z * full * 0.5;
        let f = [
            mean[0].clamp(-lim(w), lim(w)),
            mean[1].clamp(-lim(h), lim(h)),
        ];
        let fits = pieces.iter().all(|(c, half)| {
            ((c[0] - f[0]) * z).abs() + half[0] * z <= w * 0.49
                && ((c[1] - f[1]) * z).abs() + half[1] * z <= h * 0.49
        });
        if fits {
            return (z, f);
        }
        z -= 0.05;
    }
    (1.0, [0.0, 0.0])
}

fn zoomed(c: &mut Clip, z: f64, focus: [f64; 2]) {
    let t = &mut c.transform;
    let p = [t.position_x.value, t.position_y.value];
    t.position_x = Animatable::constant((p[0] - focus[0]) * z);
    t.position_y = Animatable::constant((p[1] - focus[1]) * z);
    t.scale_x = Animatable::constant(t.scale_x.value * z);
    t.scale_y = Animatable::constant(t.scale_y.value * z);
}

/// The two pictures a transition tile cuts between: pairs that differ in
/// colour and brightness, taken in turn so the tiles are not all alike.
fn transition_pictures(id: &str) -> (Pic, Pic) {
    const PAIRS: [(Pic, Pic); 6] = [
        (Pic::Landscape, Pic::Sunset),
        (Pic::Pet, Pic::Wildlife),
        (Pic::Sunset, Pic::Nap),
        (Pic::Meadow, Pic::Landscape),
        (Pic::Nap, Pic::Pet),
        (Pic::Wildlife, Pic::Meadow),
    ];
    let index = transitions::TRANSITIONS
        .iter()
        .position(|t| t.id == id)
        .unwrap_or(0);
    PAIRS[index % PAIRS.len()]
}

/// The picture an effect or filter is shown on.
fn effect_picture(id: &str) -> Option<(Pic, bool)> {
    Some(if id == "basic.chroma_key" {
        (Pic::GreenScreen, false)
    } else if let Some(def) = effects::get(id) {
        // Particles and bokeh show best on a dark picture.
        let dark = def.category == "Particles" || id == "overlay.bokeh";
        (
            if dark {
                Pic::Night
            } else {
                Pic::of(def.sample)
            },
            false,
        )
    } else {
        (Pic::of(filters::get(id)?.sample), true)
    })
}

/// Stronger settings for the tiles of motion effects.
fn tile_boost(id: &str) -> Vec<(&'static str, f64)> {
    let Some(def) = effects::get(id).filter(|d| d.kind == effects::EffectKind::Motion) else {
        return Vec::new();
    };
    let amount = effects::default_value(def, "amount") as f64;
    match id {
        _ if id.starts_with("shake.") => vec![("amount", (amount * 2.6).min(3.0))],
        "zoom.pulse" | "zoom.heartbeat" => vec![("amount", amount * 2.5)],
        "zoom.ken_burns_in" | "zoom.ken_burns_out" | "zoom.pan_left" | "zoom.pan_right" => {
            vec![("duration", 2.0)]
        }
        _ => Vec::new(),
    }
}

/// The tile project of an effect or filter, with or without it applied,
/// and whether it is a filter.
fn effect_project(id: &str, with_effect: bool) -> Option<(Project, bool)> {
    let (pic, filter) = effect_picture(id)?;
    let mut p = blank();
    let mut c = picture(&mut p, pic, 0.0, 10.0);
    if with_effect {
        let mut e = Effect::new(id);
        // Motion effects are subtle at their defaults; the tile shows them
        // bigger and quicker so the movement reads at thumbnail size.
        for (key, value) in tile_boost(id) {
            e.params
                .insert(key.to_string(), Animatable::constant(value));
        }
        c.effects.push(e);
        if !filter {
            // Effects can leave parts transparent.
            let board = picture(&mut p, Pic::Checker, 0.0, 10.0);
            put(&mut p, 0, board);
        }
    }
    put(&mut p, 1, c);
    Some((p, filter))
}

/// Builds the tile of `key`.
pub fn scene(key: Key) -> Option<Scene> {
    let lp = hover_loop(key);
    let frames = lp.times.iter().map(|t| secs(*t)).collect();
    let (project, still) = match key {
        Key::Transition(id) => {
            let d = transition_secs(id);
            let mut p = blank();
            let (from, to) = transition_pictures(id);
            let a = picture(&mut p, from, 0.0, CUT);
            let mut b = picture(&mut p, to, CUT, CUT);
            b.transition_in = Some(Transition::new(id, secs(d)));
            put(&mut p, 0, a);
            put(&mut p, 0, b);
            // Dip to color and flash shaders.
            let dip = transitions::get(id).is_some_and(|d| matches!(d.shader, 2 | 16));
            let progress = if dip { DIP_STILL } else { TRANSITION_STILL };
            (p, transition_time(id, progress))
        }
        Key::Effect(id) => (effect_project(id, true)?.0, EFFECT_AT),
        Key::Title(id) => {
            let index = titles::TITLES.iter().position(|t| t.id == id)?;
            let preset = &titles::TITLES[index];
            let len = if preset.duration > 0.0 {
                preset.duration as f64
            } else {
                5.0
            };
            let mut p = blank();
            let pic = if index % 2 == 0 {
                Pic::Sunset
            } else {
                Pic::Landscape
            };
            let mut bg = picture(&mut p, pic, 0.0, len);
            // A little darker, so light text reads.
            bg.color.exposure = Animatable::constant(-25.0);
            let spec = (preset.make)();
            let pos = [preset.position[0] as f64, preset.position[1] as f64];
            let (z, focus) = title_zoom(&spec, pos);
            let mut c = Clip::generated(Generator::Title(spec), Time::ZERO, secs(len));
            c.transform.position_x = Animatable::constant(pos[0]);
            c.transform.position_y = Animatable::constant(pos[1]);
            let anim = |a: Option<(&str, f32)>| {
                a.map(|(kind, d)| ClipAnimation {
                    kind: kind.into(),
                    duration: secs(d as f64),
                })
            };
            c.anim_in = anim(preset.anim_in);
            c.anim_out = anim(preset.anim_out);
            zoomed(&mut bg, z, focus);
            zoomed(&mut c, z, focus);
            put(&mut p, 0, bg);
            put(&mut p, 1, c);
            let d_in = preset.anim_in.map(|a| a.1 as f64).unwrap_or(0.0);
            let d_out = preset.anim_out.map(|a| a.1 as f64).unwrap_or(0.0);
            (p, (d_in + 0.5).min(len - d_out - 0.05).max(0.0))
        }
        Key::Sticker(id) => {
            let def = stickers::get(id)?;
            let mut p = blank();
            // A dark tile, so the sticker's colours stand out.
            let backdrop = Clip::generated(
                Generator::Gradient {
                    a: [0.075, 0.08, 0.11, 1.0],
                    b: [0.17, 0.18, 0.25, 1.0],
                    angle: 150.0,
                },
                Time::ZERO,
                secs(10.0),
            );
            put(&mut p, 0, backdrop);
            let spec = StickerSpec {
                kind: def.id.into(),
                color: def.color,
                color2: def.color2,
                motion: def.motion.into(),
            };
            let mut c = Clip::generated(Generator::Sticker(spec), Time::ZERO, secs(10.0));
            let fit = (2.9f64).min(4.9 / def.aspect.max(0.1) as f64);
            c.transform.scale_x = Animatable::constant(fit * 100.0);
            c.transform.scale_y = Animatable::constant(fit * 100.0);
            put(&mut p, 1, c);
            (p, if def.animated { 0.6 } else { 0.0 })
        }
        Key::Layout(id) => {
            let def = tidycuts_core::layout(id)?;
            let mut p = blank();
            let pics = [
                Pic::Wildlife,
                Pic::Sunset,
                Pic::Portrait,
                Pic::Pet,
                Pic::Landscape,
            ];
            for (i, cell) in def.cells().into_iter().enumerate() {
                let mut c = picture(&mut p, pics[i % pics.len()], 0.0, 10.0);
                c.video.cell = Some(cell);
                put(&mut p, i, c);
            }
            (p, 0.5)
        }
        Key::Visualizer(id) => {
            let mut p = blank();
            let backdrop = Clip::generated(
                Generator::Gradient {
                    a: [0.06, 0.08, 0.13, 1.0],
                    b: [0.17, 0.12, 0.31, 1.0],
                    angle: 135.0,
                },
                Time::ZERO,
                secs(10.0),
            );
            put(&mut p, 0, backdrop);
            let spec = VisualizerSpec {
                style: id.into(),
                ..Default::default()
            };
            // Shown bigger than the default so it fills the tile.
            let round = spec.is_round();
            let mut vis = Clip::generated(Generator::Visualizer(spec), Time::ZERO, secs(10.0));
            // Round ones grow evenly; wide ones grow taller than wide, which
            // redraws them at that size rather than stretching them.
            let (zx, zy) = if round {
                (140.0, 140.0)
            } else {
                (125.0, 185.0)
            };
            vis.transform.scale_x = Animatable::constant(zx);
            vis.transform.scale_y = Animatable::constant(zy);
            put(&mut p, 1, vis);
            (p, 1.0)
        }
        Key::Template(i) => {
            let t = templates::get(i)?;
            let mut p = templates::preview_project(t, STILL_SECS);
            let len = templates::length(t, STILL_SECS);
            if t.overlay {
                // Overlays are shown over a picture, as over a video, and
                // closer up so they read at tile size.
                let bg = picture(&mut p, Pic::Landscape, 0.0, len);
                put(&mut p, 0, bg);
                let (z, focus) = overlay_zoom(&p);
                for t in &mut p.tracks {
                    for c in &mut t.clips {
                        zoomed(c, z, focus);
                    }
                }
            }
            (p, t.poster.min(len - 0.05).max(0.0))
        }
    };
    let split = match key {
        Key::Effect(id) => effect_project(id, false).map(|(plain, filter)| Split {
            plain,
            still: filter.then_some(0.5),
            sweep: filter,
        }),
        _ => None,
    };
    let moving = matches!(key, Key::Effect(id)
        if effects::get(id).is_some_and(|d| d.kind == effects::EffectKind::Motion));
    let blend = if moving {
        [0.2, 0.45, 0.7, 0.95, 1.2, 1.6]
            .into_iter()
            .map(secs)
            .collect()
    } else {
        Vec::new()
    };
    Some(Scene {
        project,
        still: secs(still),
        frames,
        split,
        blend,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{catalog, LibraryTab};

    #[test]
    fn every_catalog_entry_has_a_scene_with_its_moments_inside() {
        for tab in [
            LibraryTab::Titles,
            LibraryTab::Transitions,
            LibraryTab::Effects,
            LibraryTab::Filters,
            LibraryTab::Stickers,
            LibraryTab::Layouts,
            LibraryTab::Templates,
        ] {
            for item in catalog(tab) {
                let key = Key::of(&item.drag).unwrap();
                let s = scene(key).unwrap_or_else(|| panic!("no scene for {key:?}"));
                let end = s.project.duration();
                assert!(end > Time::ZERO, "{key:?}");
                assert!(s.still < end, "{key:?}");
                assert!(s.frames.iter().all(|t| *t <= end), "{key:?}");
                assert!(s.frames.len() <= 30, "{key:?}");
            }
        }
    }

    #[test]
    fn transitions_loop_from_one_picture_to_the_other() {
        let l = hover_loop(Key::Transition("dissolve"));
        let d = transition_secs("dissolve");
        assert!((l.times[0] - (CUT - d / 2.0)).abs() < 1e-9);
        assert!((l.times.last().unwrap() - (CUT + d / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn filters_split_before_and_after_and_effects_play() {
        let filter = scene(Key::Effect("filter.vivid")).unwrap();
        let split = filter.split.expect("a filter splits");
        assert_eq!(split.still, Some(0.5));
        assert!(split.sweep);
        assert_eq!(filter.frames.len(), SWEEP_FRAMES);
        let fx = scene(Key::Effect("blur.gaussian")).unwrap();
        let split = fx.split.expect("an effect can sweep when it doesn't move");
        assert_eq!(split.still, None);
        assert!(!split.sweep);
        assert!(scene(Key::Title("title.basic")).is_none_or(|s| s.split.is_none()));
    }

    #[test]
    fn the_sweep_stays_on_the_tile_and_comes_back() {
        for i in 0..SWEEP_FRAMES {
            let p = sweep_position(i, SWEEP_FRAMES);
            assert!((0.0999..=0.9001).contains(&p), "{p}");
        }
        assert!((sweep_position(0, SWEEP_FRAMES) - 0.1).abs() < 1e-5);
        assert!(sweep_position(SWEEP_FRAMES / 2, SWEEP_FRAMES) > 0.89);
    }

    #[test]
    fn motion_effects_are_blended_and_particles_sit_on_the_dark_picture() {
        let shake = scene(Key::Effect("shake.camera")).unwrap();
        assert!(!shake.blend.is_empty());
        assert!(scene(Key::Effect("filter.vivid")).unwrap().blend.is_empty());
        assert_eq!(effect_picture("particles.snow"), Some((Pic::Night, false)));
        assert_eq!(effect_picture("filter.vivid").map(|p| p.1), Some(true));
    }

    #[test]
    fn sample_paths_round_trip() {
        for pic in [Pic::Landscape, Pic::GreenScreen, Pic::Checker] {
            assert_eq!(Pic::from_path(&pic.media().path), Some(pic));
        }
        assert_eq!(Pic::from_path(Path::new("/home/me/video.mp4")), None);
    }
}
