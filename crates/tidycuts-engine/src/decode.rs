//! Decoding video and audio through ffmpeg pipes.
//!
//! Video comes out as `yuv420p` (or RGBA for media with transparency) at
//! the size the preview or export needs, so a 4K file previewed in a 960 px
//! window only ships 960 px frames over the pipe. Hardware decoding is asked
//! for with `-hwaccel auto`; ffmpeg falls back to the CPU on its own.

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tidycuts_core::MediaItem;
use tidycuts_media::Tools;
use tidycuts_render::{Pixels, RgbaImage, YuvImage, YuvMatrix};

pub(crate) fn secs(s: f64) -> String {
    format!("{:.6}", s.max(0.0))
}

/// Kills the ffmpeg child when dropped.
pub(crate) struct KillOnDrop(pub Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// How to decode a media file's pictures.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoParams {
    pub path: PathBuf,
    /// Output size (even numbers).
    pub width: u32,
    pub height: u32,
    /// Keep transparency (decodes to RGBA instead of YUV).
    pub alpha: bool,
    pub matrix: YuvMatrix,
    /// Frames per second to produce (the file's own rate).
    pub fps: f64,
    pub hwaccel: bool,
    /// Extra filters before scaling.
    pub pre_filters: Vec<String>,
    /// Extra filters after scaling, e.g. `deflicker` or `deshake`, which
    /// are cheaper on the smaller picture.
    pub post_filters: Vec<String>,
    /// Scaling quality: bilinear for previews, lanczos for export.
    pub high_quality: bool,
}

impl VideoParams {
    pub fn for_media(media: &MediaItem, width: u32, height: u32) -> Self {
        VideoParams {
            path: media.preview_path().to_path_buf(),
            width: width.max(2) & !1,
            height: height.max(2) & !1,
            alpha: media.has_alpha,
            matrix: YuvMatrix::guess(media.color_matrix.as_deref(), media.height),
            fps: media
                .frame_rate
                .map(|f| f.as_f64())
                .filter(|f| *f > 1.0 && *f < 241.0)
                .unwrap_or(30.0),
            hwaccel: true,
            pre_filters: Vec::new(),
            post_filters: Vec::new(),
            high_quality: false,
        }
    }

    fn frame_bytes(&self) -> usize {
        if self.alpha {
            (self.width * self.height * 4) as usize
        } else {
            YuvImage::byte_len(self.width, self.height)
        }
    }

    fn filter_graph(&self, fps: bool) -> String {
        let mut f: Vec<String> = self.pre_filters.clone();
        let flags = if self.high_quality {
            "lanczos"
        } else {
            "bilinear"
        };
        f.push(format!(
            "scale={}:{}:flags={flags}:out_range=tv",
            self.width, self.height
        ));
        f.extend(self.post_filters.iter().cloned());
        if fps {
            f.push(format!("fps={:.6}", self.fps));
        }
        f.join(",")
    }

    fn command(
        &self,
        tools: &Tools,
        start: f64,
        duration: Option<f64>,
        frames: Option<u32>,
    ) -> Command {
        let mut cmd = tools.ffmpeg_cmd();
        // A proxy is a small intra-frame file: the graphics card decoder gains nothing on it,
        // and starting it costs about 90 ms on every jump (measured: 0.22 s against 0.32 s).
        let is_proxy = self
            .path
            .parent()
            .and_then(|d| d.file_name())
            .is_some_and(|n| n == "proxies");
        if self.hwaccel && !self.alpha && !is_proxy {
            cmd.args(["-hwaccel", "auto"]);
        }
        if start > 0.0 {
            cmd.args(["-ss", &secs(start)]);
        }
        if let Some(d) = duration {
            cmd.args(["-t", &secs(d)]);
        }
        cmd.arg("-i").arg(&self.path);
        if let Some(n) = frames {
            cmd.args(["-frames:v", &n.to_string()]);
        }
        cmd.args([
            "-an",
            "-sn",
            "-dn",
            "-vf",
            &self.filter_graph(frames.is_none()),
        ]);
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            if self.alpha { "rgba" } else { "yuv420p" },
            "pipe:1",
        ]);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
        cmd
    }

    pub(crate) fn wrap(&self, data: Vec<u8>) -> Pixels {
        if self.alpha {
            Pixels::Rgba(Arc::new(RgbaImage {
                width: self.width,
                height: self.height,
                pixels: data,
                premultiplied: false,
            }))
        } else {
            Pixels::Yuv420(Arc::new(YuvImage {
                width: self.width,
                height: self.height,
                data,
                matrix: self.matrix,
                full_range: false,
            }))
        }
    }
}

