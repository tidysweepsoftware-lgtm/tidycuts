//! Pictures for the library and the timeline, made on worker threads:
//! library posters, filmstrip frames (from the scrub cache, so they cost
//! almost nothing once a video is cached) and audio waveforms.

use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use tidycuts_core::{MediaItem, MediaKind};
use tidycuts_engine::scrub::MediaCache;

/// Waveform peak levels (0..=1) per second of audio.
pub const PEAKS_PER_SEC: u32 = 60;
/// Height of filmstrip frames in pixels.
const STRIP_H: u32 = 72;
/// Most filmstrip frames kept as textures.
const STRIP_MAX: usize = 900;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StripKey {
    path: PathBuf,
    frame: usize,
}

enum Job {
    Poster {
        path: PathBuf,
        size: (u32, u32),
        at: f64,
        cache: Option<Arc<MediaCache>>,
    },
    Strip {
        key: StripKey,
        cache: Arc<MediaCache>,
    },
    Wave {
        path: PathBuf,
        stream: u32,
        /// Where peaks are kept between sessions.
        cache: Option<PathBuf>,
    },
}

enum Done {
    Poster(PathBuf, Option<ColorImage>),
    Strip(StripKey, Option<ColorImage>),
    Wave((PathBuf, u32), Vec<f32>),
}

/// Newest-first work queue: what's on screen now matters more than what
/// scrolled past.
struct Queue {
    jobs: Mutex<VecDeque<Job>>,
    cv: Condvar,
}

pub struct Thumbs {
    queue: Arc<Queue>,
    done: Receiver<Done>,
    posters: HashMap<PathBuf, Option<TextureHandle>>,
    strip: HashMap<StripKey, (TextureHandle, u64)>,
    waves: HashMap<(PathBuf, u32), Arc<Vec<f32>>>,
    /// The cache folder, where waveform peaks are kept (set once the settings are known).
    pub cache_dir: Option<PathBuf>,
    pending: HashSet<String>,
    tick: u64,
}

fn shrink(img: &tidycuts_render::RgbaImage, h: u32) -> ColorImage {
    let s = h as f32 / img.height.max(1) as f32;
    let w = ((img.width as f32 * s).round() as u32).max(1);
    let h = h.min(img.height).max(1);
    let w = w.min(img.width).max(1);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let sy = ((y as f32 + 0.5) * img.height as f32 / h as f32) as u32;
        for x in 0..w {
            let sx = ((x as f32 + 0.5) * img.width as f32 / w as f32) as u32;
            let si = ((sy.min(img.height - 1) * img.width + sx.min(img.width - 1)) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&img.pixels[si..si + 4]);
        }
    }
    ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &out)
}

