//! Exporting: every frame is drawn on the GPU at full size, read back and
//! piped into ffmpeg, which encodes it (on the graphics card when one is
//! available) together with the mixed sound.

use crate::frames::{decode_size, StreamFrames};
use crate::levels::AudioLevels;
use crate::mixer::Mixer;
use crate::plan::Planner;
use crate::statics::Statics;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tidycuts_core::{FrameRate, Marker, MediaKind, Project, Time, TrackKind};
use tidycuts_media::Tools;
use tidycuts_render::{Compositor, Grade, Renderer};

/// Output file types, the common export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Container {
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    Wmv,
    Mpg,
    Ts,
    Gif,
    Mp3,
    Wav,
    M4a,
    Flac,
}

impl Container {
    pub const ALL: [Container; 13] = [
        Container::Mp4,
        Container::Mov,
        Container::Mkv,
        Container::Webm,
        Container::Avi,
        Container::Wmv,
        Container::Mpg,
        Container::Ts,
        Container::Gif,
        Container::Mp3,
        Container::Wav,
        Container::M4a,
        Container::Flac,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4",
            Container::Mov => "MOV",
            Container::Mkv => "MKV",
            Container::Webm => "WEBM",
            Container::Avi => "AVI",
            Container::Wmv => "WMV",
            Container::Mpg => "MPEG-2",
            Container::Ts => "TS",
            Container::Gif => "GIF",
            Container::Mp3 => "MP3",
            Container::Wav => "WAV",
            Container::M4a => "M4A",
            Container::Flac => "FLAC",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "mkv",
            Container::Webm => "webm",
            Container::Avi => "avi",
            Container::Wmv => "wmv",
            Container::Mpg => "mpg",
            Container::Ts => "ts",
            Container::Gif => "gif",
            Container::Mp3 => "mp3",
            Container::Wav => "wav",
            Container::M4a => "m4a",
            Container::Flac => "flac",
        }
    }

    /// Whether files of this kind can carry chapters.
    pub fn has_chapters(self) -> bool {
        matches!(
            self,
            Container::Mp4 | Container::Mov | Container::Mkv | Container::Webm | Container::M4a
        )
    }

    pub fn is_audio_only(self) -> bool {
        matches!(
            self,
            Container::Mp3 | Container::Wav | Container::M4a | Container::Flac
        )
    }

    /// Video codecs this container can hold, best first.
    pub fn video_codecs(self) -> &'static [VideoCodec] {
        use VideoCodec::*;
        match self {
            Container::Mp4 => &[H264, Hevc, Av1],
            Container::Mov => &[H264, Hevc, ProRes],
            Container::Mkv => &[H264, Hevc, Av1, Vp9],
            Container::Webm => &[Vp9, Av1],
            Container::Avi => &[Mpeg4, H264],
            Container::Wmv => &[Wmv2],
            Container::Mpg => &[Mpeg2],
            Container::Ts => &[H264, Hevc, Mpeg2],
            Container::Gif => &[Gif],
            _ => &[],
        }
    }

    pub fn audio_codec(self) -> AudioCodec {
        match self {
            Container::Webm => AudioCodec::Opus,
            Container::Avi | Container::Mpg | Container::Mp3 => AudioCodec::Mp3,
            Container::Wmv => AudioCodec::Wma,
            Container::Mov if false => AudioCodec::Pcm,
            Container::Wav => AudioCodec::Pcm,
            Container::Flac => AudioCodec::Flac,
            Container::Gif => AudioCodec::None,
            _ => AudioCodec::Aac,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Vp9,
    ProRes,
    Mpeg2,
    Mpeg4,
    Wmv2,
    Gif,
}

impl VideoCodec {
    pub fn label(self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::Hevc => "HEVC (H.265)",
            VideoCodec::Av1 => "AV1",
            VideoCodec::Vp9 => "VP9",
            VideoCodec::ProRes => "ProRes 422 HQ",
            VideoCodec::Mpeg2 => "MPEG-2",
            VideoCodec::Mpeg4 => "MPEG-4 Part 2",
            VideoCodec::Wmv2 => "WMV",
            VideoCodec::Gif => "GIF",
        }
    }

    /// ffmpeg encoders for this codec, hardware first.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            VideoCodec::H264 => &[
                "h264_nvenc",
                "h264_qsv",
                "h264_amf",
                "h264_videotoolbox",
                "libx264",
                "h264_mf",
                "libopenh264",
            ],
            VideoCodec::Hevc => &[
                "hevc_nvenc",
                "hevc_qsv",
                "hevc_amf",
                "hevc_videotoolbox",
                "libx265",
                "hevc_mf",
                "libkvazaar",
            ],
            VideoCodec::Av1 => &[
                "av1_nvenc",
                "av1_qsv",
                "av1_amf",
                "libsvtav1",
                "libaom-av1",
                "librav1e",
            ],
            VideoCodec::Vp9 => &["vp9_qsv", "libvpx-vp9"],
            VideoCodec::ProRes => &["prores_videotoolbox", "prores_ks"],
            VideoCodec::Mpeg2 => &["mpeg2_qsv", "mpeg2video"],
            VideoCodec::Mpeg4 => &["mpeg4"],
            VideoCodec::Wmv2 => &["wmv2"],
            VideoCodec::Gif => &["gif"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioCodec {
    None,
    Aac,
    Mp3,
    Opus,
    Pcm,
    Flac,
    Wma,
}

impl AudioCodec {
    fn args(self, kbps: u32) -> Vec<String> {
        let b = format!("{kbps}k");
        match self {
            AudioCodec::None => vec!["-an".into()],
            AudioCodec::Aac => vec!["-c:a".into(), "aac".into(), "-b:a".into(), b],
            AudioCodec::Mp3 => vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), b],
            AudioCodec::Opus => vec!["-c:a".into(), "libopus".into(), "-b:a".into(), b],
            AudioCodec::Pcm => vec!["-c:a".into(), "pcm_s16le".into()],
            AudioCodec::Flac => vec!["-c:a".into(), "flac".into()],
            AudioCodec::Wma => vec!["-c:a".into(), "wmav2".into(), "-b:a".into(), b],
        }
    }
}

