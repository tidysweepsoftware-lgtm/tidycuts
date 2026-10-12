//! Normalize's level guard: it only ever turns sound DOWN.
//!
//! It listens to how loud the clip is, the way people hear it (the K-weighted
//! loudness of BS.1770, see `loudness`), and when that goes above the level the
//! person allows it lowers the volume, smoothly, by just enough to bring it back
//! to that level. Everything at or under the level passes through untouched, bit
//! for bit: the volume is never raised, never evened out, and never moves
//! up and down around a quiet voice.

use crate::loudness::{k_weighting, Biquad};

/// The loudest level Normalize allows unless the person changes it, in LUFS.
pub const DEFAULT_CEILING: f64 = -18.0;
/// The range the level may be set in.
pub const CEILING_RANGE: std::ops::RangeInclusive<f64> = -30.0..=-8.0;

/// How fast the loudness reading follows the sound, in seconds. About the
/// length of one spoken phrase: slow enough that the rise and fall of single
/// syllables do not move it, fast enough that a shout is caught in a moment
/// (the louder it is, the sooner).
const DETECT_SECS: f64 = 0.25;
/// How fast the volume goes down once the sound is too loud, in seconds.
const ATTACK_SECS: f64 = 0.040;
/// How slowly it comes back up afterwards, in seconds. The loudness reading
/// itself falls over a quarter of a second, so this only rounds the corner.
const RELEASE_SECS: f64 = 0.20;
/// A turn-down smaller than this (a fiftieth of a decibel, far below what
/// anyone can hear) is rounded to none, so a clip that is back under its level
/// really goes through untouched again.
const SNAP_DB: f32 = 0.02;
/// The soft knee: the turning-down starts this many decibels before the level
/// is reached, so it fades in instead of switching on.
const KNEE_DB: f32 = 2.0;

/// A stereo level guard. Feed it one frame at a time; it adds no delay.
pub struct LevelGuard {
    filters: [[Biquad; 2]; 2],
    /// The smoothed weighted power of both channels together.
    power: f64,
    detect: f64,
    attack: f32,
    release: f32,
    ceiling: f32,
    /// How far down the volume is now, in decibels (0 or more).
    cut: f32,
    /// The gain that goes with `cut`.
    gain: f32,
}

/// How many decibels to turn the sound down when it is `over` decibels above
/// the level (negative when it is under it).
fn wanted_cut(over: f32) -> f32 {
    let half = KNEE_DB / 2.0;
    if over <= -half {
        0.0
    } else if over < half {
        (over + half) * (over + half) / (2.0 * KNEE_DB)
    } else {
        over
    }
}

fn coefficient(secs: f64, sample_rate: u32) -> f64 {
    1.0 - (-1.0 / (secs * f64::from(sample_rate))).exp()
}

impl LevelGuard {
    pub fn new(sample_rate: u32, ceiling_lufs: f64) -> LevelGuard {
        let f = k_weighting(f64::from(sample_rate));
        LevelGuard {
            filters: [f, f],
            power: 0.0,
            detect: coefficient(DETECT_SECS, sample_rate),
            attack: coefficient(ATTACK_SECS, sample_rate) as f32,
            release: coefficient(RELEASE_SECS, sample_rate) as f32,
            ceiling: ceiling_lufs.clamp(*CEILING_RANGE.start(), *CEILING_RANGE.end()) as f32,
            cut: 0.0,
            gain: 1.0,
        }
    }

    /// Changes the level (when the person moves the slider while it plays).
    pub fn set_ceiling(&mut self, ceiling_lufs: f64) {
        self.ceiling = ceiling_lufs.clamp(*CEILING_RANGE.start(), *CEILING_RANGE.end()) as f32;
    }

    /// How far down the volume is right now, in decibels.
    pub fn cut_db(&self) -> f32 {
        self.cut
    }

