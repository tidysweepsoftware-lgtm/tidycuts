//! Finding silences, scene changes and beats in the selected clip, and
//! acting on what is found. The search runs on a worker thread; its answer
//! is applied as one undo step, and only if the clip is still as it was.

use crate::app::TidyCutsApp;
use tidycuts_core::{ClipId, MediaKind, Time};
use tidycuts_engine::detect;
use tidycuts_media::Tools;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Detection {
    /// Take out stretches quieter than `noise_db` that last `min_secs`.
    Silences { noise_db: f64, min_secs: f64 },
    /// Cut where the picture changes scene; `threshold` is 0..1.
    Scenes { threshold: f64 },
    /// Mark the beat.
    Beats,
}

/// What a clip looked like when the search began. The answer is in the
/// clip's source time, so it only fits while these are unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct Print {
    start: Time,
    source_in: Time,
    source_out: Time,
    speed: f64,
    reverse: bool,
    ramp: bool,
}

pub enum Found {
    Silences(Vec<(f64, f64)>),
    Scenes(Vec<f64>),
    Beats(Vec<f64>),
}

pub struct Finished {
    clip: ClipId,
    print: Print,
    result: Result<Found, String>,
}

/// Runs one search over `range` of a file, on whatever thread calls it.
pub fn search(kind: Detection, path: &std::path::Path, range: (f64, f64)) -> Result<Found, String> {
    let tools = Tools::get().map_err(|e| e.to_string())?;
    Ok(match kind {
        Detection::Silences { noise_db, min_secs } => {
            Found::Silences(detect::silences(tools, path, range, noise_db, min_secs)?)
        }
        Detection::Scenes { threshold } => {
            Found::Scenes(detect::scene_changes(tools, path, range, threshold)?)
        }
        Detection::Beats => Found::Beats(detect::beats(tools, path, range)?),
    })
}

/// Why a clip can't be searched, in plain words.
pub fn unsuitable(app: &TidyCutsApp, clip: ClipId, kind: Detection) -> Option<&'static str> {
    let Some((_, c)) = app.project.clip(clip) else {
        return Some("Select a clip first.");
    };
    let Some(media) = c.media_id.and_then(|m| app.project.media(m)) else {
        return Some("This clip has no video or sound to look through.");
    };
    match kind {
        Detection::Scenes { .. } if media.kind != MediaKind::Video => {
            Some("Only video clips have scenes.")
        }
        Detection::Silences { .. } | Detection::Beats
            if !(media.kind == MediaKind::Audio || media.has_audio) =>
        {
            Some("This clip has no sound.")
        }
        _ => None,
    }
}

impl TidyCutsApp {
    fn print_of(&self, clip: ClipId) -> Option<Print> {
        let (_, c) = self.project.clip(clip)?;
        Some(Print {
            start: c.start,
            source_in: c.source_in,
            source_out: c.source_out,
            speed: c.speed,
            reverse: c.reverse,
            ramp: c.ramp.is_some(),
        })
    }

    /// Starts a search through a clip's file; the answer is applied when it
    /// arrives.
    pub fn start_detection(&mut self, clip: ClipId, kind: Detection) {
        if let Some(why) = unsuitable(self, clip, kind) {
            self.notify(why, true);
            return;
        }
        let (Some(print), Some((_, c))) = (self.print_of(clip), self.project.clip(clip)) else {
            return;
        };
        let Some(path) = c
            .media_id
            .and_then(|m| self.project.media(m))
            .map(|m| m.path.clone())
        else {
            return;
        };
        let range = (c.source_in.as_secs_f64(), c.source_out.as_secs_f64());
        let tx = self.detect_tx.clone();
        let ctx = self.ctx.clone();
        self.detecting += 1;
        self.notify(
            match kind {
                Detection::Silences { .. } => "Looking for silent parts…",
                Detection::Scenes { .. } => "Looking for scene changes…",
                Detection::Beats => "Listening for the beat…",
            },
            false,
        );
        std::thread::spawn(move || {
            let result = search(kind, &path, range);
            let _ = tx.send(Finished {
                clip,
                print,
                result,
            });
            ctx.request_repaint();
        });
    }

    /// Applies finished searches. Called every frame.
    pub fn poll_detections(&mut self) {
        while let Ok(done) = self.detect_rx.try_recv() {
            self.detecting = self.detecting.saturating_sub(1);
            let outcome = match done.result {
                Err(e) => Err(e),
                Ok(_) if self.print_of(done.clip).as_ref() != Some(&done.print) => {
                    Err("The clip changed while it was being searched. Try again.".to_string())
                }
                Ok(found) => self.apply_found(done.clip, found),
            };
            match outcome {
                Ok(text) => self.notify(&text, false),
                Err(text) => self.notify(&text, true),
            }
        }
    }

