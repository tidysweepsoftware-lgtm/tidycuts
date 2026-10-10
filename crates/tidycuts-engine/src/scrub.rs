//! The scrub cache: small JPEG copies of every frame (or every few frames
//! for long files), so dragging the playhead shows pictures instantly
//! instead of asking ffmpeg to seek for each one.
//!
//! Each file is split into short chunks that several ffmpeg processes encode
//! in parallel, nearest-to-the-playhead first. Frames are appended to chunk
//! files on disk and indexed in memory, so scrubbing works while the cache
//! is still being built; a finished cache is saved and reused next time.

use crate::decode::{secs, KillOnDrop};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread;
use tidycuts_core::{MediaItem, MediaKind};
use tidycuts_media::Tools;
use tidycuts_render::RgbaImage;

/// Height of cached frames.
pub const CACHE_HEIGHT: u32 = 360;
const CHUNK_SECS: f64 = 6.0;
const FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
struct Entry {
    file: u32,
    offset: u64,
    len: u32,
}

#[derive(Serialize, Deserialize)]
struct SavedIndex {
    format: u32,
    fps: f64,
    width: u32,
    height: u32,
    frames: Vec<Option<Entry>>,
}

/// The cache of one media file.
pub struct MediaCache {
    dir: PathBuf,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    index: RwLock<Vec<Option<Entry>>>,
    chunks_total: usize,
    chunks_done: AtomicUsize,
    /// Chunk number each worker should prefer (nearest the playhead).
    focus: AtomicU64,
    claimed: Mutex<Vec<bool>>,
    cancelled: AtomicBool,
}

impl MediaCache {
    /// 0..1 of the file cached so far.
    pub fn progress(&self) -> f32 {
        if self.chunks_total == 0 {
            return 1.0;
        }
        self.chunks_done.load(Ordering::Relaxed) as f32 / self.chunks_total as f32
    }

    pub fn is_complete(&self) -> bool {
        self.chunks_done.load(Ordering::Relaxed) >= self.chunks_total
    }

    fn frame_index(&self, secs: f64) -> usize {
        ((secs * self.fps + 1e-3).floor().max(0.0) as usize).min(self.frame_count.saturating_sub(1))
    }

    /// Index of the cached frame nearest `secs`, within `max_gap` frames.
    pub fn nearest(&self, secs: f64, max_gap: usize) -> Option<usize> {
        let i = self.frame_index(secs);
        let idx = self.index.read().unwrap();
        for d in 0..=max_gap {
            if i >= d && idx.get(i - d).is_some_and(|e| e.is_some()) {
                return Some(i - d);
            }
            if idx.get(i + d).is_some_and(|e| e.is_some()) {
                return Some(i + d);
            }
        }
        None
    }

    /// The JPEG bytes of a cached frame.
    pub fn jpeg(&self, frame: usize) -> Option<Vec<u8>> {
        let e = (*self.index.read().unwrap().get(frame)?)?;
        let mut f = File::open(self.dir.join(format!("c{}.jpgs", e.file))).ok()?;
        f.seek(SeekFrom::Start(e.offset)).ok()?;
        let mut buf = vec![0u8; e.len as usize];
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    /// Decodes a cached frame to RGBA.
    pub fn image(&self, frame: usize) -> Option<RgbaImage> {
        decode_jpeg(&self.jpeg(frame)?)
    }

    /// Asks the builders to work near `secs` next.
    pub fn focus(&self, secs: f64) {
        self.focus
            .store((secs / CHUNK_SECS).max(0.0) as u64, Ordering::Relaxed);
    }

    fn claim(&self) -> Option<usize> {
        let mut c = self.claimed.lock().unwrap();
        let f = (self.focus.load(Ordering::Relaxed) as usize).min(c.len().saturating_sub(1));
        // Nearest unclaimed chunk to the focus, preferring later ones (play
        // direction) on ties.
        for d in 0..c.len() {
            for i in [f + d, f.wrapping_sub(d)] {
                if i < c.len() && !c[i] {
                    c[i] = true;
                    return Some(i);
                }
            }
        }
        None
    }

    fn save(&self) {
        let saved = SavedIndex {
            format: FORMAT,
            fps: self.fps,
            width: self.width,
            height: self.height,
            frames: self.index.read().unwrap().clone(),
        };
        if let Ok(json) = serde_json::to_vec(&saved) {
            let _ = fs::write(self.dir.join("index.json"), json);
        }
    }
}

pub fn decode_jpeg(data: &[u8]) -> Option<RgbaImage> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(data), opts);
    let pixels = d.decode().ok()?;
    let info = d.info()?;
    Some(RgbaImage {
        width: info.width as u32,
        height: info.height as u32,
        pixels,
        premultiplied: true,
    })
}

/// Splits an MJPEG byte stream into whole JPEG images.
struct JpegSplitter {
    buf: Vec<u8>,
}