/// A decoded picture and the source time it shows, in seconds.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub time: f64,
    pub pixels: Pixels,
}

fn read_exact_or_eof(r: &mut impl Read, buf: &mut [u8]) -> bool {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => return false,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
    true
}

/// Decodes a few frames starting at `start` and returns them in order.
fn decode_range(
    params: &VideoParams,
    start: f64,
    len: f64,
    stop: &AtomicBool,
) -> Vec<DecodedFrame> {
    let Ok(tools) = Tools::get() else {
        return Vec::new();
    };
    let Ok(child) = params.command(tools, start, Some(len), None).spawn() else {
        return Vec::new();
    };
    let mut child = KillOnDrop(child);
    let mut out = child.0.stdout.take().unwrap();
    let mut frames = Vec::new();
    let n = params.frame_bytes();
    let mut i = 0;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let mut buf = vec![0u8; n];
        if !read_exact_or_eof(&mut out, &mut buf) {
            break;
        }
        frames.push(DecodedFrame {
            time: start + i as f64 / params.fps,
            pixels: params.wrap(buf),
        });
        i += 1;
    }
    frames
}

/// Decodes the single picture shown at `at` seconds (exact, not the nearest
/// keyframe).
pub fn decode_still(params: &VideoParams, at: f64) -> Option<Pixels> {
    let tools = Tools::get().ok()?;
    let mut p = params.clone();
    // A lone frame doesn't benefit from GPU decoding and setting it up costs
    // more than decoding on the CPU.
    p.hwaccel = false;
    let child = p.command(tools, at, None, Some(1)).spawn().ok()?;
    let mut child = KillOnDrop(child);
    let mut out = child.0.stdout.take()?;
    let mut buf = vec![0u8; p.frame_bytes()];
    if read_exact_or_eof(&mut out, &mut buf) {
        return Some(p.wrap(buf));
    }
    drop(child);
    // Asking for a time past the last frame gives nothing; step back.
    if at > 0.25 {
        return decode_still(params, at - 0.25);
    }
    None
}

