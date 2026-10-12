//! Real-time playback of the timeline with the sound card as the clock.
//!
//! One thread mixes audio ahead into the output queue; another plans video
//! frames ahead from streaming decoders. The UI thread asks for the frame
//! matching the clock on every repaint and draws it with the compositor.

use crate::frames::{decode_size, StreamFrames};
use crate::levels::AudioLevels;
use crate::mixer::{LoudnessTable, Meters, Mixer};
use crate::plan::{Planner, PreparedFrame};
use crate::previews::Cached;
use crate::statics::Statics;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tidycuts_core::{MediaKind, Project, Time, TrackKind};
use tidycuts_media::audio::{AudioOut, AudioQueue};
use tidycuts_render::Grade;

type AutoGrades = Arc<dyn Fn(&tidycuts_core::MediaItem) -> Option<Grade> + Send + Sync>;

struct Shared {
    project: Mutex<Arc<Project>>,
    playing: AtomicBool,
    quit: AtomicBool,
    /// Bumped on every start or seek so the threads restart from `origin`.
    epoch: AtomicU64,
    origin: Mutex<Time>,
    wall_start: Mutex<Instant>,
    out_size: Mutex<(u32, u32)>,
    hwaccel: AtomicBool,
    /// The hearing guard's loudest level in hundredths of a LUFS (`i32::MIN` = off).
    guard: AtomicI32,
    /// Rendered previews that stand in for the live picture where they cover it.
    previews: Mutex<Arc<Vec<Cached>>>,
}

pub struct Playback {
    shared: Arc<Shared>,
    /// Frames come tagged with the start/seek (epoch) they were planned for,
    /// so one planned before a seek is never shown after it.
    frames: Receiver<(u64, PreparedFrame)>,
    pending: Option<(u64, PreparedFrame)>,
    audio: Option<AudioOut>,
    queue: Option<Arc<AudioQueue>>,
    sample_rate: u32,
    meters: Arc<Meters>,
    /// Shown last; kept so a repaint without a new frame redraws it.
    last_time: Time,
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::SeqCst);
        self.shared.playing.store(false, Ordering::SeqCst);
    }
}

impl Playback {
    pub fn new(
        project: Arc<Project>,
        statics: Arc<Statics>,
        loudness: LoudnessTable,
        auto_grades: AutoGrades,
        levels: AudioLevels,
        repaint: Arc<dyn Fn() + Send + Sync>,
    ) -> Playback {
        let audio = AudioOut::open();
        let queue = audio.as_ref().map(|a| a.queue.clone());
        let sample_rate = audio.as_ref().map(|a| a.sample_rate).unwrap_or(48_000);
        let channels = audio.as_ref().map(|a| a.channels as usize).unwrap_or(2);
        let shared = Arc::new(Shared {
            project: Mutex::new(project),
            playing: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            origin: Mutex::new(Time::ZERO),
            wall_start: Mutex::new(Instant::now()),
            out_size: Mutex::new((960, 540)),
            hwaccel: AtomicBool::new(true),
            guard: AtomicI32::new(i32::MIN),
            previews: Mutex::new(Arc::new(Vec::new())),
        });
        let (tx, rx) = bounded::<(u64, PreparedFrame)>(4);
        {
            let shared = shared.clone();
            let queue = queue.clone();
            thread::Builder::new()
                .name("tidycuts-play-video".into())
                .spawn(move || {
                    video_thread(
                        shared,
                        tx,
                        queue,
                        sample_rate,
                        statics,
                        auto_grades,
                        levels,
                        repaint,
                    )
                })
                .expect("spawn video thread");
        }
        let meters = Arc::new(Meters::default());
        if let Some(q) = queue.clone() {
            let shared = shared.clone();
            let meters = meters.clone();
            thread::Builder::new()
                .name("tidycuts-play-audio".into())
                .spawn(move || audio_thread(shared, q, sample_rate, channels, loudness, meters))
                .expect("spawn audio thread");
        }
        Playback {
            shared,
            frames: rx,
            pending: None,
            audio,
            queue,
            sample_rate,
            meters,
            last_time: Time::ZERO,
        }
    }

    /// The levels of what is being played, for the mixer's meters.
    pub fn meters(&self) -> Arc<Meters> {
        self.meters.clone()
    }

