//! The export window, laid out the usual way: file formats on the left,
//! the file name, folder and encoding settings on the right, and the
//! length and estimated size at the bottom. Then a progress window that
//! follows `app.export_job` until the file is written.

use super::sizes::{
    export_sizes, fmt_bytes, fmt_clock, fmt_position, parse_position, rate_label, same_rate,
    sanitize_file_name, scale_to_short, size_label, stem_of, unique_path, FRAME_RATES, GIF_RATES,
};
use super::{field, footer, header, primary, reveal, secondary, switch, window};
use crate::app::TidyCutsApp;
use crate::prefs::Prefs;
use crate::{theme, widgets};
use egui::{Color32, CornerRadius, Margin, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::Duration;
use tidycuts_core::{FrameRate, MediaKind, Project, Time};
use tidycuts_engine::export::{
    self as ex, AudioCodec, Container, ExportProgress, ExportSettings, Quality, VideoCodec,
};

// ----- encoders ----------------------------------------------------------

static PROBE: Once = Once::new();
static PROBED: AtomicBool = AtomicBool::new(false);

/// The encoders that work on this computer, once they have been tried.
/// The first call starts trying them on a worker thread (it takes a moment
/// and must not freeze the window).
pub(super) fn working_encoders(ctx: &egui::Context) -> Option<&'static [String]> {
    PROBE.call_once(|| {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = ex::working_encoders();
            PROBED.store(true, Ordering::Release);
            ctx.request_repaint();
        });
    });
    PROBED
        .load(Ordering::Acquire)
        .then(|| ex::working_encoders().as_slice())
}

/// The codec an ffmpeg encoder writes.
fn codec_of(encoder: &str) -> Option<VideoCodec> {
    let e = encoder;
    Some(
        if e.starts_with("h264") || e == "libx264" || e == "libopenh264" {
            VideoCodec::H264
        } else if e.starts_with("hevc") || e == "libx265" || e == "libkvazaar" {
            VideoCodec::Hevc
        } else if e.starts_with("av1") || e.contains("av1") || e == "librav1e" {
            VideoCodec::Av1
        } else if e.starts_with("vp9") || e == "libvpx-vp9" {
            VideoCodec::Vp9
        } else if e.starts_with("prores") {
            VideoCodec::ProRes
        } else if e.starts_with("mpeg2") {
            VideoCodec::Mpeg2
        } else if e == "mpeg4" {
            VideoCodec::Mpeg4
        } else if e == "wmv2" {
            VideoCodec::Wmv2
        } else if e == "gif" {
            VideoCodec::Gif
        } else {
            return None;
        },
    )
}

/// Whether an encoder runs on the graphics card (or other media hardware).
pub(super) fn is_gpu(encoder: &str) -> bool {
    ["nvenc", "qsv", "amf", "videotoolbox", "_mf"]
        .iter()
        .any(|k| encoder.contains(k))
}

/// A readable name for an ffmpeg encoder.
pub(super) fn encoder_label(encoder: &str) -> String {
    let vendor = if encoder.contains("nvenc") {
        "NVIDIA NVENC"
    } else if encoder.contains("qsv") {
        "Intel Quick Sync"
    } else if encoder.contains("amf") {
        "AMD AMF"
    } else if encoder.contains("videotoolbox") {
        "Apple VideoToolbox"
    } else if encoder.ends_with("_mf") {
        "Windows Media Foundation"
    } else {
        match encoder {
            "libx264" => "x264",
            "libx265" => "x265",
            "libopenh264" => "OpenH264",
            "libkvazaar" => "Kvazaar",
            "libsvtav1" => "SVT-AV1",
            "libaom-av1" => "libaom",
            "librav1e" => "rav1e",
            "libvpx-vp9" => "libvpx",
            "prores_ks" => "ProRes (Kostya)",
            "mpeg2video" => "MPEG-2",
            "mpeg4" => "MPEG-4",
            "wmv2" => "WMV 8",
            "gif" => "GIF",
            other => other,
        }
    };
    format!("{vendor} · {}", if is_gpu(encoder) { "GPU" } else { "CPU" })
}

/// Encoders for `codec` that may be used (only CPU ones without `gpu`).
fn encoders_for(codec: VideoCodec, working: &[String], gpu: bool) -> Vec<String> {
    working
        .iter()
        .filter(|e| codec_of(e) == Some(codec) && (gpu || !is_gpu(e)))
        .cloned()
        .collect()
}

// ----- estimates ---------------------------------------------------------

/// Whether anything in the project makes sound (the export leaves the
/// sound out otherwise).
fn project_has_sound(project: &Project) -> bool {
    project.tracks.iter().any(|t| {
        t.clips.iter().any(|c| {
            c.media_id
                .and_then(|m| project.media(m))
                .is_some_and(|m| m.kind == MediaKind::Audio || (m.has_audio && !c.audio.muted))
        })
    })
}

/// The audio bitrate a file will have, in kbps.
fn audio_kbps(s: &ExportSettings, has_sound: bool) -> f64 {
    if !has_sound {
        return 0.0;
    }
    let pcm = s.sample_rate as f64 * 2.0 * 16.0 / 1000.0;
    match s.container.audio_codec() {
        AudioCodec::None => 0.0,
        AudioCodec::Pcm => pcm,
        AudioCodec::Flac => pcm * 0.55,
        _ => s.audio_kbps as f64,
    }
}

/// The video bitrate a file will have, roughly, in kbps.
fn video_kbps(s: &ExportSettings) -> f64 {
    if s.container.is_audio_only() {
        return 0.0;
    }
    let px_rate = s.width as f64 * s.height as f64 * s.frame_rate.as_f64();
    if s.container == Container::Gif {
        // A palette and frame differences: very roughly 0.15 bytes a pixel.
        return px_rate * 0.15 * 8.0 / 1000.0;
    }
    if s.codec == VideoCodec::ProRes {
        // ProRes 422 HQ runs at about 3.5 bits a pixel.
        return px_rate * 3.5 / 1000.0;
    }
    if let Some(k) = s.bitrate_kbps {
        return k as f64;
    }
    let efficiency = match s.codec {
        VideoCodec::Hevc => 0.65,
        VideoCodec::Av1 => 0.55,
        VideoCodec::Vp9 => 0.7,
        VideoCodec::Mpeg2 => 1.6,
        VideoCodec::Mpeg4 | VideoCodec::Wmv2 => 1.4,
        _ => 1.0,
    };
    s.suggested_kbps() as f64 * efficiency
}

/// About how many bytes an export of `secs` seconds will take.
fn estimate_bytes(s: &ExportSettings, secs: f64, has_sound: bool) -> u64 {
    let kbps = video_kbps(s) + audio_kbps(s, has_sound);
    (kbps * 1000.0 / 8.0 * secs.max(0.0)) as u64
}

