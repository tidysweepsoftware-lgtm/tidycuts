//! Render previews: stretches of the timeline drawn ahead of time into small
//! video files, so heavy effects play smoothly.
//!
//! The timeline is cut into fixed chunks of a few seconds. A chunk's key is a
//! fingerprint of everything that can change how it looks (the clips over it,
//! their media, the project's size), so a saved file stays valid exactly as
//! long as that part of the project is untouched, and any edit there makes
//! it stale on its own. Playback swaps a ready chunk in for the live picture
//! (see `Playback::set_previews`); exports never use these files.

use crate::frames::{decode_size, StreamFrames};
use crate::levels::AudioLevels;
use crate::plan::Planner;
use crate::statics::Statics;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tidycuts_core::{BlendMode, Clip, Generator, MediaItem, MediaKind, Project, Time, TrackKind};
use tidycuts_media::Tools;
use tidycuts_render::{Compositor, Grade, Renderer};

/// How long one saved piece is, in seconds.
pub const CHUNK_SECS: f64 = 4.0;
/// Previews are drawn no wider than this, to keep the files small.
pub const MAX_WIDTH: u32 = 1280;

/// One piece of the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub start: Time,
    pub end: Time,
    /// Changes whenever anything that affects the look of this piece does.
    pub key: u64,
    /// Something here is hard to play live (effects, transitions, many layers,
    /// a very big video…).
    pub heavy: bool,
}

/// A ready piece, as playback wants it: a tiny project that plays the saved
/// file at the right place on the timeline.
#[derive(Debug, Clone)]
pub struct Cached {
    pub start: Time,
    pub end: Time,
    pub project: Arc<Project>,
}

/// The size previews are drawn at: the project's, but at most `MAX_WIDTH` wide.
pub fn preview_size(project: &Project) -> (u32, u32) {
    let (w, h) = (
        project.settings.width.max(2) as f64,
        project.settings.height.max(2) as f64,
    );
    let k = (MAX_WIDTH as f64 / w).min(1.0);
    (
        ((w * k).round() as u32).max(2) & !1,
        ((h * k).round() as u32).max(2) & !1,
    )
}

/// 64-bit FNV-1a: small, and the same on every machine and Rust version.
struct Fingerprint(u64);

impl Fingerprint {
    fn new() -> Self {
        Fingerprint(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, data: &[u8]) {
        for &b in data {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn num(&mut self, n: u64) {
        self.bytes(&n.to_le_bytes());
    }

    fn json<T: serde::Serialize>(&mut self, value: &T) {
        if let Ok(v) = serde_json::to_vec(value) {
            self.bytes(&v);
        }
    }

    /// A file's size and modification time: editing a picture or video in
    /// another program makes the previews over it stale.
    fn file(&mut self, path: &Path) {
        if let Ok(meta) = std::fs::metadata(path) {
            self.num(meta.len());
            let secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            self.num(secs);
        }
    }
}

fn overlaps(c: &Clip, start: Time, end: Time) -> bool {
    c.start < end && c.end() > start
}

fn key_of(project: &Project, start: Time, end: Time, size: (u32, u32)) -> u64 {
    let mut f = Fingerprint::new();
    f.bytes(env!("CARGO_PKG_VERSION").as_bytes());
    f.num(u64::from(size.0) << 32 | u64::from(size.1));
    f.num(start.0 as u64);
    f.num(end.0 as u64);
    f.json(&project.settings);
    // Sound drives visualizers, so every track counts, not only video.
    for t in &project.tracks {
        f.bytes(&[
            t.kind as u8,
            u8::from(t.hidden),
            u8::from(t.muted),
            u8::from(t.solo),
        ]);
        f.num(t.volume.to_bits());
        for c in t.clips.iter().filter(|c| overlaps(c, start, end)) {
            // Where a clip ends only matters to this piece if it ends here, or
            // if something about its look depends on its length. Cutting or
            // trimming the end of a clip further on leaves this piece alone.
            if c.end() >= end
                && c.ramp.is_none()
                && !c.reverse
                && c.anim_out.is_none()
                && c.transition_out.is_none()
            {
                let mut view = c.clone();
                view.source_out = view.source_in;
                f.json(&view);
            } else {
                f.json(c);
            }
            if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                f.json(m);
                f.file(&m.path);
                f.file(m.preview_path());
            }
        }
    }
    f.0
}

/// Whether a clip is hard to play live.
fn clip_is_heavy(c: &Clip, media: Option<&MediaItem>) -> bool {
    c.effects.iter().any(|e| e.enabled)
        || c.transition_in.is_some()
        || c.transition_out.is_some()
        || c.blend != BlendMode::Normal
        || c.video.mask.is_some()
        || c.video.motion_blur > 0.0
        || c.video.stabilize > 0.0
        || c.video.lens_correction != 0.0
        || c.video.auto_enhance > 0.0
        || c.video.deflicker
        || c.video.shadow.enabled
        || !c.enhance.is_off()
        || c.ramp.is_some()
        || c.reverse
        || (c.speed - 1.0).abs() > 1e-9
        || c.color.lut.is_some()
        || matches!(c.generator, Some(Generator::Visualizer(_)))
        || media.is_some_and(|m| m.wants_proxy())
}

/// Several layers at once, or any clip that is heavy by itself.
fn span_is_heavy(project: &Project, start: Time, end: Time) -> bool {
    let mut layers = 0;
    for t in project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && !t.hidden)
    {
        for c in t
            .clips
            .iter()
            .filter(|c| !c.disabled && overlaps(c, start, end))
        {
            layers += 1;
            let media = c.media_id.and_then(|m| project.media(m));
            if clip_is_heavy(c, media) {
                return true;
            }
        }
    }
    layers >= 3
}

/// Whether there is anything to draw over `start..end`, and nothing that only
/// shows while editing (empty split-screen cells are not drawn in a preview).
fn is_cacheable(project: &Project, start: Time, end: Time) -> bool {
    let mut any = false;
    for t in project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && !t.hidden)
    {
        for c in t
            .clips
            .iter()
            .filter(|c| !c.disabled && overlaps(c, start, end))
        {
            if c.is_placeholder() {
                return false;
            }
            any = true;
        }
    }
    any
}