/// Quality presets, Higher / Recommended / Lower.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Quality {
    Higher,
    Recommended,
    Lower,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportSettings {
    pub path: PathBuf,
    pub container: Container,
    pub codec: VideoCodec,
    /// Use this ffmpeg encoder; `None` picks the best available.
    pub encoder: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub quality: Quality,
    /// Fixed bitrate instead of the quality preset.
    pub bitrate_kbps: Option<u32>,
    pub audio_kbps: u32,
    pub sample_rate: u32,
    /// Export only this part of the timeline.
    pub range: Option<(Time, Time)>,
    pub hardware_encode: bool,
    pub hardware_decode: bool,
    /// Write the timeline's markers into the file as chapters (formats that
    /// have them).
    #[serde(default)]
    pub chapters: bool,
    /// Bring the sound to this loudness (LUFS), for example -14 for streaming
    /// sites. `None` leaves the level as mixed.
    #[serde(default)]
    pub loudness_target: Option<f64>,
}

impl ExportSettings {
    pub fn for_project(project: &Project, path: PathBuf) -> ExportSettings {
        ExportSettings {
            path,
            container: Container::Mp4,
            codec: VideoCodec::H264,
            encoder: None,
            width: project.settings.width,
            height: project.settings.height,
            frame_rate: project.settings.frame_rate,
            quality: Quality::Recommended,
            bitrate_kbps: None,
            audio_kbps: 192,
            sample_rate: 48_000,
            range: None,
            hardware_encode: true,
            hardware_decode: true,
            chapters: true,
            loudness_target: None,
        }
    }

    /// A sensible bitrate for the size and frame rate (editors usually show one).
    pub fn suggested_kbps(&self) -> u32 {
        let px = (self.width * self.height) as f64;
        let fps = self.frame_rate.as_f64();
        let base = px * fps * 0.1 / 1000.0;
        let k = match self.quality {
            Quality::Higher => 1.5,
            Quality::Recommended => 1.0,
            Quality::Lower => 0.55,
        };
        ((base * k) as u32).clamp(500, 200_000)
    }
}

/// How an export is going.
#[derive(Debug, Clone, Default)]
pub struct ExportProgress {
    pub stage: &'static str,
    pub done: u64,
    pub total: u64,
    pub fps: f64,
    pub eta: Option<Duration>,
}

