//! The thumbnail maker: one picture (a frame of a video, a photo, or what the
//! timeline shows) with big, punchy lettering on it, saved as a PNG or JPG.
//!
//! A thumbnail is built as a small project of its own (the picture filling
//! the frame, a dark fade for the lettering to sit on, and the lettering),
//! so it is drawn by the same code as everything else.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tidycuts_core::{
    Animatable, Clip, Generator, MediaItem, MediaKind, Project, TextAlign, Time, TitleSpec,
    TrackKind,
};
use tidycuts_engine::analysis::Analysis;
use tidycuts_engine::snapshot::{self, Picture};
use tidycuts_engine::Statics;
use tidycuts_render::Renderer;

/// A picture shape.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}

pub const CANVASES: [Canvas; 3] = [
    Canvas {
        width: 1280,
        height: 720,
    },
    Canvas {
        width: 1080,
        height: 1080,
    },
    Canvas {
        width: 1080,
        height: 1920,
    },
];

/// A style of lettering.
pub struct Look {
    pub id: &'static str,
    pub name: &'static str,
    font: &'static str,
    /// Size on a 1080-pixel-high picture.
    size: f32,
    color: u32,
    outline: f32,
    outline_color: u32,
    shadow: f32,
    /// A coloured box behind the words.
    boxed: Option<u32>,
    capitals: bool,
}

pub const LOOKS: &[Look] = &[
    Look {
        id: "bold",
        name: "Bold",
        font: "Anton",
        size: 230.0,
        color: 0xffffff,
        outline: 9.0,
        outline_color: 0x000000,
        shadow: 70.0,
        boxed: None,
        capitals: true,
    },
    Look {
        id: "yellow",
        name: "Yellow",
        font: "Anton",
        size: 230.0,
        color: 0xffd60a,
        outline: 9.0,
        outline_color: 0x000000,
        shadow: 70.0,
        boxed: None,
        capitals: true,
    },
    Look {
        id: "pop",
        name: "Pop",
        font: "Bangers",
        size: 230.0,
        color: 0xffffff,
        outline: 12.0,
        outline_color: 0x1a1a2e,
        shadow: 80.0,
        boxed: None,
        capitals: true,
    },
    Look {
        id: "clean",
        name: "Clean",
        font: "Poppins Black",
        size: 190.0,
        color: 0xffffff,
        outline: 0.0,
        outline_color: 0x000000,
        shadow: 85.0,
        boxed: None,
        capitals: false,
    },
    Look {
        id: "box",
        name: "Box",
        font: "Poppins Black",
        size: 180.0,
        color: 0xffffff,
        outline: 0.0,
        outline_color: 0x000000,
        shadow: 0.0,
        boxed: Some(0xe5231b),
        capitals: true,
    },
    Look {
        id: "marker",
        name: "Marker",
        font: "Permanent Marker",
        size: 200.0,
        color: 0xffe14d,
        outline: 0.0,
        outline_color: 0x000000,
        shadow: 85.0,
        boxed: None,
        capitals: false,
    },
];

/// Finds a look by its id or (ignoring case) its name.
pub fn look_named(name: &str) -> Option<usize> {
    let name = name.trim();
    LOOKS
        .iter()
        .position(|l| l.id.eq_ignore_ascii_case(name) || l.name.eq_ignore_ascii_case(name))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Across {
    Left,
    Centre,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Down {
    Top,
    Middle,
    Bottom,
}

/// Everything that decides how the thumbnail looks.
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    pub canvas: usize,
    pub text: String,
    pub look: usize,
    /// Lettering size, percent of the look's own size.
    pub size: f64,
    pub across: Across,
    pub down: Down,
    /// 0..=100: how much extra colour and contrast the picture gets.
    pub punch: f64,
    /// 0..=100: how dark the fade behind the lettering is.
    pub shade: f64,
    /// Which moment of a video (or of the timeline), in seconds.
    pub at: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            canvas: 0,
            text: String::new(),
            look: 0,
            size: 100.0,
            across: Across::Left,
            down: Down::Bottom,
            punch: 40.0,
            shade: 60.0,
            at: 0.0,
        }
    }
}