/// How far along the whole export is, 0..1, from the stage it reports.
/// Mixing the sound comes first and is quick next to rendering the video.
/// How far a running export is, as the progress window shows it.
pub fn job_fraction(job: &crate::app::ExportJob, project: &Project) -> f32 {
    let p = job.progress.lock().map(|p| p.clone()).unwrap_or_default();
    overall_fraction(
        &p,
        job.settings.container.is_audio_only(),
        project_has_sound(project),
    )
}

fn overall_fraction(p: &ExportProgress, audio_only: bool, has_sound: bool) -> f32 {
    let f = if p.total == 0 {
        0.0
    } else {
        (p.done as f64 / p.total as f64).clamp(0.0, 1.0) as f32
    };
    match (p.stage, audio_only) {
        ("Mixing audio", true) => 0.9 * f,
        ("Encoding audio", _) => 0.95,
        ("Mixing audio", false) => 0.08 * f,
        ("Rendering video", _) if has_sound => 0.08 + 0.92 * f,
        ("Rendering video", _) => f,
        _ => 0.0,
    }
}

/// Time still to go: the renderer's own guess while rendering, otherwise
/// worked out from the time taken so far.
fn remaining(p: &ExportProgress, fraction: f32, elapsed: Duration) -> Option<Duration> {
    if p.stage == "Rendering video" {
        if let Some(eta) = p.eta {
            return Some(eta);
        }
    }
    (fraction > 0.02).then(|| elapsed.mul_f32((1.0 - fraction) / fraction))
}

// ----- the settings window -----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RangeMode {
    Whole,
    Selected,
    Custom,
}

/// Ready-made settings for the places people post videos. Sizes never grow
/// beyond the project's own, and the picture keeps the project's shape.
struct Preset {
    name: &'static str,
    detail: &'static str,
    icon: &'static str,
    container: Container,
    codec: VideoCodec,
    /// The shorter side of the picture, in pixels, at most.
    short_edge: u32,
    /// Frames per second, at most.
    fps: u32,
    quality: Quality,
    audio_kbps: u32,
}

const PRESETS: [Preset; 6] = [
    Preset {
        name: "Widescreen HD",
        detail: "1080p",
        icon: ph::FILM_SLATE,
        container: Container::Mp4,
        codec: VideoCodec::H264,
        short_edge: 1080,
        fps: 60,
        quality: Quality::Recommended,
        audio_kbps: 192,
    },
    Preset {
        name: "Widescreen 4K",
        detail: "4K",
        icon: ph::FILM_SLATE,
        container: Container::Mp4,
        codec: VideoCodec::H264,
        short_edge: 2160,
        fps: 60,
        quality: Quality::Higher,
        audio_kbps: 256,
    },
    Preset {
        name: "Vertical phone",
        detail: "30 fps",
        icon: ph::DEVICE_MOBILE,
        container: Container::Mp4,
        codec: VideoCodec::H264,
        short_edge: 1080,
        fps: 30,
        quality: Quality::Recommended,
        audio_kbps: 128,
    },
    Preset {
        name: "Square post",
        detail: "30 fps",
        icon: ph::SQUARES_FOUR,
        container: Container::Mp4,
        codec: VideoCodec::H264,
        short_edge: 1080,
        fps: 30,
        quality: Quality::Recommended,
        audio_kbps: 128,
    },
    Preset {
        name: "Small file",
        detail: "540p",
        icon: ph::CHAT_CIRCLE,
        container: Container::Mp4,
        codec: VideoCodec::H264,
        short_edge: 540,
        fps: 30,
        quality: Quality::Lower,
        audio_kbps: 96,
    },
    Preset {
        name: "Web video",
        detail: "WEBM",
        icon: ph::GLOBE,
        container: Container::Webm,
        codec: VideoCodec::Vp9,
        short_edge: 1080,
        fps: 30,
        quality: Quality::Recommended,
        audio_kbps: 128,
    },
];

const FORMAT_GROUPS: [(&str, &[Container]); 3] = [
    (
        "Video",
        &[
            Container::Mp4,
            Container::Mov,
            Container::Mkv,
            Container::Webm,
            Container::Avi,
            Container::Wmv,
            Container::Mpg,
            Container::Ts,
        ],
    ),
    ("Animation", &[Container::Gif]),
    (
        "Audio only",
        &[
            Container::Mp3,
            Container::Wav,
            Container::M4a,
            Container::Flac,
        ],
    ),
];

fn format_detail(c: Container) -> &'static str {
    match c {
        Container::Mp4 => "H.264 · HEVC · AV1",
        Container::Mov => "H.264 · ProRes",
        Container::Mkv => "Any codec",
        Container::Webm => "VP9 · AV1",
        Container::Avi => "MPEG-4",
        Container::Wmv => "Windows",
        Container::Mpg => "DVD",
        Container::Ts => "Broadcast",
        Container::Gif => "Animated",
        Container::Mp3 => "Lossy",
        Container::Wav => "Uncompressed",
        Container::M4a => "AAC",
        Container::Flac => "Lossless",
    }
}

fn format_icon(c: Container) -> &'static str {
    if c == Container::Gif {
        ph::GIF
    } else if c.is_audio_only() {
        ph::MUSIC_NOTES
    } else {
        ph::FILM_STRIP
    }
}

const AUDIO_BITRATES: [u32; 6] = [96, 128, 160, 192, 256, 320];

/// Where exports go when nothing else is known: the Videos folder.
fn videos_folder() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let videos = home.join(if cfg!(target_os = "macos") {
        "Movies"
    } else {
        "Videos"
    });
    if videos.is_dir() {
        videos
    } else {
        home
    }
}

#[derive(Clone)]
pub struct ExportDialog {
    name: String,
    folder: PathBuf,
    s: ExportSettings,
    range_mode: RangeMode,
    /// The part of the timeline picked before opening (selected clips).
    selected: Option<(Time, Time)>,
    /// Custom in and out points, in seconds.
    custom: (f64, f64),
    custom_bitrate: bool,
    /// The preset picked last, until something it set is changed by hand.
    preset: Option<usize>,
    project_size: (u32, u32),
    project_rate: FrameRate,
    /// The picture at the top: a real frame of the timeline.
    preview: super::export_preview::ExportPreview,
}

