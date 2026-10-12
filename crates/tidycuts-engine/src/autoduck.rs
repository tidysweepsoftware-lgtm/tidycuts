//! Auto-duck: finds where someone is talking and works out the volume changes
//! that lower the music there and bring it back afterwards.
//!
//! Speech is found with the voice detector inside RNNoise (it gives, for every
//! 10 ms, the chance that a voice is present), on the mix of everything on the
//! timeline except the music.

use crate::mixer::{LoudnessTable, Mixer};
use nnnoiseless::DenoiseState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tidycuts_core::{ClipId, Project, Time};

const SAMPLE_RATE: u32 = 48_000;
const FRAME: usize = DenoiseState::FRAME_SIZE;
const FRAME_SECS: f64 = FRAME as f64 / SAMPLE_RATE as f64;

/// The chance of a voice for every 10 ms of a mono 48 kHz sound
/// (-1..1 scale).
pub fn voice_chance(mono: &[f32]) -> Vec<f32> {
    let mut state = DenoiseState::new();
    let mut out = [0.0f32; FRAME];
    let mut chances = Vec::with_capacity(mono.len() / FRAME + 1);
    let mut frame = [0.0f32; FRAME];
    for chunk in mono.chunks(FRAME) {
        frame.fill(0.0);
        for (f, s) in frame.iter_mut().zip(chunk) {
            *f = if s.is_finite() { s * 32768.0 } else { 0.0 };
        }
        chances.push(state.process_frame(&mut out, &frame));
    }
    // The first answer comes before the detector has heard anything.
    if let Some(c) = chances.first_mut() {
        *c = 0.0;
    }
    chances
}

/// Where the voice is, in seconds: a stretch starts when the chance rises
/// above `on`, goes on until it has been below `off` for `hang` seconds, and
/// must last at least `min_len`.
pub fn speech_ranges(
    chances: &[f32],
    on: f32,
    off: f32,
    hang: f64,
    min_len: f64,
) -> Vec<(f64, f64)> {
    let mut ranges: Vec<(f64, f64)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut quiet = 0usize;
    let hang_frames = (hang / FRAME_SECS).ceil() as usize;
    for (i, c) in chances.iter().enumerate() {
        match start {
            None if *c >= on => {
                start = Some(i);
                quiet = 0;
            }
            Some(_) if *c < off => {
                quiet += 1;
                if quiet > hang_frames {
                    let s = start.take().unwrap();
                    ranges.push((s as f64 * FRAME_SECS, (i - quiet + 1) as f64 * FRAME_SECS));
                }
            }
            Some(_) => quiet = 0,
            None => {}
        }
    }
    if let Some(s) = start {
        let end = chances.len() - quiet;
        ranges.push((s as f64 * FRAME_SECS, end as f64 * FRAME_SECS));
    }
    ranges.retain(|(s, e)| e - s >= min_len);
    ranges
}

