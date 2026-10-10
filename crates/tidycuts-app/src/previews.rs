//! Render previews in the window: which parts of the timeline have a saved
//! preview, drawing the missing ones in the background, and handing the
//! ready ones to playback (see `tidycuts_engine::previews`).

use crate::app::TidyCutsApp;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tidycuts_core::MediaItem;
use tidycuts_engine::previews::{self, Chunk};
use tidycuts_engine::Statics;

/// Previews are kept until the folder grows past this.
const KEEP_BYTES: u64 = 4 << 30;
/// After an edit, wait this long before working out the pieces again.
const SETTLE: Duration = Duration::from_millis(200);

/// One piece of the timeline and whether its preview is saved.
pub struct Piece {
    pub chunk: Chunk,
    pub ready: bool,
}

enum Msg {
    /// Frames done and total of the piece being drawn.
    Frames(u64, u64, u64),
    Finished(u64, Result<(), String>),
    End,
}

struct Job {
    cancel: Arc<AtomicBool>,
    rx: Receiver<Msg>,
    pieces: usize,
    finished: usize,
    /// The piece being drawn now: (key, frames done, frames total).
    current: Option<(u64, u64, u64)>,
    failed: Option<String>,
}

#[derive(Default)]
pub struct PreviewState {
    pub pieces: Vec<Piece>,
    /// The project revision `pieces` was worked out for.
    seen: Option<u64>,
    changed: Option<Instant>,
    /// Keys of the ready pieces playback was last given.
    sent: Vec<u64>,
    job: Option<Job>,
    /// Bytes the preview files take, refreshed after each render.
    pub disk_bytes: u64,
}

impl PreviewState {
    /// Whether previews are being drawn.
    pub fn rendering(&self) -> bool {
        self.job.is_some()
    }

    /// How far along the drawing is, 0..1.
    pub fn progress(&self) -> Option<f32> {
        let j = self.job.as_ref()?;
        let part = j
            .current
            .map_or(0.0, |(_, d, t)| d as f32 / t.max(1) as f32);
        Some(((j.finished as f32 + part) / j.pieces.max(1) as f32).clamp(0.0, 1.0))
    }

    /// The piece being drawn now, for the bar on the ruler: its key and how
    /// much of it is done.
    pub fn drawing(&self) -> Option<(u64, f32)> {
        let (key, d, t) = self.job.as_ref()?.current?;
        Some((key, d as f32 / t.max(1) as f32))
    }

    /// Pieces that are hard to play and have no saved preview yet.
    pub fn missing(&self) -> usize {
        self.pieces
            .iter()
            .filter(|p| p.chunk.heavy && !p.ready)
            .count()
    }

    pub fn ready_count(&self) -> usize {
        self.pieces.iter().filter(|p| p.ready).count()
    }
}

impl TidyCutsApp {
    fn previews_dir(&self) -> PathBuf {
        self.prefs.cache_dir().join("previews")
    }

    /// Clears old preview files once, when the program starts.
    pub fn tidy_previews_at_start(&mut self) {
        let dir = self.previews_dir();
        self.previews.disk_bytes = previews::size_on_disk(&dir);
        std::thread::spawn(move || previews::tidy(&dir, KEEP_BYTES));
    }

    /// Works out the pieces again after the project changed, and tells
    /// playback which previews are good. `now` skips the short wait after an
    /// edit (used right before playing).
    pub fn refresh_previews(&mut self, now: bool) {
        let rev = self.revision;
        if self.previews.seen != Some(rev) {
            let changed = *self.previews.changed.get_or_insert_with(Instant::now);
            if !now && changed.elapsed() < SETTLE {
                // Nothing else may wake the window, so ask for another look.
                self.ctx.request_repaint_after(SETTLE);
                return;
            }
            let dir = self.previews_dir();
            self.previews.pieces = previews::chunks(&self.project)
                .into_iter()
                .map(|chunk| Piece {
                    ready: previews::is_ready(&dir, chunk.key),
                    chunk,
                })
                .collect();
            self.previews.seen = Some(rev);
            self.previews.changed = None;
        }
        self.send_previews();
    }

    /// Gives playback the ready pieces (only when they changed).
    fn send_previews(&mut self) {
        let keys: Vec<u64> = self
            .previews
            .pieces
            .iter()
            .filter(|p| p.ready)
            .map(|p| p.chunk.key)
            .collect();
        if keys == self.previews.sent {
            return;
        }
        let dir = self.previews_dir();
        let spans: Vec<_> = self
            .previews
            .pieces
            .iter()
            .filter(|p| p.ready)
            .map(|p| previews::cached(&self.project, &p.chunk, &dir))
            .collect();
        if let Some(e) = &mut self.engine {
            e.set_previews(spans);
        }
        self.previews.sent = keys;
    }