/// Frames decoded ahead on a background thread, forwards or backwards.
pub struct VideoStream {
    rx: Receiver<DecodedFrame>,
    stop: Arc<AtomicBool>,
    pending: Option<DecodedFrame>,
    last: Option<DecodedFrame>,
    reverse: bool,
    pub params: VideoParams,
    /// Source time the stream started at.
    pub start: f64,
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl VideoStream {
    /// Starts decoding at `start` seconds. Reverse streams play backwards
    /// from `start` towards `floor`.
    pub fn open(params: VideoParams, start: f64, reverse: bool, floor: f64) -> VideoStream {
        let (tx, rx) = bounded::<DecodedFrame>(6);
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let p = params.clone();
        thread::Builder::new()
            .name("tidycuts-video-decode".into())
            .spawn(move || {
                if reverse {
                    // Decode one-second chunks backwards and hand out each
                    // chunk's frames last-first.
                    let mut end = start;
                    while end > floor + 1e-6 && !st.load(Ordering::Relaxed) {
                        let begin = (end - 1.0).max(floor);
                        let frames = decode_range(&p, begin, end - begin + 0.5 / p.fps, &st);
                        if frames.is_empty() {
                            break;
                        }
                        for f in frames.into_iter().rev().filter(|f| f.time <= end + 1e-6) {
                            if tx.send(f).is_err() {
                                return;
                            }
                        }
                        end = begin;
                    }
                    return;
                }
                let Ok(tools) = Tools::get() else {
                    return;
                };
                let Ok(child) = p.command(tools, start, None, None).spawn() else {
                    return;
                };
                let mut child = KillOnDrop(child);
                let mut out = child.0.stdout.take().unwrap();
                let n = p.frame_bytes();
                let mut i = 0u64;
                while !st.load(Ordering::Relaxed) {
                    let mut buf = vec![0u8; n];
                    if !read_exact_or_eof(&mut out, &mut buf) {
                        break;
                    }
                    let f = DecodedFrame {
                        time: start + i as f64 / p.fps,
                        pixels: p.wrap(buf),
                    };
                    i += 1;
                    if tx.send(f).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn decoder");
        VideoStream {
            rx,
            stop,
            pending: None,
            last: None,
            reverse,
            params,
            start,
        }
    }

    fn past(&self, f: &DecodedFrame, t: f64) -> bool {
        let half = 0.5 / self.params.fps;
        if self.reverse {
            f.time < t - half
        } else {
            f.time > t + half
        }
    }

    /// The frame showing at source time `t`, dropping older frames. Waits up
    /// to `wait` for the decoder to catch up; returns the newest frame it has
    /// when it can't.
    pub fn frame_at(&mut self, t: f64, wait: Duration) -> Option<DecodedFrame> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            if let Some(p) = self.pending.take() {
                if self.past(&p, t) {
                    self.pending = Some(p);
                    return self.last.clone();
                }
                self.last = Some(p);
            }
            let now = std::time::Instant::now();
            let got = if now >= deadline {
                self.rx.try_recv().map_err(|_| RecvTimeoutError::Timeout)
            } else {
                self.rx.recv_timeout(deadline - now)
            };
            match got {
                Ok(f) => self.pending = Some(f),
                Err(_) => return self.last.clone(),
            }
        }
    }

    /// Whether this stream can serve `t` without restarting (it is at or
    /// just behind `t` in its direction of play).
    pub fn can_reach(&self, t: f64) -> bool {
        let pos = self
            .pending
            .as_ref()
            .or(self.last.as_ref())
            .map(|f| f.time)
            .unwrap_or(self.start);
        let fd = 1.0 / self.params.fps;
        if self.reverse {
            t <= pos + fd && t > pos - 3.0
        } else {
            t >= pos - fd && t < pos + 3.0
        }
    }
}

/// How to decode a clip's sound: the filters come from the clip's audio
/// settings (speed, pitch, clean-up, equalizer, voice effects).
#[derive(Debug, Clone, PartialEq)]
pub struct AudioParams {
    pub path: PathBuf,
    pub stream: u32,
    pub sample_rate: u32,
    pub filters: Vec<String>,
}

/// Interleaved stereo f32 samples decoded ahead on a background thread.
pub struct AudioStream {
    rx: Receiver<Vec<f32>>,
    stop: Arc<AtomicBool>,
    buf: Vec<f32>,
    pos: usize,
    done: bool,
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl AudioStream {
    /// Decodes `len` seconds of source starting at `start` (both in source
    /// time, before the filters change the speed).
    pub fn open(p: &AudioParams, start: f64, len: Option<f64>) -> AudioStream {
        let (tx, rx) = bounded::<Vec<f32>>(16);
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let p = p.clone();
        thread::Builder::new()
            .name("tidycuts-audio-decode".into())
            .spawn(move || {
                let Ok(tools) = Tools::get() else {
                    return;
                };
                let mut cmd = tools.ffmpeg_cmd();
                if start > 0.0 {
                    cmd.args(["-ss", &secs(start)]);
                }
                if let Some(l) = len {
                    cmd.args(["-t", &secs(l)]);
                }
                cmd.arg("-i").arg(&p.path);
                cmd.args(["-vn", "-sn", "-dn", "-map", &format!("0:a:{}?", p.stream)]);
                let mut filters = p.filters.clone();
                filters.push(format!(
                    "aresample={}:async=1,aformat=sample_fmts=flt:channel_layouts=stereo",
                    p.sample_rate
                ));
                cmd.args(["-af", &filters.join(",")]);
                cmd.args([
                    "-f",
                    "f32le",
                    "-ac",
                    "2",
                    "-ar",
                    &p.sample_rate.to_string(),
                    "pipe:1",
                ]);
                cmd.stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null());
                let Ok(child) = cmd.spawn() else {
                    return;
                };
                let mut child = KillOnDrop(child);
                let mut out = child.0.stdout.take().unwrap();
                let mut bytes = vec![0u8; 8192 * 4];
                let mut carry: Vec<u8> = Vec::new();
                while !st.load(Ordering::Relaxed) {
                    let n = match out.read(&mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    carry.extend_from_slice(&bytes[..n]);
                    let whole = carry.len() / 4 * 4;
                    let samples: Vec<f32> = carry[..whole]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();
                    carry.drain(..whole);
                    if tx.send(samples).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn audio decoder");
        AudioStream {
            rx,
            stop,
            buf: Vec::new(),
            pos: 0,
            done: false,
        }
    }

    /// Copies up to `out.len()` samples, waiting up to `wait` for data.
    /// Returns how many were written; fewer means the stream is behind or
    /// finished (see [`AudioStream::finished`]).
    pub fn read(&mut self, out: &mut [f32], wait: Duration) -> usize {
        let mut written = 0;
        let deadline = std::time::Instant::now() + wait;
        while written < out.len() {
            if self.pos >= self.buf.len() {
                if self.done {
                    break;
                }
                let now = std::time::Instant::now();
                let got = if now >= deadline {
                    self.rx.try_recv().map_err(|e| match e {
                        crossbeam_channel::TryRecvError::Empty => RecvTimeoutError::Timeout,
                        crossbeam_channel::TryRecvError::Disconnected => {
                            RecvTimeoutError::Disconnected
                        }
                    })
                } else {
                    self.rx.recv_timeout(deadline - now)
                };
                match got {
                    Ok(b) => {
                        self.buf = b;
                        self.pos = 0;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        self.done = true;
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                }
                continue;
            }
            let n = (self.buf.len() - self.pos).min(out.len() - written);
            out[written..written + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            written += n;
        }
        written
    }

    pub fn finished(&self) -> bool {
        self.done && self.pos >= self.buf.len()
    }
}

/// Reads a whole file's audio as mono peaks for waveforms: one value (max
/// absolute sample) per `1 / per_sec` seconds.
pub fn waveform(path: &Path, stream: u32, per_sec: u32) -> Vec<f32> {
    let p = AudioParams {
        path: path.to_path_buf(),
        stream,
        sample_rate: 8000,
        filters: Vec::new(),
    };
    let mut s = AudioStream::open(&p, 0.0, None);
    let window = (8000 / per_sec.max(1)) as usize * 2;
    let mut out = Vec::new();
    let mut buf = vec![0f32; window];
    loop {
        let n = s.read(&mut buf, Duration::from_secs(5));
        if n == 0 {
            break;
        }
        out.push(buf[..n].iter().fold(0f32, |m, v| m.max(v.abs())));
        if n < window && s.finished() {
            break;
        }
    }
    out
}

/// Where the peaks of a file's audio stream are kept: named by the file's path, size
/// and change time, so an edited file gets new peaks.
fn peaks_file(cache_dir: &Path, path: &Path, stream: u32, per_sec: u32) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    (stream, per_sec).hash(&mut h);
    if let Ok(m) = std::fs::metadata(path) {
        m.len().hash(&mut h);
        if let Ok(t) = m.modified() {
            t.hash(&mut h);
        }
    }
    cache_dir
        .join("waveforms")
        .join(format!("{:016x}.peaks", h.finish()))
}

const PEAKS_MAGIC: &[u8; 4] = b"TCW1";

fn read_peaks(file: &Path) -> Option<Vec<f32>> {
    let bytes = std::fs::read(file).ok()?;
    let body = bytes.strip_prefix(PEAKS_MAGIC)?;
    if body.len() % 4 != 0 {
        return None;
    }
    Some(
        body.as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
    )
}

fn write_peaks(file: &Path, peaks: &[f32]) {
    let Some(dir) = file.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let mut bytes = Vec::with_capacity(4 + peaks.len() * 4);
    bytes.extend_from_slice(PEAKS_MAGIC);
    for p in peaks {
        bytes.extend_from_slice(&p.to_le_bytes());
    }
    // Written aside and renamed, so a half-written file is never read.
    let tmp = file.with_extension("tmp");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, file);
    }
}

/// Like [`waveform`], but the peaks are kept in `cache_dir`: the whole sound of a
/// file is decoded once, not every time the program starts.
pub fn waveform_cached(cache_dir: &Path, path: &Path, stream: u32, per_sec: u32) -> Vec<f32> {
    let file = peaks_file(cache_dir, path, stream, per_sec);
    if let Some(p) = read_peaks(&file).filter(|p| !p.is_empty()) {
        return p;
    }
    let peaks = waveform(path, stream, per_sec);
    if !peaks.is_empty() {
        write_peaks(&file, &peaks);
    }
    peaks
}

#[cfg(test)]
mod peaks_tests {
    use super::*;

    #[test]
    fn peaks_survive_a_round_trip_and_bad_files_are_ignored() {
        let dir = std::env::temp_dir().join(format!("tidycuts-peaks-{}", std::process::id()));
        let file = dir.join("waveforms").join("x.peaks");
        let peaks = vec![0.0, 0.25, 1.0, 0.5];
        write_peaks(&file, &peaks);
        assert_eq!(read_peaks(&file), Some(peaks));
        std::fs::write(&file, b"nope").unwrap();
        assert_eq!(read_peaks(&file), None);
        std::fs::write(&file, b"TCW1abc").unwrap();
        assert_eq!(read_peaks(&file), None);
        assert_eq!(read_peaks(&dir.join("missing")), None);
        // Different streams and rates are different files.
        let p = Path::new("/a.mp4");
        assert_ne!(peaks_file(&dir, p, 0, 100), peaks_file(&dir, p, 1, 100));
        assert_ne!(peaks_file(&dir, p, 0, 100), peaks_file(&dir, p, 0, 50));
        let _ = std::fs::remove_dir_all(dir);
    }
}