impl ExportDialog {
    pub fn new(project: &Project, prefs: &Prefs, project_path: Option<&Path>) -> Self {
        let folder = prefs
            .export_folder
            .clone()
            .filter(|f| f.is_dir())
            .or_else(|| project_path.and_then(|p| p.parent().map(Path::to_path_buf)))
            .unwrap_or_else(videos_folder);
        let base = project_path
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.name.clone());
        let base = sanitize_file_name(&base);
        let mut s = ExportSettings::for_project(project, PathBuf::new());
        s.hardware_encode = prefs.hardware_encode;
        s.hardware_decode = prefs.hardware_decode;
        s.sample_rate = if project.settings.sample_rate == 44_100 {
            44_100
        } else {
            48_000
        };
        let name = stem_of(&unique_path(&folder, &base, s.container.extension()));
        ExportDialog {
            name,
            folder,
            s,
            range_mode: RangeMode::Whole,
            selected: None,
            custom: (0.0, project.duration().as_secs_f64()),
            custom_bitrate: false,
            preset: None,
            project_size: (project.settings.width, project.settings.height),
            project_rate: project.settings.frame_rate,
            preview: Default::default(),
        }
    }

    pub fn set_selected_range(&mut self, range: (Time, Time)) {
        if range.1 > range.0 {
            self.selected = Some(range);
            self.range_mode = RangeMode::Selected;
            self.custom = (range.0.as_secs_f64(), range.1.as_secs_f64());
        }
    }

    fn set_container(&mut self, c: Container, working: Option<&[String]>) {
        let was_gif = self.s.container == Container::Gif;
        self.s.container = c;
        let codecs = c.video_codecs();
        if !codecs.is_empty() && !codecs.contains(&self.s.codec) {
            self.s.codec = codecs
                .iter()
                .copied()
                .find(|k| working.is_none_or(|w| !encoders_for(*k, w, true).is_empty()))
                .unwrap_or(codecs[0]);
        }
        if self
            .s
            .encoder
            .as_deref()
            .is_some_and(|e| codec_of(e) != Some(self.s.codec))
        {
            self.s.encoder = None;
        }
        let (pw, ph) = self.project_size;
        if c == Container::Gif && !was_gif {
            let (w, h) = scale_to_short(pw, ph, 480);
            if w < pw {
                (self.s.width, self.s.height) = (w, h);
            }
            self.s.frame_rate = FrameRate::new(15, 1);
        } else if was_gif && c != Container::Gif {
            (self.s.width, self.s.height) = (pw, ph);
            self.s.frame_rate = self.project_rate;
        }
        if c.audio_codec() == AudioCodec::Opus {
            self.s.sample_rate = 48_000;
        }
    }

    /// The part of the timeline that will be exported.
    fn range(&self, app: &TidyCutsApp) -> Option<(Time, Time)> {
        match self.range_mode {
            RangeMode::Whole => None,
            RangeMode::Selected => self.selected.or_else(|| selection_range(app)),
            RangeMode::Custom => Some((
                Time::from_secs_f64(self.custom.0),
                Time::from_secs_f64(self.custom.1),
            )),
        }
    }

    fn path(&self) -> PathBuf {
        self.folder.join(format!(
            "{}.{}",
            sanitize_file_name(&self.name),
            self.s.container.extension()
        ))
    }

    /// The finished settings to hand to the exporter.
    fn settings(&self, app: &TidyCutsApp) -> ExportSettings {
        let mut s = self.s.clone();
        s.path = self.path();
        s.range = self.range(app);
        if !self.custom_bitrate {
            s.bitrate_kbps = None;
        }
        s
    }

    fn apply_preset(&mut self, i: usize, working: Option<&[String]>) {
        let p = &PRESETS[i];
        self.set_container(p.container, working);
        if p.container.video_codecs().contains(&p.codec)
            && working.is_none_or(|w| !encoders_for(p.codec, w, true).is_empty())
        {
            self.s.codec = p.codec;
            self.s.encoder = None;
        }
        let (pw, ph) = self.project_size;
        let (w, h) = if pw.min(ph) > p.short_edge {
            scale_to_short(pw, ph, p.short_edge)
        } else {
            (pw, ph)
        };
        (self.s.width, self.s.height) = (w, h);
        self.s.frame_rate = if self.project_rate.as_f64() > f64::from(p.fps) {
            FrameRate::new(p.fps, 1)
        } else {
            self.project_rate
        };
        self.s.quality = p.quality;
        self.s.audio_kbps = p.audio_kbps;
        self.custom_bitrate = false;
        self.preset = Some(i);
    }

    fn formats(&mut self, ui: &mut egui::Ui, working: Option<&[String]>) {
        ui.add_space(4.0);
        widgets::group_title(ui, "Presets");
        ui.add_space(2.0);
        for (i, p) in PRESETS.iter().enumerate() {
            let r = widgets::list_item(ui, self.preset == Some(i), p.icon, p.name, p.detail);
            if r.clicked() {
                self.apply_preset(i, working);
            }
        }
        ui.add_space(6.0);
        for (title, list) in FORMAT_GROUPS {
            ui.add_space(4.0);
            widgets::group_title(ui, title);
            ui.add_space(2.0);
            for c in list {
                let r = widgets::list_item(
                    ui,
                    self.s.container == *c,
                    format_icon(*c),
                    c.label(),
                    format_detail(*c),
                );
                if r.clicked() && self.s.container != *c {
                    self.set_container(*c, working);
                    self.preset = None;
                }
            }
            ui.add_space(6.0);
        }
    }

    fn file_rows(&mut self, ui: &mut egui::Ui, app: &TidyCutsApp) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            // A picture of the frame at the playhead.
            let (thumb, _) = ui.allocate_exact_size(egui::vec2(176.0, 99.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(thumb, CornerRadius::same(8), Color32::BLACK);
            let (pw, ph) = self.project_size;
            let a = pw.max(1) as f32 / ph.max(1) as f32;
            let mut size = thumb.size();
            if size.x / size.y > a {
                size.x = size.y * a;
            } else {
                size.y = size.x / a;
            }
            let pic = Rect::from_center_size(thumb.center(), size);
            if self.s.container.is_audio_only() {
                p.text(
                    thumb.center(),
                    egui::Align2::CENTER_CENTER,
                    ph::WAVEFORM,
                    egui::FontId::proportional(40.0),
                    theme::AUDIO_WAVE,
                );
            } else {
                // A real frame of what will be exported (the selected part when
                // that is what is exported).
                let at = match (self.range_mode, self.selected) {
                    (RangeMode::Selected, Some((a, b))) => super::export_preview::good_moment(
                        &app.project,
                        Time(a.0 + (b.0 - a.0) / 2),
                    ),
                    _ => super::export_preview::good_moment(&app.project, app.playhead()),
                };
                let drawn = self.preview.paint(ui.ctx(), app, at, p, pic);
                if !drawn {
                    // Still drawing (or no graphics device): the live player's picture.
                    if let Some(tex) = app.engine.as_ref().and_then(|e| e.texture) {
                        p.image(
                            tex,
                            pic,
                            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                }
            }
            p.rect_stroke(
                thumb,
                CornerRadius::same(8),
                Stroke::new(1.0, theme::BORDER),
                StrokeKind::Inside,
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.add_space(6.0);
                widgets::labeled_row(ui, "Name", 64.0, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .desired_width(300.0)
                            .hint_text("File name"),
                    );
                    ui.label(
                        RichText::new(format!(".{}", self.s.container.extension()))
                            .color(theme::TEXT_DIM),
                    );
                });
                widgets::labeled_row(ui, "Save to", 64.0, |ui| {
                    widgets::path_box(ui, &self.folder.display().to_string(), 300.0);
                    if ui
                        .add(
                            theme::outline_button(ph::FOLDER_OPEN).min_size(egui::vec2(34.0, 28.0)),
                        )
                        .on_hover_text("Choose the folder")
                        .clicked()
                    {
                        let mut d = rfd::FileDialog::new().set_title("Save the export in");
                        if self.folder.is_dir() {
                            d = d.set_directory(&self.folder);
                        }
                        if let Some(f) = d.pick_folder() {
                            self.folder = f;
                        }
                    }
                });
            });
        });
    }

    fn video_rows(&mut self, ui: &mut egui::Ui, working: Option<&[String]>) {
        let s = &mut self.s;
        widgets::group_title(ui, "Video");
        field(ui, "Quality", |ui| {
            let mut i = match s.quality {
                Quality::Higher => 0,
                Quality::Recommended => 1,
                Quality::Lower => 2,
            };
            ui.scope(|ui| {
                ui.set_max_width(360.0);
                if widgets::segmented(ui, &mut i, &["Higher", "Recommended", "Lower"]) {
                    s.quality = [Quality::Higher, Quality::Recommended, Quality::Lower][i];
                }
            });
        });
        field(ui, "Resolution", |ui| {
            let (pw, ph) = self.project_size;
            let mut sizes = export_sizes(pw, ph);
            if !sizes.contains(&(s.width, s.height)) {
                sizes.push((s.width, s.height));
            }
            let labels: Vec<((u32, u32), String)> = sizes
                .iter()
                .map(|&(w, h)| {
                    let l = if (w, h) == (pw, ph) {
                        format!("{} (project)", size_label(w, h))
                    } else {
                        size_label(w, h)
                    };
                    ((w, h), l)
                })
                .collect();
            let refs: Vec<((u32, u32), &str)> =
                labels.iter().map(|(v, l)| (*v, l.as_str())).collect();
            let mut v = (s.width, s.height);
            if widgets::choice(ui, "ex-size", &mut v, &refs) {
                (s.width, s.height) = v;
            }
        });
        field(ui, "Frame rate", |ui| {
            let list: &[(FrameRate, &str)] = if s.container == Container::Gif {
                &GIF_RATES
            } else {
                &FRAME_RATES
            };
            let mut options: Vec<(FrameRate, String)> = vec![(
                self.project_rate,
                format!("{} (project)", rate_label(self.project_rate)),
            )];
            for (r, l) in list {
                if !same_rate(*r, self.project_rate) {
                    options.push((*r, l.to_string()));
                }
            }
            if !options.iter().any(|(r, _)| same_rate(*r, s.frame_rate)) {
                options.push((s.frame_rate, rate_label(s.frame_rate)));
            }
            let mut v = options
                .iter()
                .find(|(r, _)| same_rate(*r, s.frame_rate))
                .map(|(r, _)| *r)
                .unwrap_or(s.frame_rate);
            let refs: Vec<(FrameRate, &str)> =
                options.iter().map(|(r, l)| (*r, l.as_str())).collect();
            if widgets::choice(ui, "ex-fps", &mut v, &refs) {
                s.frame_rate = v;
            }
        });
        if s.container != Container::Gif {
            field(ui, "Video codec", |ui| {
                egui::ComboBox::from_id_salt("ex-codec")
                    .selected_text(s.codec.label())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for k in s.container.video_codecs() {
                            let ok = working.is_none_or(|w| !encoders_for(*k, w, true).is_empty());
                            let label = if ok {
                                k.label().to_string()
                            } else {
                                format!("{} (not on this computer)", k.label())
                            };
                            let r =
                                ui.add_enabled(ok, egui::Button::selectable(s.codec == *k, label));
                            if r.clicked() && s.codec != *k {
                                s.codec = *k;
                                s.encoder = None;
                            }
                        }
                    });
            });
            field(ui, "Encoder", |ui| {
                let choices = working
                    .map(|w| encoders_for(s.codec, w, s.hardware_encode))
                    .unwrap_or_default();
                let auto = match (working, s.encoder.is_none()) {
                    (Some(_), true) => match ex::pick_encoder(s) {
                        Some(e) => format!("Auto ({})", encoder_label(&e)),
                        None => "Auto (none available)".into(),
                    },
                    (None, _) => "Auto (checking…)".into(),
                    _ => "Auto".into(),
                };
                let shown = match &s.encoder {
                    Some(e) => encoder_label(e),
                    None => auto.clone(),
                };
                egui::ComboBox::from_id_salt("ex-encoder")
                    .selected_text(shown)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(s.encoder.is_none(), auto).clicked() {
                            s.encoder = None;
                        }
                        for e in &choices {
                            let on = s.encoder.as_deref() == Some(e.as_str());
                            if ui
                                .selectable_label(on, encoder_label(e))
                                .on_hover_text(e.as_str())
                                .clicked()
                            {
                                s.encoder = Some(e.clone());
                            }
                        }
                    });
                ui.add_space(10.0);
                let mut gpu = s.hardware_encode;
                let has_gpu = working
                    .is_none_or(|w| w.iter().any(|e| is_gpu(e) && codec_of(e) == Some(s.codec)));
                ui.scope(|ui| switch(ui, &mut gpu, "GPU acceleration"))
                    .response
                    .on_hover_text(if has_gpu {
                        "Use the graphics card's encoder when it can: much faster, slightly larger files"
                    } else {
                        "No graphics card encoder for this codec was found, so the CPU is used"
                    });
                if gpu != s.hardware_encode {
                    s.hardware_encode = gpu;
                    if !gpu && s.encoder.as_deref().is_some_and(is_gpu) {
                        s.encoder = None;
                    }
                }
            });
            field(ui, "Bitrate", |ui| {
                let mut i = usize::from(self.custom_bitrate);
                ui.scope(|ui| {
                    ui.set_max_width(170.0);
                    if widgets::segmented(ui, &mut i, &["Auto", "Custom"]) {
                        self.custom_bitrate = i == 1;
                        if self.custom_bitrate && s.bitrate_kbps.is_none() {
                            s.bitrate_kbps = Some(s.suggested_kbps());
                        }
                    }
                });
                ui.add_space(6.0);
                let suggested = s.suggested_kbps();
                if self.custom_bitrate {
                    let mut k = s.bitrate_kbps.unwrap_or(suggested);
                    if ui
                        .add_sized(
                            [110.0, 26.0],
                            egui::DragValue::new(&mut k)
                                .range(100..=400_000)
                                .speed(50.0)
                                .suffix(" kbps"),
                        )
                        .changed()
                    {
                        s.bitrate_kbps = Some(k);
                    }
                    ui.label(
                        RichText::new(format!("suggested {suggested} kbps"))
                            .size(12.0)
                            .color(theme::TEXT_DIM),
                    );
                } else {
                    ui.label(
                        RichText::new(format!("Set by quality, about {suggested} kbps"))
                            .size(12.5)
                            .color(theme::TEXT_DIM),
                    );
                }
            });
        }
    }

    fn audio_rows(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.s;
        let codec = s.container.audio_codec();
        widgets::group_title(ui, "Audio");
        field(ui, "Audio bitrate", |ui| {
            if matches!(codec, AudioCodec::Pcm | AudioCodec::Flac) {
                ui.label(
                    RichText::new(if codec == AudioCodec::Pcm {
                        "Uncompressed (16-bit)"
                    } else {
                        "Lossless"
                    })
                    .color(theme::TEXT_DIM),
                );
            } else {
                let labels: Vec<(u32, String)> = AUDIO_BITRATES
                    .iter()
                    .map(|k| (*k, format!("{k} kbps")))
                    .collect();
                let refs: Vec<(u32, &str)> = labels.iter().map(|(k, l)| (*k, l.as_str())).collect();
                widgets::choice(ui, "ex-abr", &mut s.audio_kbps, &refs);
            }
        });
        field(ui, "Loudness", |ui| {
            // Off, or bring the whole sound to what a kind of site expects.
            let marks: [(Option<f64>, &str); 3] = [
                (None, "Off"),
                (Some(-14.0), "Streaming (-14)"),
                (Some(-23.0), "Broadcast (-23)"),
            ];
            let mut i = marks
                .iter()
                .position(|m| m.0 == s.loudness_target)
                .unwrap_or(0);
            ui.scope(|ui| {
                ui.set_max_width(360.0);
                let labels: Vec<&str> = marks.iter().map(|m| m.1).collect();
                if widgets::segmented(ui, &mut i, &labels) {
                    s.loudness_target = marks[i].0;
                }
            })
            .response
            .on_hover_text(
                "Sets how loud the finished video is, so it plays at the right level on                  YouTube, social sites or TV. Use View > Loudness to see how loud it is now.",
            );
        });
        field(ui, "Sample rate", |ui| {
            if codec == AudioCodec::Opus {
                ui.label(RichText::new("48 kHz (Opus)").color(theme::TEXT_DIM));
            } else {
                let rates = [44_100, 48_000];
                let mut i = rates
                    .iter()
                    .position(|r| *r == s.sample_rate)
                    .unwrap_or(usize::MAX);
                ui.scope(|ui| {
                    ui.set_max_width(220.0);
                    if widgets::segmented(ui, &mut i, &["44.1 kHz", "48 kHz"]) {
                        s.sample_rate = rates[i];
                    }
                });
            }
        });
    }

    fn range_rows(&mut self, ui: &mut egui::Ui, app: &TidyCutsApp) {
        let len = app.project.duration().as_secs_f64();
        let selection = self.selected.or_else(|| selection_range(app));
        widgets::group_title(ui, "Range");
        field(ui, "Export", |ui| {
            let mut i = match self.range_mode {
                RangeMode::Whole => 0,
                RangeMode::Selected => 1,
                RangeMode::Custom => 2,
            };
            ui.scope(|ui| {
                ui.set_max_width(390.0);
                if widgets::segmented(ui, &mut i, &["Whole project", "Selected clips", "Custom"]) {
                    match i {
                        0 => self.range_mode = RangeMode::Whole,
                        1 if selection.is_some() => self.range_mode = RangeMode::Selected,
                        1 => {}
                        _ => {
                            if self.range_mode != RangeMode::Custom {
                                if let Some((a, b)) = selection {
                                    self.custom = (a.as_secs_f64(), b.as_secs_f64());
                                }
                            }
                            self.range_mode = RangeMode::Custom;
                        }
                    }
                }
            });
        });
        match self.range_mode {
            RangeMode::Custom => {
                field(ui, "", |ui| {
                    ui.label(RichText::new("From").color(theme::TEXT_DIM));
                    ui.add_sized(
                        [96.0, 26.0],
                        egui::DragValue::new(&mut self.custom.0)
                            .range(0.0..=len)
                            .speed(0.05)
                            .custom_formatter(|v, _| fmt_position(v))
                            .custom_parser(parse_position),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("To").color(theme::TEXT_DIM));
                    ui.add_sized(
                        [96.0, 26.0],
                        egui::DragValue::new(&mut self.custom.1)
                            .range(0.0..=len)
                            .speed(0.05)
                            .custom_formatter(|v, _| fmt_position(v))
                            .custom_parser(parse_position),
                    );
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Playhead as start").size(12.0),
                        ))
                        .clicked()
                    {
                        self.custom.0 = app.playhead().as_secs_f64();
                    }
                });
            }
            RangeMode::Selected => {
                if let Some((a, b)) = selection {
                    field(ui, "", |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} to {}",
                                fmt_position(a.as_secs_f64()),
                                fmt_position(b.as_secs_f64())
                            ))
                            .color(theme::TEXT_DIM),
                        );
                    });
                }
            }
            RangeMode::Whole => {}
        }
        if selection.is_none() {
            field(ui, "", |ui| {
                ui.label(
                    RichText::new("Select clips on the timeline to export just them.")
                        .size(12.0)
                        .color(theme::TEXT_FAINT),
                );
            });
        }
    }

    /// Why the export can't start yet, if it can't.
    fn problem(
        &self,
        app: &TidyCutsApp,
        working: Option<&[String]>,
        has_sound: bool,
    ) -> Option<String> {
        if app.engine.is_none() {
            return Some("Exporting needs a graphics device.".into());
        }
        if app.ffmpeg_error.is_some() {
            return Some("Exporting needs FFmpeg, which wasn't found.".into());
        }
        if app.project.duration() == Time::ZERO {
            return Some("Add something to the timeline first.".into());
        }
        if self.name.trim().is_empty() {
            return Some("Type a name for the file.".into());
        }
        if !self.folder.is_dir() {
            return Some("The folder doesn't exist. Choose another one.".into());
        }
        if let Some((a, b)) = self.range(app) {
            if b <= a {
                return Some("The end of the range must come after its start.".into());
            }
        }
        if self.s.container.is_audio_only() && !has_sound {
            return Some("Nothing on the timeline makes sound.".into());
        }
        if !self.s.container.is_audio_only() {
            if let Some(w) = working {
                if encoders_for(self.s.codec, w, true).is_empty() {
                    return Some(format!(
                        "No {} encoder works on this computer. Pick another codec or format.",
                        self.s.codec.label()
                    ));
                }
            }
        }
        None
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let working = working_encoders(ctx);
        let has_sound = project_has_sound(&app.project);
        let secs = match self.range(app) {
            Some((a, b)) => (b - a).as_secs_f64(),
            None => app.project.duration().as_secs_f64(),
        };
        let problem = self.problem(app, working, has_sound);
        let exists = self.path().exists();
        let mut export = false;
        let mut cancel = false;
        let (_, keys) = window(ctx, "export", 900.0, |ui| {
            cancel |= header(
                ui,
                ph::EXPORT,
                "Export",
                "Save the timeline as a video or audio file",
            );
            super::scroll_middle(ui, |ui| {
                ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(12, 12))
                    .show(ui, |ui| {
                        ui.set_width(196.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            self.formats(ui, working)
                        });
                    });
                let line_x = ui.cursor().left() + 0.5;
                ui.add_space(1.0);
                super::body(ui, |ui| {
                    self.file_rows(ui, app);
                    ui.add_space(6.0);
                    if !self.s.container.is_audio_only() {
                        self.video_rows(ui, working);
                        ui.add_space(6.0);
                    }
                    if self.s.container.audio_codec() != AudioCodec::None {
                        self.audio_rows(ui);
                        ui.add_space(6.0);
                    }
                    if self.s.container.has_chapters() && !app.project.markers.is_empty() {
                        widgets::group_title(ui, "Chapters");
                        field(ui, "Markers", |ui| {
                            ui.scope(|ui| switch(ui, &mut self.s.chapters, "Add a chapter at each marker"))
                                .response
                                .on_hover_text(
                                    "Players that support chapters let people jump between your markers",
                                );
                        });
                        ui.add_space(6.0);
                    }
                    self.range_rows(ui, app);
                    if let Some(p) = &problem {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("{}  {p}", ph::WARNING_CIRCLE))
                                .color(theme::WARNING),
                        );
                    } else if exists {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "{}  A file with this name is already there and will be replaced.",
                                ph::INFO
                            ))
                            .size(12.5)
                            .color(theme::WARNING),
                        );
                    }
                });
                let r = ui.min_rect();
                ui.painter()
                    .vline(line_x, r.y_range(), Stroke::new(1.0, theme::BORDER));
            })
            });
            let est = estimate_bytes(&self.settings(app), secs, has_sound);
            let summary = if self.s.container.is_audio_only() {
                format!(
                    "{} · {} kHz",
                    self.s.container.label(),
                    self.s.sample_rate as f64 / 1000.0
                )
            } else {
                format!(
                    "{}×{} · {} · {}",
                    self.s.width,
                    self.s.height,
                    rate_label(self.s.frame_rate),
                    if self.s.container == Container::Gif {
                        "GIF"
                    } else {
                        self.s.codec.label()
                    }
                )
            };
            footer(
                ui,
                |ui| {
                    let stat = |ui: &mut egui::Ui, icon: &str, label: &str, value: String| {
                        ui.label(RichText::new(icon).color(theme::TEXT_DIM));
                        ui.label(RichText::new(label).size(12.5).color(theme::TEXT_DIM));
                        ui.label(RichText::new(value).size(12.5).color(theme::TEXT));
                        ui.add_space(8.0);
                    };
                    stat(ui, ph::CLOCK, "Duration", fmt_clock(secs));
                    stat(ui, ph::FILE, "Size", format!("≈ {}", fmt_bytes(est)));
                    ui.label(RichText::new(summary).size(12.5).color(theme::TEXT_FAINT));
                },
                |ui| {
                    if primary(ui, &format!("{}  Export", ph::EXPORT), problem.is_none()).clicked()
                    {
                        export = true;
                    }
                    if secondary(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
            );
        });
        if (export || keys.enter) && problem.is_none() {
            let settings = self.settings(app);
            let before = app.export_job.as_ref().map(|j| j.started);
            let queued_before = app.export_queue.len();
            app.start_export(settings);
            if app.export_queue.len() > queued_before {
                // Another export is running: this one is queued, and the window is done.
                app.dialogs.last_export = Some(self.clone());
                return false;
            }
            let started = app.export_job.as_ref().map(|j| j.started);
            if started.is_some() && started != before {
                app.dialogs.progress = Some(ProgressDialog {
                    has_sound: Some(has_sound),
                });
                app.dialogs.last_export = Some(self.clone());
                return false;
            }
        }
        !(cancel || keys.escape)
    }
}