fn rgb(hex: u32, alpha: f32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        alpha,
    ]
}

/// How far the drawn lettering reaches from the title's centre, as
/// `[left, top, right, bottom]` in design pixels (negative to the left/up).
pub fn ink(spec: &TitleSpec) -> [f64; 4] {
    let scale = 0.25;
    let r = tidycuts_render::text::render_title(spec, scale, 1.0, None);
    let img = &r.image;
    let (w, h) = (img.width as usize, img.height as usize);
    let solid = |x: usize, y: usize| img.pixels[(y * w + x) * 4 + 3] > 24;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if solid(x, y) {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 || y1 <= y0 {
        let (hw, hh) = (r.size[0] as f64 * 0.5, r.size[1] as f64 * 0.5);
        return [-hw, -hh, hw, hh];
    }
    let (cx, cy) = (w as f64 * 0.5, h as f64 * 0.5);
    let s = scale as f64;
    [
        (x0 as f64 - cx) / s,
        (y0 as f64 - cy) / s,
        (x1 as f64 - cx) / s,
        (y1 as f64 - cy) / s,
    ]
}

/// The lettering for these settings, shrunk to fit the picture, and where
/// its centre goes (in design pixels from the picture's centre).
fn lettering(s: &Settings, canvas: &Canvas) -> (TitleSpec, [f64; 2]) {
    let look = &LOOKS[s.look.min(LOOKS.len() - 1)];
    let text = if look.capitals {
        s.text.to_uppercase()
    } else {
        s.text.clone()
    };
    let mut spec = TitleSpec {
        style: "plain".into(),
        text,
        font: look.font.into(),
        size: look.size * (s.size / 100.0).clamp(0.2, 3.0) as f32,
        color: rgb(look.color, 1.0),
        align: match s.across {
            Across::Left => TextAlign::Left,
            Across::Centre => TextAlign::Center,
            Across::Right => TextAlign::Right,
        },
        outline: look.outline,
        outline_color: rgb(look.outline_color, 1.0),
        shadow: look.shadow,
        background: look.boxed.is_some(),
        background_color: rgb(look.boxed.unwrap_or(0), 1.0),
        line_spacing: 1.0,
        ..TitleSpec::default()
    };
    // Design pixels: the picture as if it were 1080 pixels high.
    let k = canvas.height as f64 / 1080.0;
    let (wd, hd) = (canvas.width as f64 / k, canvas.height as f64 / k);
    let margin = 0.055 * wd.min(hd);
    let mut b = ink(&spec);
    let fit = ((wd - 2.0 * margin) / (b[2] - b[0]).max(1.0))
        .min((hd - 2.0 * margin) / (b[3] - b[1]).max(1.0));
    if fit < 1.0 {
        spec.size *= fit as f32;
        b = ink(&spec);
    }
    let x = match s.across {
        Across::Left => -wd / 2.0 + margin - b[0],
        Across::Centre => -(b[0] + b[2]) / 2.0,
        Across::Right => wd / 2.0 - margin - b[2],
    };
    let y = match s.down {
        Down::Top => -hd / 2.0 + margin - b[1],
        Down::Middle => -(b[1] + b[3]) / 2.0,
        Down::Bottom => hd / 2.0 - margin - b[3],
    };
    (spec, [x, y])
}

/// The dark fades behind the lettering: one from the side the words are on
/// and one from the top or bottom edge, or an even darkening when the words
/// are in the middle.
fn shade_clips(s: &Settings, canvas: &Canvas, len: Time) -> Vec<Clip> {
    if s.shade <= 0.0 || s.text.trim().is_empty() {
        return Vec::new();
    }
    let (w, h) = (canvas.width as f64, canvas.height as f64);
    // How much of the picture a fade covers.
    let cover = 0.78;
    let side = s.across != Across::Centre;
    let edge = s.down != Down::Middle;
    let strength = s.shade / 100.0 * 0.85;
    let mut clips = Vec::new();
    if !side && !edge {
        let dim = rgb(0, (strength * 0.5) as f32);
        clips.push(Clip::generated(Generator::Solid(dim), Time::ZERO, len));
    }
    // Two fades overlap in the corner, so each is a little lighter.
    let alpha = if side && edge {
        strength * 0.8
    } else {
        strength
    } as f32;
    let (dark, clear) = (rgb(0, alpha), rgb(0, 0.0));
    if side {
        let left = s.across == Across::Left;
        let mut c = Clip::generated(
            Generator::Gradient {
                a: if left { dark } else { clear },
                b: if left { clear } else { dark },
                angle: 0.0,
            },
            Time::ZERO,
            len,
        );
        c.transform.scale_x = Animatable::constant(cover * 100.0);
        let shift = (1.0 - cover) * w / 2.0;
        c.transform.position_x = Animatable::constant(if left { -shift } else { shift });
        clips.push(c);
    }
    if edge {
        let top = s.down == Down::Top;
        let mut c = Clip::generated(
            Generator::Gradient {
                a: if top { dark } else { clear },
                b: if top { clear } else { dark },
                angle: 90.0,
            },
            Time::ZERO,
            len,
        );
        c.transform.scale_y = Animatable::constant(cover * 100.0);
        let shift = (1.0 - cover) * h / 2.0;
        c.transform.position_y = Animatable::constant(if top { -shift } else { shift });
        clips.push(c);
    }
    for c in &mut clips {
        c.name = Some("Fade".into());
    }
    clips
}

/// The thumbnail as a project, with `base` (a picture or a video) filling
/// the frame.
pub fn build(s: &Settings, base: &MediaItem) -> Project {
    let canvas = &CANVASES[s.canvas.min(CANVASES.len() - 1)];
    let mut p = Project::new("Thumbnail");
    p.settings.width = canvas.width;
    p.settings.height = canvas.height;
    p.settings.magnetic = false;
    let len = Time::from_millis(1000);
    let media = p.add_media(base.clone());
    let main = p
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .map(|t| t.id)
        .expect("a new project has a video track");

    let mut pic = Clip::new(base, Time::ZERO);
    pic.media_id = Some(media);
    if base.kind == MediaKind::Video {
        let last = (base.duration.as_secs_f64() - 0.1).max(0.0);
        let at = Time::from_secs_f64(s.at.clamp(0.0, last));
        pic.source_in = at;
        pic.source_out = (at + len)
            .min(base.duration)
            .max(at + Time::from_millis(100));
    }
    let punch = s.punch.clamp(0.0, 100.0);
    pic.color.contrast = Animatable::constant(punch * 0.22);
    pic.color.saturation = Animatable::constant(punch * 0.30);
    pic.color.vibrance = Animatable::constant(punch * 0.25);
    if let Some(id) = p.place_clip(pic, main, Time::ZERO) {
        p.crop_to_fit(id);
    }

    for fade in shade_clips(s, canvas, len) {
        if let Some(t) = p.add_track(TrackKind::Video) {
            p.place_clip(fade, t, Time::ZERO);
        }
    }
    if !s.text.trim().is_empty() {
        let (spec, [x, y]) = lettering(s, canvas);
        let k = canvas.height as f64 / 1080.0;
        let mut title = Clip::generated(Generator::Title(spec), Time::ZERO, len);
        title.transform.position_x = Animatable::constant(x * k);
        title.transform.position_y = Animatable::constant(y * k);
        if let Some(t) = p.add_track(TrackKind::Video) {
            p.place_clip(title, t, Time::ZERO);
        }
    }
    p
}

/// Draws the thumbnail `width` pixels wide (its full size when `None`).
pub fn render(
    project: &Project,
    width: Option<u32>,
    renderer: Arc<Renderer>,
    analysis: &Analysis,
) -> Picture {
    let statics = Statics::default();
    let analysis = analysis.clone();
    let auto = move |m: &MediaItem| analysis.auto_grade(m);
    snapshot::render(project, Time::ZERO, width, renderer, &statics, &auto)
}

/// A flat picture of the timeline at `at`, as a photo the thumbnail can
/// use. `dir` is a folder for the temporary file.
pub fn picture_of_timeline(
    timeline: &Project,
    at: Time,
    dir: &Path,
    renderer: Arc<Renderer>,
    analysis: &Analysis,
) -> Result<MediaItem, String> {
    if timeline.duration() == Time::ZERO {
        return Err("The timeline is empty.".into());
    }
    let at = at.min(timeline.duration() - Time::from_millis(1));
    let statics = Statics::default();
    let a = analysis.clone();
    let auto = move |m: &MediaItem| a.auto_grade(m);
    let pic = snapshot::render(timeline, at, Some(1920), renderer, &statics, &auto);
    let path = dir.join(format!("timeline-{}.png", uuid::Uuid::new_v4().simple()));
    snapshot::write_png(&path, pic.width, pic.height, &pic.rgba)?;
    tidycuts_media::probe(&path).map_err(|e| e.to_string())
}

/// Writes the finished thumbnail to `path`: a JPG when the name ends in
/// .jpg or .jpeg, otherwise a PNG.
pub fn save(pic: &Picture, path: &Path) -> Result<(), String> {
    let jpg = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"));
    if !jpg {
        return snapshot::write_png(path, pic.width, pic.height, &pic.rgba);
    }
    // The PNG goes through ffmpeg, which TidyCuts already needs.
    let tmp = path.with_extension("tidycuts-tmp.png");
    snapshot::write_png(&tmp, pic.width, pic.height, &pic.rgba)?;
    let result = (|| {
        let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
        let out = tools
            .ffmpeg_cmd()
            .args(["-y", "-i"])
            .arg(&tmp)
            .args(["-q:v", "2", "-pix_fmt", "yuvj420p"])
            .arg(path)
            .output()
            .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "Could not make the JPG: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

/// The first "<stem> N.<ext>" in `dir` that doesn't exist yet.
pub fn free_name(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    (1..)
        .map(|n| dir.join(format!("{stem} {n}.{ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}.{ext}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(w: u32, h: u32) -> MediaItem {
        let mut m = MediaItem::new(
            PathBuf::from("/nonexistent/photo.png"),
            MediaKind::Image,
            MediaItem::DEFAULT_IMAGE_DURATION,
        );
        m.width = w;
        m.height = h;
        m
    }

    fn settings(text: &str) -> Settings {
        Settings {
            text: text.into(),
            ..Settings::default()
        }
    }

    #[test]
    fn looks_can_be_found_by_id_or_name() {
        assert_eq!(look_named("Yellow"), Some(1));
        assert_eq!(look_named(" pop "), Some(2));
        assert_eq!(look_named("nothing like it"), None);
    }

    #[test]
    fn a_thumbnail_is_a_filled_picture_with_a_fade_and_the_words() {
        let p = build(&settings("Big news"), &photo(1920, 1080));
        assert_eq!((p.settings.width, p.settings.height), (1280, 720));
        let video: Vec<_> = p
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .collect();
        // The picture, one fade down the side and one along the bottom, the words.
        assert_eq!(video.len(), 4);
        let all: Vec<&Clip> = p.tracks.iter().flat_map(|t| t.clips.iter()).collect();
        assert!(all
            .iter()
            .any(|c| matches!(c.generator, Some(Generator::Title(_)))));
        assert!(all
            .iter()
            .any(|c| matches!(c.generator, Some(Generator::Gradient { .. }))));
        let pic = all.iter().find(|c| c.media_id.is_some()).unwrap();
        // A wide photo on a wide picture: no bars, so no zoom beyond fit.
        assert!((pic.transform.scale_x.value - 100.0).abs() < 0.5);
    }

    #[test]
    fn a_tall_photo_is_zoomed_to_fill_a_wide_picture() {
        let p = build(&settings(""), &photo(1000, 2000));
        let pic = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.media_id.is_some())
            .unwrap();
        assert!(pic.transform.scale_x.value > 300.0);
        // No words, no fade.
        assert_eq!(
            p.tracks
                .iter()
                .filter(|t| t.kind == TrackKind::Video)
                .count(),
            1
        );
    }

    #[test]
    fn a_video_uses_the_moment_asked_for() {
        let mut v = photo(1920, 1080);
        v.kind = MediaKind::Video;
        v.duration = Time::from_millis(20_000);
        let mut s = settings("");
        s.at = 12.5;
        let p = build(&s, &v);
        let c = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.media_id.is_some())
            .unwrap();
        assert!((c.source_in.as_secs_f64() - 12.5).abs() < 1e-6);
        assert!(c.duration() > Time::ZERO);
        // Asking for a moment past the end falls back to the last bit.
        s.at = 99.0;
        let c = build(&s, &v);
        let c = c
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.media_id.is_some())
            .unwrap()
            .clone();
        assert!(c.source_in < v.duration && c.source_out <= v.duration);
    }

    #[test]
    fn lettering_stays_inside_the_picture_on_every_side() {
        for (canvas, c) in CANVASES.iter().enumerate() {
            for across in [Across::Left, Across::Centre, Across::Right] {
                for down in [Down::Top, Down::Middle, Down::Bottom] {
                    let mut s = settings("A very long headline that has to be made smaller");
                    s.canvas = canvas;
                    s.across = across;
                    s.down = down;
                    let (spec, [x, y]) = lettering(&s, c);
                    let k = c.height as f64 / 1080.0;
                    let (wd, hd) = (c.width as f64 / k, c.height as f64 / k);
                    let b = ink(&spec);
                    assert!(x + b[0] >= -wd / 2.0 - 1.0, "{across:?} {down:?} left");
                    assert!(x + b[2] <= wd / 2.0 + 1.0, "{across:?} {down:?} right");
                    assert!(y + b[1] >= -hd / 2.0 - 1.0, "{across:?} {down:?} top");
                    assert!(y + b[3] <= hd / 2.0 + 1.0, "{across:?} {down:?} bottom");
                }
            }
        }
    }

    #[test]
    fn the_words_sit_on_the_side_chosen() {
        let c = &CANVASES[0];
        let mut s = settings("Left or right");
        s.across = Across::Left;
        let (_, [left, _]) = lettering(&s, c);
        s.across = Across::Right;
        let (_, [right, _]) = lettering(&s, c);
        s.across = Across::Centre;
        let (_, [mid, _]) = lettering(&s, c);
        assert!(left < mid && mid < right);
    }

    #[test]
    fn a_jpg_is_made_when_the_name_says_so() {
        let dir = std::env::temp_dir().join(format!("tidycuts-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let px: Vec<u8> = (0..64 * 36)
            .flat_map(|i| [(i % 255) as u8, 90, 160, 255])
            .collect();
        let pic = Picture {
            width: 64,
            height: 36,
            rgba: px,
        };
        let jpg = dir.join("t.jpg");
        save(&pic, &jpg).unwrap();
        let bytes = std::fs::read(&jpg).unwrap();
        assert_eq!(
            &bytes[..3],
            &[0xff, 0xd8, 0xff],
            "a JPG starts with FF D8 FF"
        );
        let png = dir.join("t.png");
        save(&pic, &png).unwrap();
        assert_eq!(&std::fs::read(&png).unwrap()[..4], b"\x89PNG");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