    pub fn has_audio_device(&self) -> bool {
        self.audio.is_some()
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::SeqCst)
    }

    pub fn set_project(&self, project: Arc<Project>) {
        *self.shared.project.lock().unwrap() = project;
    }

    /// The rendered previews to play instead of the live picture. Sound is
    /// always mixed from the real project.
    pub fn set_previews(&self, previews: Vec<Cached>) {
        *self.shared.previews.lock().unwrap() = Arc::new(previews);
    }

    /// Output size of the preview, in pixels.
    pub fn set_output_size(&self, w: u32, h: u32) {
        *self.shared.out_size.lock().unwrap() = (w.max(2), h.max(2));
    }

    /// Turns the hearing guard on, with the loudest allowed level in LUFS, or
    /// off (`None`). It only protects playback; exports are not touched.
    pub fn set_hearing_guard(&self, ceiling: Option<f32>) {
        let v = ceiling.map_or(i32::MIN, |c| (c * 100.0).round() as i32);
        self.shared.guard.store(v, Ordering::SeqCst);
    }

    pub fn set_hardware_decoding(&self, on: bool) {
        self.shared.hwaccel.store(on, Ordering::SeqCst);
    }

    /// Starts playing from `at`.
    pub fn play(&mut self, at: Time) {
        *self.shared.origin.lock().unwrap() = at;
        *self.shared.wall_start.lock().unwrap() = Instant::now();
        if let Some(q) = &self.queue {
            q.clear();
            q.set_paused(false);
        }
        self.pending = None;
        while self.frames.try_recv().is_ok() {}
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
        self.shared.playing.store(true, Ordering::SeqCst);
        self.last_time = at;
    }

    /// Stops and returns where playback got to.
    pub fn pause(&mut self) -> Time {
        let t = self.position();
        self.shared.playing.store(false, Ordering::SeqCst);
        self.shared.epoch.fetch_add(1, Ordering::SeqCst);
        if let Some(q) = &self.queue {
            q.set_paused(true);
            q.clear();
        }
        self.pending = None;
        while self.frames.try_recv().is_ok() {}
        t
    }

    /// The playback clock: sound played so far, or the wall clock without
    /// a sound device.
    pub fn position(&self) -> Time {
        let origin = *self.shared.origin.lock().unwrap();
        if !self.is_playing() {
            return self.last_time;
        }
        match &self.queue {
            Some(q) => {
                origin + Time::from_secs_f64(q.played_frames() as f64 / self.sample_rate as f64)
            }
            None => {
                origin
                    + Time::from_secs_f64(
                        self.shared
                            .wall_start
                            .lock()
                            .unwrap()
                            .elapsed()
                            .as_secs_f64(),
                    )
            }
        }
    }

    /// The newest prepared frame that is due, if a new one is.
    pub fn poll(&mut self) -> Option<PreparedFrame> {
        if !self.is_playing() {
            return None;
        }
        let now = self.position();
        let epoch_origin = *self.shared.origin.lock().unwrap();
        let epoch = self.shared.epoch.load(Ordering::SeqCst);
        let mut best: Option<PreparedFrame> = None;
        loop {
            let (tag, f) = match self.pending.take() {
                Some(f) => f,
                None => match self.frames.try_recv() {
                    Ok(f) => f,
                    Err(_) => break,
                },
            };
            if tag != epoch || f.time < epoch_origin {
                continue;
            }
            if f.time <= now + Time::from_millis(8) {
                best = Some(f);
            } else {
                self.pending = Some((tag, f));
                break;
            }
        }
        if let Some(f) = &best {
            self.last_time = f.time;
        }
        best
    }
}