/// From the start of the first selected clip to the end of the last.
fn selection_range(app: &TidyCutsApp) -> Option<(Time, Time)> {
    app.selection
        .iter()
        .filter_map(|id| app.project.clip(*id).map(|(_, c)| (c.start, c.end())))
        .fold(None, |acc: Option<(Time, Time)>, (a, b)| {
            Some(acc.map_or((a, b), |(x, y)| (x.min(a), y.max(b))))
        })
}

// ----- progress ----------------------------------------------------------

/// Follows `app.export_job`: progress while it runs, then the result.
#[derive(Default)]
pub struct ProgressDialog {
    /// Whether the export mixes sound before rendering (for the progress
    /// bar's weighting); worked out from the project when not known.
    has_sound: Option<bool>,
}

/// Keeps the export windows in step with the running job: the settings
/// window gives way to progress while a job runs, and an export that ends
/// while its window is hidden says so.
pub fn watch_job(app: &mut TidyCutsApp) {
    let Some(job) = &app.export_job else {
        return;
    };
    // (An export window opened while another export runs stays open: what is set there
    // goes into the queue.)
    if let Some((result, _)) = &job.finished {
        if app.dialogs.announced == Some(job.started) {
            return;
        }
        let name = job
            .settings
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let message = match result {
            Ok(()) => Some((format!("Export finished: {name}"), false)),
            Err(e) if e.contains("cancelled") => None,
            Err(e) => Some((format!("Export failed: {e}"), true)),
        };
        app.dialogs.announced = Some(job.started);
        if app.dialogs.progress.is_none() {
            if let Some((text, error)) = message {
                app.notify(&text, error);
            }
        }
    }
}

