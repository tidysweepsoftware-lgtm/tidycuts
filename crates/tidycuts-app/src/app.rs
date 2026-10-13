//! The editor's state and every action the menus, shortcuts and panels can
//! take. Panels live in their own modules and call into this one.

use crate::dialogs::{self, DialogState};
use crate::engine::{self, Engine};
use crate::inspector::{self, InspectorState};
use crate::library::{self, LibraryState};
use crate::prefs::{self, ImportSize, Prefs};
use crate::preview::{self, PreviewMode, PreviewState};
use crate::sounds::SoundState;
use crate::theme;
use crate::thumbs::Thumbs;
use crate::timeline::{self, TimelineState};
use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Key, KeyboardShortcut, Modifiers, Rect, Sense,
    UiBuilder,
};
use egui_phosphor::regular as ph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tidycuts_core::{
    is_project_file, Clip, ClipAnimation, ClipId, Edge, Effect, Generator, MediaId, MediaItem,
    MediaKind, Project, StickerSpec, Time, TrackId, TrackKind, Transition,
    LEGACY_PROJECT_EXTENSIONS, MAX_AUDIO_TRACKS, MAX_VIDEO_TRACKS, PROJECT_EXTENSION,
};
use tidycuts_engine::export::{ExportProgress, ExportSettings};
use tidycuts_media::Tools;

const MAX_UNDO: usize = 200;
/// Height of the title bar.
const BAR_H: f32 = 40.0;
/// Width of the properties panel on the right.
const DOCK_W: f32 = 310.0;

/// Something that must wait for the "save changes?" question.
#[derive(Debug, Clone)]
pub enum Pending {
    New,
    Open(Option<PathBuf>),
    Close,
}

struct Toast {
    text: String,
    error: bool,
    until: Instant,
}

type ImportResult = (PathBuf, Result<MediaItem, String>);

/// A running export.
pub struct ExportJob {
    pub settings: ExportSettings,
    pub progress: Arc<Mutex<ExportProgress>>,
    pub cancel: Arc<AtomicBool>,
    pub result: Receiver<Result<(), String>>,
    pub started: Instant,
    /// Set when the export ends: the outcome, kept for the result screen.
    pub finished: Option<(Result<(), String>, Duration)>,
}

pub struct TidyCutsApp {
    pub project: Project,
    pub project_path: Option<PathBuf>,
    pub dirty: bool,
    /// Bumped on every change to `project`, so views can tell when to refresh.
    pub revision: u64,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// When the last coalescing edit happened, and to which clip.
    last_coalesced: Option<(ClipId, Instant, usize)>,
    pub prefs: Prefs,

    pub selected_media: Option<MediaId>,
    /// Selected clips; the last one is the one the properties panel shows.
    pub selection: Vec<ClipId>,
    pub inspector_open: bool,
    /// Left-menus layout: the left side shows the properties of the
    /// selected clip instead of the library.
    pub left_shows_props: bool,
    /// A panel edge was being dragged last frame (saved when let go).
    layout_dragged: bool,
    pub inspector: InspectorState,
    pub library: LibraryState,
    pub timeline: TimelineState,
    pub preview: PreviewState,
    pub dialogs: DialogState,

    pub engine: Option<Engine>,
    pub gpu_error: Option<String>,
    pub thumbs: Thumbs,
    /// Built-in sound effects and music (see `sounds.rs`).
    pub sounds: SoundState,
    /// Snapshots and freeze frames being made (see `stills.rs`).
    pub stills: crate::stills::StillJobs,
    /// The connection Claude edits through (see `mcp.rs`).
    pub mcp: crate::mcp::McpState,
    pub clipboard: Vec<(TrackKind, Clip)>,
    clipboard_track: Option<TrackId>,

    import_tx: Sender<ImportResult>,
    import_rx: Receiver<ImportResult>,
    /// Files being probed, in the order they were picked, so the library
    /// lists them in that order whichever probe finishes first.
    import_order: VecDeque<PathBuf>,
    import_done: HashMap<PathBuf, Result<MediaItem, String>>,
    pub importing: usize,
    /// Media already asked about (keep 4K or downscale), so we ask once.
    asked_size: HashSet<MediaId>,
    pub(crate) asked_proxy: HashSet<MediaId>,

    toasts: Vec<Toast>,
    pub ffmpeg_error: Option<String>,
    pub export_job: Option<ExportJob>,

    pending: Option<Pending>,
    allow_close: bool,
    was_playing: bool,
    last_autosave: Instant,
    /// Voiceovers being made, and their answers.
    pub(crate) voice_tx: Sender<crate::voiceover::Done>,
    pub(crate) voice_rx: Receiver<crate::voiceover::Done>,
    /// Proxy files being made, and their answers.
    pub(crate) proxy_jobs: HashMap<MediaId, crate::proxies::Job>,
    pub(crate) proxy_tx: Sender<crate::proxies::Done>,
    pub(crate) proxy_rx: Receiver<crate::proxies::Done>,
    /// For worker threads that need to wake the window.
    pub(crate) ctx: egui::Context,
    /// Searches through clips (silences, scenes, beats) and their answers.
    pub(crate) detect_tx: Sender<crate::detection::Finished>,
    pub(crate) detect_rx: Receiver<crate::detection::Finished>,
    pub(crate) detecting: usize,
    pub(crate) duck_tx: Sender<crate::autoduck::Finished>,
    pub(crate) duck_rx: Receiver<crate::autoduck::Finished>,
    pub(crate) sync_tx: Sender<crate::detection::SyncFinished>,
    pub(crate) sync_rx: Receiver<crate::detection::SyncFinished>,
    /// The audio mixer window and its meters.
    pub mixer: crate::mixer::MixerState,
    /// The scopes window (waveform, parade, vectorscope, histogram).
    pub scopes: crate::scopes::ScopesState,
    /// Which parts of the timeline have a saved preview, and the drawing of the rest.
    pub previews: crate::previews::PreviewState,
    /// When the mixer last changed something, for one undo step per drag.
    mix_coalesced: Option<(Instant, usize)>,
    /// The backup file this session last wrote, removed once it isn't needed.
    autosave_written: Option<PathBuf>,
    /// Backups left by a crashed session, offered when the app starts.
    recovery: Vec<(PathBuf, std::time::SystemTime)>,
}