/// Encoders that work on this machine, found once by encoding a tiny clip
/// with each candidate (hardware encoders fail without the hardware).
pub fn working_encoders() -> &'static Vec<String> {
    static FOUND: OnceLock<Vec<String>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let Ok(tools) = Tools::get() else {
            return Vec::new();
        };
        let listed = tools
            .ffmpeg_cmd()
            .arg("-encoders")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let all: Vec<&str> = [
            VideoCodec::H264,
            VideoCodec::Hevc,
            VideoCodec::Av1,
            VideoCodec::Vp9,
            VideoCodec::ProRes,
            VideoCodec::Mpeg2,
            VideoCodec::Mpeg4,
            VideoCodec::Wmv2,
            VideoCodec::Gif,
        ]
        .iter()
        .flat_map(|c| c.candidates().iter().copied())
        .collect();
        let handles: Vec<_> = all
            .into_iter()
            .filter(|e| listed.split_whitespace().any(|w| w == *e))
            .map(|e| {
                let tools = tools.clone();
                std::thread::spawn(move || probe_encoder(&tools, e, &[]).then(|| e.to_string()))
            })
            .collect();
        let found: Vec<String> = handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect();
        // The extra NVENC quality options are only used where they work.
        let extras: Vec<String> = found
            .iter()
            .filter(|e| e.contains("nvenc") && probe_encoder(tools, e, NVENC_EXTRAS))
            .cloned()
            .collect();
        let _ = NVENC_OK.set(extras);
        found
    })
}

/// Options that make NVENC's pictures better: a look-ahead and adaptive
/// quantisation, which spend the bits where the eye notices.
const NVENC_EXTRAS: &[&str] = &["-tune", "hq", "-spatial-aq", "1", "-rc-lookahead", "20"];

/// The NVENC encoders that accept [`NVENC_EXTRAS`] (older cards or builds
/// may not).
static NVENC_OK: OnceLock<Vec<String>> = OnceLock::new();

/// Encodes a few frames of grey with `encoder` (and `extra` options) to see
/// whether it works on this computer.
fn probe_encoder(tools: &Tools, encoder: &str, extra: &[&str]) -> bool {
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args([
        "-f",
        "lavfi",
        "-i",
        "color=c=gray:s=256x144:r=25:d=0.2",
        "-frames:v",
        "3",
    ]);
    cmd.args(["-c:v", encoder]);
    if encoder == "gif" {
        cmd.args(["-pix_fmt", "rgb8"]);
    } else if !encoder.starts_with("prores") {
        cmd.args([
            "-pix_fmt",
            if encoder.contains("qsv") {
                "nv12"
            } else {
                "yuv420p"
            },
        ]);
    }
    cmd.args(extra);
    cmd.args(["-f", "null", "-"]);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn is_hardware(encoder: &str) -> bool {
    ["nvenc", "qsv", "amf", "videotoolbox", "_mf"]
        .iter()
        .any(|k| encoder.contains(k))
}

/// The encoder an export will use.
pub fn pick_encoder(s: &ExportSettings) -> Option<String> {
    let working = working_encoders();
    if let Some(e) = &s.encoder {
        if working.contains(e) {
            return Some(e.clone());
        }
    }
    let mut list: Vec<&str> = s
        .codec
        .candidates()
        .iter()
        .copied()
        .filter(|e| s.hardware_encode || !is_hardware(e))
        .collect();
    // The encoder on the card that draws the picture comes first: on a
    // computer with, say, an Intel chip and an AMD card, the card is faster.
    if let Some(key) = preferred_hardware() {
        list.sort_by_key(|e| !e.contains(key));
    }
    list.into_iter()
        .find(|e| working.iter().any(|w| w == *e))
        .map(|e| e.to_string())
}

/// The kind of hardware encoder that sits on the graphics card in use.
fn preferred_hardware() -> Option<&'static str> {
    use tidycuts_render::gpu::Vendor;
    match tidycuts_render::gpu::adapter()?.vendor {
        Vendor::Nvidia => Some("nvenc"),
        Vendor::Amd => Some("amf"),
        Vendor::Intel => Some("qsv"),
        Vendor::Apple => Some("videotoolbox"),
        Vendor::Other => None,
    }
}