fn worker(q: Arc<Queue>, tx: Sender<Done>, ctx: egui::Context) {
    loop {
        let job = {
            let mut jobs = q.jobs.lock().unwrap();
            loop {
                if let Some(j) = jobs.pop_back() {
                    break j;
                }
                jobs = q.cv.wait(jobs).unwrap();
            }
        };
        let done = match job {
            Job::Poster {
                path,
                size,
                at,
                cache,
            } => {
                let from_cache = cache.and_then(|c| {
                    let i = c.nearest(at, (c.fps * 3.0) as usize)?;
                    c.image(i)
                });
                let img = match from_cache {
                    Some(i) => Some(shrink(&i, 180)),
                    None => tidycuts_media::grab_frame(&path, at, size, 320, 180)
                        .ok()
                        .map(|f| {
                            ColorImage::from_rgba_unmultiplied(
                                [f.width as usize, f.height as usize],
                                &f.pixels,
                            )
                        }),
                };
                Done::Poster(path, img)
            }
            Job::Strip { key, cache } => Done::Strip(
                key.clone(),
                cache.image(key.frame).map(|i| shrink(&i, STRIP_H)),
            ),
            Job::Wave {
                path,
                stream,
                cache,
            } => {
                let peaks = match cache {
                    Some(dir) => {
                        tidycuts_engine::decode::waveform_cached(&dir, &path, stream, PEAKS_PER_SEC)
                    }
                    None => tidycuts_engine::decode::waveform(&path, stream, PEAKS_PER_SEC),
                };
                Done::Wave((path, stream), peaks)
            }
        };
        if tx.send(done).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

impl Thumbs {
    pub fn new(ctx: &egui::Context) -> Self {
        let queue = Arc::new(Queue {
            jobs: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
        });
        let (tx, rx) = channel();
        let workers = std::thread::available_parallelism()
            .map(|n| n.get() / 2)
            .unwrap_or(2)
            .clamp(2, 4);
        for n in 0..workers {
            let (q, tx, ctx) = (queue.clone(), tx.clone(), ctx.clone());
            std::thread::Builder::new()
                .name(format!("tidycuts-thumbs-{n}"))
                .spawn(move || worker(q, tx, ctx))
                .expect("spawn thumbnail worker");
        }
        Thumbs {
            queue,
            done: rx,
            posters: HashMap::new(),
            strip: HashMap::new(),
            waves: HashMap::new(),
            cache_dir: None,
            pending: HashSet::new(),
            tick: 0,
        }
    }

    fn push(&mut self, tag: String, job: Job) {
        if !self.pending.insert(tag) {
            return;
        }
        let mut jobs = self.queue.jobs.lock().unwrap();
        jobs.push_back(job);
        // Drop the oldest requests if the queue grows long (scrolled away).
        while jobs.len() > 400 {
            jobs.pop_front();
        }
        self.queue.cv.notify_one();
    }

    /// Moves finished work into textures. Call once per frame.
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.tick += 1;
        while let Ok(done) = self.done.try_recv() {
            match done {
                Done::Poster(path, img) => {
                    self.pending.remove(&format!("p{}", path.display()));
                    let tex = img.map(|i| {
                        ctx.load_texture(
                            format!("poster-{}", path.display()),
                            i,
                            TextureOptions::LINEAR,
                        )
                    });
                    self.posters.insert(path, tex);
                }
                Done::Strip(key, img) => {
                    self.pending
                        .remove(&format!("s{}#{}", key.path.display(), key.frame));
                    if let Some(i) = img {
                        let tex = ctx.load_texture(
                            format!("strip-{}-{}", key.path.display(), key.frame),
                            i,
                            TextureOptions::LINEAR,
                        );
                        self.strip.insert(key, (tex, self.tick));
                    }
                }
                Done::Wave(key, peaks) => {
                    self.pending
                        .remove(&format!("w{}#{}", key.0.display(), key.1));
                    self.waves.insert(key, Arc::new(peaks));
                }
            }
        }
        if self.strip.len() > STRIP_MAX {
            let mut ages: Vec<u64> = self.strip.values().map(|v| v.1).collect();
            ages.sort_unstable();
            let cut = ages[self.strip.len() - STRIP_MAX * 3 / 4];
            self.strip.retain(|_, v| v.1 >= cut);
        }
    }

    /// The library picture for a media item (`None` while loading or for
    /// audio).
    pub fn poster(
        &mut self,
        media: &MediaItem,
        cache: Option<Arc<MediaCache>>,
    ) -> Option<TextureHandle> {
        if media.kind == MediaKind::Audio {
            return None;
        }
        if let Some(t) = self.posters.get(&media.path) {
            return t.clone();
        }
        // Video posters come from a moment in, past fades from black.
        let at = if media.kind == MediaKind::Video {
            (media.duration.as_secs_f64() * 0.1).min(2.0)
        } else {
            0.0
        };
        // Wait for the scrub cache's first chunk rather than starting ffmpeg.
        if let Some(c) = &cache {
            c.nearest(at, (c.fps * 3.0) as usize)?;
        }
        self.push(
            format!("p{}", media.path.display()),
            Job::Poster {
                path: media.path.clone(),
                size: (media.width, media.height),
                at,
                cache,
            },
        );
        None
    }

    /// A filmstrip frame near `secs`, or the closest one already loaded.
    pub fn strip_frame(
        &mut self,
        media: &MediaItem,
        cache: &Arc<MediaCache>,
        secs: f64,
    ) -> Option<TextureHandle> {
        let want = cache.nearest(secs, (cache.fps * 4.0) as usize)?;
        let key = StripKey {
            path: media.path.clone(),
            frame: want,
        };
        if let Some((t, used)) = self.strip.get_mut(&key) {
            *used = self.tick;
            return Some(t.clone());
        }
        self.push(
            format!("s{}#{}", key.path.display(), key.frame),
            Job::Strip {
                key: key.clone(),
                cache: cache.clone(),
            },
        );
        // Meanwhile show the nearest loaded frame of the same file.
        let span = (cache.fps * 10.0) as usize;
        (1..span).find_map(|d| {
            [want.checked_sub(d), Some(want + d)]
                .into_iter()
                .flatten()
                .find_map(|f| {
                    self.strip
                        .get(&StripKey {
                            path: media.path.clone(),
                            frame: f,
                        })
                        .map(|(t, _)| t.clone())
                })
        })
    }

    /// Waveform peaks for a file's audio stream, once measured.
    pub fn waveform(&mut self, media: &MediaItem, stream: u32) -> Option<Arc<Vec<f32>>> {
        let key = (media.path.clone(), stream);
        if let Some(w) = self.waves.get(&key) {
            return Some(w.clone());
        }
        if media.kind == MediaKind::Audio || media.has_audio {
            self.push(
                format!("w{}#{}", media.path.display(), stream),
                Job::Wave {
                    path: media.path.clone(),
                    stream,
                    cache: self.cache_dir.clone(),
                },
            );
        }
        None
    }

    /// Forgets everything about a file (after it changed on disk).
    pub fn forget(&mut self, media: &MediaItem) {
        self.posters.remove(&media.path);
        self.strip.retain(|k, _| k.path != media.path);
        self.waves.retain(|k, _| k.0 != media.path);
    }
}
