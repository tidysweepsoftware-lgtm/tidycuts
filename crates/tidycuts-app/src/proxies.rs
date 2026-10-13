//! Making and removing proxy files (small copies of big videos for smooth
//! editing) in the background.

use crate::app::TidyCutsApp;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tidycuts_core::{MediaId, USE_PROXIES};
use tidycuts_engine::proxy;
use tidycuts_media::Tools;

/// A proxy being made.
pub struct Job {
    cancel: Arc<AtomicBool>,
    /// Progress in thousandths.
    progress: Arc<AtomicU32>,
}

pub type Done = (MediaId, Result<PathBuf, String>);

/// How many proxies are made at the same time. FFmpeg already uses every core for one,
/// so more at once only makes each of them (and the editor) slower.
const AT_ONCE: usize = 2;

static RUNNING: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
static FREED: std::sync::Condvar = std::sync::Condvar::new();

/// A place in the queue; given back when dropped.
struct Slot;

impl Slot {
    /// Waits for a free place (stays stoppable), or `None` if stopped while waiting.
    fn wait(cancel: &AtomicBool) -> Option<Slot> {
        let mut n = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            if *n < AT_ONCE {
                *n += 1;
                return Some(Slot);
            }
            n = FREED
                .wait_timeout(n, std::time::Duration::from_millis(250))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        FREED.notify_one();
    }
}

impl TidyCutsApp {
    /// Follows the preference for using proxies.
    pub fn sync_proxy_pref(&self) {
        USE_PROXIES.store(self.prefs.use_proxies, Ordering::Relaxed);
    }

    /// How far along the proxy of a media item is (0..1), if one is being made.
    pub fn proxy_progress(&self, id: MediaId) -> Option<f32> {
        self.proxy_jobs
            .get(&id)
            .map(|j| j.progress.load(Ordering::Relaxed) as f32 / 1000.0)
    }

    /// Puts a freshly probed file into the project, and starts its proxy when
    /// it is big and proxies are set to be made automatically.
    pub fn add_imported_media(&mut self, item: tidycuts_core::MediaItem) {
        if let Some(e) = &self.engine {
            // Start building the scrub cache right away.
            let _ = e.scrub.get(&item);
        }
        let (id, wants) = (item.id, item.wants_proxy());
        self.project.add_media(item);
        if wants && self.prefs.use_proxies && self.prefs.auto_proxies {
            self.make_proxy(id);
        }
    }

    /// Starts making a proxy of a video. Returns false if it can't be done.
    pub fn make_proxy(&mut self, id: MediaId) -> bool {
        let Some(media) = self.project.media(id).cloned() else {
            return false;
        };
        if media.kind != tidycuts_core::MediaKind::Video
            || media.proxy.as_ref().is_some_and(|p| p.is_file())
            || self.proxy_jobs.contains_key(&id)
        {
            return false;
        }
        let Ok(tools) = Tools::get() else {
            self.notify("Making proxies needs FFmpeg.", true);
            return false;
        };
        let out = proxy::proxy_path(&self.prefs.cache_dir(), &media);
        let job = Job {
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU32::new(0)),
        };
        let (cancel, progress) = (job.cancel.clone(), job.progress.clone());
        self.proxy_jobs.insert(id, job);
        let (tx, ctx): (Sender<Done>, _) = (self.proxy_tx.clone(), self.ctx.clone());
        self.notify(
            &format!(
                "Making a smaller copy of {} for smooth editing…",
                media.name
            ),
            false,
        );
        std::thread::spawn(move || {
            let ctx2 = ctx.clone();
            let Some(_slot) = Slot::wait(&cancel) else {
                let _ = tx.send((id, Err("Cancelled.".into())));
                ctx.request_repaint();
                return;
            };
            let result = proxy::make_proxy(tools, &media, &out, &cancel, |p| {
                progress.store((p * 1000.0) as u32, Ordering::Relaxed);
                ctx2.request_repaint();
            })
            .map(|()| out);
            let _ = tx.send((id, result));
            ctx.request_repaint();
        });
        true
    }

    /// Stops a proxy being made, or deletes a finished one.
    pub fn remove_proxy(&mut self, id: MediaId) {
        if let Some(job) = self.proxy_jobs.get(&id) {
            job.cancel.store(true, Ordering::Relaxed);
            return;
        }
        let path = self.project.media_mut(id).and_then(|m| m.proxy.take());
        if let Some(p) = path {
            let _ = std::fs::remove_file(p);
            self.touch();
            if let Some(e) = &mut self.engine {
                e.invalidate();
            }
            self.notify("Proxy removed. The original is used again.", false);
        }
    }

    /// Takes in finished proxies. Called every frame.
    pub fn poll_proxies(&mut self) {
        while let Ok((id, result)) = self.proxy_rx.try_recv() {
            self.proxy_jobs.remove(&id);
            let name = self
                .project
                .media(id)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            match result {
                Ok(path) => {
                    if let Some(m) = self.project.media_mut(id) {
                        m.proxy = Some(path);
                        self.touch();
                        if let Some(e) = &mut self.engine {
                            e.invalidate();
                        }
                        self.notify(&format!("{name} is ready for smooth editing."), false);
                    } else {
                        // The video left the project meanwhile.
                        let _ = std::fs::remove_file(path);
                    }
                }
                Err(e) if e == "Cancelled." => {}
                Err(e) => self.notify(&e, true),
            }
        }
        if !self.proxy_jobs.is_empty() {
            self.ctx
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}