/// Encoder options for a quality level or bitrate.
fn encoder_args(enc: &str, s: &ExportSettings) -> Vec<String> {
    let mut a: Vec<String> = vec!["-c:v".into(), enc.into()];
    let q = |hi: u32, mid: u32, lo: u32| match s.quality {
        Quality::Higher => hi,
        Quality::Recommended => mid,
        Quality::Lower => lo,
    };
    let q_str = |hi: &'static str, mid: &'static str, lo: &'static str| -> &'static str {
        match s.quality {
            Quality::Higher => hi,
            Quality::Recommended => mid,
            Quality::Lower => lo,
        }
    };
    let push = |a: &mut Vec<String>, xs: &[&str]| a.extend(xs.iter().map(|x| x.to_string()));
    if let Some(kbps) = s.bitrate_kbps {
        let b = format!("{kbps}k");
        push(
            &mut a,
            &[
                "-b:v",
                &b,
                "-maxrate",
                &format!("{}k", kbps * 3 / 2),
                "-bufsize",
                &format!("{}k", kbps * 2),
            ],
        );
    } else if enc.contains("nvenc") {
        push(
            &mut a,
            &[
                "-rc",
                "vbr",
                "-cq",
                &q(19, 23, 28).to_string(),
                "-b:v",
                "0",
                "-preset",
                q_str("p6", "p5", "p4"),
            ],
        );
    } else if enc.contains("qsv") {
        push(
            &mut a,
            &[
                "-global_quality",
                &q(20, 24, 29).to_string(),
                "-preset",
                "medium",
            ],
        );
    } else if enc.contains("amf") {
        let qp = q(20, 24, 29).to_string();
        push(
            &mut a,
            &[
                "-rc", "cqp", "-qp_i", &qp, "-qp_p", &qp, "-quality", "quality",
            ],
        );
    } else if enc.contains("videotoolbox") && !enc.starts_with("prores") {
        push(&mut a, &["-q:v", &q(70, 58, 45).to_string()]);
    } else if enc == "libx264" {
        push(
            &mut a,
            &["-crf", &q(18, 21, 26).to_string(), "-preset", "medium"],
        );
    } else if enc == "libx265" {
        push(
            &mut a,
            &[
                "-crf",
                &q(20, 24, 28).to_string(),
                "-preset",
                "medium",
                "-tag:v",
                "hvc1",
            ],
        );
    } else if enc == "libsvtav1" {
        push(
            &mut a,
            &["-crf", &q(24, 30, 38).to_string(), "-preset", "8"],
        );
    } else if enc == "libaom-av1" {
        push(
            &mut a,
            &[
                "-crf",
                &q(24, 30, 38).to_string(),
                "-b:v",
                "0",
                "-cpu-used",
                "6",
                "-row-mt",
                "1",
            ],
        );
    } else if enc == "librav1e" {
        push(&mut a, &["-qp", &q(70, 90, 120).to_string(), "-speed", "8"]);
    } else if enc == "libvpx-vp9" {
        push(
            &mut a,
            &[
                "-crf",
                &q(24, 31, 38).to_string(),
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-deadline",
                "good",
                "-cpu-used",
                "3",
            ],
        );
    } else if enc == "prores_ks" {
        push(&mut a, &["-profile:v", "3", "-vendor", "apl0"]);
    } else if enc == "gif" {
    } else {
        // Encoders without a quality mode get a bitrate.
        let b = format!("{}k", s.suggested_kbps());
        push(&mut a, &["-b:v", &b]);
    }
    if enc.contains("nvenc") && NVENC_OK.get().is_some_and(|ok| ok.iter().any(|e| e == enc)) {
        push(&mut a, NVENC_EXTRAS);
    }
    if enc.contains("hevc")
        && !a.contains(&"-tag:v".to_string())
        && matches!(s.container, Container::Mp4 | Container::Mov)
    {
        push(&mut a, &["-tag:v", "hvc1"]);
    }
    a
}

/// Whether the graphics card draws the picture as video colours (NV12) for
/// this encoder, so ffmpeg has no colour conversion to do. Not for GIF
/// (which wants every pixel's own colour) or ProRes (10-bit, full-size
/// colour).
fn gpu_draws_video_colours(enc: &str, container: Container) -> bool {
    container != Container::Gif && !enc.starts_with("prores")
}