fn audio_thread(
    shared: Arc<Shared>,
    queue: Arc<AudioQueue>,
    sr: u32,
    channels: usize,
    loudness: LoudnessTable,
    meters: Arc<Meters>,
) {
    let mut mixer = Mixer::new(sr, Duration::from_millis(60), loudness);
    mixer.meters = Some(meters.clone());
    let mut epoch = u64::MAX;
    let mut cursor = Time::ZERO;
    let block = 1024usize;
    let mut stereo = vec![0f32; block * 2];
    let mut out = vec![0f32; block * channels];
    while !shared.quit.load(Ordering::Relaxed) {
        if !shared.playing.load(Ordering::Relaxed) {
            meters.clear();
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let e = shared.epoch.load(Ordering::SeqCst);
        if e != epoch {
            epoch = e;
            mixer.reset();
            cursor = *shared.origin.lock().unwrap();
        }
        // Keep about a fifth of a second queued.
        if queue.buffered() > (sr as usize / 5) * channels {
            thread::sleep(Duration::from_millis(4));
            continue;
        }
        let project = shared.project.lock().unwrap().clone();
        let g = shared.guard.load(Ordering::Relaxed);
        mixer.set_guard((g != i32::MIN).then(|| g as f32 / 100.0));
        mixer.mix(&project, cursor, &mut stereo);
        if shared.epoch.load(Ordering::SeqCst) != epoch {
            continue;
        }
        for i in 0..block {
            let (l, r) = (stereo[i * 2], stereo[i * 2 + 1]);
            let o = &mut out[i * channels..(i + 1) * channels];
            match channels {
                1 => o[0] = (l + r) * 0.5,
                _ => {
                    o.iter_mut().for_each(|s| *s = 0.0);
                    o[0] = l;
                    o[1] = r;
                }
            }
        }
        queue.push(&out);
        cursor += Time::from_secs_f64(block as f64 / sr as f64);
        if cursor > project.duration() + Time::from_secs_f64(0.5) {
            // Past the end: let the video thread stop playback.
            thread::sleep(Duration::from_millis(10));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn video_thread(
    shared: Arc<Shared>,
    tx: Sender<(u64, PreparedFrame)>,
    queue: Option<Arc<AudioQueue>>,
    sr: u32,
    statics: Arc<Statics>,
    auto_grades: AutoGrades,
    levels: AudioLevels,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    let mut frames = StreamFrames::new(Duration::from_millis(40), false, true);
    let mut epoch = u64::MAX;
    let mut next = Time::ZERO;
    let clock = |shared: &Shared| -> Time {
        let origin = *shared.origin.lock().unwrap();
        match &queue {
            Some(q) => origin + Time::from_secs_f64(q.played_frames() as f64 / sr as f64),
            None => {
                origin
                    + Time::from_secs_f64(shared.wall_start.lock().unwrap().elapsed().as_secs_f64())
            }
        }
    };
    while !shared.quit.load(Ordering::Relaxed) {
        if !shared.playing.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let project = shared.project.lock().unwrap().clone();
        let fps = project.settings.frame_rate;
        let fd = fps.frame_duration();
        let e = shared.epoch.load(Ordering::SeqCst);
        if e != epoch {
            epoch = e;
            frames = StreamFrames::new(
                Duration::from_millis(40),
                false,
                shared.hwaccel.load(Ordering::Relaxed),
            );
            next = fps.snap(*shared.origin.lock().unwrap());
            // The first frame may take a moment while decoders start.
            frames.wait = Duration::from_millis(1500);
        }
        let now = clock(&shared);
        if now >= project.duration() {
            shared.playing.store(false, Ordering::SeqCst);
            repaint();
            continue;
        }
        // Fallen behind: skip to the frame due next.
        if now > next + fd + fd {
            next = fps.snap(now + fd);
        }
        let (w, h) = *shared.out_size.lock().unwrap();
        let out_scale = w as f32 / project.settings.width.max(1) as f32;
        // Where a rendered preview covers this moment, play that instead.
        let previews = shared.previews.lock().unwrap().clone();
        let current = previews.iter().find(|c| c.start <= next && next < c.end);
        let plan: &Project = current.map_or(&*project, |c| &*c.project);
        // Start decoders for clips beginning soon.
        let soon = next + Time::from_secs_f64(1.5);
        match current {
            // Live: start decoders for clips beginning soon, and the saved
            // previews that are just ahead.
            None => {
                prefetch_starting(&mut frames, plan, next, soon, (w, h));
                for c in previews
                    .iter()
                    .filter(|c| c.start > next && c.start <= soon)
                {
                    prefetch_starting(
                        &mut frames,
                        &c.project,
                        c.start - Time::from_millis(1),
                        c.start,
                        (w, h),
                    );
                }
            }
            // Inside a saved preview nothing live is decoded until its end
            // draws near; then the live clips are got ready for the hand-over.
            Some(c) if c.end <= soon => {
                prefetch_active(&mut frames, &project, c.end, (w, h));
                prefetch_starting(&mut frames, &project, c.end, soon, (w, h));
                let following = previews
                    .iter()
                    .find(|n| n.start == c.end)
                    .map(|n| n.project.clone());
                if let Some(n) = following {
                    prefetch_starting(&mut frames, &n, c.end - Time::from_millis(1), c.end, (w, h));
                }
            }
            Some(_) => {}
        }
        let auto = auto_grades.clone();
        let auto_fn = move |m: &tidycuts_core::MediaItem| auto(m);
        let mut planner = Planner {
            project: plan,
            out_scale,
            frames: &mut frames,
            statics: &statics,
            auto_grade: &auto_fn,
            levels: &levels,
            show_empty_cells: current.is_none(),
            used_statics: Vec::new(),
        };
        let prepared = planner.build(next);
        frames.end_frame();
        frames.wait = Duration::from_millis(40);
        if shared.epoch.load(Ordering::SeqCst) != epoch {
            continue;
        }
        // Wait for room, but notice seeks and stops.
        let mut item = (epoch, prepared);
        loop {
            match tx.try_send(item) {
                Ok(()) => break,
                Err(TrySendError::Full(back)) => {
                    item = back;
                    if shared.epoch.load(Ordering::SeqCst) != epoch
                        || shared.quit.load(Ordering::Relaxed)
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(3));
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        repaint();
        next += fd;
    }
}

/// Starts the decoders of video clips that begin in `(after, upto]`.
fn prefetch_starting(
    frames: &mut StreamFrames,
    project: &Project,
    after: Time,
    upto: Time,
    size: (u32, u32),
) {
    for track in project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && !t.hidden)
    {
        for c in track
            .clips
            .iter()
            .filter(|c| c.start > after && c.start <= upto && !c.disabled)
        {
            if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                if m.kind == MediaKind::Video {
                    frames.prefetch(
                        c,
                        m,
                        c.source_time(c.start).as_secs_f64(),
                        decode_size(m, size),
                    );
                }
            }
        }
    }
}

/// Starts the decoders of the video clips that are playing at `at`, so the
/// live picture is ready when a rendered preview ends.
fn prefetch_active(frames: &mut StreamFrames, project: &Project, at: Time, size: (u32, u32)) {
    for track in project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && !t.hidden)
    {
        for c in track
            .clips
            .iter()
            .filter(|c| c.start <= at && at < c.end() && !c.disabled)
        {
            if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                if m.kind == MediaKind::Video {
                    frames.prefetch(c, m, c.source_time(at).as_secs_f64(), decode_size(m, size));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::previews::Cached;
    use tidycuts_core::Generator;
    use tidycuts_render::{Item, Source};

    fn solid_project(color: [f32; 4], secs: f64, start: f64) -> Project {
        let mut p = Project::new("t");
        p.settings.magnetic = false;
        let track = p.tracks[0].id;
        p.add_generated(
            Generator::Solid(color),
            Time::from_secs_f64(secs),
            track,
            Time::from_secs_f64(start),
        )
        .unwrap();
        p
    }

    /// The colour of the first solid layer of a frame.
    fn colour(f: &PreparedFrame) -> Option<[f32; 4]> {
        f.frame.items.iter().find_map(|i| match i {
            Item::Layer(l) => match &l.source {
                Source::Solid(c) => Some(*c),
                _ => None,
            },
            _ => None,
        })
    }

    /// Plays from `at` for a moment and returns the colour of the newest frame.
    fn play_and_look(pb: &mut Playback, at: Time) -> Option<[f32; 4]> {
        pb.play(at);
        let mut seen = None;
        for _ in 0..200 {
            std::thread::sleep(Duration::from_millis(10));
            if let Some(f) = pb.poll() {
                if f.time >= at {
                    seen = colour(&f);
                    break;
                }
            }
        }
        pb.pause();
        seen
    }

    #[test]
    fn a_saved_preview_stands_in_for_the_live_picture_only_where_it_covers() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        // Ten seconds of red. A stand-in "saved preview" in blue covers 4 s..8 s.
        let live = Arc::new(solid_project(red, 10.0, 0.0));
        let standin = Arc::new(solid_project(blue, 4.0, 4.0));
        let mut pb = Playback::new(
            live,
            Arc::new(Statics::default()),
            LoudnessTable::default(),
            Arc::new(|_| None),
            AudioLevels::default(),
            Arc::new(|| {}),
        );
        pb.set_previews(vec![Cached {
            start: Time::from_secs_f64(4.0),
            end: Time::from_secs_f64(8.0),
            project: standin,
        }]);
        assert_eq!(play_and_look(&mut pb, Time::from_secs_f64(1.0)), Some(red));
        assert_eq!(play_and_look(&mut pb, Time::from_secs_f64(5.0)), Some(blue));
        assert_eq!(play_and_look(&mut pb, Time::from_secs_f64(9.0)), Some(red));
        // Without previews everything is live again.
        pb.set_previews(Vec::new());
        assert_eq!(play_and_look(&mut pb, Time::from_secs_f64(5.0)), Some(red));
    }

    #[test]
    fn jumping_back_while_playing_shows_the_new_place_at_once() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let mut pb = Playback::new(
            Arc::new(solid_project(red, 20.0, 0.0)),
            Arc::new(Statics::default()),
            LoudnessTable::default(),
            Arc::new(|_| None),
            AudioLevels::default(),
            Arc::new(|| {}),
        );
        for _ in 0..5 {
            // Play far in, let the planner get ahead, then jump back.
            pb.play(Time::from_secs_f64(15.0));
            std::thread::sleep(Duration::from_millis(60));
            pb.play(Time::from_secs_f64(2.0));
            let mut shown = None;
            for _ in 0..100 {
                std::thread::sleep(Duration::from_millis(10));
                if let Some(f) = pb.poll() {
                    shown = Some(f.time);
                    break;
                }
            }
            let t = shown.expect("a picture arrives within a second of the jump");
            assert!(
                t >= Time::from_secs_f64(2.0) && t < Time::from_secs_f64(3.0),
                "the picture is from the new place, not the old one: {t:?}"
            );
        }
        pb.pause();
    }
}
