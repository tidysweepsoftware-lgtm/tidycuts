//! Where the pictures of video clips come from: exact frames decoded on
//! demand (kept in a memory cache), the scrub cache while dragging, and
//! streaming decoders while playing or exporting.

use crate::decode::{decode_still, VideoParams, VideoStream};
use crate::scrub::{MediaCache, ScrubCache};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tidycuts_core::{Clip, ClipId, MediaItem, MediaKind};
use tidycuts_render::Pixels;

/// Supplies decoded pictures to the frame planner.
pub trait FrameProvider {
    /// The picture of `media` showing at `source_secs`, decoded at `size`
    /// (even width and height). `None` when nothing is available yet.
    fn frame(
        &mut self,
        clip: &Clip,
        media: &MediaItem,
        source_secs: f64,
        size: (u32, u32),
    ) -> Option<Pixels>;
}

fn pixels_bytes(p: &Pixels) -> usize {
    match p {
        Pixels::Rgba(i) => i.pixels.len(),
        Pixels::Yuv420(i) => i.data.len(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FrameKey {
    path: PathBuf,
    frame: i64,
    w: u32,
    h: u32,
}

/// A memory cache of decoded frames, bounded by bytes.
struct Lru {
    map: HashMap<FrameKey, (Pixels, u64)>,
    order: VecDeque<(FrameKey, u64)>,
    bytes: usize,
    budget: usize,
    tick: u64,
}

impl Lru {
    fn new(budget: usize) -> Lru {
        Lru {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            budget,
            tick: 0,
        }
    }

    fn get(&mut self, k: &FrameKey) -> Option<Pixels> {
        self.tick += 1;
        let tick = self.tick;
        let e = self.map.get_mut(k)?;
        e.1 = tick;
        self.order.push_back((k.clone(), tick));
        Some(e.0.clone())
    }

    fn put(&mut self, k: FrameKey, p: Pixels) {
        self.tick += 1;
        self.bytes += pixels_bytes(&p);
        if let Some(old) = self.map.insert(k.clone(), (p, self.tick)) {
            self.bytes -= pixels_bytes(&old.0);
        }
        self.order.push_back((k, self.tick));
        while self.bytes > self.budget {
            let Some((k, t)) = self.order.pop_front() else {
                break;
            };
            // Skip stale order entries for keys used again later.
            if self.map.get(&k).is_some_and(|e| e.1 == t) {
                let (p, _) = self.map.remove(&k).unwrap();
                self.bytes -= pixels_bytes(&p);
            }
        }
        if self.order.len() > self.map.len() * 4 + 64 {
            let map = &self.map;
            self.order
                .retain(|(k, t)| map.get(k).is_some_and(|e| e.1 == *t));
        }
    }
}

struct Request {
    params: VideoParams,
    at: f64,
    key: FrameKey,
}

struct Shared {
    lru: Mutex<Lru>,
    /// One pending request per file: newer requests replace older ones, so
    /// fast scrubbing never builds a backlog.
    lanes: Mutex<(HashMap<PathBuf, Request>, Vec<PathBuf>)>,
    cv: Condvar,
    last: Mutex<HashMap<PathBuf, Pixels>>,
    jpeg: Mutex<Lru>,
    notify: Arc<dyn Fn() + Send + Sync>,
}

/// Exact frames and scrub frames for the preview.
pub struct FrameServer {
    shared: Arc<Shared>,
    pub scrub: Arc<ScrubCache>,
}

fn frame_number(media_fps: f64, secs: f64) -> i64 {
    (secs * media_fps + 1e-3).floor() as i64
}

impl FrameServer {
    pub fn new(
        scrub: Arc<ScrubCache>,
        memory_budget: usize,
        workers: usize,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> FrameServer {
        let shared = Arc::new(Shared {
            lru: Mutex::new(Lru::new(memory_budget)),
            lanes: Mutex::new((HashMap::new(), Vec::new())),
            cv: Condvar::new(),
            last: Mutex::new(HashMap::new()),
            jpeg: Mutex::new(Lru::new(96 << 20)),
            notify,
        });
        for n in 0..workers.max(1) {
            let s = shared.clone();
            thread::Builder::new()
                .name(format!("tidycuts-exact-{n}"))
                .spawn(move || exact_worker(s))
                .expect("spawn frame worker");
        }
        FrameServer { shared, scrub }
    }

    fn key(params: &VideoParams, at: f64) -> FrameKey {
        FrameKey {
            path: params.path.clone(),
            frame: frame_number(params.fps, at),
            w: params.width,
            h: params.height,
        }
    }

    /// An exact frame if it's in memory.
    pub fn cached(&self, params: &VideoParams, at: f64) -> Option<Pixels> {
        self.shared.lru.lock().unwrap().get(&Self::key(params, at))
    }

    /// Asks for an exact frame to be decoded in the background.
    pub fn request(&self, params: &VideoParams, at: f64) {
        let key = Self::key(params, at);
        let mut lanes = self.shared.lanes.lock().unwrap();
        if lanes.0.get(&params.path).is_some_and(|r| r.key == key) {
            return;
        }
        let path = params.path.clone();
        lanes.0.insert(
            path.clone(),
            Request {
                params: params.clone(),
                at,
                key,
            },
        );
        lanes.1.retain(|p| *p != path);
        lanes.1.push(path);
        self.shared.cv.notify_one();
    }

    /// The most recent exact frame decoded for a file, at any time.
    pub fn last(&self, params: &VideoParams) -> Option<Pixels> {
        self.shared.last.lock().unwrap().get(&params.path).cloned()
    }

    /// The scrub-cache frame nearest `at`, decoded.
    pub fn scrub_frame(&self, media: &MediaItem, at: f64, max_gap_secs: f64) -> Option<Pixels> {
        let cache: Arc<MediaCache> = self.scrub.get(media)?;
        cache.focus(at);
        let gap = (max_gap_secs * cache.fps).ceil() as usize;
        let i = cache.nearest(at, gap)?;
        let key = FrameKey {
            path: media.path.clone(),
            frame: i as i64,
            w: cache.width,
            h: cache.height,
        };
        if let Some(p) = self.shared.jpeg.lock().unwrap().get(&key) {
            return Some(p);
        }
        let img = cache.image(i)?;
        let p = Pixels::Rgba(Arc::new(img));
        self.shared.jpeg.lock().unwrap().put(key, p.clone());
        Some(p)
    }

    /// Decodes a frame right now on the calling thread (used for stills).
    pub fn decode_now(&self, params: &VideoParams, at: f64) -> Option<Pixels> {
        if let Some(p) = self.cached(params, at) {
            return Some(p);
        }
        let p = decode_still(params, at)?;
        self.shared
            .lru
            .lock()
            .unwrap()
            .put(Self::key(params, at), p.clone());
        Some(p)
    }
}

fn exact_worker(s: Arc<Shared>) {
    loop {
        let req = {
            let mut lanes = s.lanes.lock().unwrap();
            loop {
                if let Some(path) = lanes.1.first().cloned() {
                    lanes.1.remove(0);
                    if let Some(r) = lanes.0.remove(&path) {
                        break r;
                    }
                    continue;
                }
                lanes = s.cv.wait(lanes).unwrap();
            }
        };
        if s.lru.lock().unwrap().get(&req.key).is_some() {
            continue;
        }
        if let Some(p) = decode_still(&req.params, req.at) {
            s.last
                .lock()
                .unwrap()
                .insert(req.params.path.clone(), p.clone());
            s.lru.lock().unwrap().put(req.key, p);
            (s.notify)();
        }
    }
}

/// Decode size for a clip: the media fitted inside `target`, never larger
/// than the file itself.
pub fn decode_size(media: &MediaItem, target: (u32, u32)) -> (u32, u32) {
    let (mw, mh) = (media.width.max(2) as f64, media.height.max(2) as f64);
    let s = (target.0 as f64 / mw).min(target.1 as f64 / mh).min(1.0);
    (
        ((mw * s).round() as u32).max(2) & !1,
        ((mh * s).round() as u32).max(2) & !1,
    )
}

/// Pictures for the paused or scrubbing preview: never blocks.
pub struct PreviewFrames<'a> {
    pub server: &'a FrameServer,
    /// Dragging the playhead: prefer instant scrub-cache frames.
    pub scrubbing: bool,
    /// Set when some picture isn't the exact one yet.
    pub incomplete: bool,
    pub hwaccel: bool,
}

impl FrameProvider for PreviewFrames<'_> {
    fn frame(
        &mut self,
        _clip: &Clip,
        media: &MediaItem,
        source_secs: f64,
        size: (u32, u32),
    ) -> Option<Pixels> {
        let mut params = VideoParams::for_media(media, size.0, size.1);
        params.hwaccel = self.hwaccel;
        if media.kind == MediaKind::Image {
            params.alpha = true;
            return self.server.decode_now(&params, 0.0);
        }
        let at = source_secs.clamp(
            0.0,
            (media.duration.as_secs_f64() - 0.5 / params.fps).max(0.0),
        );
        if let Some(p) = self.server.cached(&params, at) {
            return Some(p);
        }
        self.server.request(&params, at);
        self.incomplete = true;
        let gap = if self.scrubbing { 2.0 } else { 0.25 };
        self.server
            .scrub_frame(media, at, gap)
            .or_else(|| self.server.last(&params))
    }
}

/// Filters that need neighbouring frames (flicker removal),
/// so they only run on streaming decoders: playback and export.
pub fn temporal_filters(clip: &Clip) -> Vec<String> {
    let mut f = Vec::new();
    if clip.video.deflicker {
        f.push("deflicker=size=7:mode=pm".to_string());
    }
    f
}

/// Pictures for playback and export, from one streaming decoder per clip.
pub struct StreamFrames {
    streams: HashMap<ClipId, (VideoStream, u64)>,
    stills: HashMap<(PathBuf, u32, u32), Pixels>,
    tick: u64,
    /// How long to wait for a late frame: short for playback, long for export.
    pub wait: Duration,
    pub high_quality: bool,
    pub hwaccel: bool,
    pub pre_filters: HashMap<ClipId, Vec<String>>,
}

impl StreamFrames {
    pub fn new(wait: Duration, high_quality: bool, hwaccel: bool) -> StreamFrames {
        StreamFrames {
            streams: HashMap::new(),
            stills: HashMap::new(),
            tick: 0,
            wait,
            high_quality,
            hwaccel,
            pre_filters: HashMap::new(),
        }
    }

    /// Call once per output frame: closes decoders not used for a while.
    pub fn end_frame(&mut self) {
        self.tick += 1;
        let tick = self.tick;
        self.streams.retain(|_, (_, used)| tick - *used < 30);
    }

    fn params(&self, clip: &Clip, media: &MediaItem, size: (u32, u32)) -> VideoParams {
        let mut p = VideoParams::for_media(media, size.0, size.1);
        p.high_quality = self.high_quality;
        if self.high_quality {
            // Exports always read the original, never a proxy.
            p.path = media.path.clone();
        }
        p.hwaccel = self.hwaccel;
        if let Some(f) = self.pre_filters.get(&clip.id) {
            p.pre_filters = f.clone();
        }
        p.post_filters = temporal_filters(clip);
        p
    }

    /// Starts a clip's decoder ahead of time so its first frame is ready.
    pub fn prefetch(&mut self, clip: &Clip, media: &MediaItem, source_secs: f64, size: (u32, u32)) {
        if media.kind != MediaKind::Video {
            return;
        }
        // Already running: count it as used now, or it would be closed one frame
        // before the cut it was started for.
        if let Some((_, used)) = self.streams.get_mut(&clip.id) {
            *used = self.tick;
            return;
        }
        let params = self.params(clip, media, size);
        let s = VideoStream::open(
            params,
            source_secs.max(0.0),
            clip.reverse,
            clip.source_in.as_secs_f64(),
        );
        self.streams.insert(clip.id, (s, self.tick));
    }
}

impl FrameProvider for StreamFrames {
    fn frame(
        &mut self,
        clip: &Clip,
        media: &MediaItem,
        source_secs: f64,
        size: (u32, u32),
    ) -> Option<Pixels> {
        if media.kind == MediaKind::Image {
            let key = (media.path.clone(), size.0, size.1);
            if let Some(p) = self.stills.get(&key) {
                return Some(p.clone());
            }
            let mut params = self.params(clip, media, size);
            params.alpha = true;
            let p = decode_still(&params, 0.0)?;
            self.stills.insert(key, p.clone());
            return Some(p);
        }
        let params = self.params(clip, media, size);
        let at = source_secs.clamp(
            0.0,
            (media.duration.as_secs_f64() - 0.5 / params.fps).max(0.0),
        );
        let tick = self.tick;
        let reusable = self
            .streams
            .get(&clip.id)
            .is_some_and(|(s, _)| s.params == params && s.can_reach(at));
        if !reusable {
            let s = VideoStream::open(params, at, clip.reverse, clip.source_in.as_secs_f64());
            self.streams.insert(clip.id, (s, tick));
        }
        let (s, used) = self.streams.get_mut(&clip.id)?;
        *used = tick;
        s.frame_at(at, self.wait).map(|f| f.pixels)
    }
}