    /// Searches a clip right now (the caller waits) and applies the answer.
    /// Used by the assistant's tools.
    pub fn detect_now(&mut self, clip: ClipId, kind: Detection) -> Result<String, String> {
        if let Some(why) = unsuitable(self, clip, kind) {
            return Err(why.to_string());
        }
        let (_, c) = self.project.clip(clip).ok_or("That clip is gone.")?;
        let path = c
            .media_id
            .and_then(|m| self.project.media(m))
            .map(|m| m.path.clone())
            .ok_or("This clip has no file to search.")?;
        let range = (c.source_in.as_secs_f64(), c.source_out.as_secs_f64());
        let found = search(kind, &path, range)?;
        self.apply_found(clip, found)
    }

    /// Takes the given stretches (in the file's own seconds) out of a clip
    /// and closes the gaps, as one undo step.
    pub fn remove_silences(
        &mut self,
        clip: ClipId,
        ranges: Vec<(f64, f64)>,
    ) -> Result<String, String> {
        self.apply_found(clip, Found::Silences(ranges))
    }

    /// Acts on what was found. `Ok` says what was done, `Err` why nothing was.
    fn apply_found(&mut self, clip: ClipId, found: Found) -> Result<String, String> {
        match found {
            Found::Silences(ranges) => {
                if ranges.is_empty() {
                    return Ok(
                        "No silent parts found. Try a higher level or a shorter pause.".into(),
                    );
                }
                let mut removed = None;
                self.edit(|p| {
                    removed = p.remove_source_ranges(clip, &ranges);
                    removed.is_some()
                });
                match removed {
                    Some(t) if t > Time::ZERO => Ok(format!(
                        "Removed {} silent part{} ({:.1} s).",
                        ranges.len(),
                        if ranges.len() == 1 { "" } else { "s" },
                        t.as_secs_f64()
                    )),
                    _ => Err("The silent parts couldn't be cut (is the track locked?).".into()),
                }
            }
            Found::Scenes(times) => {
                let mut cuts = 0;
                self.edit(|p| {
                    cuts = p.split_at_source_times(clip, &times);
                    cuts > 0
                });
                if cuts == 0 {
                    Ok("No scene changes found. Try again with a lower threshold.".into())
                } else {
                    Ok(format!(
                        "Split into {} clips at the scene changes.",
                        cuts + 1
                    ))
                }
            }
            Found::Beats(times) => {
                let mut marked = 0;
                self.edit(|p| {
                    marked = p.add_markers_at_source_times(clip, &times);
                    marked > 0
                });
                if marked == 0 {
                    return Ok("No clear beat found in this clip.".into());
                }
                let bpm = tempo(&times)
                    .map(|b| format!(" (about {b:.0} beats a minute)"))
                    .unwrap_or_default();
                Ok(format!("Added {marked} markers on the beat{bpm}."))
            }
        }
    }
}

/// A clip to line up: where its sound is and which part is used.
struct SyncInput {
    clip: ClipId,
    path: std::path::PathBuf,
    range: (f64, f64),
    print: Print,
}

pub struct SyncFinished {
    reference: ClipId,
    result: Result<Vec<(ClipId, Print, tidycuts_engine::sync::Offset)>, String>,
    reference_print: Print,
}

fn sync_compute(
    inputs: Vec<SyncInput>,
) -> Result<Vec<(ClipId, Print, tidycuts_engine::sync::Offset)>, String> {
    use tidycuts_engine::sync::offset_between;
    let tools = Tools::get().map_err(|e| e.to_string())?;
    let reference = &inputs[0];
    inputs[1..]
        .iter()
        .map(|o| {
            let off = offset_between(
                tools,
                (&reference.path, reference.range),
                (&o.path, o.range),
                60.0,
            )?;
            Ok((o.clip, o.print.clone(), off))
        })
        .collect()
}