/// Frames in a piece (rounded up so the last moment is covered).
fn frames_in(project: &Project, start: Time, end: Time) -> i64 {
    let fps = project.settings.frame_rate.as_f64();
    (((end - start).as_secs_f64() * fps) - 1e-6).ceil().max(1.0) as i64
}

/// The pieces of the timeline that have something to draw.
pub fn chunks(project: &Project) -> Vec<Chunk> {
    let total = project.duration();
    let fps = project.settings.frame_rate;
    let per = ((CHUNK_SECS * fps.as_f64()).round() as i64).max(1);
    let size = preview_size(project);
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let start = fps.time_of_frame(i * per);
        if start >= total {
            break;
        }
        let end = fps.time_of_frame((i + 1) * per).min(total);
        i += 1;
        if end <= start || !is_cacheable(project, start, end) {
            continue;
        }
        out.push(Chunk {
            start,
            end,
            key: key_of(project, start, end, size),
            heavy: span_is_heavy(project, start, end),
        });
    }
    out
}

/// Where the file for a piece with this key lives.
pub fn file_for(dir: &Path, key: u64) -> PathBuf {
    dir.join(format!("{key:016x}.mov"))
}

/// Whether the file for this key has been written completely.
pub fn is_ready(dir: &Path, key: u64) -> bool {
    std::fs::metadata(file_for(dir, key)).is_ok_and(|m| m.len() > 0)
}

/// The tiny project that plays a ready piece.
pub fn cached(project: &Project, chunk: &Chunk, dir: &Path) -> Cached {
    let fps = project.settings.frame_rate;
    let (w, h) = preview_size(project);
    let frames = frames_in(project, chunk.start, chunk.end);
    let mut media = MediaItem::new(
        file_for(dir, chunk.key),
        MediaKind::Video,
        fps.time_of_frame(frames),
    );
    media.width = w;
    media.height = h;
    media.frame_rate = Some(fps);
    // Written as full-range BT.709, which is what the file says it is.
    media.color_matrix = Some("bt709".into());
    media.full_range = true;
    let mut p = Project::new("preview");
    p.settings = project.settings.clone();
    // The clip must stay where it is, not slide to the start.
    p.settings.magnetic = false;
    let clip = Clip::new(&media, chunk.start);
    p.add_media(media);
    if let Some(track) = p.tracks.iter().find(|t| t.kind == TrackKind::Video) {
        let id = track.id;
        p.place_clip(clip, id, chunk.start);
    }
    Cached {
        start: chunk.start,
        end: chunk.end,
        project: Arc::new(p),
    }
}