impl TidyCutsApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Vec<PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        egui_extras_install(&cc.egui_ctx);
        let prefs = Prefs::load();
        let (import_tx, import_rx) = channel();
        let (detect_tx, detect_rx) = channel();
        let (duck_tx, duck_rx) = channel();
        let (sync_tx, sync_rx) = channel();
        let (proxy_tx, proxy_rx) = channel();
        let (voice_tx, voice_rx) = channel();
        let (engine, gpu_error) = match &cc.wgpu_render_state {
            Some(rs) => {
                let e = Engine::new(rs, &cc.egui_ctx, prefs.cache_dir(), prefs.hardware_decode);
                e.set_hearing_guard(prefs.hearing_guard);
                (Some(e), None)
            }
            None => (
                None,
                Some("No graphics device was found, so the preview can't be shown.".to_string()),
            ),
        };
        let project = Project {
            settings: prefs.project_defaults.clone(),
            ..Default::default()
        };
        let mut timeline = TimelineState::default();
        timeline.snap = prefs.snapping;
        let mut app = TidyCutsApp {
            project,
            project_path: None,
            dirty: false,
            revision: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            last_coalesced: None,
            selected_media: None,
            selection: Vec::new(),
            inspector_open: true,
            left_shows_props: false,
            layout_dragged: false,
            inspector: InspectorState::default(),
            library: LibraryState::default(),
            timeline,
            preview: PreviewState::default(),
            dialogs: DialogState::default(),
            engine,
            gpu_error,
            thumbs: Thumbs::new(&cc.egui_ctx),
            sounds: SoundState::new(&cc.egui_ctx),
            stills: Default::default(),
            mcp: Default::default(),
            clipboard: Vec::new(),
            clipboard_track: None,
            import_tx,
            import_rx,
            import_order: VecDeque::new(),
            import_done: HashMap::new(),
            importing: 0,
            asked_size: HashSet::new(),
            asked_proxy: HashSet::new(),
            toasts: Vec::new(),
            ffmpeg_error: Tools::get().err().map(|e| e.to_string()),
            export_job: None,
            pending: None,
            allow_close: false,
            was_playing: false,
            last_autosave: Instant::now(),
            voice_tx,
            voice_rx,
            proxy_jobs: HashMap::new(),
            proxy_tx,
            proxy_rx,
            ctx: cc.egui_ctx.clone(),
            mixer: Default::default(),
            scopes: Default::default(),
            previews: Default::default(),
            mix_coalesced: None,
            detect_tx,
            detect_rx,
            detecting: 0,
            duck_tx,
            duck_rx,
            sync_tx,
            sync_rx,
            autosave_written: None,
            recovery: if open.is_empty() {
                prefs::leftover_autosaves()
            } else {
                Vec::new()
            },
            prefs,
        };
        app.sync_proxy_pref();
        app.tidy_previews_at_start();
        app.thumbs.cache_dir = Some(app.prefs.cache_dir());
        app.mcp.sync(&app.prefs, &cc.egui_ctx);
        // A project file (e.g. double-clicked) or media passed on the command line.
        match open.as_slice() {
            [p] if is_project_file(p) => app.open_project_at(p),
            [] => {}
            _ => app.import_paths(open),
        }
        app
    }

    // ----- history -------------------------------------------------------

    fn push_undo(&mut self, before: Project) {
        self.undo.push(before);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last_coalesced = None;
    }

    /// Records the current state for undo. Call before changing `project`.
    pub fn checkpoint(&mut self) {
        self.push_undo(self.project.clone());
        self.touch();
    }

    /// Records an already-taken snapshot for undo (for edits made in place).
    pub fn record(&mut self, before: Project) {
        self.push_undo(before);
        self.touch();
    }

    /// Runs an edit; when it reports a change, it becomes one undo step.
    pub fn edit(&mut self, f: impl FnOnce(&mut Project) -> bool) -> bool {
        let before = self.project.clone();
        if f(&mut self.project) {
            self.record(before);
            self.prune_selection();
            true
        } else {
            false
        }
    }

    /// A change from the mixer's faders. A drag is one undo step.
    pub fn edit_mix(&mut self, ctx: &egui::Context, f: impl FnOnce(&mut Project)) {
        let busy = ctx.input(|i| i.pointer.any_down());
        let coalesce = busy
            && self.mix_coalesced.is_some_and(|(at, depth)| {
                at.elapsed() < Duration::from_millis(2000) && depth == self.undo.len()
            });
        if !coalesce {
            self.push_undo(self.project.clone());
        }
        f(&mut self.project);
        self.mix_coalesced = Some((Instant::now(), self.undo.len()));
        self.touch();
    }

    /// Marks the project changed without recording undo.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.revision += 1;
        // Come back after the edit settles to update what depends on it
        // (the render previews on the ruler); an idle window won't repaint on its own.
        self.ctx.request_repaint_after(Duration::from_millis(250));
    }

    /// Replaces a clip with an edited copy (from the properties panel).
    /// Continuous changes (a slider drag, typing) become one undo step.
    pub fn commit_clip(&mut self, ctx: &egui::Context, clip: Clip) {
        let id = clip.id;
        let Some(current) = self.project.clip_mut(id) else {
            return;
        };
        if *current == clip {
            return;
        }
        let busy = ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused().is_some());
        let coalesce = self.last_coalesced.is_some_and(|(c, at, depth)| {
            c == id && at.elapsed() < Duration::from_millis(1500) && depth == self.undo.len()
        }) && busy;
        if !coalesce {
            let before = self.project.clone();
            self.push_undo(before);
        }
        if let Some(c) = self.project.clip_mut(id) {
            *c = clip;
        }
        self.last_coalesced = Some((id, Instant::now(), self.undo.len()));
        self.touch();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, prev));
            self.after_history_change();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, next));
            self.after_history_change();
        }
    }

    fn after_history_change(&mut self) {
        self.last_coalesced = None;
        self.prune_selection();
        if self
            .selected_media
            .is_some_and(|m| self.project.media(m).is_none())
        {
            self.selected_media = None;
        }
        self.touch();
    }

    // ----- selection -----------------------------------------------------

    /// The clip the properties panel shows.
    pub fn selected_clip(&self) -> Option<ClipId> {
        self.selection.last().copied()
    }

    pub fn is_selected(&self, id: ClipId) -> bool {
        self.selection.contains(&id)
    }

    /// Selects a clip; with `additive` it toggles it in the selection.
    pub fn select(&mut self, id: ClipId, additive: bool) {
        if additive {
            if let Some(i) = self.selection.iter().position(|c| *c == id) {
                self.selection.remove(i);
            } else {
                self.selection.push(id);
            }
        } else {
            self.selection = vec![id];
        }
        if let Some((_, c)) = self.project.clip(id) {
            if c.media_id.is_some() {
                self.selected_media = c.media_id;
            }
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(|c| c.id))
            .collect();
    }

    fn prune_selection(&mut self) {
        let p = &self.project;
        self.selection.retain(|c| p.clip(*c).is_some());
    }

    /// Shows the properties panel on the right (it stays open between clips).
    pub fn open_inspector(&mut self) {
        self.inspector_open = true;
        self.left_shows_props = true;
    }

    // ----- transport -----------------------------------------------------

    pub fn is_playing(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| e.is_playing())
    }

    /// Where the timeline playhead is.
    pub fn playhead(&self) -> Time {
        self.timeline.playhead
    }

    /// Moves the timeline playhead (switching the preview to the timeline).
    pub fn seek(&mut self, t: Time) {
        let t = t.max(Time::ZERO).min(self.project.duration());
        profile_event(&format!(
            "seek to {:.2} playing={}",
            t.as_secs_f64(),
            self.is_playing()
        ));
        FRAMES_TO_LOG.store(3, std::sync::atomic::Ordering::Relaxed);
        if self.preview.mode != PreviewMode::Timeline {
            self.show_timeline();
        }
        self.timeline.playhead = t;
        if self.is_playing() {
            let size = self.preview.render_size;
            let (project, rev) = (&self.project, self.revision);
            if let Some(e) = &mut self.engine {
                // The new place shows at once from the scrub cache; playing
                // from there takes a moment to start.
                e.show_still(project, rev, None, t, size, true);
                e.play(project, rev, t, size, true);
            }
        }
    }

    /// Moves the source preview's playhead.
    pub fn seek_source(&mut self, t: Time) {
        self.preview.source_time = t.max(Time::ZERO);
        if self.is_playing() {
            self.pause();
            self.toggle_play();
        }
    }

    pub fn toggle_play(&mut self) {
        if self.is_playing() {
            self.pause();
            return;
        }
        let size = self.preview.render_size;
        match self.preview.mode {
            PreviewMode::Timeline => {
                let len = self.project.duration();
                if len == Time::ZERO {
                    return;
                }
                if self.timeline.playhead >= len - self.project.settings.frame_rate.frame_duration()
                {
                    self.timeline.playhead = Time::ZERO;
                }
                // Previews of parts edited a moment ago must not be played.
                self.refresh_previews(true);
                let (project, rev, at) = (&self.project, self.revision, self.timeline.playhead);
                if let Some(e) = &mut self.engine {
                    e.play(project, rev, at, size, true);
                }
            }
            PreviewMode::Source => {
                let Some(id) = self.preview.source_media else {
                    return;
                };
                let Some(m) = self.media_item(id).cloned() else {
                    return;
                };
                if self.preview.source_time >= m.duration - Time::from_millis(50) {
                    self.preview.source_time = Time::ZERO;
                }
                let p = engine::source_project(&m, &self.project.settings);
                let rev = source_revision(id);
                let at = self.preview.source_time;
                if let Some(e) = &mut self.engine {
                    e.play(&p, rev, at, size, false);
                }
            }
        }
        self.was_playing = true;
    }

    pub fn pause(&mut self) {
        if let Some(e) = &mut self.engine {
            if e.is_playing() {
                let t = e.pause();
                match self.preview.mode {
                    PreviewMode::Timeline => {
                        self.timeline.playhead = self
                            .project
                            .settings
                            .frame_rate
                            .snap(t)
                            .min(self.project.duration())
                    }
                    PreviewMode::Source => self.preview.source_time = t,
                }
            }
        }
        self.was_playing = false;
    }

    /// Steps the playhead by whole frames.
    pub fn step_frames(&mut self, n: i64) {
        self.pause();
        let fps = self.project.settings.frame_rate;
        match self.preview.mode {
            PreviewMode::Timeline => {
                let f = fps.frames_at(self.timeline.playhead) + n;
                self.seek(fps.time_of_frame(f.max(0)));
            }
            PreviewMode::Source => {
                let f = fps.frames_at(self.preview.source_time) + n;
                self.preview.source_time = fps.time_of_frame(f.max(0));
            }
        }
    }

    /// Jumps to the previous or next cut on any track (Up/Down arrows).
    pub fn jump_to_edit(&mut self, forward: bool) {
        let t = self.playhead();
        let mut cuts: Vec<Time> = self
            .project
            .tracks
            .iter()
            .flat_map(|tr| tr.clips.iter().flat_map(|c| [c.start, c.end()]))
            .chain([Time::ZERO])
            .collect();
        cuts.sort();
        cuts.dedup();
        let eps = Time::from_millis(1);
        let next = if forward {
            cuts.into_iter().find(|c| *c > t + eps)
        } else {
            cuts.into_iter().rev().find(|c| *c < t - eps)
        };
        if let Some(n) = next {
            self.seek(n);
        }
    }

    /// A media item of the project, or a built-in sound that isn't in the
    /// project yet.
    pub fn media_item(&self, id: MediaId) -> Option<&MediaItem> {
        self.project
            .media(id)
            .or_else(|| self.sounds.media_by_id(id))
    }

    pub fn show_source(&mut self, media: MediaId) {
        self.pause();
        if self.preview.source_media != Some(media) {
            self.preview.source_time = Time::ZERO;
        }
        self.preview.source_media = Some(media);
        self.preview.mode = PreviewMode::Source;
    }

    pub fn show_timeline(&mut self) {
        if self.preview.mode != PreviewMode::Timeline {
            self.pause();
            self.preview.mode = PreviewMode::Timeline;
        }
    }

    // ----- clip editing --------------------------------------------------

    /// Clips an action applies to: the selection, or else whatever sits
    /// under the playhead on unlocked tracks.
    /// Clips a split would cut: the selected ones under the playhead, or
    /// else every unlocked clip under it.
    pub fn targets_at_playhead(&self) -> Vec<ClipId> {
        let t = self.playhead();
        let selected: Vec<ClipId> = self
            .selection
            .iter()
            .copied()
            .filter(|c| {
                self.project
                    .clip(*c)
                    .is_some_and(|(tr, c)| c.contains(t) && !tr.locked)
            })
            .collect();
        if !selected.is_empty() {
            return selected;
        }
        self.project
            .tracks
            .iter()
            .filter(|tr| !tr.locked)
            .filter_map(|tr| tr.clip_at(t).map(|c| c.id))
            .collect()
    }

    /// Splits clips at the playhead (Ctrl+B and the playhead's scissors).
    pub fn split_at_playhead(&mut self) {
        let t = self.playhead();
        let targets = self.targets_at_playhead();
        if targets.is_empty() {
            self.notify("Move the playhead over a clip to split it.", false);
            return;
        }
        let mut new_right = Vec::new();
        self.edit(|p| {
            for c in &targets {
                if let Some(r) = p.split_clip(*c, t) {
                    new_right.push(r);
                }
            }
            !new_right.is_empty()
        });
    }

    /// Deletes the selected clips; `ripple` closes the gaps they leave.
    /// Adds a marker at the playhead (or removes the one already there, so
    /// M twice undoes itself).
    pub fn add_marker_at_playhead(&mut self) {
        let at = self.playhead();
        let one_frame = Time::from_secs_f64(1.0 / self.project.settings.frame_rate.as_f64());
        let near = self.project.marker_near(at, one_frame).map(|m| m.id);
        match near {
            Some(id) => self.delete_marker(id),
            None => {
                let mut added = None;
                self.edit(|p| {
                    added = Some(p.add_marker(at));
                    true
                });
                self.timeline.selected_marker = added;
            }
        }
    }

    pub fn delete_marker(&mut self, id: tidycuts_core::MarkerId) {
        if self.edit(|p| p.remove_marker(id)) && self.timeline.selected_marker == Some(id) {
            self.timeline.selected_marker = None;
        }
    }

    /// Moves the playhead to the next (or previous) marker.
    pub fn jump_to_marker(&mut self, forward: bool) {
        let from = self.playhead();
        if let Some(t) = self.project.next_marker(from, forward).map(|m| m.time) {
            self.pause();
            self.seek(t);
        }
    }

    pub fn delete_selection(&mut self, ripple: bool) {
        if self.selection.is_empty() {
            if let Some(id) = self.timeline.selected_marker {
                self.delete_marker(id);
            }
            return;
        }
        let ids = self.selection.clone();
        let mut locked = false;
        let done = self.edit(|p| {
            let mut any = false;
            for id in &ids {
                let ok = if ripple {
                    p.ripple_delete_clip(*id)
                } else {
                    p.delete_clip(*id)
                };
                any |= ok;
                locked |= !ok && p.clip(*id).is_some();
            }
            any
        });
        if done {
            self.selection.clear();
        }
        if locked {
            self.notify("Clips on locked tracks were left alone.", false);
        }
    }

    /// Closes the empty space under the pointer on a track.
    pub fn delete_gap(&mut self, track: TrackId, at: Time) {
        if !self.edit(|p| p.delete_gap(track, at)) {
            self.notify("There's no gap there to close.", false);
        }
    }

    pub fn copy_selection(&mut self) {
        let mut items: Vec<(TrackKind, Clip, TrackId)> = self
            .selection
            .iter()
            .filter_map(|id| {
                self.project
                    .clip(*id)
                    .map(|(t, c)| (t.kind, c.clone(), t.id))
            })
            .collect();
        items.sort_by_key(|(_, c, _)| c.start);
        self.clipboard_track = items
            .iter()
            .find(|(k, _, _)| *k == TrackKind::Video)
            .map(|(_, _, t)| *t);
        self.clipboard = items.into_iter().map(|(k, c, _)| (k, c)).collect();
        if !self.clipboard.is_empty() {
            self.notify(&format!("Copied {} clip(s).", self.clipboard.len()), false);
        }
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selection(false);
    }

    pub fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let at = self.playhead();
        let items = self.clipboard.clone();
        let track = self
            .clipboard_track
            .filter(|t| self.project.track(*t).is_some());
        let mut pasted = Vec::new();
        self.edit(|p| {
            pasted = p.paste_clips(&items, track, at);
            !pasted.is_empty()
        });
        if pasted.is_empty() {
            self.notify("There's no room to paste there.", true);
        } else {
            self.selection = pasted;
        }
    }

    pub fn duplicate_selection(&mut self) {
        let ids = self.selection.clone();
        let mut new = Vec::new();
        self.edit(|p| {
            for id in &ids {
                if let Some(n) = p.duplicate_clip(*id) {
                    new.push(n);
                }
            }
            !new.is_empty()
        });
        if !new.is_empty() {
            self.selection = new;
        }
    }

    /// Moves the sound of the selected video clips to their own audio
    /// clips (Ctrl+Alt+D).
    pub fn detach_audio(&mut self) {
        let ids: Vec<ClipId> = self
            .selection
            .iter()
            .copied()
            .filter(|id| self.can_detach(*id))
            .collect();
        if ids.is_empty() {
            self.notify("Select a video clip with sound to detach its audio.", false);
            return;
        }
        let mut made = Vec::new();
        self.edit(|p| {
            for id in &ids {
                if let Some(a) = p.detach_audio(*id) {
                    made.push(a);
                }
            }
            !made.is_empty()
        });
        if made.len() < ids.len() {
            self.notify(
                &format!("No free audio track was left (the limit is {MAX_AUDIO_TRACKS})."),
                true,
            );
        } else {
            self.notify(
                "Audio detached to its own track. It stays linked: cutting or deleting one does the same to the other.",
                false,
            );
        }
    }

    /// Cuts the link between each selected clip and the clips it is cut and
    /// deleted together with, so they can be edited on their own.
    pub fn unlink_selection(&mut self) {
        let ids = self.selection.clone();
        let done = self.edit(|p| {
            let mut any = false;
            for id in &ids {
                any |= p.unlink_clip(*id);
            }
            any
        });
        if done {
            self.notify(
                "Unlinked: they can now be cut and deleted separately.",
                false,
            );
        }
    }

    /// Whether a clip is a video clip whose sound can be detached.
    pub fn can_detach(&self, id: ClipId) -> bool {
        self.project.clip(id).is_some_and(|(t, c)| {
            t.kind == TrackKind::Video
                && !c.audio.muted
                && c.media_id
                    .and_then(|m| self.project.media(m))
                    .is_some_and(|m| m.kind == MediaKind::Video && m.has_audio)
        })
    }

    /// Trims the start or end of clips to the playhead (Alt+[ / Alt+]).
    pub fn trim_to_playhead(&mut self, edge: Edge) {
        let t = self.playhead();
        let targets = self.targets_at_playhead();
        if targets.is_empty() {
            self.notify("Move the playhead over a clip to trim it.", false);
            return;
        }
        self.edit(|p| {
            let mut any = false;
            for c in &targets {
                any |= p.trim_clip(*c, edge, t).is_some_and(|d| d != Time::ZERO);
            }
            any
        });
    }

    /// Applies `f` to every selected clip as one undo step.
    pub fn for_selected(&mut self, f: impl Fn(&mut Clip)) {
        let ids = self.selection.clone();
        self.edit(|p| {
            let mut any = false;
            for id in &ids {
                if let Some(c) = p.clip_mut(*id) {
                    let before = c.clone();
                    f(c);
                    any |= *c != before;
                }
            }
            any
        });
    }

    pub fn rotate_selection_90(&mut self) {
        let ids = self.selection.clone();
        self.edit(|p| ids.iter().fold(false, |a, id| p.rotate_clip_90(*id) | a));
    }

    pub fn crop_selection_to_fit(&mut self) {
        let ids = self.selection.clone();
        self.edit(|p| ids.iter().fold(false, |a, id| p.crop_to_fit(*id) | a));
    }

    pub fn set_speed(&mut self, id: ClipId, speed: f64) {
        if !self.edit(|p| p.set_speed(id, speed)) {
            self.notify(
                "The speed couldn't be changed (is the track locked?).",
                false,
            );
        }
    }

    pub fn toggle_reverse(&mut self) {
        self.for_selected(|c| {
            if c.media_id.is_some() {
                c.reverse = !c.reverse
            }
        });
    }

    pub fn toggle_mute(&mut self) {
        let all_muted = self
            .selection
            .iter()
            .filter_map(|id| self.project.clip(*id))
            .all(|(_, c)| c.audio.muted);
        self.for_selected(|c| c.audio.muted = !all_muted);
    }

    pub fn toggle_disable(&mut self) {
        let all_off = self
            .selection
            .iter()
            .filter_map(|id| self.project.clip(*id))
            .all(|(_, c)| c.disabled);
        self.for_selected(|c| c.disabled = !all_off);
    }

    pub fn set_mark(&mut self, mark: Option<u8>) {
        self.for_selected(|c| c.mark = mark);
    }

    /// Replaces the media a clip plays with another library item.
    pub fn replace_clip_media(&mut self, clip: ClipId, media: MediaId) {
        if !self.edit(|p| p.replace_clip_media(clip, media)) {
            self.notify("That media can't replace this clip.", true);
        }
    }

    /// Selects every clip on the clip's track from it onwards.
    pub fn select_from(&mut self, clip: ClipId) {
        if let Some((t, c)) = self.project.clip(clip) {
            let start = c.start;
            self.selection = t
                .clips
                .iter()
                .filter(|o| o.start >= start)
                .map(|o| o.id)
                .collect();
        }
    }

    // ----- adding to the timeline -----------------------------------------

    /// Puts media on the timeline: at a spot when given, else at the end of
    /// its default track.
    pub fn add_media_to_timeline(&mut self, media: MediaId, at: Option<(TrackId, Time)>) {
        let Some(m) = self.media_item(media).cloned() else {
            return;
        };
        let first_video = m.kind == MediaKind::Video && !self.has_visual_clips();
        // Built-in sounds go in at the playhead, like titles; other media
        // goes on the end of its track.
        let sound = crate::sounds::sound_id_of(&m.path).is_some();
        let playhead = self.playhead();
        let before = self.project.clone();
        if self.project.media(media).is_none() {
            self.project.add_media(m.clone());
        }
        let placed = match at {
            Some((track, t)) => self.project.add_clip(media, track, t),
            None if sound => self
                .project
                .audio_track_for(playhead, m.duration)
                .and_then(|track| self.project.add_clip(media, track, playhead)),
            None => self.project.append_clip(media),
        };
        if placed.is_some() {
            self.record(before);
            self.prune_selection();
        } else {
            self.project = before;
        }
        match placed {
            Some(c) => {
                self.selection = vec![c];
                self.check_media_proxy(&m);
                if first_video {
                    self.check_media_size(&m);
                }
            }
            None => self.notify("There's no unlocked track for that kind of media.", true),
        }
    }

    fn has_visual_clips(&self) -> bool {
        self.project
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Video && !t.clips.is_empty())
    }

    /// The video track a new title or sticker goes on: a free overlay
    /// track at the playhead.
    fn overlay_at(&mut self, at: Time, len: Time) -> Option<TrackId> {
        self.project.overlay_track_for(at, len)
    }

    /// Adds a generated clip (title, sticker, color) at the playhead on an
    /// overlay track, or at `at` on a given track.
    pub fn add_generated(
        &mut self,
        generator: Generator,
        len: Time,
        at: Option<(TrackId, Time)>,
        setup: impl FnOnce(&mut Clip),
    ) -> Option<ClipId> {
        let before = self.project.clone();
        let (track, t) = match at {
            Some(x) => x,
            None => {
                let t = self.playhead();
                match self.overlay_at(t, len) {
                    Some(tr) => (tr, t),
                    None => {
                        self.project = before;
                        self.notify(
                            &format!(
                                "All {MAX_VIDEO_TRACKS} video tracks are busy at the playhead."
                            ),
                            true,
                        );
                        return None;
                    }
                }
            }
        };
        let mut clip = Clip::generated(generator, t, len);
        setup(&mut clip);
        match self.project.place_clip(clip, track, t) {
            Some(id) => {
                self.record(before);
                self.selection = vec![id];
                Some(id)
            }
            None => {
                self.project = before;
                self.notify("That track is locked.", true);
                None
            }
        }
    }

    /// Adds a title from the catalog.
    pub fn add_title(&mut self, preset_id: &str, at: Option<(TrackId, Time)>) {
        use tidycuts_render::catalog::titles;
        let Some(preset) = titles::get(preset_id).or_else(|| titles::TITLES.first()) else {
            return;
        };
        let k = self.project.settings.height as f64 / 1080.0;
        let len = Time::from_secs_f64(if preset.duration > 0.0 {
            preset.duration as f64
        } else {
            self.prefs.title_duration
        });
        let id = self.add_generated(Generator::Title((preset.make)()), len, at, |c| {
            c.transform.position_x.value = preset.position[0] as f64 * k;
            c.transform.position_y.value = preset.position[1] as f64 * k;
            c.anim_in = preset.anim_in.map(|(kind, d)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(d as f64),
            });
            c.anim_out = preset.anim_out.map(|(kind, d)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(d as f64),
            });
        });
        if id.is_some() {
            self.inspector.request_tab("Title");
            self.open_inspector();
        }
    }

    /// Adds a title with its own words, start and length (for Claude): on a
    /// free overlay track.
    pub fn add_title_with(
        &mut self,
        preset_id: &str,
        text: Option<&str>,
        start: Time,
        len: Option<Time>,
    ) -> Option<ClipId> {
        use tidycuts_render::catalog::titles;
        let preset = titles::get(preset_id).or_else(|| titles::TITLES.first())?;
        let k = self.project.settings.height as f64 / 1080.0;
        let len = len.unwrap_or_else(|| {
            Time::from_secs_f64(if preset.duration > 0.0 {
                preset.duration as f64
            } else {
                self.prefs.title_duration
            })
        });
        let track = self.project.overlay_track_for(start, len)?;
        let mut spec = (preset.make)();
        if let Some(t) = text {
            spec.text = t.to_string();
        }
        self.add_generated(Generator::Title(spec), len, Some((track, start)), |c| {
            c.transform.position_x.value = preset.position[0] as f64 * k;
            c.transform.position_y.value = preset.position[1] as f64 * k;
            c.anim_in = preset.anim_in.map(|(kind, d)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(d as f64),
            });
            c.anim_out = preset.anim_out.map(|(kind, d)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(d as f64),
            });
        })
    }

    /// Runs the calls Claude has made since the last frame.
    fn poll_mcp(&mut self, ctx: &egui::Context) {
        let jobs = self.mcp.take_jobs();
        if jobs.is_empty() {
            return;
        }
        let more = jobs.len() >= 8;
        for job in jobs {
            crate::mcp_tools::run(self, job);
        }
        if more {
            ctx.request_repaint();
        }
    }

    /// Adds a sticker from the catalog.
    pub fn add_sticker(&mut self, kind: &str, at: Option<(TrackId, Time)>) {
        use tidycuts_render::catalog::stickers;
        let Some(def) = stickers::get(kind) else {
            return;
        };
        let spec = StickerSpec {
            kind: def.id.into(),
            color: def.color,
            color2: def.color2,
            motion: def.motion.into(),
        };
        let len = Time::from_secs_f64(self.prefs.sticker_duration);
        self.add_generated(Generator::Sticker(spec), len, at, |_| {});
    }

    /// Adds a color or gradient background clip.
    pub fn add_solid(&mut self, generator: Generator, at: Option<(TrackId, Time)>) {
        let len = Time::from_secs_f64(self.prefs.image_duration);
        let at = at.or_else(|| self.project.main_track().map(|m| (m, self.playhead())));
        self.add_generated(generator, len, at, |_| {});
    }

    /// Adds an effect or filter to clips; with no clips given, as an
    /// adjustment clip over everything below (filter clip).
    pub fn apply_effect(&mut self, kind: &str, clips: &[ClipId], at: Option<(TrackId, Time)>) {
        if clips.is_empty() {
            let len = Time::from_secs_f64(self.prefs.image_duration);
            self.add_effect_clip(kind, at, len);
            return;
        }
        let ids = clips.to_vec();
        let kind = kind.to_string();
        let applied = self.edit(|p| {
            let mut any = false;
            for id in &ids {
                if let Some(c) = p.clip_mut(*id) {
                    if c.media_id.is_some() || c.generator.is_some() {
                        c.effects.push(Effect::new(kind.clone()));
                        any = true;
                    }
                }
            }
            any
        });
        if applied {
            self.selection = ids;
            self.inspector.request_tab("Effects");
            self.open_inspector();
        }
    }

    /// Adds an effect or filter as a clip of its own (an adjustment clip) on
    /// a track of its own, over everything below it.
    pub fn add_effect_clip(&mut self, kind: &str, at: Option<(TrackId, Time)>, len: Time) {
        let kind = kind.to_string();
        self.add_generated(Generator::Adjustment, len, at, move |c| {
            c.effects.push(Effect::new(kind));
        });
    }

    /// Sets the transition into a clip (from the clip before it).
    pub fn set_transition(&mut self, clip: ClipId, kind: &str) {
        use tidycuts_render::catalog::transitions;
        let secs = transitions::get(kind)
            .map(|d| d.duration as f64)
            .filter(|d| *d > 0.0)
            .unwrap_or(self.prefs.transition_duration);
        let kind = kind.to_string();
        self.edit(|p| match p.clip_mut(clip) {
            Some(c) => {
                c.transition_in = Some(Transition::new(
                    kind,
                    Time::from_secs_f64(secs).min(c.duration()),
                ));
                true
            }
            None => false,
        });
    }

    /// Sets the transition to nothing at a clip's end.
    pub fn set_transition_out(&mut self, clip: ClipId, kind: &str) {
        let secs = self.prefs.transition_duration;
        let kind = kind.to_string();
        self.edit(|p| match p.clip_mut(clip) {
            Some(c) => {
                c.transition_out = Some(Transition::new(
                    kind,
                    Time::from_secs_f64(secs).min(c.duration()),
                ));
                true
            }
            None => false,
        });
    }

    pub fn set_animation(&mut self, clip: ClipId, out: bool, kind: Option<&str>) {
        let d = Time::from_secs_f64(self.prefs.animation_duration);
        let kind = kind.map(str::to_string);
        self.edit(|p| match p.clip_mut(clip) {
            Some(c) => {
                let len = c.duration();
                let slot = if out { &mut c.anim_out } else { &mut c.anim_in };
                let keep = slot.as_ref().map(|a| a.duration).unwrap_or(d);
                *slot = kind.map(|k| ClipAnimation {
                    kind: k,
                    duration: keep.min(len),
                });
                true
            }
            None => false,
        });
    }

    // ----- tracks --------------------------------------------------------

    pub fn add_track(&mut self, kind: TrackKind, index: Option<usize>) -> Option<TrackId> {
        if !self.project.can_add_track(kind) {
            let max = if kind == TrackKind::Video {
                MAX_VIDEO_TRACKS
            } else {
                MAX_AUDIO_TRACKS
            };
            let what = if kind == TrackKind::Video {
                "video"
            } else {
                "audio"
            };
            self.notify(
                &format!("A project can have up to {max} {what} tracks."),
                true,
            );
            return None;
        }
        let mut id = None;
        self.edit(|p| {
            id = match index {
                Some(i) => p.insert_track(kind, i),
                None => p.add_track(kind),
            };
            id.is_some()
        });
        id
    }

    pub fn remove_track(&mut self, id: TrackId) {
        let had_clips = self.project.track(id).is_some_and(|t| !t.clips.is_empty());
        if self.edit(|p| p.remove_track(id)) {
            if had_clips {
                self.notify("Track deleted. Press Ctrl+Z to bring it back.", false);
            }
        } else {
            self.notify("The main video track can't be deleted.", false);
        }
    }

    pub fn set_magnetic(&mut self, on: bool) {
        self.edit(|p| {
            p.set_magnetic(on);
            true
        });
    }

    // ----- import --------------------------------------------------------

    pub fn import_dialog(&mut self) {
        let files = rfd::FileDialog::new()
            .set_title("Import media")
            .add_filter(
                "Videos, audio and images",
                &[
                    "mp4", "mov", "mkv", "avi", "webm", "m4v", "wmv", "flv", "mts", "m2ts", "ts",
                    "mpg", "mpeg", "3gp", "gif", "mp3", "wav", "aac", "m4a", "flac", "ogg", "opus",
                    "wma", "aiff", "png", "jpg", "jpeg", "bmp", "webp", "tif", "tiff", "heic",
                ],
            )
            .add_filter("All files", &["*"])
            .pick_files();
        if let Some(files) = files {
            self.import_paths(files);
        }
    }

    pub fn import_paths(&mut self, paths: Vec<PathBuf>) {
        if self.ffmpeg_error.is_some() {
            self.notify("Media can't be imported until FFmpeg is installed.", true);
            return;
        }
        for path in paths {
            if path.is_dir() {
                let mut inner: Vec<PathBuf> = std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file())
                    .collect();
                inner.sort();
                self.import_paths(inner);
                continue;
            }
            if self.project.media.iter().any(|m| m.path == path) {
                continue;
            }
            if self.import_order.contains(&path) {
                continue;
            }
            self.importing += 1;
            self.import_order.push_back(path.clone());
            let tx = self.import_tx.clone();
            std::thread::spawn(move || {
                let result = tidycuts_media::probe(&path).map_err(|e| e.to_string());
                let _ = tx.send((path, result));
            });
        }
    }

    fn poll_imports(&mut self) {
        let mut added = Vec::new();
        while let Ok((path, result)) = self.import_rx.try_recv() {
            self.importing = self.importing.saturating_sub(1);
            self.import_done.insert(path, result);
        }
        while let Some(result) = self
            .import_order
            .front()
            .and_then(|p| self.import_done.remove(p))
        {
            let path = self.import_order.pop_front().unwrap_or_default();
            match result {
                Ok(item) => added.push(item),
                Err(e) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    log::warn!("import of {} failed: {e}", path.display());
                    self.notify(&format!("Couldn't import {name}: it isn't a supported video, audio or image file."), true);
                }
            }
        }
        if added.is_empty() {
            return;
        }
        self.checkpoint();
        // The size question comes when the first video goes on the
        // timeline, not on import.
        for item in added {
            self.selected_media = Some(item.id);
            self.add_imported_media(item);
        }
    }

    /// Asks (or applies the saved answer) whether the project should take a
    /// video's size and frame rate, e.g. keep 4K or downscale to 1080p.
    pub fn check_media_size(&mut self, m: &MediaItem) {
        if m.kind != MediaKind::Video || m.width == 0 || !self.asked_size.insert(m.id) {
            return;
        }
        let s = &self.project.settings;
        let fps_differs = m
            .frame_rate
            .is_some_and(|f| (f.as_f64() - s.frame_rate.as_f64()).abs() > 0.01);
        if (m.width, m.height) == (s.width, s.height) && !fps_differs {
            return;
        }
        match self.prefs.import_size {
            ImportSize::Ask => self.dialogs.ask_import_size(m.id),
            ImportSize::MatchMedia => self.match_project_to(m.id),
            ImportSize::KeepProject => {}
            ImportSize::Downscale1080 => {
                let (w, h) = crate::dialogs::sizes::downscale(m.width, m.height, 1080);
                let fps = m.frame_rate;
                self.edit(|p| {
                    p.settings.width = w;
                    p.settings.height = h;
                    if let Some(f) = fps {
                        p.settings.frame_rate = f;
                    }
                    true
                });
                self.notify(&format!("Project set to {w}×{h} for {}.", m.name), false);
            }
        }
    }

    /// Changes the project's size and frame rate to a video's.
    pub fn match_project_to(&mut self, media: MediaId) {
        let Some(m) = self.project.media(media).cloned() else {
            return;
        };
        self.edit(|p| {
            p.settings.width = m.width.max(2) & !1;
            p.settings.height = m.height.max(2) & !1;
            if let Some(f) = m.frame_rate {
                p.settings.frame_rate = f;
            }
            true
        });
        self.notify(
            &format!(
                "Project set to {}×{} to match {}.",
                m.width, m.height, m.name
            ),
            false,
        );
    }

    pub fn remove_media(&mut self, media: MediaId) {
        self.checkpoint();
        if let Some(m) = self.project.media(media) {
            if let Some(e) = &self.engine {
                e.scrub.forget(&m.path);
            }
            self.thumbs.forget(m);
        }
        self.project.remove_media(media);
        if self.selected_media == Some(media) {
            self.selected_media = None;
        }
        if self.preview.source_media == Some(media) {
            self.preview.source_media = None;
            self.show_timeline();
        }
        self.prune_selection();
    }

    // ----- files ---------------------------------------------------------

    pub fn request_new(&mut self) {
        self.guard(Pending::New);
    }

    pub fn request_open(&mut self, path: Option<PathBuf>) {
        self.guard(Pending::Open(path));
    }

    fn guard(&mut self, action: Pending) {
        if self.dirty {
            self.pending = Some(action);
        } else {
            self.run_pending(action);
        }
    }

    fn run_pending(&mut self, action: Pending) {
        match action {
            Pending::New => {
                let p = Project {
                    settings: self.prefs.project_defaults.clone(),
                    ..Default::default()
                };
                self.reset(p, None);
            }
            Pending::Open(path) => {
                let path = path.or_else(|| {
                    rfd::FileDialog::new()
                        .set_title("Open project")
                        .add_filter(
                            "TidyCuts project",
                            &[PROJECT_EXTENSION, LEGACY_PROJECT_EXTENSIONS[0]],
                        )
                        .pick_file()
                });
                if let Some(p) = path {
                    self.open_project_at(&p);
                }
            }
            Pending::Close => self.allow_close = true,
        }
    }

    pub fn open_project_at(&mut self, path: &Path) {
        match Project::load(path) {
            Ok(mut p) => {
                crate::sounds::restore_sounds(&mut p);
                let missing = p.media.iter().filter(|m| !m.path.exists()).count();
                self.reset(p, Some(path.to_path_buf()));
                self.prefs.add_recent(path);
                self.prefs.save();
                if missing > 0 {
                    self.notify(
                        &format!("{missing} media file(s) in this project can't be found on disk."),
                        true,
                    );
                }
            }
            Err(e) => self.notify(&format!("Couldn't open the project: {e}"), true),
        }
    }

    fn reset(&mut self, project: Project, path: Option<PathBuf>) {
        self.pause();
        self.discard_autosave();
        self.project = project;
        self.project_path = path;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.last_coalesced = None;
        self.selection.clear();
        self.selected_media = None;
        self.preview = PreviewState::default();
        let snap = self.timeline.snap;
        self.timeline = TimelineState::default();
        self.timeline.snap = snap;
        self.asked_size.clear();
        self.asked_proxy.clear();
        self.revision += 1;
        if let Some(e) = &self.engine {
            for m in &self.project.media {
                let _ = e.scrub.get(m);
            }
        }
    }

    /// Saves to the current file, asking for one first if needed.
    pub fn save(&mut self) -> bool {
        match self.project_path.clone() {
            Some(p) => self.save_to(&p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        let path = rfd::FileDialog::new()
            .set_title("Save project")
            .set_file_name(format!("{}.{PROJECT_EXTENSION}", self.project.name))
            .add_filter("TidyCuts project", &[PROJECT_EXTENSION])
            .save_file();
        match path {
            Some(mut p) => {
                if p.extension().is_none_or(|e| e != PROJECT_EXTENSION) {
                    p.set_extension(PROJECT_EXTENSION);
                }
                if let Some(stem) = p.file_stem() {
                    self.project.name = stem.to_string_lossy().into_owned();
                }
                self.save_to(&p)
            }
            None => false,
        }
    }

    pub(crate) fn save_to(&mut self, path: &Path) -> bool {
        match self.project.save(path) {
            Ok(()) => {
                self.project_path = Some(path.to_path_buf());
                self.dirty = false;
                self.discard_autosave();
                self.prefs.add_recent(path);
                self.prefs.save();
                self.notify("Project saved.", false);
                true
            }
            Err(e) => {
                self.notify(&format!("Couldn't save: {e}"), true);
                false
            }
        }
    }

    /// Writes a backup copy of an unsaved project every few minutes.
    fn autosave(&mut self, ctx: &egui::Context) {
        let every = self.prefs.autosave_minutes;
        if every == 0 || !self.dirty {
            return;
        }
        let due = Duration::from_secs(every as u64 * 60);
        let since = self.last_autosave.elapsed();
        if since < due {
            // Come back when it's due even if nothing else happens.
            ctx.request_repaint_after(due - since);
            return;
        }
        self.last_autosave = Instant::now();
        let dir = prefs::autosave_dir();
        let _ = std::fs::create_dir_all(&dir);
        let name: String = self
            .project
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = dir.join(format!("{name} (autosave).{PROJECT_EXTENSION}"));
        match self.project.save(&path) {
            Ok(()) => self.autosave_written = Some(path),
            Err(e) => log::warn!("autosave failed: {e}"),
        }
    }

    /// Removes this session's backup: the work it protected is saved or was
    /// given up on purpose.
    fn discard_autosave(&mut self) {
        if let Some(p) = self.autosave_written.take() {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Opens a backup left by a crashed session as an unsaved project.
    fn restore_backup(&mut self, path: &Path) {
        match Project::load(path) {
            Ok(mut p) => {
                crate::sounds::restore_sounds(&mut p);
                self.reset(p, None);
                // It has to be saved to keep.
                self.dirty = true;
                let _ = std::fs::remove_file(path);
                self.recovery.retain(|(q, _)| q != path);
                self.notify("Your unsaved work is back. Save it to keep it.", false);
            }
            Err(e) => self.notify(&format!("Couldn't restore that backup: {e}"), true),
        }
    }

    /// At start: offers the work a crash left behind.
    /// Shows the welcome tour the first time, once nothing else is asking
    /// for attention.
    fn first_run_tour(&mut self) {
        if self.prefs.tour_done
            || !self.recovery.is_empty()
            || self.pending.is_some()
            || self.dialogs.is_modal_open()
        {
            return;
        }
        self.prefs.tour_done = true;
        self.prefs.save();
        self.dialogs.open_tour();
    }

    fn recovery_modal(&mut self, ctx: &egui::Context) {
        if self.recovery.is_empty() || self.pending.is_some() {
            return;
        }
        enum Choice {
            Restore(PathBuf),
            Delete,
            Later,
        }
        let mut choice = None;
        egui::Modal::new(Id::new("recovery")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Restore your unsaved work?");
            ui.add_space(4.0);
            ui.label("TidyCuts closed without saving last time. These backups were kept:");
            ui.add_space(10.0);
            for (path, time) in &self.recovery {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().replace(" (autosave)", ""))
                    .unwrap_or_default();
                let ago = time.elapsed().map(ago_label).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).strong());
                    ui.label(egui::RichText::new(ago).color(theme::TEXT_DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::accent_button("Restore")).clicked() {
                            choice = Some(Choice::Restore(path.clone()));
                        }
                    });
                });
                ui.add_space(4.0);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(theme::outline_button("Delete the backups"))
                    .clicked()
                {
                    choice = Some(Choice::Delete);
                }
                if ui.add(theme::outline_button("Not now")).clicked() {
                    choice = Some(Choice::Later);
                }
            });
        });
        match choice {
            Some(Choice::Restore(p)) => self.restore_backup(&p),
            Some(Choice::Delete) => {
                for (p, _) in self.recovery.drain(..) {
                    let _ = std::fs::remove_file(p);
                }
            }
            Some(Choice::Later) => self.recovery.clear(),
            None => {}
        }
    }

    // ----- export --------------------------------------------------------

    pub fn start_export(&mut self, settings: ExportSettings) {
        self.pause();
        let Some(engine) = &self.engine else {
            self.notify("Exporting needs a graphics device.", true);
            return;
        };
        let project = self.project.clone();
        let renderer = engine.renderer.clone();
        let analysis = engine.analysis.clone();
        let progress = Arc::new(Mutex::new(ExportProgress::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let s = settings.clone();
        let (p2, c2) = (progress.clone(), cancel.clone());
        std::thread::Builder::new()
            .name("tidycuts-export".into())
            .spawn(move || {
                let statics = tidycuts_engine::Statics::default();
                let auto = move |m: &MediaItem| analysis.auto_grade(m);
                let r = tidycuts_engine::export::export(
                    &project,
                    &s,
                    renderer,
                    &statics,
                    &auto,
                    &c2,
                    |p| {
                        *p2.lock().unwrap() = p;
                    },
                );
                let _ = tx.send(r);
            })
            .expect("spawn export thread");
        if let Some(dir) = settings.path.parent() {
            self.prefs.export_folder = Some(dir.to_path_buf());
            self.prefs.save();
        }
        self.export_job = Some(ExportJob {
            settings,
            progress,
            cancel,
            result: rx,
            started: Instant::now(),
            finished: None,
        });
    }

    fn poll_export(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.export_job else {
            return;
        };
        if job.finished.is_none() {
            if let Ok(r) = job.result.try_recv() {
                job.finished = Some((r, job.started.elapsed()));
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }

    // ----- feedback ------------------------------------------------------

    pub fn notify(&mut self, text: &str, error: bool) {
        let secs = if error { 6 } else { 3 };
        self.toasts.retain(|t| t.text != text);
        self.toasts.push(Toast {
            text: text.to_string(),
            error,
            until: Instant::now() + Duration::from_secs(secs),
        });
    }

    fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        let screen = ctx.content_rect();
        let mut y = screen.bottom() - 20.0;
        let painter =
            ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("toasts")));
        for t in self.toasts.iter().rev() {
            let galley = painter.layout(
                t.text.clone(),
                FontId::proportional(13.5),
                Color32::WHITE,
                440.0,
            );
            let size = galley.size() + egui::vec2(28.0, 18.0);
            let rect = Rect::from_min_size(
                egui::pos2(screen.center().x - size.x / 2.0, y - size.y),
                size,
            );
            painter.rect_filled(
                rect.translate(egui::vec2(0.0, 2.0)),
                CornerRadius::same(8),
                Color32::from_black_alpha(90),
            );
            painter.rect_filled(
                rect,
                CornerRadius::same(8),
                if t.error {
                    theme::DANGER
                } else {
                    theme::PANEL_BG_3
                },
            );
            painter.galley(rect.min + egui::vec2(14.0, 9.0), galley, Color32::WHITE);
            y -= size.y + 8.0;
        }
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    // ----- input ---------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let alt = Modifiers::ALT;
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let take = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if take(sc(cmd | shift, Key::S)) {
            self.save_as();
        }
        if take(sc(cmd, Key::S)) {
            self.save();
        }
        if take(sc(cmd, Key::O)) {
            self.request_open(None);
        }
        if take(sc(cmd, Key::N)) {
            self.request_new();
        }
        if take(sc(cmd, Key::I)) {
            self.import_dialog();
        }
        if take(sc(cmd, Key::E)) {
            self.dialogs
                .open_export(&self.project, &self.prefs, self.project_path.as_deref());
        }
        if take(sc(cmd | shift, Key::Comma)) {
            self.dialogs.open_project_settings(&self.project);
        }
        if take(sc(cmd, Key::Comma)) {
            self.dialogs.open_preferences(&self.prefs);
        }
        if take(sc(cmd | shift, Key::Z)) || take(sc(cmd, Key::Y)) {
            self.redo();
        }
        if take(sc(cmd, Key::Z)) {
            self.undo();
        }
        // Everything below edits clips or moves the playhead, so it waits
        // while a text field has focus.
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        if take(sc(cmd, Key::B)) {
            self.split_at_playhead();
        }
        if take(sc(cmd | alt, Key::D)) {
            self.detach_audio();
        }
        if take(sc(cmd | alt, Key::F)) {
            self.freeze_frame();
        }
        if take(sc(Modifiers::NONE, Key::M)) {
            self.add_marker_at_playhead();
        }
        if take(sc(alt, Key::ArrowLeft)) {
            self.jump_to_marker(false);
        }
        if take(sc(alt, Key::ArrowRight)) {
            self.jump_to_marker(true);
        }
        if take(sc(cmd, Key::D)) {
            self.duplicate_selection();
        }
        if take(sc(cmd, Key::C)) {
            self.copy_selection();
        }
        if take(sc(cmd, Key::X)) {
            self.cut_selection();
        }
        if take(sc(cmd, Key::V)) {
            self.paste();
        }
        if take(sc(cmd, Key::A)) {
            self.select_all();
        }
        if take(sc(cmd, Key::T)) {
            self.add_title("", None);
        }
        if take(sc(cmd, Key::R)) {
            if let Some(c) = self.selected_clip() {
                self.inspector.request_tab("Speed");
                self.selection = vec![c];
                self.open_inspector();
            }
        }
        if take(sc(alt, Key::OpenBracket)) {
            self.trim_to_playhead(Edge::Start);
        }
        if take(sc(alt, Key::CloseBracket)) {
            self.trim_to_playhead(Edge::End);
        }
        if take(sc(alt, Key::C)) {
            let visual = self.selected_clip().filter(|c| {
                self.project
                    .clip(*c)
                    .is_some_and(|(t, _)| t.kind == TrackKind::Video)
            });
            if let Some(c) = visual {
                self.dialogs.open_crop(c);
            }
        }
        if take(sc(shift, Key::Delete)) || take(sc(shift, Key::Backspace)) {
            self.delete_selection(true);
        }
        if take(sc(Modifiers::NONE, Key::Delete)) || take(sc(Modifiers::NONE, Key::Backspace)) {
            self.delete_selection(false);
        }
        if take(sc(Modifiers::NONE, Key::Space)) {
            self.toggle_play();
        }
        if take(sc(Modifiers::NONE, Key::ArrowLeft)) {
            self.step_frames(-1);
        }
        if take(sc(Modifiers::NONE, Key::ArrowRight)) {
            self.step_frames(1);
        }
        if take(sc(shift, Key::ArrowLeft)) {
            self.step_frames(-(self.project.settings.frame_rate.as_f64().round() as i64));
        }
        if take(sc(shift, Key::ArrowRight)) {
            self.step_frames(self.project.settings.frame_rate.as_f64().round() as i64);
        }
        if take(sc(Modifiers::NONE, Key::ArrowUp)) {
            self.jump_to_edit(false);
        }
        if take(sc(Modifiers::NONE, Key::ArrowDown)) {
            self.jump_to_edit(true);
        }
        if take(sc(Modifiers::NONE, Key::Home)) {
            self.pause();
            self.seek(Time::ZERO);
        }
        if take(sc(Modifiers::NONE, Key::End)) {
            self.pause();
            self.seek(self.project.duration());
        }
        if take(sc(Modifiers::NONE, Key::F)) || take(sc(alt, Key::Enter)) {
            self.preview.toggle_fullscreen(ctx);
        }
        if take(sc(Modifiers::NONE, Key::Escape)) {
            if self.preview.fullscreen {
                self.preview.toggle_fullscreen(ctx);
            } else {
                self.clear_selection();
            }
        }
        if take(sc(Modifiers::NONE, Key::N)) {
            self.timeline.snap = !self.timeline.snap;
            self.prefs.snapping = self.timeline.snap;
        }
        if take(sc(cmd, Key::Equals)) || take(sc(cmd, Key::Plus)) {
            self.timeline.zoom_by(1.5);
        }
        if take(sc(cmd, Key::Minus)) {
            self.timeline.zoom_by(1.0 / 1.5);
        }
    }

    fn dropped_files(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if files.is_empty() {
            return;
        }
        if let [one] = files.as_slice() {
            if is_project_file(one) {
                self.guard(Pending::Open(Some(one.clone())));
                return;
            }
        }
        self.import_paths(files);
    }

    // ----- layout --------------------------------------------------------

    fn top_bar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let cy = rect.center().y;
        // The logo: a violet tile with a white play triangle, then the name.
        let tile = Rect::from_min_size(
            egui::pos2(rect.left() + 14.0, cy - 13.0),
            egui::vec2(26.0, 26.0),
        );
        ui.painter()
            .rect_filled(tile, CornerRadius::same(7), theme::ACCENT);
        let c = tile.center() + egui::vec2(0.8, 0.0);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(c.x - 4.2, c.y - 5.8),
                egui::pos2(c.x - 4.2, c.y + 5.8),
                egui::pos2(c.x + 5.6, c.y),
            ],
            Color32::WHITE,
            egui::Stroke::NONE,
        ));
        let name =
            ui.painter()
                .layout_no_wrap("TidyCuts".into(), theme::bold_font(15.0), theme::TEXT);
        let name_right = tile.right() + 10.0 + name.size().x;
        ui.painter().galley(
            egui::pos2(tile.right() + 10.0, cy - name.size().y / 2.0),
            name,
            theme::TEXT,
        );

        // The menus, 20 pixels after the name.
        let menu_rect = Rect::from_min_size(
            egui::pos2(name_right + 20.0 + 1.5, rect.top()),
            egui::vec2(320.0, rect.height()),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(menu_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                ui.spacing_mut().item_spacing.x = 4.0;
                // The row is as tall as the buttons, so it centres in the bar.
                ui.spacing_mut().interact_size.y = 28.0;
                ui.visuals_mut().override_text_color = None;
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.fg_stroke = egui::Stroke::new(1.0, theme::TEXT_DIM);
                w.hovered.fg_stroke = egui::Stroke::new(1.0, theme::TEXT);
                w.open.fg_stroke = egui::Stroke::new(1.0, theme::TEXT);
                for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
                    s.corner_radius = CornerRadius::same(8);
                }
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.hovered.weak_bg_fill = theme::PANEL_BG_2;
                w.open.weak_bg_fill = theme::PANEL_BG_2;
                egui::MenuBar::new().ui(ui, |ui| {
                    // The menu bar's own style squeezes the buttons: give them room.
                    ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    crate::menus::menus(self, ui)
                });
            },
        );

        let title = format!(
            "{}{}",
            self.project.name,
            if self.dirty { "  •" } else { "" }
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            title,
            theme::medium_font(13.5),
            theme::TEXT_DIM,
        );

        // Right side, 20 pixels apart: settings, help, Export.
        let busy = self
            .export_job
            .as_ref()
            .is_some_and(|j| j.finished.is_none());
        let label = match &self.export_job {
            Some(j) if busy => {
                let f = crate::dialogs::job_fraction(j, &self.project);
                format!("Exporting {:.0}%", f * 100.0)
            }
            _ => "Export".to_string(),
        };
        let text_w = ui
            .painter()
            .layout_no_wrap(label.clone(), theme::strong_font(13.5), Color32::WHITE)
            .size()
            .x;
        // padding 16 + icon 15 + gap 8 + text + padding 16
        let w = (16.0 + 15.0 + 8.0 + text_w + 16.0).round();
        let export = Rect::from_min_size(
            egui::pos2(rect.right() - 14.0 - w, cy - 17.0),
            egui::vec2(w, 34.0),
        );
        let resp = ui.interact(export, Id::new("export-button"), Sense::click());
        let fill = if resp.hovered() {
            theme::ACCENT.lerp_to_gamma(Color32::WHITE, 0.1)
        } else {
            theme::ACCENT
        };
        ui.painter()
            .rect_filled(export, CornerRadius::same(6), fill);
        ui.painter().text(
            egui::pos2(export.left() + 16.0 + 7.5, cy),
            Align2::CENTER_CENTER,
            ph::EXPORT,
            FontId::proportional(15.0),
            Color32::WHITE,
        );
        ui.painter().text(
            egui::pos2(export.left() + 16.0 + 15.0 + 8.0, cy),
            Align2::LEFT_CENTER,
            label,
            theme::strong_font(13.5),
            Color32::WHITE,
        );
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Export the video (Ctrl+E)")
            .clicked()
        {
            if busy {
                self.dialogs.show_export_progress();
            } else {
                self.dialogs
                    .open_export(&self.project, &self.prefs, self.project_path.as_deref());
            }
        }
        let mut x = export.left() - 20.0;
        for (icon, tip, key) in [
            (ph::QUESTION, "About TidyCuts", "about"),
            (ph::GEAR, "Preferences", "prefs"),
        ] {
            let r = Rect::from_center_size(egui::pos2(x - 16.0, cy), egui::vec2(32.0, 32.0));
            x = r.left() - 20.0;
            let resp = ui.interact(r, Id::new(("top-icon", key)), Sense::click());
            if resp.hovered() {
                ui.painter()
                    .rect_filled(r, CornerRadius::same(9), theme::PANEL_BG_2);
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(17.0),
                if resp.hovered() {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                },
            );
            if resp.on_hover_text(tip).clicked() {
                if key == "prefs" {
                    self.dialogs.open_preferences(&self.prefs);
                } else {
                    self.dialogs.open_about();
                }
            }
        }
    }

    /// A draggable gap between panels. Returns where the pointer is while it
    /// is being dragged.
    fn splitter(ui: &mut egui::Ui, id: &str, rect: Rect, vertical: bool) -> Option<egui::Pos2> {
        let resp = ui.interact(rect, Id::new(id), Sense::drag());
        let cursor = if vertical {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        };
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(cursor);
            let line = if vertical {
                Rect::from_center_size(rect.center(), egui::vec2(2.0, rect.height()))
            } else {
                Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 2.0))
            };
            ui.painter()
                .rect_filled(line, CornerRadius::ZERO, theme::ACCENT);
        }
        if resp.dragged() {
            resp.interact_pointer_pos()
        } else {
            None
        }
    }

    /// What the properties panel says when no clip is selected.
    fn nothing_selected(ui: &mut egui::Ui) {
        let r = ui.max_rect();
        let c = r.center() - egui::vec2(0.0, 24.0);
        ui.painter().text(
            c - egui::vec2(0.0, 30.0),
            Align2::CENTER_CENTER,
            ph::SLIDERS_HORIZONTAL,
            FontId::proportional(34.0),
            theme::TEXT_FAINT,
        );
        ui.painter().text(
            c + egui::vec2(0.0, 8.0),
            Align2::CENTER_CENTER,
            "Nothing selected",
            theme::heading_font(15.0),
            theme::TEXT_DIM,
        );
        let job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: r.width() - 56.0,
                ..Default::default()
            },
            halign: egui::Align::Center,
            ..egui::text::LayoutJob::simple(
                "Click a clip on the timeline to change its size, colour, speed, sound and more."
                    .into(),
                FontId::proportional(12.5),
                theme::TEXT_FAINT,
                r.width() - 56.0,
            )
        };
        // The job is centred on the point it is painted at.
        let galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(egui::pos2(c.x, c.y + 30.0), galley, theme::TEXT_FAINT);
    }

    fn card(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui)) {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme::PANEL_BG);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect);
        add(&mut child);
    }

    fn unsaved_modal(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else {
            return;
        };
        let mut choice = None;
        egui::Modal::new(Id::new("unsaved")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Save changes?");
            ui.add_space(4.0);
            ui.label(format!(
                "\"{}\" has changes that aren't saved.",
                self.project.name
            ));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.add(theme::accent_button("Save")).clicked() {
                    choice = Some(0);
                }
                if ui.add(theme::outline_button("Don't save")).clicked() {
                    choice = Some(1);
                }
                if ui.add(theme::outline_button("Cancel")).clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                if self.save() {
                    self.run_pending(action.clone());
                    if matches!(action, Pending::Close) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            Some(1) => {
                self.pending = None;
                self.dirty = false;
                self.run_pending(action.clone());
                if matches!(action, Pending::Close) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }

    fn banner(ui: &mut egui::Ui, rect: Rect, text: &str) {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), theme::DANGER);
        ui.painter().text(
            rect.left_center() + egui::vec2(12.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(13.0),
            Color32::WHITE,
        );
    }

    /// Renders the preview picture for this frame.
    fn tick_engine(&mut self, ctx: &egui::Context) {
        let size = self.preview.render_size;
        let Some(engine) = &mut self.engine else {
            return;
        };
        let _prof = profile_span("engine");
        let playing = engine.is_playing();
        if playing {
            if self.preview.mode == PreviewMode::Timeline {
                let _p = profile_span("sync");
                engine.sync_playback(&self.project, self.revision);
            }
            engine.tick_playing(size);
            let pos = engine.position();
            match self.preview.mode {
                PreviewMode::Timeline => self.timeline.playhead = pos.min(self.project.duration()),
                PreviewMode::Source => self.preview.source_time = pos,
            }
            ctx.request_repaint();
            return;
        }
        if self.was_playing {
            // Reached the end by itself.
            self.was_playing = false;
            if self.preview.mode == PreviewMode::Timeline {
                self.timeline.playhead = self.project.duration();
            }
        }
        match self.preview.mode {
            PreviewMode::Timeline => {
                // Skimming: the picture under the mouse, not the playhead's.
                let skim = self.timeline.skim;
                let at = skim.unwrap_or(self.timeline.playhead);
                engine.show_still(
                    &self.project,
                    self.revision,
                    None,
                    at,
                    size,
                    self.timeline.scrubbing || skim.is_some(),
                );
                profile_event(&format!(
                    "still at {:.2} exact_pending={} source={}",
                    at.as_secs_f64(),
                    engine.pending_exact(),
                    tidycuts_engine::frames::LAST_SOURCE.load(std::sync::atomic::Ordering::Relaxed)
                ));
                if skim.is_some() {
                    // The picture drawn now is first painted next frame.
                    ctx.request_repaint_after(Duration::from_millis(30));
                }
            }
            PreviewMode::Source => {
                let Some(m) = self
                    .preview
                    .source_media
                    .and_then(|id| self.project.media(id).or(self.sounds.media_by_id(id)))
                else {
                    return;
                };
                let p = engine::source_project(m, &self.project.settings);
                let at = self.preview.source_time.min(m.duration);
                engine.show_still(
                    &p,
                    source_revision(m.id),
                    Some(m.id),
                    at,
                    size,
                    self.preview.scrubbing,
                );
            }
        }
    }
}

/// Frame timing for finding stutter: when `TIDYCUTS_PROFILE` names a file,
/// one line per window frame is appended (time since the last frame, time
/// spent in it, and in the picture step).
pub(crate) struct ProfileSpan(&'static str, Instant);

type ProfileLog = Option<(Instant, Vec<(&'static str, f64)>)>;

static PROFILE: std::sync::Mutex<ProfileLog> = std::sync::Mutex::new(None);

fn profile_on() -> Option<std::path::PathBuf> {
    static PATH: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("TIDYCUTS_PROFILE").map(std::path::PathBuf::from))
        .clone()
}

fn profile_frame(scrubbing: bool, playing: bool) -> Option<ProfileFrame> {
    profile_on()?;
    Some(ProfileFrame {
        start: Instant::now(),
        scrubbing,
        playing,
    })
}

struct ProfileFrame {
    start: Instant,
    scrubbing: bool,
    playing: bool,
}

impl Drop for ProfileFrame {
    fn drop(&mut self) {
        use std::io::Write;
        let Some(path) = profile_on() else { return };
        let mut g = PROFILE.lock().unwrap();
        let (last, spans) = g.get_or_insert((self.start, Vec::new()));
        let gap = self.start.duration_since(*last).as_secs_f64() * 1000.0;
        *last = self.start;
        let total = self.start.elapsed().as_secs_f64() * 1000.0;
        let parts: Vec<String> = spans
            .drain(..)
            .map(|(n, ms)| format!("{n}={ms:.1}"))
            .collect();
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(
                f,
                "gap={gap:.1} frame={total:.1} scrub={} play={} {}",
                self.scrubbing as u8,
                self.playing as u8,
                parts.join(" ")
            );
        }
    }
}

/// Playing frames still to be logged after a jump (timing log only).
pub(crate) static FRAMES_TO_LOG: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

/// One line in the timing log for something that happened (a click, a picture).
pub(crate) fn profile_event(text: &str) {
    use std::io::Write;
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let Some(path) = profile_on() else { return };
    let ms = START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "EV {ms:.0} {text}");
    }
}

