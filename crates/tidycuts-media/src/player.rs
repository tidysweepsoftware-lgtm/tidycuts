//! Real-time playback of a list of segments (one source file, or the clips
//! on the timeline) with audio as the master clock.

use crate::audio::{AudioOut, AudioQueue};
use crate::ffmpeg::{secs_arg, Tools};
use crate::frames::{fit_within, grab_frame, scale_filter, RgbaFrame};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tidycuts_core::{FrameRate, Time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Video,
    Image,
    Audio,
}

/// A span of the output that comes from one file.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub path: PathBuf,
    pub kind: SegmentKind,
    /// Where the segment starts in the output.
    pub start: Time,
    /// Where in the file it starts.
    pub source_in: Time,
    pub duration: Time,
    /// Source picture size, used to pick the preview size.
    pub size: (u32, u32),
    pub has_audio: bool,
}

impl Segment {
    pub fn end(&self) -> Time {
        self.start + self.duration
    }
}

/// What the preview should show.
#[derive(Debug, Clone)]
pub enum Picture {
    Frame(Arc<RgbaFrame>),
    Black,
}

struct TimedPicture {
    pts: Time,
    picture: Picture,
}

/// Kills the ffmpeg child when dropped so stopped playback leaves nothing running.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Session {
    stop: Arc<AtomicBool>,
    frames: Receiver<TimedPicture>,
    pending: Option<TimedPicture>,
    started_at: Time,
    wall_start: Instant,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

pub struct Player {
    video: Vec<Segment>,
    audio: Vec<Segment>,
    duration: Time,
    fps: FrameRate,
    max_size: (u32, u32),
    audio_out: Option<AudioOut>,
    audio_tried: bool,
    session: Option<Session>,
    position: Time,
    picture: Picture,
    /// Still frame requested while paused.
    still_rx: Option<Receiver<(Time, Picture)>>,
    still_for: Option<Time>,
}

impl Default for Player {
    fn default() -> Self {
        Player::new()
    }
}

impl Player {
    pub fn new() -> Self {
        Player {
            video: Vec::new(),
            audio: Vec::new(),
            duration: Time::ZERO,
            fps: FrameRate::new(25, 1),
            max_size: (960, 540),
            audio_out: None,
            audio_tried: false,
            session: None,
            position: Time::ZERO,
            picture: Picture::Black,
            still_rx: None,
            still_for: None,
        }
    }

    /// Replaces what's being played. Keeps the playhead where it is (clamped)
    /// and refreshes the still frame.
    pub fn load(
        &mut self,
        video: Vec<Segment>,
        audio: Vec<Segment>,
        duration: Time,
        fps: FrameRate,
    ) {
        let same = self.video == video
            && self.audio == audio
            && self.duration == duration
            && self.fps == fps;
        if same {
            return;
        }
        let was_playing = self.is_playing();
        self.session = None;
        self.video = video;
        self.audio = audio;
        self.duration = duration;
        self.fps = fps;
        self.position = self.position.min(duration);
        if was_playing {
            self.play();
        } else {
            self.request_still(self.position);
        }
    }

    /// Size the preview decodes at. Bigger looks sharper but costs more CPU.
    pub fn set_max_size(&mut self, w: u32, h: u32) {
        self.max_size = (w.max(16), h.max(16));
    }

    pub fn duration(&self) -> Time {
        self.duration
    }

    pub fn position(&self) -> Time {
        self.position
    }

    pub fn picture(&self) -> &Picture {
        &self.picture
    }

    pub fn is_playing(&self) -> bool {
        self.session.is_some()
    }

    pub fn has_audio_device(&self) -> bool {
        self.audio_out.is_some()
    }

    pub fn toggle(&mut self) {
        if self.is_playing() {
            self.pause()
        } else {
            self.play()
        }
    }

    pub fn play(&mut self) {
        if self.duration <= Time::ZERO {
            return;
        }
        if self.position >= self.duration - self.fps.frame_duration() {
            self.position = Time::ZERO;
        }
        if !self.audio_tried {
            self.audio_tried = true;
            self.audio_out = AudioOut::open();
        }
        self.session = None;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = bounded(4);
        let start = self.position;
        {
            let (segs, stop, fps, max, end) = (
                self.video.clone(),
                stop.clone(),
                self.fps,
                self.max_size,
                self.duration,
            );
            thread::spawn(move || video_thread(segs, start, end, fps, max, tx, stop));
        }
        if let Some(out) = &self.audio_out {
            out.queue.clear();
            out.queue.set_paused(false);
            let (segs, stop, q, rate, ch, end) = (
                self.audio.clone(),
                stop.clone(),
                out.queue.clone(),
                out.sample_rate,
                out.channels,
                self.duration,
            );
            thread::spawn(move || audio_thread(segs, start, end, rate, ch, q, stop));
        }
        self.session = Some(Session {
            stop,
            frames: rx,
            pending: None,
            started_at: start,
            wall_start: Instant::now(),
        });
    }