/// Draws one piece and saves it. `progress` gets (frames done, frames total).
/// Writes to a temporary name first, so a half-written file is never used.
#[allow(clippy::too_many_arguments)]
pub fn render_chunk(
    project: &Project,
    chunk: &Chunk,
    dir: &Path,
    renderer: Arc<Renderer>,
    statics: &Statics,
    auto_grade: &dyn Fn(&MediaItem) -> Option<Grade>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let tools = Tools::get().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not make the preview folder: {e}"))?;
    let out = file_for(dir, chunk.key);
    let tmp = out.with_extension("part");
    let fps = project.settings.frame_rate;
    let (w, h) = preview_size(project);
    let total = frames_in(project, chunk.start, chunk.end) as u64;

    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
        .arg(format!("{w}x{h}"))
        .args(["-r", &format!("{}/{}", fps.num, fps.den), "-i", "pipe:0"])
        .args([
            "-vf",
            "scale=out_color_matrix=bt709:out_range=pc,format=yuvj420p",
            "-c:v",
            "mjpeg",
            "-q:v",
            "5",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-color_range",
            "pc",
            "-an",
            "-f",
            "mov",
        ])
        .arg(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("ffmpeg has no input")?;
    let stderr = child.stderr.take();
    let err_text = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = std::io::Read::read_to_string(&mut e, &mut s);
        }
        s
    });

    let mut comp = Compositor::new(renderer);
    let target = comp.make_target(w, h, wgpu::TextureFormat::Rgba8Unorm, true);
    // The same decoders live playback uses (proxies included), but waiting
    // for every frame rather than skipping.
    let mut frames = StreamFrames::new(Duration::from_secs(20), false, true);
    let levels = AudioLevels::default();
    levels.set_blocking(true);
    let out_scale = w as f32 / project.settings.width.max(1) as f32;
    let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(3);
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for buf in rx {
            stdin.write_all(&buf)?;
        }
        Ok(())
    });
    let mut stopped = false;
    for i in 0..total {
        if cancel.load(Ordering::Relaxed) {
            stopped = true;
            break;
        }
        let t = chunk.start + fps.time_of_frame(i as i64);
        // Start the decoders of clips that begin soon.
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
        let px = comp.render_to_cpu(&f.frame, &target);
        if tx.send(px).is_err() {
            break;
        }
        progress(i + 1, total);
    }
    drop(tx);
    let _ = writer.join();
    if stopped {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&tmp);
        return Err("Rendering was stopped.".into());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        let text = err_text.join().unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        let last: Vec<&str> = text.lines().rev().take(3).collect();
        return Err(format!(
            "ffmpeg could not write the preview: {}",
            last.into_iter().rev().collect::<Vec<_>>().join(" ")
        ));
    }
    std::fs::rename(&tmp, &out).map_err(|e| format!("Could not save the preview: {e}"))
}