impl TidyCutsApp {
    /// The clips to line up, earliest first: each with sound, at normal speed.
    fn sync_inputs(&self, ids: &[ClipId]) -> Result<Vec<SyncInput>, String> {
        if ids.len() < 2 {
            return Err("Select at least two clips to line up.".into());
        }
        let mut inputs = Vec::new();
        for id in ids {
            let (_, c) = self.project.clip(*id).ok_or("A selected clip is gone.")?;
            let media = c
                .media_id
                .and_then(|m| self.project.media(m))
                .filter(|m| m.kind == MediaKind::Audio || m.has_audio)
                .ok_or("Every clip needs sound to be lined up by it.")?;
            if c.speed != 1.0 || c.reverse || c.ramp.is_some() {
                return Err(
                    "Clips that are sped up, slowed down or reversed can't be lined up by sound."
                        .into(),
                );
            }
            inputs.push(SyncInput {
                clip: *id,
                path: media.path.clone(),
                range: (c.source_in.as_secs_f64(), c.source_out.as_secs_f64()),
                print: self.print_of(*id).ok_or("A selected clip is gone.")?,
            });
        }
        inputs.sort_by_key(|i| i.print.start);
        Ok(inputs)
    }

    /// Lines the selected clips up by their sound, on a worker thread.
    pub fn start_sync(&mut self, ids: &[ClipId]) {
        let inputs = match self.sync_inputs(ids) {
            Ok(i) => i,
            Err(e) => {
                self.notify(&e, true);
                return;
            }
        };
        let reference = inputs[0].clip;
        let reference_print = inputs[0].print.clone();
        let tx = self.sync_tx.clone();
        let ctx = self.ctx.clone();
        self.detecting += 1;
        self.notify("Listening to line the clips up…", false);
        std::thread::spawn(move || {
            let result = sync_compute(inputs);
            let _ = tx.send(SyncFinished {
                reference,
                result,
                reference_print,
            });
            ctx.request_repaint();
        });
    }

    /// Lines clips up right now (the caller waits). For the assistant's tools.
    pub fn sync_now(&mut self, ids: &[ClipId]) -> Result<String, String> {
        let inputs = self.sync_inputs(ids)?;
        let reference = inputs[0].clip;
        let reference_print = inputs[0].print.clone();
        let result = sync_compute(inputs)?;
        self.apply_sync(reference, &reference_print, result)
    }

    pub fn poll_syncs(&mut self) {
        while let Ok(done) = self.sync_rx.try_recv() {
            self.detecting = self.detecting.saturating_sub(1);
            let outcome = match done.result {
                Err(e) => Err(e),
                Ok(found) => self.apply_sync(done.reference, &done.reference_print, found),
            };
            match outcome {
                Ok(t) => self.notify(&t, false),
                Err(t) => self.notify(&t, true),
            }
        }
    }

    fn apply_sync(
        &mut self,
        reference: ClipId,
        reference_print: &Print,
        found: Vec<(ClipId, Print, tidycuts_engine::sync::Offset)>,
    ) -> Result<String, String> {
        if self.print_of(reference).as_ref() != Some(reference_print) {
            return Err("The clips changed while they were being listened to. Try again.".into());
        }
        let mut good = Vec::new();
        let mut weak = 0;
        for (id, print, off) in found {
            if self.print_of(id).as_ref() != Some(&print) {
                return Err(
                    "The clips changed while they were being listened to. Try again.".into(),
                );
            }
            // A clear peak is far above the noise around it.
            if off.confidence >= 4.0 {
                good.push((id, off.lag));
            } else {
                weak += 1;
            }
        }
        if good.is_empty() {
            return Err(
                "Couldn't find the same sound in these clips, so nothing was moved.".into(),
            );
        }
        let mut moved = 0;
        self.edit(|p| {
            moved = p.place_synced(reference, &good);
            moved > 0
        });
        let extra = if weak > 0 {
            format!(
                " {weak} clip{} had no clear match and stayed put.",
                if weak == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        };
        Ok(format!(
            "Lined up {} clips by their sound.{extra}",
            good.len() + 1
        ))
    }
}

/// Beats a minute, from the middle gap between neighbouring beats.
fn tempo(beats: &[f64]) -> Option<f64> {
    let mut gaps: Vec<f64> = beats.windows(2).map(|w| w[1] - w[0]).collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_by(f64::total_cmp);
    let mid = gaps[gaps.len() / 2];
    (mid > 0.05).then(|| 60.0 / mid)
}

#[cfg(test)]
mod tests {
    use super::tempo;

    #[test]
    fn tempo_comes_from_the_middle_gap() {
        let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.5).collect();
        assert!((tempo(&beats).unwrap() - 120.0).abs() < 1e-6);
        assert!(tempo(&[1.0]).is_none());
    }
}