fn pixel_format(enc: &str) -> &'static str {
    if enc.starts_with("prores") {
        "yuv422p10le"
    } else if enc.contains("qsv") {
        "nv12"
    } else {
        "yuv420p"
    }
}

/// Writes the mixed sound of `range` to a 32-bit float WAV file.
fn write_wav(
    project: &Project,
    range: (Time, Time),
    sr: u32,
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> std::io::Result<()> {
    let mut mixer = Mixer::new(sr, Duration::from_secs(30));
    let total = ((range.1 - range.0).as_secs_f64() * sr as f64).round() as u64;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let data_len = (total * 2 * 4) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&3u16.to_le_bytes())?; // IEEE float
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&sr.to_le_bytes())?;
    f.write_all(&(sr * 8).to_le_bytes())?;
    f.write_all(&8u16.to_le_bytes())?;
    f.write_all(&32u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    let block = 4096u64;
    let mut buf = vec![0f32; block as usize * 2];
    let mut done = 0u64;
    while done < total {
        if cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("cancelled"));
        }
        let n = block.min(total - done) as usize;
        let at = range.0 + Time::from_secs_f64(done as f64 / sr as f64);
        mixer.mix(project, at, &mut buf[..n * 2]);
        for s in &buf[..n * 2] {
            f.write_all(&s.to_le_bytes())?;
        }
        done += n as u64;
        progress(done as f64 / total.max(1) as f64);
    }
    f.flush()
}

/// Whether anything in the project makes sound.
fn has_sound(project: &Project) -> bool {
    project.tracks.iter().any(|t| {
        t.clips.iter().any(|c| {
            c.media_id
                .and_then(|m| project.media(m))
                .is_some_and(|m| m.kind == MediaKind::Audio || (m.has_audio && !c.audio.muted))
        })
    })
}

/// An ffmpeg metadata file that turns markers into chapters for the part of
/// the timeline `range`, or `None` when there are none inside it. A chapter
/// lasts until the next marker; a first chapter called "Start" is added when
/// the first marker is not at the very beginning, because players expect
/// chapters to start at zero.
pub fn chapters_metadata(markers: &[Marker], range: (Time, Time)) -> Option<String> {
    let mut points: Vec<(i64, String)> = markers
        .iter()
        .filter(|m| m.time >= range.0 && m.time < range.1)
        .map(|m| {
            let ms = ((m.time - range.0).as_secs_f64() * 1000.0).round() as i64;
            let name = if m.name.trim().is_empty() {
                "Marker".to_string()
            } else {
                m.name.trim().to_string()
            };
            (ms, name)
        })
        .collect();
    if points.is_empty() {
        return None;
    }
    points.sort_by_key(|p| p.0);
    points.dedup_by_key(|p| p.0);
    if points[0].0 > 500 {
        points.insert(0, (0, "Start".to_string()));
    }
    let end = ((range.1 - range.0).as_secs_f64() * 1000.0).round() as i64;
    let mut out = String::from(";FFMETADATA1\n");
    for (i, (start, name)) in points.iter().enumerate() {
        let stop = points.get(i + 1).map_or(end, |p| p.0);
        // Values may not contain these characters unescaped.
        let title: String = name
            .chars()
            .flat_map(|c| match c {
                '=' | ';' | '#' | '\\' => vec!['\\', c],
                '\n' | '\r' => vec![' '],
                c => vec![c],
            })
            .collect();
        out.push_str(&format!(
            "[CHAPTER]\nTIMEBASE=1/1000\nSTART={start}\nEND={}\ntitle={title}\n",
            stop.max(start + 1)
        ));
    }
    Some(out)
}