/// The disk the preview files take, in bytes.
pub fn size_on_disk(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Deletes every preview file.
pub fn delete_all(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Keeps the folder under `max_bytes` by deleting the oldest files first, and
/// removes leftovers of renders that were cut short.
pub fn tidy(dir: &Path, max_bytes: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let path = e.path();
        let leftover = path.extension().is_some_and(|x| x == "part");
        if leftover
            && modified
                .elapsed()
                .is_ok_and(|age| age > Duration::from_secs(3600))
        {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        files.push((modified, meta.len(), path));
    }
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_core::{Effect, Generator, Project, Time};

    fn s(secs: f64) -> Time {
        Time::from_secs_f64(secs)
    }

    /// A project with a colour clip lasting `secs` on the main track.
    fn project(secs: f64) -> Project {
        let mut p = Project::new("t");
        p.settings.width = 640;
        p.settings.height = 360;
        let track = p.tracks[0].id;
        p.add_generated(
            Generator::Solid([0.2, 0.4, 0.8, 1.0]),
            s(secs),
            track,
            Time::ZERO,
        )
        .unwrap();
        p
    }

    #[test]
    fn the_timeline_is_cut_into_chunks_that_cover_it() {
        let p = project(10.0);
        let c = chunks(&p);
        assert_eq!(c.len(), 3); // 4 s + 4 s + 2 s
        assert_eq!(c[0].start, Time::ZERO);
        assert_eq!(c[0].end, c[1].start);
        assert_eq!(c[1].end, c[2].start);
        assert_eq!(c[2].end, s(10.0));
        // Different pieces have different keys.
        assert_ne!(c[0].key, c[1].key);
        assert_eq!(chunks(&Project::new("empty")), vec![]);
    }

    #[test]
    fn a_gap_with_nothing_in_it_has_no_chunk() {
        let mut p = project(2.0);
        let second = p.add_track(TrackKind::Video).unwrap();
        p.add_generated(Generator::Solid([1.0; 4]), s(2.0), second, s(12.0))
            .unwrap();
        let c = chunks(&p);
        // 0-4 holds the first clip, 4-8 and 8-12 hold nothing, 12-14 the second.
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].start, Time::ZERO);
        assert_eq!(c[1].start, s(12.0));
    }

    #[test]
    fn an_edit_only_changes_the_keys_of_the_chunks_it_touches() {
        let mut p = project(10.0);
        let before = chunks(&p);
        // An effect on the part of the clip that lies in the last chunk only:
        // split at 8 s, then change the second half.
        let first = p.tracks[0].clips[0].id;
        let second = p.split_clip(first, s(8.0)).unwrap();
        p.clip_mut(second)
            .unwrap()
            .effects
            .push(Effect::new("blur.gaussian"));
        let after = chunks(&p);
        assert_eq!(before[0].key, after[0].key);
        assert_eq!(before[1].key, after[1].key);
        assert_ne!(before[2].key, after[2].key);
        assert!(after[2].heavy && !after[0].heavy);
    }

    #[test]
    fn project_size_and_media_files_change_the_keys() {
        let mut p = project(3.0);
        let a = chunks(&p)[0].key;
        p.settings.width = 1280;
        p.settings.height = 720;
        let b = chunks(&p)[0].key;
        assert_ne!(a, b);
        // The playhead-independent parts stay the same run to run.
        assert_eq!(b, chunks(&p)[0].key);
    }

    #[test]
    fn previews_are_never_wider_than_the_limit() {
        let mut p = Project::new("big");
        p.settings.width = 3840;
        p.settings.height = 2160;
        assert_eq!(preview_size(&p), (1280, 720));
        p.settings.width = 640;
        p.settings.height = 360;
        assert_eq!(preview_size(&p), (640, 360));
    }

    #[test]
    fn a_ready_chunk_becomes_a_tiny_project_that_plays_the_file() {
        let p = project(10.0);
        let chunk = &chunks(&p)[1];
        let dir = std::env::temp_dir().join("tidycuts-previews-unit");
        let c = cached(&p, chunk, &dir);
        assert_eq!((c.start, c.end), (chunk.start, chunk.end));
        let clip = &c.project.tracks[0].clips[0];
        assert_eq!(clip.start, chunk.start, "the clip must stay at the chunk");
        assert!(clip.end() >= chunk.end);
        let media = c.project.media(clip.media_id.unwrap()).unwrap();
        assert_eq!(media.path, file_for(&dir, chunk.key));
    }

    #[test]
    fn old_files_are_removed_first_to_stay_under_the_limit() {
        let dir = std::env::temp_dir().join(format!("tidycuts-tidy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let older = dir.join("a.mov");
        let newer = dir.join("b.mov");
        std::fs::write(&older, vec![0u8; 600]).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&newer, vec![0u8; 600]).unwrap();
        assert_eq!(size_on_disk(&dir), 1200);
        tidy(&dir, 700);
        assert!(!older.exists() && newer.exists());
        delete_all(&dir);
        assert!(!dir.exists());
    }

    #[test]
    fn a_drawn_chunk_matches_the_live_picture() {
        let Ok(renderer) = tidycuts_render::gpu::headless() else {
            eprintln!("no graphics device here; skipping");
            return;
        };
        if Tools::get().is_err() {
            eprintln!("no ffmpeg here; skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("tidycuts-render-{}", std::process::id()));
        let statics = Statics::default();
        let auto = |_: &MediaItem| None;
        // Strong colours show a mix-up of video colour standards at once.
        for (n, colour) in [
            [0.9, 0.1, 0.1, 1.0],
            [0.1, 0.8, 0.2, 1.0],
            [0.1, 0.2, 0.9, 1.0],
            [0.9, 0.8, 0.1, 1.0],
        ]
        .into_iter()
        .enumerate()
        {
            let mut p = Project::new("t");
            p.settings.width = 320;
            p.settings.height = 180;
            let track = p.tracks[0].id;
            p.add_generated(
                Generator::Solid(colour),
                s(0.5 + n as f64 * 0.1),
                track,
                Time::ZERO,
            )
            .unwrap();
            let chunk = chunks(&p).remove(0);
            let mut last = (0, 0);
            render_chunk(
                &p,
                &chunk,
                &dir,
                renderer.clone(),
                &statics,
                &auto,
                &AtomicBool::new(false),
                |d, t| last = (d, t),
            )
            .unwrap();
            assert_eq!(last.0, last.1);
            assert!(is_ready(&dir, chunk.key));
            assert!(!file_for(&dir, chunk.key).with_extension("part").exists());

            // Play the saved file through the planner and compare with the real thing.
            let span = cached(&p, &chunk, &dir);
            let live = crate::snapshot::render(&p, s(0.3), None, renderer.clone(), &statics, &auto);
            let saved = crate::snapshot::render(
                &span.project,
                s(0.3),
                None,
                renderer.clone(),
                &statics,
                &auto,
            );
            assert_eq!((live.width, live.height), (saved.width, saved.height));
            let mid = ((live.height / 2) * live.width + live.width / 2) as usize * 4;
            for ch in 0..3 {
                let (a, b) = (live.rgba[mid + ch] as i32, saved.rgba[mid + ch] as i32);
                assert!(
                    (a - b).abs() <= 5,
                    "colour {colour:?}, channel {ch}: live {a}, saved {b}"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