    pub fn pause(&mut self) {
        if self.session.take().is_some() {
            if let Some(out) = &self.audio_out {
                out.queue.set_paused(true);
                out.queue.clear();
            }
            self.position = self.fps.snap(self.position);
        }
    }

    /// Stops and returns to the start, like the preview's stop button.
    pub fn stop(&mut self) {
        self.pause();
        self.seek(Time::ZERO);
    }

    pub fn seek(&mut self, t: Time) {
        let t = t.max(Time::ZERO).min(self.duration);
        let playing = self.is_playing();
        self.pause();
        self.position = t;
        if playing {
            self.play();
        } else {
            self.request_still(t);
        }
    }

    pub fn step_frames(&mut self, n: i64) {
        let f = self.fps.frames_at(self.position) + n;
        self.seek(self.fps.time_of_frame(f.max(0)));
    }

    fn request_still(&mut self, t: Time) {
        if self.still_for == Some(t) && self.still_rx.is_some() {
            return;
        }
        let (tx, rx) = bounded(1);
        self.still_rx = Some(rx);
        self.still_for = Some(t);
        let seg = self
            .video
            .iter()
            .find(|s| t >= s.start && t < s.end())
            .cloned();
        let max = self.max_size;
        thread::spawn(move || {
            let pic = match seg {
                Some(s) => {
                    let at = if s.kind == SegmentKind::Image {
                        0.0
                    } else {
                        (s.source_in + (t - s.start)).as_secs_f64()
                    };
                    match grab_frame(&s.path, at, s.size, max.0, max.1) {
                        Ok(f) => Picture::Frame(Arc::new(f)),
                        Err(e) => {
                            log::warn!("preview frame failed: {e}");
                            Picture::Black
                        }
                    }
                }
                None => Picture::Black,
            };
            let _ = tx.send((t, pic));
        });
    }

    fn clock(&self, s: &Session) -> Time {
        match &self.audio_out {
            Some(out) => {
                let secs = out.queue.played_frames() as f64 / out.sample_rate as f64;
                s.started_at + Time::from_secs_f64(secs)
            }
            None => s.started_at + Time::from_secs_f64(s.wall_start.elapsed().as_secs_f64()),
        }
    }

