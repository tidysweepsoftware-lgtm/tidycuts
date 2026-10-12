//! Auto-duck: lowers a music clip wherever someone is talking. The search for
//! speech runs on a worker thread (it mixes the rest of the timeline and
//! listens to it); the answer becomes volume points on the music clip, as one
//! undo step, and only if the clip is still where it was.

use crate::app::TidyCutsApp;
use std::sync::atomic::AtomicBool;
use tidycuts_core::{Animatable, ClipId, Easing, Keyframe, Time};
use tidycuts_engine::autoduck;

/// How quickly the music goes down before a voice and comes back after it.
const ATTACK: f64 = 0.35;
const RELEASE: f64 = 0.9;
/// The default dip, in decibels.
pub const LOWER_DB: f64 = 12.0;

pub struct Finished {
    clip: ClipId,
    start: Time,
    duration: Time,
    lower_db: f64,
    result: Result<Vec<(f64, f64)>, String>,
}

/// Why a clip can't be ducked, in plain words.
pub fn unsuitable(app: &TidyCutsApp, clip: ClipId) -> Option<&'static str> {
    let Some((track, c)) = app.project.clip(clip) else {
        return Some("Select the music clip first.");
    };
    let sound = c
        .media_id
        .and_then(|m| app.project.media(m))
        .is_some_and(|m| m.kind == tidycuts_core::MediaKind::Audio || m.has_audio);
    if !sound || c.generator.is_some() {
        return Some("Select a clip with sound (the music) first.");
    }
    if track.locked {
        return Some("That clip's track is locked.");
    }
    if (c.speed - 1.0).abs() > 1e-9 || c.ramp.is_some() {
        return Some("Auto-duck needs the music at normal speed.");
    }
    None
}

/// The volume points that lower `base` by `lower_db` under `speech` (times in
/// clip seconds), as keyframes.
pub fn keyframes(speech: &[(f64, f64)], clip_len: f64, base: f64, lower_db: f64) -> Vec<Keyframe> {
    let ducked = base * 10f64.powf(-lower_db / 20.0);
    autoduck::duck_points(speech, clip_len, base, ducked, ATTACK, RELEASE)
        .into_iter()
        .map(|(t, v)| Keyframe {
            at: Time::from_secs_f64(t),
            value: v,
            easing: Easing::EaseInOut,
        })
        .collect()
}

impl TidyCutsApp {
    /// Looks for speech on the rest of the timeline and lowers `clip` there.
    pub fn start_auto_duck(&mut self, clip: ClipId, lower_db: f64) {
        if let Some(why) = unsuitable(self, clip) {
            self.notify(why, true);
            return;
        }
        let Some((_, c)) = self.project.clip(clip) else {
            return;
        };
        let (start, duration) = (c.start, c.duration());
        let project = self.project.clone();
        let table = self
            .engine
            .as_ref()
            .map(|e| e.analysis.loudness.clone())
            .unwrap_or_default();
        let tx = self.duck_tx.clone();
        let ctx = self.ctx.clone();
        self.notify("Listening for speech…", false);
        std::thread::spawn(move || {
            let never = AtomicBool::new(false);
            let result = autoduck::find_speech(&project, clip, table, &never);
            let _ = tx.send(Finished {
                clip,
                start,
                duration,
                lower_db,
                result,
            });
            ctx.request_repaint();
        });
    }

    /// Applies finished searches. Called every frame.
    pub fn poll_auto_duck(&mut self) {
        while let Ok(done) = self.duck_rx.try_recv() {
            let outcome = match done.result {
                Err(e) => Err(e),
                Ok(_)
                    if self.project.clip(done.clip).is_none_or(|(_, c)| {
                        c.start != done.start || c.duration() != done.duration
                    }) =>
                {
                    Err("The clip changed while it was being checked. Try again.".to_string())
                }
                Ok(speech) => {
                    let from = done.start.as_secs_f64();
                    let local: Vec<(f64, f64)> =
                        speech.iter().map(|(s, e)| (s - from, e - from)).collect();
                    let len = done.duration.as_secs_f64();
                    let base = self
                        .project
                        .clip(done.clip)
                        .map_or(100.0, |(_, c)| c.volume.value);
                    let points = keyframes(&local, len, base, done.lower_db);
                    if points.is_empty() {
                        Ok(0)
                    } else {
                        let n = local.iter().filter(|(s, e)| *e > 0.0 && *s < len).count();
                        self.edit(|p| match p.clip_mut(done.clip) {
                            Some(c) => {
                                c.volume = Animatable {
                                    value: base,
                                    keyframes: points,
                                };
                                true
                            }
                            None => false,
                        });
                        Ok(n)
                    }
                }
            };
            match outcome {
                Err(e) => self.notify(&e, true),
                Ok(0) => self.notify(
                    "No speech found over this music, so nothing was lowered.",
                    false,
                ),
                Ok(n) => self.notify(
                    &format!(
                        "The music now lowers {} dB under {} spoken part{}.",
                        done.lower_db,
                        n,
                        if n == 1 { "" } else { "s" }
                    ),
                    false,
                ),
            }
        }
    }
}