pub(crate) fn profile_span(name: &'static str) -> Option<ProfileSpan> {
    profile_on()?;
    Some(ProfileSpan(name, Instant::now()))
}

impl Drop for ProfileSpan {
    fn drop(&mut self) {
        let ms = self.1.elapsed().as_secs_f64() * 1000.0;
        if let Some((_, spans)) = PROFILE.lock().unwrap().as_mut() {
            spans.push((self.0, ms));
        }
    }
}

/// A revision number for a source preview that never equals a timeline one.
fn source_revision(id: MediaId) -> u64 {
    (1u64 << 63) | (id.as_u128() as u64 & !(1u64 << 63))
}

/// Image loaders for library thumbnails are not needed; kept as a hook.
fn egui_extras_install(_ctx: &egui::Context) {}

/// The smallest window that is laid out; anything smaller is a minimized
/// window (the real minimum, in `main.rs`, is larger).
const MIN_DRAW: (f32, f32) = (480.0, 320.0);

impl eframe::App for TidyCutsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let _prof = profile_frame(self.timeline.scrubbing, self.is_playing());

        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }

        self.poll_imports();
        self.poll_sounds();
        self.poll_stills();
        self.poll_mcp(&ctx);
        self.poll_export(&ctx);
        self.thumbs.poll(&ctx);
        self.dropped_files(&ctx);
        if self.pending.is_none() && !self.dialogs.is_modal_open() {
            self.shortcuts(&ctx);
        }

        let full = ui.max_rect();
        ui.painter()
            .rect_filled(full, CornerRadius::ZERO, theme::WINDOW_BG);

        // A minimized window is reported as (nearly) zero in size. Nothing
        // can be laid out in that, and the panels' maths would go negative,
        // so draw nothing until the window is back; polling above carries on.
        if full.width() < MIN_DRAW.0 || full.height() < MIN_DRAW.1 {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }

        if self.preview.fullscreen {
            preview::show_fullscreen(self, ui, full);
        } else {
            let bar = Rect::from_min_size(full.min, egui::vec2(full.width(), BAR_H));
            ui.painter()
                .rect_filled(bar, CornerRadius::ZERO, theme::BAR_BG);
            ui.painter().hline(
                bar.x_range(),
                bar.bottom() - 0.5,
                egui::Stroke::new(1.0, theme::BORDER),
            );
            self.top_bar(ui, bar);

            let mut body = Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
            for err in [self.ffmpeg_error.clone(), self.gpu_error.clone()]
                .into_iter()
                .flatten()
            {
                let banner = Rect::from_min_size(body.min, egui::vec2(body.width(), 30.0));
                Self::banner(ui, banner, &err);
                body.min.y += 30.0;
            }

            // The panels across the top and the timeline below; they touch and
            // are told apart by thin lines. Where each sits depends on the
            // layout preset.
            let preset = self.prefs.layout_preset;
            let [split_x, split_y] = self.prefs.fractions();
            let top_h = (body.height() * split_y).round();
            let top = Rect::from_min_size(body.min, egui::vec2(body.width(), top_h));
            let show_dock = preset.has_dock() && self.inspector_open;
            let dock_max = (body.width() * 0.45).max(DOCK_W);
            let dock_w = if show_dock {
                self.prefs.dock_w.clamp(DOCK_W, dock_max)
            } else {
                0.0
            };
            let span = |a: f32, b: f32| {
                Rect::from_min_max(
                    egui::pos2(top.left() + a, top.top()),
                    egui::pos2(top.left() + b, top.bottom()),
                )
            };
            let w = top.width();
            let (lib_rect, prev_rect, dock_rect) = match preset {
                crate::prefs::LayoutPreset::Standard => {
                    let rest = w - dock_w;
                    let lw = (rest * split_x).round();
                    (
                        span(0.0, lw),
                        span(lw, rest),
                        show_dock.then(|| span(rest, w)),
                    )
                }
                crate::prefs::LayoutPreset::Mirrored => {
                    let rest = w - dock_w;
                    let lw = (rest * split_x).round();
                    (
                        span(w - lw, w),
                        span(dock_w, w - lw),
                        show_dock.then(|| span(0.0, dock_w)),
                    )
                }
                crate::prefs::LayoutPreset::LeftMenus => {
                    let lw = (w * split_x).round();
                    (span(0.0, lw), span(lw, w), None)
                }
            };
            let tl_rect = Rect::from_min_max(egui::pos2(body.left(), top.bottom()), body.max);

            Self::card(ui, lib_rect, |ui| library::show(self, ui));
            Self::card(ui, prev_rect, |ui| preview::show(self, ui));
            if let Some(dock) = dock_rect {
                Self::card(ui, dock, |ui| {
                    if self.selected_clip().is_some() {
                        inspector::show(self, ui);
                    } else {
                        Self::nothing_selected(ui);
                    }
                });
            }
            Self::card(ui, tl_rect, |ui| timeline::show(self, ui));

            // The lines between panels, then the grab handles on top.
            let line = egui::Stroke::new(1.0, theme::BORDER);
            let p = ui.painter();
            p.hline(body.x_range(), tl_rect.top() + 0.5, line);
            let lib_left_of_preview = lib_rect.left() < prev_rect.left();
            let lib_edge = if lib_left_of_preview {
                lib_rect.right()
            } else {
                lib_rect.left()
            };
            p.vline(lib_edge - 0.5, body.top()..=lib_rect.bottom(), line);
            let dock_edge = dock_rect.map(|d| {
                if d.left() > prev_rect.left() {
                    d.left()
                } else {
                    d.right()
                }
            });
            if let Some(x) = dock_edge {
                p.vline(x - 0.5, body.top()..=lib_rect.bottom(), line);
            }
            let grab = 4.0;
            let vgrab = |x: f32| {
                Rect::from_min_max(
                    egui::pos2(x - grab, body.top()),
                    egui::pos2(x + grab, lib_rect.bottom()),
                )
            };
            let hsplit = Rect::from_min_max(
                egui::pos2(body.left(), tl_rect.top() - grab),
                egui::pos2(body.right(), tl_rect.top() + grab),
            );
            let mut fractions = [split_x, split_y];
            let mut new_dock_w = self.prefs.dock_w;
            if let Some(pos) = Self::splitter(ui, "split-x", vgrab(lib_edge), true) {
                let rest = (w - dock_w).max(1.0);
                fractions[0] = match preset {
                    crate::prefs::LayoutPreset::Standard => (pos.x - top.left()) / rest,
                    crate::prefs::LayoutPreset::Mirrored => (top.right() - pos.x) / rest,
                    crate::prefs::LayoutPreset::LeftMenus => (pos.x - top.left()) / w,
                }
                .clamp(0.2, 0.8);
            }
            if let (Some(x), Some(d)) = (dock_edge, dock_rect) {
                if let Some(pos) = Self::splitter(ui, "split-dock", vgrab(x), true) {
                    new_dock_w = if d.left() > prev_rect.left() {
                        top.right() - pos.x
                    } else {
                        pos.x - top.left()
                    }
                    .clamp(DOCK_W, dock_max);
                }
            }
            if let Some(pos) = Self::splitter(ui, "split-y", hsplit, false) {
                fractions[1] = ((pos.y - body.top()) / body.height()).clamp(0.25, 0.75);
            }
            let changed = fractions != [split_x, split_y] || new_dock_w != self.prefs.dock_w;
            if changed {
                self.prefs.set_fractions(fractions);
                self.prefs.dock_w = new_dock_w;
                if !ctx.input(|i| i.pointer.any_down()) {
                    self.prefs.save();
                }
            }
            if ctx.input(|i| i.pointer.any_released()) && self.layout_dragged {
                self.prefs.save();
            }
            self.layout_dragged = ui.ctx().input(|i| i.pointer.any_down()) && changed;
        }

        self.unsaved_modal(&ctx);
        self.recovery_modal(&ctx);
        self.first_run_tour();
        dialogs::show(self, &ctx);
        self.tick_engine(&ctx);
        self.draw_toasts(&ctx);
        self.autosave(&ctx);
        self.poll_detections();
        self.poll_auto_duck();
        self.poll_syncs();
        self.poll_proxies();
        self.poll_previews();
        self.poll_voiceovers();
        crate::mixer::show(self, &ctx);
        crate::scopes::show(self, &ctx);
        if self.importing > 0 {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self) {
        if let Some(job) = &self.export_job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        // The picture-drawing program may still be holding the graphics card.
        tidycuts_engine::imagegen_service::shutdown();
        // A normal exit: nothing needs recovering.
        self.discard_autosave();
        self.prefs.save();
    }
}

/// "3 minutes ago", for a backup's age.
fn ago_label(age: Duration) -> String {
    let (n, unit) = match age.as_secs() {
        s if s < 90 => return "just now".into(),
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86_400 => (s / 3600, "hour"),
        s => (s / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_ages_read_plainly() {
        assert_eq!(ago_label(Duration::from_secs(20)), "just now");
        assert_eq!(ago_label(Duration::from_secs(5 * 60)), "5 minutes ago");
        assert_eq!(ago_label(Duration::from_secs(3600)), "1 hour ago");
        assert_eq!(ago_label(Duration::from_secs(3 * 86_400)), "3 days ago");
    }

    #[test]
    fn source_revisions_never_collide_with_timeline_ones() {
        let id = uuid::Uuid::new_v4();
        assert!(source_revision(id) >= 1 << 63);
    }
}