/// The volume points for a music clip: `base` normally, `ducked` while there
/// is speech, with `attack` seconds to go down (ending where the voice starts)
/// and `release` seconds to come back (starting where the voice ends).
/// Times are measured from the start of the clip; `speech` is on the same
/// scale. Returns (time, value) pairs in order.
pub fn duck_points(
    speech: &[(f64, f64)],
    clip_len: f64,
    base: f64,
    ducked: f64,
    attack: f64,
    release: f64,
) -> Vec<(f64, f64)> {
    // Stretches closer than a full dip and recovery are one stretch.
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for &(s, e) in speech {
        let (s, e) = (s.max(0.0), e.min(clip_len));
        if e <= s {
            continue;
        }
        match merged.last_mut() {
            Some(last) if s - last.1 < attack + release => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    // A point is added only if it comes after the last one.
    fn push(points: &mut Vec<(f64, f64)>, t: f64, v: f64) {
        if points.last().is_none_or(|p| t > p.0 + 1e-3) {
            points.push((t, v));
        }
    }
    let mut points: Vec<(f64, f64)> = Vec::new();
    for (s, e) in merged {
        if s <= 1e-3 {
            // Already talking when the music starts.
            push(&mut points, 0.0, ducked);
        } else {
            let down_from = (s - attack).max(0.0);
            if down_from <= 1e-3 {
                push(&mut points, 0.0, base);
            } else {
                push(&mut points, down_from, base);
            }
            push(&mut points, s, ducked);
        }
        if e >= clip_len - 1e-3 {
            // Talking to the end: stay down.
            push(&mut points, clip_len, ducked);
        } else {
            push(&mut points, e, ducked);
            let back = e + release;
            if back <= clip_len {
                push(&mut points, back, base);
            } else {
                // The clip ends part of the way back up.
                let v = ducked + (base - ducked) * (clip_len - e) / release;
                push(&mut points, clip_len, v);
            }
        }
    }
    points
}

/// Finds where there is speech on the timeline, leaving `exclude` (the music)
/// out of the mix. Times are timeline seconds.
pub fn find_speech(
    project: &Project,
    exclude: ClipId,
    loudness: LoudnessTable,
    cancel: &AtomicBool,
) -> Result<Vec<(f64, f64)>, String> {
    let mut p = project.clone();
    if let Some(c) = p.clip_mut(exclude) {
        c.disabled = true;
    }
    let total = project.duration();
    if total <= Time::ZERO {
        return Err("There is nothing on the timeline.".into());
    }
    let mut mixer = Mixer::new(SAMPLE_RATE, Duration::from_secs(30), loudness);
    let frames_total = (total.as_secs_f64() * SAMPLE_RATE as f64) as usize;
    let mut mono: Vec<f32> = Vec::with_capacity(frames_total);
    let mut buf = vec![0f32; 4096 * 2];
    let mut done = 0usize;
    while done < frames_total {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled.".into());
        }
        let n = 4096.min(frames_total - done);
        mixer.mix(
            &p,
            Time::from_secs_f64(done as f64 / SAMPLE_RATE as f64),
            &mut buf[..n * 2],
        );
        mono.extend(
            buf[..n * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|s| (s[0] + s[1]) * 0.5),
        );
        done += n;
    }
    let chances = voice_chance(&mono);
    Ok(speech_ranges(&chances, 0.6, 0.35, 0.35, 0.3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chances(pattern: &[(f32, f64)]) -> Vec<f32> {
        // (value, seconds) runs.
        pattern
            .iter()
            .flat_map(|(v, s)| std::iter::repeat_n(*v, (s / FRAME_SECS).round() as usize))
            .collect()
    }

    #[test]
    fn short_blips_are_not_speech_and_gaps_are_bridged() {
        let c = chances(&[
            (0.0, 1.0),
            (0.9, 0.1), // a blip
            (0.0, 1.0),
            (0.9, 2.0), // a sentence
            (0.1, 0.2), // a breath, shorter than the hang time
            (0.9, 1.0),
            (0.0, 2.0),
        ]);
        let r = speech_ranges(&c, 0.6, 0.35, 0.35, 0.3);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(
            (r[0].0 - 2.1).abs() < 0.05 && (r[0].1 - 5.3).abs() < 0.1,
            "{r:?}"
        );
    }

    #[test]
    fn speech_that_runs_to_the_end_is_kept() {
        let c = chances(&[(0.0, 1.0), (0.9, 1.0)]);
        let r = speech_ranges(&c, 0.6, 0.35, 0.35, 0.3);
        assert_eq!(r.len(), 1);
        assert!((r[0].1 - 2.0).abs() < 0.05);
    }

    #[test]
    fn the_music_dips_before_the_voice_and_returns_after() {
        let p = duck_points(&[(5.0, 8.0)], 30.0, 100.0, 25.0, 0.3, 0.8);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], (4.7, 100.0));
        assert_eq!(p[1], (5.0, 25.0));
        assert_eq!(p[2], (8.0, 25.0));
        assert!((p[3].0 - 8.8).abs() < 1e-9 && p[3].1 == 100.0);
        // Times only go forward.
        assert!(p.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn close_stretches_share_one_dip() {
        let p = duck_points(&[(5.0, 6.0), (6.5, 8.0)], 30.0, 100.0, 25.0, 0.3, 0.8);
        assert_eq!(p.len(), 4, "{p:?}");
        assert!((p[2].0 - 8.0).abs() < 1e-9);
    }

    #[test]
    fn speech_at_the_very_start_and_end_stays_inside_the_clip() {
        let p = duck_points(&[(0.0, 3.0), (28.0, 40.0)], 30.0, 100.0, 25.0, 0.3, 0.8);
        assert!(p.iter().all(|(t, _)| (0.0..=30.0).contains(t)), "{p:?}");
        assert!(p.windows(2).all(|w| w[0].0 < w[1].0), "{p:?}");
        // It starts ducked, since the voice is there from the first moment,
        // and a voice that talks to the end keeps it down.
        assert_eq!(p[0], (0.0, 25.0));
        assert_eq!(*p.last().unwrap(), (30.0, 25.0));
        // In between it comes back up.
        assert!(p.iter().any(|(_, v)| *v == 100.0));
    }

    #[test]
    fn silence_has_no_voice() {
        let c = voice_chance(&vec![0.0; 48_000]);
        assert!(c.iter().all(|v| *v < 0.3));
    }
}