/// Mixes `range` of the project (the whole timeline when `None`) and measures
/// how loud it is. This is what the Loudness window shows.
pub fn measure_project_loudness(
    project: &Project,
    range: Option<(Time, Time)>,
    cancel: &AtomicBool,
) -> Result<crate::loudness::Report, String> {
    let range = range.unwrap_or((Time::ZERO, project.duration()));
    if range.1 <= range.0 || !has_sound(project) {
        return Err("There is no sound on the timeline.".into());
    }
    let tmp = std::env::temp_dir().join(format!(
        "tidycuts-loudness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let wav = tmp.join("mix.wav");
    let result = write_wav(project, range, 48_000, &wav, cancel, |_| {})
        .map_err(|e| {
            if cancel.load(Ordering::Relaxed) {
                "Cancelled.".to_string()
            } else {
                format!("Could not mix the audio: {e}")
            }
        })
        .and_then(|()| crate::loudness::measure_file(&wav));
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Exports the project. `progress` is called often; `cancel` stops it.
#[allow(clippy::too_many_arguments)]
pub fn export(
    project: &Project,
    s: &ExportSettings,
    renderer: Arc<Renderer>,
    statics: &Statics,
    auto_grade: &dyn Fn(&tidycuts_core::MediaItem) -> Option<Grade>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(ExportProgress),
) -> Result<(), String> {
    let tools = Tools::get().map_err(|e| e.to_string())?;
    let range = s.range.unwrap_or((Time::ZERO, project.duration()));
    if range.1 <= range.0 {
        return Err("There is nothing on the timeline to export.".into());
    }
    // Camera-shake removal needs its one-off analysis before the first frame.
    for t in &project.tracks {
        for c in t.clips.iter().filter(|c| c.video.stabilize > 0.0) {
            if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                progress(ExportProgress {
                    stage: "Analysing camera shake",
                    ..Default::default()
                });
                crate::stabilize::finish(m, (c.source_in, c.source_out), cancel);
            }
        }
    }
    let tmp = std::env::temp_dir().join(format!("tidycuts-export-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let wav = tmp.join("mix.wav");
    let sound = s.container.audio_codec() != AudioCodec::None && has_sound(project);
    if sound {
        write_wav(project, range, s.sample_rate, &wav, cancel, |f| {
            progress(ExportProgress {
                stage: "Mixing audio",
                done: (f * 1000.0) as u64,
                total: 1000,
                ..Default::default()
            })
        })
        .map_err(|e| {
            if cancel.load(Ordering::Relaxed) {
                "Export cancelled.".to_string()
            } else {
                format!("Could not mix the audio: {e}")
            }
        })?;
    }

    let mut cmd = tools.ffmpeg_cmd();
    cmd.arg("-y");
    let video = !s.container.is_audio_only();
    let (w, h) = (s.width.max(2) & !1, s.height.max(2) & !1);
    let fps = s.frame_rate;
    let enc = if video {
        Some(
            pick_encoder(s)
                .ok_or_else(|| format!("No {} encoder works on this computer.", s.codec.label()))?,
        )
    } else {
        None
    };
    let gpu_yuv = enc
        .as_deref()
        .is_some_and(|e| gpu_draws_video_colours(e, s.container));
    if video {
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            if gpu_yuv { "nv12" } else { "rgba" },
            "-s",
            &format!("{w}x{h}"),
        ]);
        if gpu_yuv {
            // What the graphics card drew: limited range, BT.709.
            cmd.args(["-color_range", "tv", "-colorspace", "bt709"]);
        }
        cmd.args(["-r", &format!("{}/{}", fps.num, fps.den), "-i", "pipe:0"]);
    }
    if sound {
        cmd.arg("-i").arg(&wav);
    } else if !video {
        return Err("There is no sound to export.".into());
    }
    // Chapters come from the markers, as one more input holding only metadata.
    if s.chapters && s.container.has_chapters() {
        if let Some(text) = chapters_metadata(&project.markers, range) {
            let meta = tmp.join("chapters.txt");
            std::fs::write(&meta, text)
                .map_err(|e| format!("Could not write the chapters: {e}"))?;
            let index = usize::from(video) + usize::from(sound);
            cmd.args(["-f", "ffmetadata", "-i"]).arg(&meta);
            cmd.args(["-map_metadata", &index.to_string()]);
        }
    }
    if let Some(enc) = &enc {
        if s.container == Container::Gif {
            cmd.args(["-filter_complex", "[0:v]split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a"]);
        } else {
            if !gpu_yuv {
                cmd.args([
                    "-vf",
                    &format!(
                        "scale=out_color_matrix=bt709:out_range=tv,format={}",
                        pixel_format(enc)
                    ),
                ]);
            }
            cmd.args(["-color_range", "tv"]);
            cmd.args([
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "bt709",
            ]);
        }
        cmd.args(encoder_args(enc, s));
    }
    if sound {
        if let Some(target) = s.loudness_target {
            progress(ExportProgress {
                stage: "Setting the loudness",
                done: 0,
                total: 1000,
                ..Default::default()
            });
            let filter = crate::loudness::normalize_filter(&wav, target)
                .map_err(|e| format!("Could not set the loudness: {e}"))?;
            cmd.args(["-af", &filter]);
        }
        cmd.args(s.container.audio_codec().args(s.audio_kbps));
        cmd.args(["-ar", &s.sample_rate.to_string()]);
    }
    if matches!(
        s.container,
        Container::Mp4 | Container::Mov | Container::M4a
    ) {
        cmd.args(["-movflags", "+faststart"]);
    }
    if video && sound {
        cmd.args(["-map", "0:v:0", "-map", "1:a:0", "-shortest"]);
    }
    cmd.arg(&s.path);
    cmd.stdin(if video { Stdio::piped() } else { Stdio::null() });
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    let stderr = child.stderr.take();
    let err_text = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = std::io::Read::read_to_string(&mut e, &mut s);
        }
        s
    });

    if video {
        let mut stdin = child.stdin.take().unwrap();
        let mut comp = Compositor::new(renderer);
        let target =
            (!gpu_yuv).then(|| comp.make_target(w, h, wgpu::TextureFormat::Rgba8Unorm, true));
        // Two sets of planes: one is read back while the next is drawn.
        let planes = gpu_yuv.then(|| [comp.make_nv12_target(w, h), comp.make_nv12_target(w, h)]);
        let mut pending: Option<(usize, wgpu::SubmissionIndex)> = None;
        let mut frames = StreamFrames::new(Duration::from_secs(20), true, s.hardware_decode);
        // Visualizers wait for their sound to decode rather than show silence.
        let levels = AudioLevels::default();
        levels.set_blocking(true);
        let total = fps.frames_at(range.1 - range.0).max(1) as u64;
        let out_scale = w as f32 / project.settings.width.max(1) as f32;
        let started = Instant::now();
        // Rendering and writing overlap: a writer thread feeds ffmpeg.
        let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(3);
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for buf in rx {
                stdin.write_all(&buf)?;
            }
            Ok(())
        });
        for i in 0..total {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let t = range.0 + fps.time_of_frame(i as i64);
            // Start the next clips' decoders early.
            let soon = t + Time::from_secs_f64(1.0);
            for track in project
                .tracks
                .iter()
                .filter(|tr| tr.kind == TrackKind::Video && !tr.hidden)
            {
                for c in track
                    .clips
                    .iter()
                    .filter(|c| c.start > t && c.start <= soon && !c.disabled)
                {
                    if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                        if m.kind == MediaKind::Video {
                            frames.prefetch(
                                c,
                                m,
                                c.source_time(c.start).as_secs_f64(),
                                decode_size(m, (w, h)),
                            );
                        }
                    }
                }
            }
            let mut planner = Planner {
                project,
                out_scale,
                frames: &mut frames,
                statics,
                auto_grade,
                levels: &levels,
                show_empty_cells: false,
                used_statics: Vec::new(),
            };
            let f = planner.build(t);
            frames.end_frame();
            for (k, img) in &f.statics {
                if !comp.has_static(*k) {
                    comp.store_static(*k, img);
                }
            }
            if let Some(planes) = &planes {
                // The card starts on this frame while the previous one is
                // collected; the next frame's decoding overlaps with it too.
                let slot = (i % 2) as usize;
                let index = comp.submit_nv12(&f.frame, &planes[slot]);
                if let Some((earlier, was)) = pending.replace((slot, index)) {
                    if tx.send(comp.read_nv12(&planes[earlier], was)).is_err() {
                        break;
                    }
                }
            } else if let Some(target) = &target {
                let px = comp.render_to_cpu(&f.frame, target);
                if tx.send(px).is_err() {
                    break;
                }
            }
            let el = started.elapsed().as_secs_f64();
            let rate = (i + 1) as f64 / el.max(1e-6);
            progress(ExportProgress {
                stage: "Rendering video",
                done: i + 1,
                total,
                fps: rate,
                eta: Some(Duration::from_secs_f64(
                    (total - i - 1) as f64 / rate.max(1e-6),
                )),
            });
        }
        if let (Some(planes), Some((slot, index))) = (&planes, pending) {
            let last = comp.read_nv12(&planes[slot], index);
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send(last);
            }
        }
        drop(tx);
        let _ = writer.join();
    } else {
        progress(ExportProgress {
            stage: "Encoding audio",
            done: 0,
            total: 1,
            ..Default::default()
        });
    }
    if cancel.load(Ordering::Relaxed) {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&s.path);
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("Export cancelled.".into());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&tmp);
    if !status.success() {
        let text = err_text.join().unwrap_or_default();
        let last: Vec<&str> = text.lines().rev().take(4).collect();
        return Err(format!(
            "ffmpeg could not write the file: {}",
            last.into_iter().rev().collect::<Vec<_>>().join(" ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(at: f64, name: &str) -> Marker {
        Marker::new(Time::from_secs_f64(at), name)
    }

    #[test]
    fn the_card_draws_video_colours_except_for_gif_and_prores() {
        assert!(gpu_draws_video_colours("libx264", Container::Mp4));
        assert!(gpu_draws_video_colours("h264_nvenc", Container::Mp4));
        assert!(gpu_draws_video_colours("h264_qsv", Container::Mp4));
        assert!(!gpu_draws_video_colours("gif", Container::Gif));
        assert!(!gpu_draws_video_colours("prores_ks", Container::Mov));
        assert!(!gpu_draws_video_colours(
            "prores_videotoolbox",
            Container::Mov
        ));
    }

    #[test]
    fn nvenc_spends_more_effort_on_higher_quality() {
        let mut s = ExportSettings::for_project(&Project::new("x"), PathBuf::new());
        let preset = |s: &ExportSettings| {
            let a = encoder_args("h264_nvenc", s);
            let i = a.iter().position(|x| x == "-preset").unwrap();
            a[i + 1].clone()
        };
        s.quality = Quality::Higher;
        assert_eq!(preset(&s), "p6");
        s.quality = Quality::Recommended;
        assert_eq!(preset(&s), "p5");
        s.quality = Quality::Lower;
        assert_eq!(preset(&s), "p4");
    }

    #[test]
    fn markers_become_chapters_inside_the_range() {
        let range = (Time::from_secs_f64(10.0), Time::from_secs_f64(70.0));
        let markers = [
            marker(5.0, "before"),
            marker(30.0, "Part two = the; best"),
            marker(20.0, "Intro"),
            marker(90.0, "after"),
        ];
        let text = chapters_metadata(&markers, range).unwrap();
        // "Intro" is 10 s into the range, so a Start chapter fills the beginning.
        assert!(text.starts_with(
            ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=10000\ntitle=Start"
        ));
        assert!(text.contains("START=10000\nEND=20000\ntitle=Intro"));
        assert!(text.contains("START=20000\nEND=60000\ntitle=Part two \\= the\\; best"));
        assert!(!text.contains("before") && !text.contains("after"));
        assert!(chapters_metadata(&[marker(500.0, "x")], range).is_none());
        assert!(chapters_metadata(&[], range).is_none());
    }

    #[test]
    fn ffmpeg_reads_the_chapters_we_write() {
        let Ok(tools) = tidycuts_media::Tools::get() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tidycuts-chap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let meta = dir.join("chapters.txt");
        let range = (Time::ZERO, Time::from_secs_f64(4.0));
        let text = chapters_metadata(&[marker(0.0, "One"), marker(2.0, "Two")], range).unwrap();
        std::fs::write(&meta, text).unwrap();
        let out = dir.join("out.mkv");
        let status = tools
            .ffmpeg_cmd()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x64:d=4",
                "-f",
                "ffmetadata",
                "-i",
            ])
            .arg(&meta)
            .args(["-map", "0:v", "-map_metadata", "1", "-c:v", "mpeg4"])
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success());
        let probe = tools
            .ffprobe_cmd()
            .args(["-show_chapters", "-of", "csv"])
            .arg(&out)
            .output()
            .unwrap();
        let listing = String::from_utf8_lossy(&probe.stdout).to_string();
        assert!(
            listing.contains("One") && listing.contains("Two"),
            "{listing}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
