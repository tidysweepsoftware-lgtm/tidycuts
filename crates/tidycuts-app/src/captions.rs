//! Automatic captions on the timeline: the captions found in a clip's sound
//! become title clips, on a track of their own, as one undo step.

use crate::app::TidyCutsApp;
use tidycuts_core::{Clip, ClipAnimation, ClipId, Generator, MediaKind, Time, TrackKind};
use tidycuts_engine::captions::Caption;
use tidycuts_render::catalog::titles;

/// The caption looks: the subtitle presets of the titles catalog.
pub fn styles() -> Vec<(&'static str, &'static str)> {
    titles::TITLES
        .iter()
        .filter(|t| t.category == "Subtitles")
        .map(|t| (t.id, t.name))
        .collect()
}

/// Why a clip can't be captioned, in plain words.
pub fn unsuitable(app: &TidyCutsApp, clip: ClipId) -> Option<&'static str> {
    let Some((track, c)) = app.project.clip(clip) else {
        return Some("Select a clip with speech first.");
    };
    let sound = c
        .media_id
        .and_then(|m| app.project.media(m))
        .is_some_and(|m| m.kind == MediaKind::Audio || m.has_audio);
    if !sound || c.generator.is_some() {
        return Some("Select a video or audio clip with speech first.");
    }
    if track.locked {
        return Some("That clip's track is locked.");
    }
    if c.reverse || c.ramp.is_some() {
        return Some("Captions need the clip to play forwards at a steady speed.");
    }
    None
}

impl TidyCutsApp {
    /// Opens the Auto Captions window for the selected clip, or says what is
    /// needed first.
    pub fn open_auto_captions(&mut self) {
        match self.selected_clip() {
            Some(id) => match unsuitable(self, id) {
                None => self.dialogs.open_captions(id),
                Some(why) => self.notify(why, true),
            },
            None => self.notify("Select a video or audio clip with speech first.", true),
        }
    }

    /// Puts `captions` (times in the clip's source, in seconds) on the
    /// timeline as titles in the look `preset`. Returns how many were added.
    pub fn add_captions(
        &mut self,
        clip: ClipId,
        captions: &[Caption],
        preset: &str,
    ) -> Option<usize> {
        let (_, c) = self.project.clip(clip)?;
        let (start, speed) = (c.start.as_secs_f64(), c.speed.max(0.01));
        let (src_in, src_out) = (c.source_in.as_secs_f64(), c.source_out.as_secs_f64());
        let preset = titles::get(preset)
            .or_else(|| titles::get("subtitle.classic"))
            .or_else(|| titles::TITLES.first())?;
        let k = self.project.settings.height as f64 / 1080.0;
        let before = self.project.clone();
        let track = match self.project.add_track(TrackKind::Video) {
            Some(t) => t,
            None => {
                let last = captions.last()?;
                let len = Time::from_secs_f64(last.end);
                self.project
                    .overlay_track_for(Time::from_secs_f64(start), len)?
            }
        };
        let mut added = 0;
        for cap in captions {
            // Only what is inside the part of the file the clip shows.
            if cap.end <= src_in || cap.start >= src_out {
                continue;
            }
            let s = (cap.start.max(src_in) - src_in) / speed + start;
            let e = (cap.end.min(src_out) - src_in) / speed + start;
            if e - s < 0.1 {
                continue;
            }
            let mut spec = (preset.make)();
            spec.text = cap.text.clone();
            let mut title = Clip::generated(
                Generator::Title(spec),
                Time::from_secs_f64(s),
                Time::from_secs_f64(e - s),
            );
            title.transform.position_x.value = preset.position[0] as f64 * k;
            title.transform.position_y.value = preset.position[1] as f64 * k;
            // Captions pop on and off, so no long fades.
            title.anim_in = preset.anim_in.map(|(kind, _)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(0.12),
            });
            title.anim_out = preset.anim_out.map(|(kind, _)| ClipAnimation {
                kind: kind.into(),
                duration: Time::from_secs_f64(0.12),
            });
            if self
                .project
                .place_clip(title, track, Time::from_secs_f64(s))
                .is_some()
            {
                added += 1;
            }
        }
        if added == 0 {
            self.project = before;
            return Some(0);
        }
        self.record(before);
        Some(added)
    }
}
