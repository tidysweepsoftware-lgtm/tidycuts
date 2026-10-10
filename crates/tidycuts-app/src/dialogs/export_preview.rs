//! The picture at the top of the Export window: a real frame of the
//! timeline, drawn the way the export will draw it. It does not borrow the
//! live player's picture, which is black wherever nothing is showing (the
//! playhead past the last clip, say).

use crate::app::TidyCutsApp;
use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use tidycuts_core::{MediaItem, Project, Time, TrackKind};

/// Width of the picture drawn for the window, in pixels (shown at about half
/// that, so it is sharp on high-density screens).
const WIDTH: u32 = 352;

enum State {
    Idle,
    Loading(Receiver<Option<ColorImage>>),
    Ready(TextureHandle),
    Failed,
}

pub struct ExportPreview {
    state: State,
    /// The moment the picture was asked for.
    at: Option<Time>,
}

/// A copy starts without a picture and draws its own.
impl Clone for ExportPreview {
    fn clone(&self) -> Self {
        ExportPreview::default()
    }
}

impl Default for ExportPreview {
    fn default() -> Self {
        ExportPreview {
            state: State::Idle,
            at: None,
        }
    }
}

/// A moment worth showing: the playhead when something is on screen there,
/// otherwise the middle of the first picture on the timeline.
pub fn good_moment(project: &Project, playhead: Time) -> Time {
    let end = project.duration();
    if end == Time::ZERO {
        return Time::ZERO;
    }
    let last = end - Time::from_millis(1);
    let visual = || {
        project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video && !t.hidden)
            .flat_map(|t| t.clips.iter())
    };
    if visual().any(|c| c.contains(playhead)) {
        return playhead.min(last);
    }
    match visual().min_by_key(|c| c.start) {
        Some(c) => Time(c.start.0 + c.duration().0 / 2).min(last),
        None => playhead.min(last),
    }
}

impl ExportPreview {
    /// Asks for the frame at `at` (once per moment) and draws whatever is
    /// ready into `rect`. Returns false when there is nothing to draw, so
    /// the caller can fall back to something else.
    pub fn paint(
        &mut self,
        ctx: &Context,
        app: &TidyCutsApp,
        at: Time,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) -> bool {
        if self.at != Some(at) {
            self.at = Some(at);
            self.start(app, at);
        }
        if let State::Loading(rx) = &self.state {
            match rx.try_recv() {
                Ok(Some(image)) => {
                    let tex = ctx.load_texture("export-preview", image, TextureOptions::LINEAR);
                    self.state = State::Ready(tex);
                }
                Ok(None) | Err(TryRecvError::Disconnected) => self.state = State::Failed,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(60))
                }
            }
        }
        match &self.state {
            State::Ready(tex) => {
                painter.image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                true
            }
            _ => false,
        }
    }

    fn start(&mut self, app: &TidyCutsApp, at: Time) {
        let Some(engine) = &app.engine else {
            self.state = State::Failed;
            return;
        };
        let project = app.project.clone();
        let renderer = engine.renderer.clone();
        let analysis = engine.analysis.clone();
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("tidycuts-export-preview".into())
            .spawn(move || {
                let statics = tidycuts_engine::Statics::default();
                let auto = move |m: &MediaItem| analysis.auto_grade(m);
                let pic = tidycuts_engine::snapshot::render(
                    &project,
                    at,
                    Some(WIDTH),
                    renderer,
                    &statics,
                    &auto,
                );
                let size = [pic.width as usize, pic.height as usize];
                let image = (pic.rgba.len() >= size[0] * size[1] * 4)
                    .then(|| ColorImage::from_rgba_unmultiplied(size, &pic.rgba));
                let _ = tx.send(image);
            });
        self.state = match spawned {
            Ok(_) => State::Loading(rx),
            Err(_) => State::Failed,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_core::{Clip, Generator, TrackKind};

    fn project_with_clip_at(start_secs: f64) -> Project {
        let mut p = Project::new("t");
        let t = p.add_track(TrackKind::Video).unwrap();
        let clip = Clip::generated(
            Generator::Solid([1.0, 0.0, 0.0, 1.0]),
            Time::from_secs_f64(start_secs),
            Time::from_secs_f64(4.0),
        );
        p.place_clip(clip, t, Time::from_secs_f64(start_secs));
        p
    }

    #[test]
    fn the_playhead_is_used_when_a_picture_is_there() {
        let p = project_with_clip_at(0.0);
        assert_eq!(
            good_moment(&p, Time::from_secs_f64(1.5)),
            Time::from_secs_f64(1.5)
        );
    }

    #[test]
    fn past_the_end_the_middle_of_the_first_picture_is_used() {
        let p = project_with_clip_at(0.0);
        let at = good_moment(&p, Time::from_secs_f64(30.0));
        assert!(at > Time::ZERO && at < p.duration(), "{at:?}");
    }

    #[test]
    fn an_empty_timeline_shows_the_start() {
        let p = Project::new("t");
        assert_eq!(good_moment(&p, Time::from_secs_f64(3.0)), Time::ZERO);
    }
}
