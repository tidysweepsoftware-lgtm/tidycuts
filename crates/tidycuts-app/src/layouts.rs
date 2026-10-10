//! Split-screen layouts and audio visualizers: adding them to the timeline
//! and dropping media into empty cells.

use crate::app::TidyCutsApp;
use tidycuts_core::{ClipId, Generator, MediaId, Time, VisualizerSpec, VISUALIZER_STYLES};

impl TidyCutsApp {
    /// Lays out the cells of a split-screen layout from `at` (the playhead
    /// when `None`): one empty cell per video track, as long as the
    /// project's end (or the usual picture length on an empty timeline).
    pub fn add_layout(&mut self, id: &str, at: Option<Time>) {
        let Some(def) = tidycuts_core::layout(id) else {
            return;
        };
        let at = at.unwrap_or_else(|| self.playhead());
        let len = match self.project.duration() {
            end if end > at + Time::from_secs_f64(1.0) => end - at,
            _ => Time::from_secs_f64(self.prefs.image_duration.max(1.0)),
        };
        let before = self.project.clone();
        let ids = self.project.add_layout(&def.cells(), at, len);
        if ids.is_empty() {
            self.project = before;
            self.notify(
                &format!(
                    "\u{201C}{}\u{201D} needs {} free video tracks there (a project can have up to {}).",
                    def.name,
                    def.cells().len(),
                    tidycuts_core::MAX_VIDEO_TRACKS
                ),
                true,
            );
            return;
        }
        self.record(before);
        self.selection = ids;
        self.notify("Drop media onto each empty cell to fill it.", false);
    }

    /// Adds an audio visualizer of a style from [`VISUALIZER_STYLES`].
    pub fn add_visualizer(&mut self, style: &str, at: Option<(tidycuts_core::TrackId, Time)>) {
        let Some((id, _)) = VISUALIZER_STYLES.iter().find(|(id, _)| *id == style) else {
            return;
        };
        let spec = VisualizerSpec {
            style: (*id).into(),
            ..Default::default()
        };
        let len = match self.project.duration() {
            end if end > Time::from_secs_f64(2.0) => end,
            _ => Time::from_secs_f64(10.0),
        };
        let start = self.playhead();
        let len = if at.is_none() && start < len {
            (len - start).max(Time::from_secs_f64(2.0))
        } else {
            len
        };
        self.add_generated(Generator::Visualizer(spec), len, at, |_| {});
    }

    /// Puts media into an empty cell.
    pub fn fill_cell(&mut self, clip: ClipId, media: MediaId) {
        if self.edit(|p| p.fill_cell(clip, media)) {
            self.selection = vec![clip];
        } else {
            self.notify(
                "That media can't go in this cell (is the track locked, or is it only sound?).",
                true,
            );
        }
    }
}