    /// Advances playback. Call once per UI frame; returns true when the
    /// picture changed and should be re-uploaded.
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.still_rx {
            if let Ok((t, pic)) = rx.try_recv() {
                if !self.is_playing() && t == self.position {
                    self.picture = pic;
                    changed = true;
                }
                self.still_rx = None;
            }
        }
        let Some(mut s) = self.session.take() else {
            return changed;
        };
        let now = self.clock(&s).min(self.duration);
        self.position = now;
        let mut finished = false;
        loop {
            if s.pending.is_none() {
                match s.frames.try_recv() {
                    Ok(p) => s.pending = Some(p),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
            if s.pending.as_ref().is_some_and(|p| p.pts <= now) {
                self.picture = s.pending.take().unwrap().picture;
                changed = true;
            } else {
                break;
            }
        }
        if now >= self.duration || (finished && self.audio_out.is_none() && now >= self.duration) {
            self.session = Some(s);
            self.pause();
            self.position = self.duration;
            self.still_for = None;
        } else {
            self.session = Some(s);
        }
        changed
    }
}

/// Segments that are at least partly after `from`, with their skip offset.
fn remaining(segs: &[Segment], from: Time) -> impl Iterator<Item = (&Segment, Time)> {
    segs.iter()
        .filter(move |s| s.end() > from)
        .map(move |s| (s, (from - s.start).max(Time::ZERO)))
}

fn video_thread(
    segs: Vec<Segment>,
    from: Time,
    end: Time,
    fps: FrameRate,
    max: (u32, u32),
    tx: Sender<TimedPicture>,
    stop: Arc<AtomicBool>,
) {
    let send = |pts: Time, picture: Picture| -> bool {
        !stop.load(Ordering::SeqCst) && tx.send(TimedPicture { pts, picture }).is_ok()
    };
    let mut cursor = from;
    for (seg, skip) in remaining(&segs, from) {
        if seg.start > cursor && !send(cursor, Picture::Black) {
            return;
        }
        let seg_from = seg.start + skip;
        match seg.kind {
            SegmentKind::Image => {
                let pic = grab_frame(&seg.path, 0.0, seg.size, max.0, max.1)
                    .map(|f| Picture::Frame(Arc::new(f)))
                    .unwrap_or(Picture::Black);
                if !send(seg_from, pic) {
                    return;
                }
            }
            SegmentKind::Video => {
                if !stream_video(seg, skip, fps, max, &send) {
                    return;
                }
            }
            SegmentKind::Audio => continue,
        }
        cursor = seg.end();
    }
    if cursor < end {
        let _ = send(cursor, Picture::Black);
    }
}

/// Decodes one video segment frame by frame. Returns false if playback stopped.
fn stream_video(
    seg: &Segment,
    skip: Time,
    fps: FrameRate,
    max: (u32, u32),
    send: &dyn Fn(Time, Picture) -> bool,
) -> bool {
    let Ok(tools) = Tools::get() else {
        return send(seg.start + skip, Picture::Black);
    };
    let (w, h) = fit_within(seg.size.0, seg.size.1, max.0, max.1);
    let len = seg.duration - skip;
    let filter = format!("fps={}/{},{}", fps.num, fps.den, scale_filter(w, h));
    let child = tools
        .ffmpeg_cmd()
        .args(["-ss", &secs_arg((seg.source_in + skip).as_secs_f64())])
        .arg("-i")
        .arg(&seg.path)
        .args([
            "-t",
            &secs_arg(len.as_secs_f64()),
            "-an",
            "-vf",
            &filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => KillOnDrop(c),
        Err(e) => {
            log::warn!("could not start ffmpeg: {e}");
            return send(seg.start + skip, Picture::Black);
        }
    };
    let mut out = child.0.stdout.take().unwrap();
    let frame_len = RgbaFrame::byte_len(w, h);
    let first = seg.start + skip;
    let mut index: i64 = 0;
    loop {
        let mut buf = vec![0u8; frame_len];
        if out.read_exact(&mut buf).is_err() {
            return true;
        }
        let pts = first + Time::from_secs_f64(index as f64 / fps.as_f64());
        if pts >= seg.end() {
            return true;
        }
        if !send(
            pts,
            Picture::Frame(Arc::new(RgbaFrame {
                width: w,
                height: h,
                pixels: buf,
            })),
        ) {
            return false;
        }
        index += 1;
    }
}

fn audio_thread(
    segs: Vec<Segment>,
    from: Time,
    end: Time,
    rate: u32,
    channels: u16,
    queue: Arc<AudioQueue>,
    stop: Arc<AtomicBool>,
) {
    let ch = channels as usize;
    // Keep about half a second queued; more makes seeking feel laggy.
    let high_water = rate as usize * ch / 2;
    let push = |data: &[f32]| -> bool {
        for chunk in data.chunks(4096) {
            while queue.buffered() > high_water {
                if stop.load(Ordering::SeqCst) {
                    return false;
                }
                thread::sleep(Duration::from_millis(5));
            }
            if stop.load(Ordering::SeqCst) {
                return false;
            }
            queue.push(chunk);
        }
        true
    };
    let silence = |len: Time| -> bool {
        let frames = (len.as_secs_f64() * rate as f64).round() as usize;
        let block = vec![0.0f32; 4096 * ch];
        let mut left = frames * ch;
        while left > 0 {
            let n = left.min(block.len());
            if !push(&block[..n]) {
                return false;
            }
            left -= n;
        }
        true
    };
    let mut cursor = from;
    let audible = segs
        .iter()
        .filter(|s| s.has_audio && s.kind != SegmentKind::Image)
        .cloned()
        .collect::<Vec<_>>();
    for (seg, skip) in remaining(&audible, from) {
        let seg_from = seg.start + skip;
        if seg_from > cursor && !silence(seg_from - cursor) {
            return;
        }
        let len = seg.duration - skip;
        if !stream_audio(seg, skip, len, rate, channels, &push) {
            return;
        }
        // Pad if the file was shorter than the segment claims.
        let pushed_until = seg_from + len;
        cursor = pushed_until;
    }
    if cursor < end {
        silence(end - cursor);
    }
}

fn stream_audio(
    seg: &Segment,
    skip: Time,
    len: Time,
    rate: u32,
    channels: u16,
    push: &dyn Fn(&[f32]) -> bool,
) -> bool {
    let want_samples = (len.as_secs_f64() * rate as f64).round() as usize * channels as usize;
    let Ok(tools) = Tools::get() else {
        return push(&vec![0.0; want_samples]);
    };
    let child = tools
        .ffmpeg_cmd()
        .args(["-ss", &secs_arg((seg.source_in + skip).as_secs_f64())])
        .arg("-i")
        .arg(&seg.path)
        .args([
            "-t",
            &secs_arg(len.as_secs_f64()),
            "-vn",
            "-ac",
            &channels.to_string(),
            "-ar",
            &rate.to_string(),
        ])
        .args(["-f", "f32le", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => KillOnDrop(c),
        Err(_) => return push(&vec![0.0; want_samples]),
    };
    let mut out = child.0.stdout.take().unwrap();
    let mut bytes = vec![0u8; 16384];
    let mut carry: Vec<u8> = Vec::new();
    let mut sent = 0usize;
    loop {
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
        let take = samples.len().min(want_samples - sent);
        if !push(&samples[..take]) {
            return false;
        }
        sent += take;
        if sent >= want_samples {
            return true;
        }
    }
    // Keep the clock in step if the file ended early.
    if sent < want_samples {
        return push(&vec![0.0; want_samples - sent]);
    }
    true
}
