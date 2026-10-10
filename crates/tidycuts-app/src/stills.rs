//! Pictures made from the timeline: the Snapshot button (the frame under
//! the playhead at full size, saved as a PNG into the library) and Freeze
//! Frame (a clip's own frame held as a still in the clip).
//!
//! Both do their decoding and drawing on a worker thread and hand the
//! finished picture back to the window, which adds it to the project.

use crate::app::TidyCutsApp;
use crate::prefs::config_dir;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use tidycuts_core::{ClipId, MediaItem, MediaKind, Time, TrackKind};

/// How long a freeze frame lasts when it goes in (default).
pub const FREEZE_SECS: f64 = 5.0;

enum Job {
    Snapshot,
    Freeze { clip: ClipId, at: Time, len: Time },
}

type Done = (Job, Result<MediaItem, String>);

pub struct StillJobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// Pictures still being made.
    pub busy: usize,
}

impl Default for StillJobs {
    fn default() -> Self {
        let (tx, rx) = channel();
        StillJobs { tx, rx, busy: 0 }
    }
}

/// The first "<stem> N.png" in `dir` that doesn't exist yet.
fn free_name(dir: &Path, stem: &str) -> PathBuf {
    (1..)
        .map(|n| dir.join(format!("{stem} {n}.png")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}.png")))
}

impl TidyCutsApp {
    /// Where pictures made by the editor go: beside the saved project, or
    /// in TidyCuts' own folder until it's saved.
    pub(crate) fn stills_dir(&self) -> PathBuf {
        match self.project_path.as_ref().and_then(|p| p.parent()) {
            Some(dir) if !dir.as_os_str().is_empty() => dir.join("TidyCuts pictures"),
            _ => config_dir().join("pictures"),
        }
    }

    /// Saves the picture at the playhead (everything on the timeline, at the
    /// project's size) as a PNG and adds it to the library.
    pub fn take_snapshot(&mut self) {
        self.pause();
        let Some(engine) = &self.engine else {
            self.notify("Snapshots need a graphics device.", true);
            return;
        };
        if self.project.duration() == Time::ZERO {
            self.notify(
                "There is nothing on the timeline to take a snapshot of.",
                true,
            );
            return;
        }
        let path = free_name(&self.stills_dir(), "Snapshot");
        let project = self.project.clone();
        let at = self
            .playhead()
            .min(project.duration() - Time::from_millis(1));
        let renderer = engine.renderer.clone();
        let analysis = engine.analysis.clone();
        let tx = self.stills.tx.clone();
        self.stills.busy += 1;
        std::thread::Builder::new()
            .name("tidycuts-snapshot".into())
            .spawn(move || {
                let statics = tidycuts_engine::Statics::default();
                let auto = move |m: &MediaItem| analysis.auto_grade(m);
                let made = tidycuts_engine::snapshot::snapshot(
                    &project, at, renderer, &statics, &auto, &path,
                )
                .and_then(|()| tidycuts_media::probe(&path).map_err(|e| e.to_string()));
                let _ = tx.send((Job::Snapshot, made));
            })
            .expect("spawn snapshot thread");
    }

    /// Holds the frame of the selected video clip under the playhead for
    /// five seconds (a Freeze Frame).
    pub fn freeze_frame(&mut self) {
        let at = self.playhead();
        let Some((id, clip, media)) = self.selected_clip().and_then(|id| {
            let (track, clip) = self.project.clip(id)?;
            let media = clip.media_id.and_then(|m| self.project.media(m))?;
            (track.kind == TrackKind::Video && media.kind == MediaKind::Video && clip.contains(at))
                .then(|| (id, clip.clone(), media.clone()))
        }) else {
            self.notify(
                "Select a video clip and put the playhead on it to freeze a frame.",
                true,
            );
            return;
        };
        if self.project.clip(id).is_some_and(|(t, _)| t.locked) {
            self.notify("That track is locked.", true);
            return;
        }
        // The clip's own picture, without its effects: the still gets those
        // from the clip's settings, like the clip it came from.
        let secs = clip.source_time(at).as_secs_f64().max(0.0);
        let path = free_name(&self.stills_dir(), "Freeze frame");
        let tx = self.stills.tx.clone();
        let len = Time::from_secs_f64(FREEZE_SECS);
        self.stills.busy += 1;
        std::thread::Builder::new()
            .name("tidycuts-freeze".into())
            .spawn(move || {
                let made = (|| {
                    let size = (media.width, media.height);
                    let f = tidycuts_media::grab_frame(&media.path, secs, size, size.0, size.1)
                        .map_err(|e| e.to_string())?;
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    tidycuts_engine::snapshot::write_png(&path, f.width, f.height, &f.pixels)?;
                    tidycuts_media::probe(&path).map_err(|e| e.to_string())
                })();
                let _ = tx.send((Job::Freeze { clip: id, at, len }, made));
            })
            .expect("spawn freeze thread");
    }

    /// Adds the pictures the workers have finished to the project.
    pub fn poll_stills(&mut self) {
        while let Ok((job, made)) = self.stills.rx.try_recv() {
            self.stills.busy = self.stills.busy.saturating_sub(1);
            let item = match made {
                Ok(item) => item,
                Err(e) => {
                    log::warn!("making a picture failed: {e}");
                    self.notify("Couldn't make that picture.", true);
                    continue;
                }
            };
            let still = item.id;
            match job {
                Job::Snapshot => {
                    let name = item
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.checkpoint();
                    self.project.add_media(item);
                    self.selected_media = Some(still);
                    self.notify(&format!("Saved {name} to the library."), false);
                }
                Job::Freeze { clip, at, len } => {
                    let placed = self.edit(|p| {
                        p.add_media(item);
                        p.freeze_frame(clip, at, still, len).is_some()
                    });
                    if !placed {
                        self.notify("That frame can't be frozen any more.", true);
                    }
                }
            }
        }
    }
}