    /// Takes one frame and gives it back, turned down if it is too loud.
    pub fn process(&mut self, frame: [f32; 2]) -> [f32; 2] {
        // A broken sample from a decoder must not poison the filters for good.
        let frame = frame.map(|s| if s.is_finite() { s } else { 0.0 });
        let mut p = 0.0;
        for (ch, s) in frame.iter().enumerate() {
            let mut v = f64::from(*s);
            for f in &mut self.filters[ch] {
                v = f.run(v);
            }
            p += v * v;
        }
        self.power += (p - self.power) * self.detect;
        let want = if self.power > 1e-12 {
            let lufs = (-0.691 + 10.0 * self.power.log10()) as f32;
            wanted_cut(lufs - self.ceiling)
        } else {
            0.0
        };
        let step = if want > self.cut {
            self.attack
        } else {
            self.release
        };
        let before = self.cut;
        self.cut += (want - self.cut) * step;
        if want == 0.0 && self.cut < SNAP_DB {
            self.cut = 0.0;
        }
        if self.cut != before {
            self.gain = 10f32.powf(-self.cut / 20.0);
        }
        if self.cut == 0.0 {
            // Nothing to turn down: the sound goes through as it is.
            frame
        } else {
            [frame[0] * self.gain, frame[1] * self.gain]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loudness::Momentary;

    const SR: u32 = 48_000;

    /// Stereo 1 kHz sine of amplitude `amp`: a full-scale one reads 0 LUFS, so
    /// the reading in LUFS is about 20 log10(amp).
    fn tone(amp: f32, secs: f32) -> Vec<[f32; 2]> {
        (0..(secs * SR as f32) as usize)
            .map(|i| {
                let s = amp * (std::f32::consts::TAU * 1000.0 * i as f32 / SR as f32).sin();
                [s, s]
            })
            .collect()
    }

    fn run(g: &mut LevelGuard, input: &[[f32; 2]]) -> Vec<[f32; 2]> {
        input.iter().map(|f| g.process(*f)).collect()
    }

    fn lufs(frames: &[[f32; 2]]) -> f32 {
        let mut m = Momentary::new(SR);
        let flat: Vec<f32> = frames.iter().flatten().copied().collect();
        m.push(&flat);
        m.lufs().unwrap_or(-99.0)
    }

    #[test]
    fn sound_under_the_level_passes_through_exactly() {
        // -26 LUFS against a level of -18: not touched at all.
        let mut g = LevelGuard::new(SR, -18.0);
        let x = tone(0.05, 3.0);
        let y = run(&mut g, &x);
        assert_eq!(x, y);
        assert_eq!(g.cut_db(), 0.0);
        // Not even a little above its own level, just under the level.
        let mut g = LevelGuard::new(SR, -18.0);
        let x = tone(0.11, 3.0); // -19.2 LUFS
        assert_eq!(run(&mut g, &x), x);
    }

    #[test]
    fn loud_sound_is_brought_down_to_the_level_and_never_up() {
        let mut g = LevelGuard::new(SR, -18.0);
        let x = tone(0.5, 4.0); // about -6 LUFS
        let y = run(&mut g, &x);
        // Every frame is the same or quieter than what came in.
        for (a, b) in x.iter().zip(&y) {
            assert!(b[0].abs() <= a[0].abs() + 1e-6, "{a:?} {b:?}");
        }
        // After a moment the loudness sits at the level.
        let settled = lufs(&y[y.len() - SR as usize..]);
        assert!((settled + 18.0).abs() < 0.5, "{settled}");
        assert!((g.cut_db() - 12.0).abs() < 0.5, "{}", g.cut_db());
    }

    #[test]
    fn a_shout_is_caught_quickly_and_the_voice_after_it_comes_back_whole() {
        let mut g = LevelGuard::new(SR, -18.0);
        let mut x = tone(0.04, 1.0); // a quiet voice, -28 LUFS
        let shout_at = x.len();
        x.extend(tone(0.6, 0.8)); // a shout, about -4.4 LUFS
        let after = x.len();
        x.extend(tone(0.04, 3.0)); // the quiet voice again
        let y = run(&mut g, &x);
        // Before the shout: untouched.
        assert_eq!(&x[..shout_at], &y[..shout_at]);
        // The last 200 ms of the shout is already close to the level.
        let tail = lufs(&y[after - SR as usize / 5..after]);
        assert!(tail < -15.5, "{tail}");
        // Two seconds after it, the quiet voice is back at its own level.
        let back = &y[after + 2 * SR as usize..];
        let original = &x[after + 2 * SR as usize..];
        let diff = lufs(original) - lufs(back);
        assert!(diff.abs() < 0.1, "{diff}");
        assert_eq!(g.cut_db(), 0.0);
    }

    #[test]
    fn the_volume_never_moves_around_a_voice_that_stays_under_the_level() {
        // A speech-like rise and fall, 3 times a second, between -32 and -18.4 LUFS.
        let n = 6 * SR as usize;
        let x: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                let t = i as f32 / SR as f32;
                let env = 0.075 + 0.045 * (std::f32::consts::TAU * 3.0 * t).sin();
                let s = env * (std::f32::consts::TAU * 1000.0 * t).sin();
                [s, s]
            })
            .collect();
        let mut g = LevelGuard::new(SR, -18.0);
        let y = run(&mut g, &x);
        assert_eq!(x, y);
    }

    #[test]
    fn speech_peaks_above_the_level_are_turned_down_smoothly_not_pumped() {
        // The same rise and fall, but peaking at about -10 LUFS: above the level.
        let n = 8 * SR as usize;
        let x: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                let t = i as f32 / SR as f32;
                let env = 0.17 + 0.13 * (std::f32::consts::TAU * 3.0 * t).sin();
                let s = env * (std::f32::consts::TAU * 1000.0 * t).sin();
                [s, s]
            })
            .collect();
        let mut g = LevelGuard::new(SR, -18.0);
        let mut cuts = Vec::new();
        for (i, f) in x.iter().enumerate() {
            g.process(*f);
            if i > 4 * SR as usize && i % 48 == 0 {
                cuts.push(g.cut_db());
            }
        }
        let (lo, hi) = cuts
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), c| (l.min(*c), h.max(*c)));
        // The reduction ripples by well under 2 dB although the sound itself
        // swings by 16 dB.
        assert!(hi - lo < 2.0, "{lo}..{hi}");
        assert!(lo > 0.5, "it should be doing something: {lo}");
    }

    #[test]
    fn the_level_can_be_changed_while_it_plays() {
        let mut g = LevelGuard::new(SR, -30.0);
        let x = tone(0.1, 3.0); // -20 LUFS
        run(&mut g, &x);
        assert!(g.cut_db() > 8.0, "{}", g.cut_db());
        g.set_ceiling(-8.0);
        run(&mut g, &x);
        assert_eq!(g.cut_db(), 0.0);
    }

    #[test]
    fn silence_and_garbage_are_safe() {
        let mut g = LevelGuard::new(SR, -18.0);
        assert_eq!(g.process([0.0, 0.0]), [0.0, 0.0]);
        // A broken sample becomes silence and the guard carries on as before.
        assert_eq!(g.process([f32::NAN, f32::INFINITY]), [0.0, 0.0]);
        assert_eq!(g.process([0.1, 0.1]), [0.1, 0.1]);
        // The turning-down curve: none well under, all of it well over.
        assert_eq!(wanted_cut(-5.0), 0.0);
        assert!((wanted_cut(3.0) - 3.0).abs() < 1e-6);
        assert!(wanted_cut(0.0) > 0.0 && wanted_cut(0.0) < 1.0);
    }
}
