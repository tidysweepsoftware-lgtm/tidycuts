//! Auto Reframe: fits a wide video to a tall or square project by cropping to
//! the part that matters and following it with keyframes.

use crate::app::TidyCutsApp;
use std::sync::atomic::AtomicBool;
use tidycuts_core::{ClipId, MediaKind, Time};

/// The answer of a finished look at a clip: where the crop should be and when.
pub struct ReframeDone {
    pub clip: ClipId,
    pub result: Result<Vec<(f64, f32)>, String>,
}

/// A keyframe about every half second is plenty for a calm pan.
const KEY_EVERY: usize = 2;

impl TidyCutsApp {
    /// Starts looking at a clip to work out how to crop it to the project's shape.
    pub fn start_reframe(&mut self, id: ClipId) {
        let Some((_, clip)) = self.project.clip(id) else {
            return;
        };
        let Some(media) = clip.media_id.and_then(|m| self.project.media(m)).cloned() else {
            return;
        };
        if media.kind != MediaKind::Video || media.width == 0 || media.height == 0 {
            self.notify("Auto Reframe works on videos.", true);
            return;
        }
        let (cw, ch) = (
            self.project.settings.width.max(1) as f32,
            self.project.settings.height.max(1) as f32,
        );
        let aspect = cw / ch;
        let video_aspect = media.width as f32 / media.height as f32;
        if aspect >= video_aspect - 0.02 {
            self.notify(
                "This video is not wider than the project, so there is nothing to crop. Choose a vertical or square project first (File > Project Settings).",
                true,
            );
            return;
        }
        let from = clip.source_in.as_secs_f64();
        let len = (clip.source_out - clip.source_in).as_secs_f64().max(0.1);
        let tx = self.reframe_tx.clone();
        let ctx = self.ctx.clone();
        self.notify("Looking at the video to find what to follow…", false);
        std::thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            let result = tidycuts_engine::reframe::analyse(&media, from, len, aspect, &cancel)
                .ok_or_else(|| "Couldn't read the video to reframe it.".to_string());
            let _ = tx.send(ReframeDone { clip: id, result });
            ctx.request_repaint();
        });
    }

    /// Applies the finished looks: a fill-the-screen zoom and a following pan.
    pub fn poll_reframes(&mut self) {
        while let Ok(done) = self.reframe_rx.try_recv() {
            match done.result {
                Err(e) => self.notify(&e, true),
                Ok(path) => self.apply_reframe(done.clip, &path),
            }
        }
    }

    fn apply_reframe(&mut self, id: ClipId, path: &[(f64, f32)]) {
        let Some((_, clip)) = self.project.clip(id) else {
            return;
        };
        let Some(m) = clip.media_id.and_then(|m| self.project.media(m)) else {
            return;
        };
        let (cw, ch) = (
            f64::from(self.project.settings.width.max(1)),
            f64::from(self.project.settings.height.max(1)),
        );
        let (mw, mh) = (f64::from(m.width.max(1)), f64::from(m.height.max(1)));
        // The picture is first fitted inside the frame; this scale then makes it fill it.
        let fitted = (cw / mw).min(ch / mh);
        let fill = (cw / (mw * fitted)).max(ch / (mh * fitted));
        let shown_w = mw * fitted * fill;
        let max_shift = ((shown_w - cw) / 2.0).max(0.0);
        let keys: Vec<(f64, f64)> = path
            .iter()
            .step_by(KEY_EVERY)
            .map(|(src, c)| {
                let local = self
                    .project
                    .clip(id)
                    .map_or(0.0, |(_, c)| c.local_secs(*src));
                let shift = ((0.5 - f64::from(*c)) * shown_w).clamp(-max_shift, max_shift);
                (local.max(0.0), shift.round())
            })
            .collect();
        let pct = (fill * 100.0).round();
        let done = self.edit(|p| {
            let Some(c) = p.clip_mut(id) else {
                return false;
            };
            c.transform.scale_x = tidycuts_core::Animatable::constant(pct);
            c.transform.scale_y = tidycuts_core::Animatable::constant(pct);
            c.transform.position_y = tidycuts_core::Animatable::constant(0.0);
            c.transform.position_x = tidycuts_core::Animatable::constant(0.0);
            for (t, x) in &keys {
                c.transform
                    .position_x
                    .set_keyframe(Time::from_secs_f64(*t), *x);
            }
            true
        });
        if done {
            self.notify(
                "Reframed. The picture now fills the screen and follows what moves. Undo (Ctrl+Z) puts it back.",
                false,
            );
        }
    }
}