    /// Starts drawing the parts that are hard to play and have no preview.
    pub fn render_previews(&mut self) {
        if self.previews.job.is_some() {
            return;
        }
        self.refresh_previews(true);
        let todo: Vec<Chunk> = self
            .previews
            .pieces
            .iter()
            .filter(|p| p.chunk.heavy && !p.ready)
            .map(|p| p.chunk.clone())
            .collect();
        if todo.is_empty() {
            let text = if self.previews.pieces.iter().any(|p| p.chunk.heavy) {
                "Everything that needs a preview is ready."
            } else {
                "Nothing here needs a preview: it should play smoothly as it is."
            };
            self.notify(text, false);
            return;
        }
        let Some(engine) = &self.engine else {
            self.notify("Previews need a graphics device.", true);
            return;
        };
        let (renderer, analysis) = (engine.renderer.clone(), engine.analysis.clone());
        let project = self.project.clone();
        let dir = self.previews_dir();
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = self.ctx.clone();
        let flag = cancel.clone();
        let count = todo.len();
        std::thread::Builder::new()
            .name("tidycuts-previews".into())
            .spawn(move || {
                let statics = Statics::default();
                let auto = move |m: &MediaItem| analysis.auto_grade(m);
                for chunk in todo {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let key = chunk.key;
                    let mut last = 0;
                    let result = previews::render_chunk(
                        &project,
                        &chunk,
                        &dir,
                        renderer.clone(),
                        &statics,
                        &auto,
                        &flag,
                        |done, total| {
                            if done == total || done >= last + 5 {
                                last = done;
                                let _ = tx.send(Msg::Frames(key, done, total));
                                ctx.request_repaint();
                            }
                        },
                    );
                    let stopped = flag.load(Ordering::Relaxed);
                    let _ = tx.send(Msg::Finished(key, result));
                    ctx.request_repaint();
                    if stopped {
                        break;
                    }
                }
                let _ = tx.send(Msg::End);
                ctx.request_repaint();
            })
            .expect("spawn preview thread");
        self.previews.job = Some(Job {
            cancel,
            rx,
            pieces: count,
            finished: 0,
            current: None,
            failed: None,
        });
        self.notify("Rendering previews…", false);
    }

    /// Stops the drawing that is going on.
    pub fn cancel_previews(&mut self) {
        if let Some(j) = &self.previews.job {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Deletes every saved preview.
    pub fn delete_previews(&mut self) {
        self.cancel_previews();
        previews::delete_all(&self.previews_dir());
        for p in &mut self.previews.pieces {
            p.ready = false;
        }
        self.previews.disk_bytes = 0;
        self.send_previews();
        self.notify("Preview files deleted.", false);
    }

    /// Follows the drawing, once per frame.
    pub fn poll_previews(&mut self) {
        self.refresh_previews(false);
        let Some(job) = &mut self.previews.job else {
            return;
        };
        let mut ended = false;
        let mut ready: Vec<u64> = Vec::new();
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                Msg::Frames(key, done, total) => job.current = Some((key, done, total)),
                Msg::Finished(key, result) => {
                    job.finished += 1;
                    job.current = None;
                    match result {
                        Ok(()) => ready.push(key),
                        Err(e) if e.contains("stopped") => {}
                        Err(e) => job.failed = Some(e),
                    }
                }
                Msg::End => ended = true,
            }
        }
        let stopped = job.cancel.load(Ordering::Relaxed);
        let failed = job.failed.clone();
        if !ready.is_empty() {
            for p in &mut self.previews.pieces {
                if ready.contains(&p.chunk.key) {
                    p.ready = true;
                }
            }
            self.send_previews();
        }
        if ended {
            self.previews.job = None;
            self.previews.disk_bytes = previews::size_on_disk(&self.previews_dir());
            match (stopped, failed) {
                (_, Some(e)) => {
                    log::warn!("render preview failed: {e}");
                    self.notify("A preview could not be made.", true);
                }
                (true, None) => self.notify("Rendering stopped.", false),
                (false, None) => self.notify("Previews are ready.", false),
            }
            let dir = self.previews_dir();
            std::thread::spawn(move || previews::tidy(&dir, KEEP_BYTES));
        }
    }
}