/// Says how the finished export went, even when its progress window is open.
pub fn announce_finished(app: &mut TidyCutsApp) {
    let Some(job) = &app.export_job else {
        return;
    };
    let Some((result, _)) = &job.finished else {
        return;
    };
    if app.dialogs.announced == Some(job.started) {
        return;
    }
    let name = job
        .settings
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let message = match result {
        Ok(()) => Some((format!("Export finished: {name}"), false)),
        Err(e) if e.contains("cancelled") => None,
        Err(e) => Some((format!("Export failed: {e}"), true)),
    };
    app.dialogs.announced = Some(job.started);
    if let Some((text, error)) = message {
        app.notify(&text, error);
    }
}

impl ProgressDialog {
    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let has_sound = *self
            .has_sound
            .get_or_insert_with(|| project_has_sound(&app.project));
        let Some(job) = &app.export_job else {
            return false;
        };
        let s = job.settings.clone();
        let name = s
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let progress = job.progress.lock().unwrap().clone();
        let elapsed = job.started.elapsed();
        let cancelling = job.cancel.load(Ordering::Relaxed);
        let finished = job.finished.as_ref().map(|(r, d)| (r.clone(), *d));
        let mut hide = false;
        let mut another = false;
        let mut close = false;
        let mut cancel = false;
        let mut back = false;
        let mut open_folder = false;
        let (_, keys) = window(ctx, "export-progress", 680.0, |ui| {
            let (icon, title) = match &finished {
                None if cancelling => (ph::HOURGLASS, "Cancelling…"),
                None => (ph::EXPORT, "Exporting"),
                Some((Ok(()), _)) => (ph::CHECK_CIRCLE, "Export finished"),
                Some((Err(e), _)) if e.contains("cancelled") => (ph::X, "Export cancelled"),
                Some((Err(_), _)) => (ph::WARNING_CIRCLE, "Export failed"),
            };
            let x = header(ui, icon, title, &name);
            match &finished {
                None => hide |= x,
                Some(_) => close |= x,
            }
            super::body(ui, |ui| match &finished {
                None => {
                    let f = overall_fraction(&progress, s.container.is_audio_only(), has_sound);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.0}%", f * 100.0))
                                .font(theme::heading_font(28.0))
                                .color(Color32::WHITE),
                        );
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            let stage = if progress.stage.is_empty() {
                                "Starting…"
                            } else {
                                progress.stage
                            };
                            ui.label(RichText::new(stage).color(theme::TEXT));
                            if progress.stage == "Rendering video" {
                                ui.label(
                                    RichText::new(format!(
                                        "Frame {} of {} · {:.0} fps",
                                        progress.done, progress.total, progress.fps
                                    ))
                                    .size(12.0)
                                    .color(theme::TEXT_DIM),
                                );
                            }
                        });
                    });
                    ui.add_space(4.0);
                    widgets::progress_bar(
                        ui,
                        f,
                        10.0,
                        if cancelling {
                            theme::TEXT_FAINT
                        } else {
                            theme::ACCENT
                        },
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ph::TIMER).color(theme::TEXT_DIM));
                        ui.label(
                            RichText::new(format!("Elapsed {}", fmt_clock(elapsed.as_secs_f64())))
                                .color(theme::TEXT_DIM),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let left = remaining(&progress, f, elapsed)
                                .map(|d| format!("About {} left", fmt_clock(d.as_secs_f64())))
                                .unwrap_or_else(|| "Working out the time left…".into());
                            ui.label(RichText::new(left).color(theme::TEXT_DIM));
                        });
                    });
                    ui.add_space(4.0);
                    widgets::path_box(ui, &s.path.display().to_string(), ui.available_width());
                }
                Some((Ok(()), took)) => {
                    let size = std::fs::metadata(&s.path).map(|m| m.len()).unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ph::CHECK_CIRCLE)
                                .size(30.0)
                                .color(theme::ACCENT),
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Your file is ready.").color(Color32::WHITE));
                            ui.label(
                                RichText::new(format!(
                                    "{} · made in {}",
                                    fmt_bytes(size),
                                    fmt_clock(took.as_secs_f64())
                                ))
                                .size(12.5)
                                .color(theme::TEXT_DIM),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    widgets::path_box(ui, &s.path.display().to_string(), ui.available_width());
                }
                Some((Err(e), _)) => {
                    let cancelled = e.contains("cancelled");
                    let text = if cancelled {
                        "The export was stopped and nothing was saved. Your settings are kept \
                         if you want to try again."
                    } else {
                        e.as_str()
                    };
                    ui.add(
                        egui::Label::new(RichText::new(text).color(if cancelled {
                            theme::TEXT
                        } else {
                            theme::DANGER
                        }))
                        .wrap(),
                    );
                    if !cancelled {
                        widgets::hint(
                            ui,
                            "Try another format or encoder, or turn off GPU acceleration.",
                        );
                    }
                }
            });
            footer(
                ui,
                |ui| {
                    if finished.is_none() {
                        ui.label(
                            RichText::new("You can hide this and keep editing.")
                                .size(12.5)
                                .color(theme::TEXT_DIM),
                        );
                    }
                },
                |ui| match &finished {
                    None => {
                        let r = ui.add_enabled(
                            !cancelling,
                            theme::outline_button(
                                RichText::new("Cancel export").color(theme::DANGER),
                            )
                            .min_size(egui::vec2(120.0, 34.0)),
                        );
                        if r.clicked() {
                            cancel = true;
                        }
                        if secondary(ui, &format!("{}  Add another export…", ph::PLUS)).clicked()
                        {
                            another = true;
                        }
                        if secondary(ui, "Hide").clicked() {
                            hide = true;
                        }
                    }
                    Some((result, _)) => {
                        if primary(ui, "Close", true).clicked() {
                            close = true;
                        }
                        if result.is_ok()
                            && secondary(ui, &format!("{}  Open folder", ph::FOLDER_OPEN)).clicked()
                        {
                            open_folder = true;
                        }
                        if result.is_err() && secondary(ui, "Back to settings").clicked() {
                            back = true;
                        }
                    }
                },
            );
        });
        if cancel {
            job.cancel.store(true, Ordering::Relaxed);
        }
        if open_folder {
            if let Err(e) = reveal(&s.path) {
                app.notify(&format!("Couldn't open the folder: {e}"), true);
            }
        }
        if another && finished.is_none() {
            // The settings window opens; what is set there waits in the queue.
            app.dialogs
                .open_export(&app.project, &app.prefs, app.project_path.as_deref());
            return false;
        }
        match finished {
            None => !(hide || keys.escape),
            Some(_) => {
                if close || back || keys.escape || keys.enter {
                    app.export_job = None;
                    if back {
                        app.dialogs.export = app.dialogs.last_export.clone();
                    }
                    false
                } else {
                    true
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(w: u32, h: u32, fps: u32) -> ExportSettings {
        let mut p = Project::default();
        p.settings.width = w;
        p.settings.height = h;
        p.settings.frame_rate = FrameRate::new(fps, 1);
        ExportSettings::for_project(&p, PathBuf::from("/tmp/x.mp4"))
    }

    fn dialog(w: u32, h: u32, fps: u32) -> ExportDialog {
        let mut p = Project::default();
        p.settings.width = w;
        p.settings.height = h;
        p.settings.frame_rate = FrameRate::new(fps, 1);
        ExportDialog::new(&p, &Prefs::default(), None)
    }

    fn preset(name: &str) -> usize {
        PRESETS.iter().position(|p| p.name == name).unwrap()
    }

    #[test]
    fn presets_shrink_but_never_enlarge_and_keep_the_shape() {
        let mut d = dialog(3840, 2160, 60);
        d.apply_preset(preset("Widescreen HD"), None);
        assert_eq!((d.s.width, d.s.height), (1920, 1080));
        assert_eq!(d.s.frame_rate, FrameRate::new(60, 1));
        d.apply_preset(preset("Small file"), None);
        assert_eq!((d.s.width, d.s.height), (960, 540));
        assert_eq!(d.s.frame_rate, FrameRate::new(30, 1));
        assert_eq!(d.s.audio_kbps, 96);
        // A small project is not blown up to fit a big preset.
        let mut small = dialog(1280, 720, 24);
        small.apply_preset(preset("Widescreen 4K"), None);
        assert_eq!((small.s.width, small.s.height), (1280, 720));
        assert_eq!(small.s.frame_rate, FrameRate::new(24, 1));
        // A vertical project keeps its shape.
        let mut tall = dialog(1080, 1920, 30);
        tall.apply_preset(preset("Vertical phone"), None);
        assert_eq!((tall.s.width, tall.s.height), (1080, 1920));
        tall.apply_preset(preset("Web video"), None);
        assert_eq!(tall.s.container, Container::Webm);
        assert_eq!(tall.s.codec, VideoCodec::Vp9);
    }

    #[test]
    fn encoders_map_to_their_codecs() {
        assert_eq!(codec_of("libx264"), Some(VideoCodec::H264));
        assert_eq!(codec_of("h264_nvenc"), Some(VideoCodec::H264));
        assert_eq!(codec_of("hevc_qsv"), Some(VideoCodec::Hevc));
        assert_eq!(codec_of("libsvtav1"), Some(VideoCodec::Av1));
        assert_eq!(codec_of("libaom-av1"), Some(VideoCodec::Av1));
        assert_eq!(codec_of("libvpx-vp9"), Some(VideoCodec::Vp9));
        assert_eq!(codec_of("prores_ks"), Some(VideoCodec::ProRes));
        assert_eq!(codec_of("mpeg2video"), Some(VideoCodec::Mpeg2));
        assert_eq!(codec_of("gif"), Some(VideoCodec::Gif));
        assert_eq!(codec_of("pcm_s16le"), None);
        assert!(is_gpu("h264_nvenc") && !is_gpu("libx264"));
        assert_eq!(encoder_label("h264_nvenc"), "NVIDIA NVENC · GPU");
        assert_eq!(encoder_label("libx264"), "x264 · CPU");
        let w = vec!["libx264".to_string(), "h264_nvenc".into(), "libx265".into()];
        assert_eq!(encoders_for(VideoCodec::H264, &w, true).len(), 2);
        assert_eq!(encoders_for(VideoCodec::H264, &w, false), vec!["libx264"]);
    }

    #[test]
    fn size_estimate_follows_bitrate_and_length() {
        let s = settings(1920, 1080, 25);
        // 1920×1080×25×0.1 = 5184 kbps of video plus 192 kbps of sound.
        let ten = estimate_bytes(&s, 10.0, true);
        assert_eq!(ten, ((5184.0 + 192.0) * 1000.0 / 8.0 * 10.0) as u64);
        assert_eq!(estimate_bytes(&s, 20.0, true), ten * 2);
        assert!(estimate_bytes(&s, 10.0, false) < ten);
        let mut fixed = s.clone();
        fixed.bitrate_kbps = Some(8000);
        assert_eq!(estimate_bytes(&fixed, 1.0, false), 1_000_000);
        let mut hevc = s.clone();
        hevc.codec = VideoCodec::Hevc;
        assert!(estimate_bytes(&hevc, 10.0, false) < estimate_bytes(&s, 10.0, false));
        let mut wav = s.clone();
        wav.container = Container::Wav;
        // 48 kHz, 16-bit stereo: 192,000 bytes a second.
        assert_eq!(estimate_bytes(&wav, 10.0, true), 1_920_000);
        assert_eq!(estimate_bytes(&wav, 10.0, false), 0);
    }

    #[test]
    fn progress_is_weighted_by_stage() {
        let p = |stage: &'static str, done, total| ExportProgress {
            stage,
            done,
            total,
            ..Default::default()
        };
        assert_eq!(overall_fraction(&p("", 0, 0), false, true), 0.0);
        let mixing = overall_fraction(&p("Mixing audio", 500, 1000), false, true);
        assert!((mixing - 0.04).abs() < 1e-6);
        let half = overall_fraction(&p("Rendering video", 50, 100), false, true);
        assert!((half - 0.54).abs() < 1e-6);
        let silent = overall_fraction(&p("Rendering video", 50, 100), false, false);
        assert!((silent - 0.5).abs() < 1e-6);
        let audio = overall_fraction(&p("Mixing audio", 1000, 1000), true, true);
        assert!((audio - 0.9).abs() < 1e-6);
        let r = remaining(&p("Mixing audio", 1, 2), 0.5, Duration::from_secs(10));
        assert_eq!(r, Some(Duration::from_secs(10)));
        assert_eq!(remaining(&p("", 0, 0), 0.0, Duration::from_secs(1)), None);
    }

    #[test]
    fn gif_export_starts_small_and_slow() {
        let p = Project::default();
        let mut d = ExportDialog::new(&p, &Prefs::default(), None);
        d.set_container(Container::Gif, None);
        assert_eq!((d.s.width, d.s.height), (852, 480));
        assert_eq!(d.s.frame_rate, FrameRate::new(15, 1));
        d.set_container(Container::Mp4, None);
        assert_eq!((d.s.width, d.s.height), (1920, 1080));
        d.set_container(Container::Webm, None);
        assert_eq!(d.s.codec, VideoCodec::Vp9);
        assert_eq!(d.s.sample_rate, 48_000);
    }
}