impl JpegSplitter {
    fn push(&mut self, data: &[u8], mut out: impl FnMut(&[u8])) {
        self.buf.extend_from_slice(data);
        let mut start = 0;
        let mut i = 2;
        while i + 1 < self.buf.len() {
            // End-of-image marker. Inside the compressed data 0xFF is always
            // followed by 0x00 or a restart marker, so this is unambiguous.
            if self.buf[i] == 0xFF && self.buf[i + 1] == 0xD9 {
                out(&self.buf[start..i + 2]);
                start = i + 2;
                i = start + 2;
                continue;
            }
            i += 1;
        }
        self.buf.drain(..start);
    }
}

struct Job {
    cache: Arc<MediaCache>,
    path: PathBuf,
    duration: f64,
}

/// All scrub caches, plus the worker threads that build them.
pub struct ScrubCache {
    root: PathBuf,
    caches: Mutex<HashMap<PathBuf, Arc<MediaCache>>>,
    queue: Arc<(Mutex<Vec<Job>>, Condvar)>,
    /// Called when new frames land, so the UI can repaint.
    notify: Arc<dyn Fn() + Send + Sync>,
}

fn cache_key(media: &MediaItem) -> String {
    let mut h = DefaultHasher::new();
    media.path.hash(&mut h);
    if let Ok(m) = fs::metadata(&media.path) {
        m.len().hash(&mut h);
        if let Ok(t) = m.modified() {
            t.hash(&mut h);
        }
    }
    FORMAT.hash(&mut h);
    CACHE_HEIGHT.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Frame rate to cache at: every frame of short clips, fewer for long ones
/// to bound disk use.
fn cache_fps(media: &MediaItem) -> f64 {
    let native = media
        .frame_rate
        .map(|f| f.as_f64())
        .filter(|f| *f > 1.0)
        .unwrap_or(30.0);
    let secs = media.duration.as_secs_f64();
    let cap = if secs <= 300.0 {
        30.0
    } else if secs <= 1800.0 {
        15.0
    } else {
        10.0
    };
    native.min(cap)
}

impl ScrubCache {
    pub fn new(
        root: PathBuf,
        workers: usize,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<ScrubCache> {
        let _ = fs::create_dir_all(&root);
        let sc = Arc::new(ScrubCache {
            root,
            caches: Mutex::new(HashMap::new()),
            queue: Arc::new((Mutex::new(Vec::new()), Condvar::new())),
            notify,
        });
        for n in 0..workers.max(1) {
            let q = sc.queue.clone();
            let notify = sc.notify.clone();
            thread::Builder::new()
                .name(format!("tidycuts-scrub-{n}"))
                .spawn(move || worker(q, notify))
                .expect("spawn scrub worker");
        }
        sc
    }

    /// The cache for a video, starting to build it if needed. Images and
    /// audio have no cache.
    pub fn get(&self, media: &MediaItem) -> Option<Arc<MediaCache>> {
        if media.kind != MediaKind::Video || media.width == 0 {
            return None;
        }
        if let Some(c) = self.caches.lock().unwrap().get(&media.path) {
            return Some(c.clone());
        }
        let cache = self.open(media);
        self.caches
            .lock()
            .unwrap()
            .insert(media.path.clone(), cache.clone());
        if !cache.is_complete() {
            let (lock, cv) = &*self.queue;
            lock.lock().unwrap().push(Job {
                cache: cache.clone(),
                path: media.preview_path().to_path_buf(),
                duration: media.duration.as_secs_f64(),
            });
            cv.notify_all();
        }
        Some(cache)
    }

    /// Stops building a file's cache (it was removed from the project).
    pub fn forget(&self, path: &Path) {
        if let Some(c) = self.caches.lock().unwrap().remove(path) {
            c.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn open(&self, media: &MediaItem) -> Arc<MediaCache> {
        let dir = self.root.join(cache_key(media));
        let fps = cache_fps(media);
        let height = CACHE_HEIGHT.min(media.height.max(2)) & !1;
        let width = ((media.width as f64 * height as f64 / media.height.max(1) as f64).round()
            as u32)
            .max(2)
            & !1;
        let secs = media.duration.as_secs_f64();
        let frame_count = ((secs * fps).ceil() as usize).max(1);
        let chunks_total = ((secs / CHUNK_SECS).ceil() as usize).max(1);
        if let Some(saved) = fs::read(dir.join("index.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<SavedIndex>(&b).ok())
            .filter(|s| s.format == FORMAT && s.frames.len() == frame_count)
        {
            return Arc::new(MediaCache {
                dir,
                fps: saved.fps,
                width: saved.width,
                height: saved.height,
                frame_count,
                index: RwLock::new(saved.frames),
                chunks_total,
                chunks_done: AtomicUsize::new(chunks_total),
                focus: AtomicU64::new(0),
                claimed: Mutex::new(vec![true; chunks_total]),
                cancelled: AtomicBool::new(false),
            });
        }
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        Arc::new(MediaCache {
            dir,
            fps,
            width,
            height,
            frame_count,
            index: RwLock::new(vec![None; frame_count]),
            chunks_total,
            chunks_done: AtomicUsize::new(0),
            focus: AtomicU64::new(0),
            claimed: Mutex::new(vec![false; chunks_total]),
            cancelled: AtomicBool::new(false),
        })
    }

    /// Total size of the cache folder in bytes.
    pub fn disk_usage(&self) -> u64 {
        fn walk(p: &Path) -> u64 {
            fs::read_dir(p)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| match e.metadata() {
                            Ok(m) if m.is_dir() => walk(&e.path()),
                            Ok(m) => m.len(),
                            Err(_) => 0,
                        })
                        .sum()
                })
                .unwrap_or(0)
        }
        walk(&self.root)
    }

    /// Deletes every cache not used by the given files.
    pub fn clear_unused(&self, keep: &[MediaItem]) {
        let keep: Vec<String> = keep.iter().map(cache_key).collect();
        if let Ok(rd) = fs::read_dir(&self.root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !keep.contains(&name) {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
        }
    }
}

fn worker(queue: Arc<(Mutex<Vec<Job>>, Condvar)>, notify: Arc<dyn Fn() + Send + Sync>) {
    loop {
        // Take a chunk from the newest job that still has work.
        let (job, chunk) = {
            let (lock, cv) = &*queue;
            let mut jobs = lock.lock().unwrap();
            loop {
                jobs.retain(|j| !j.cache.cancelled.load(Ordering::Relaxed));
                let mut found = None;
                for j in jobs.iter().rev() {
                    if let Some(c) = j.cache.claim() {
                        found = Some((
                            Job {
                                cache: j.cache.clone(),
                                path: j.path.clone(),
                                duration: j.duration,
                            },
                            c,
                        ));
                        break;
                    }
                }
                if let Some(f) = found {
                    break f;
                }
                jobs.retain(|j| !j.cache.is_complete());
                jobs = cv.wait(jobs).unwrap();
            }
        };
        build_chunk(&job, chunk, &*notify);
        let done = job.cache.chunks_done.fetch_add(1, Ordering::Relaxed) + 1;
        if done >= job.cache.chunks_total {
            job.cache.save();
        }
        notify();
    }
}

fn build_chunk(job: &Job, chunk: usize, notify: &dyn Fn()) {
    let Ok(tools) = Tools::get() else {
        return;
    };
    let c = &job.cache;
    let start = chunk as f64 * CHUNK_SECS;
    let len = CHUNK_SECS.min(job.duration - start).max(0.0);
    if len <= 0.0 {
        return;
    }
    let first = (start * c.fps).round() as usize;
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-hwaccel", "auto", "-threads", "2"]);
    if start > 0.0 {
        cmd.args(["-ss", &secs(start)]);
    }
    cmd.args(["-t", &secs(len)]);
    cmd.arg("-i").arg(&job.path);
    cmd.args([
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &format!(
            "fps={:.6},scale={}:{}:flags=fast_bilinear",
            c.fps, c.width, c.height
        ),
        "-c:v",
        "mjpeg",
        "-q:v",
        "5",
        "-f",
        "image2pipe",
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
    let file_path = c.dir.join(format!("c{chunk}.jpgs"));
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&file_path)
    else {
        return;
    };
    let mut split = JpegSplitter { buf: Vec::new() };
    let mut offset = 0u64;
    let mut n = 0usize;
    let mut buf = vec![0u8; 256 * 1024];
    let mut since_notify = 0;
    loop {
        if c.cancelled.load(Ordering::Relaxed) {
            return;
        }
        let read = match out.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(r) => r,
        };
        let mut images: Vec<Vec<u8>> = Vec::new();
        split.push(&buf[..read], |img| images.push(img.to_vec()));
        for img in images {
            if file.write_all(&img).is_err() {
                return;
            }
            // The frame must be on disk before readers can find it.
            let _ = file.flush();
            let i = first + n;
            if i < c.frame_count {
                c.index.write().unwrap()[i] = Some(Entry {
                    file: chunk as u32,
                    offset,
                    len: img.len() as u32,
                });
            }
            offset += img.len() as u64;
            n += 1;
            since_notify += 1;
            if since_notify >= 15 {
                since_notify = 0;
                notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_finds_whole_images() {
        let a = [0xFF, 0xD8, 1, 2, 0xFF, 0x00, 3, 0xFF, 0xD9];
        let b = [0xFF, 0xD8, 9, 0xFF, 0xD9];
        let mut stream = a.to_vec();
        stream.extend_from_slice(&b);
        let mut s = JpegSplitter { buf: Vec::new() };
        let mut got: Vec<Vec<u8>> = Vec::new();
        // Feed in awkward pieces.
        for piece in stream.chunks(4) {
            s.push(piece, |img| got.push(img.to_vec()));
        }
        assert_eq!(got, vec![a.to_vec(), b.to_vec()]);
    }
}
