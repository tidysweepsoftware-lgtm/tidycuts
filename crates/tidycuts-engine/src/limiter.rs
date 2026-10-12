//! A look-ahead peak limiter: keeps sound from going over a ceiling without
//! clicks or the harshness of clipping.
//!
//! It delays the sound by a couple of milliseconds, so it can see a peak
//! coming and start turning the volume down just before it arrives, then let
//! the volume come back up smoothly afterwards. Nothing that comes out is
//! louder than the ceiling.

use std::collections::VecDeque;

/// How far ahead it looks, in seconds.
const LOOKAHEAD_SECS: f64 = 0.0025;
/// How long the volume takes to come most of the way back, in seconds.
const RELEASE_SECS: f64 = 0.060;

/// A stereo limiter. Feed it one frame at a time.
pub struct PeakLimiter {
    ceiling: f32,
    /// Look-ahead in frames (also the delay, less one).
    n: usize,
    /// Frames waiting to come out.
    delay: VecDeque<[f32; 2]>,
    /// The lowest gain needed in the last `n` frames, as (frame number, gain),
    /// kept in rising order so its front is the lowest.
    lowest: VecDeque<(u64, f32)>,
    count: u64,
    /// The lowest gain with a slow release.
    held: f32,
    /// The last `n` of those, and their sum, for the moving average.
    recent: VecDeque<f32>,
    sum: f32,
    release: f32,
}

impl PeakLimiter {
    pub fn new(sample_rate: u32, ceiling: f32) -> PeakLimiter {
        let rate = f64::from(sample_rate);
        let n = ((LOOKAHEAD_SECS * rate) as usize).max(4);
        PeakLimiter {
            ceiling: ceiling.max(1e-3),
            n,
            delay: VecDeque::with_capacity(n + 1),
            lowest: VecDeque::new(),
            count: 0,
            held: 1.0,
            recent: VecDeque::with_capacity(n + 1),
            sum: 0.0,
            release: (1.0 - (-1.0 / (RELEASE_SECS * rate)).exp()) as f32,
        }
    }

    /// Takes one frame and gives the frame from `n - 1` frames ago, with the
    /// volume turned down as far as the peaks around it need.
    pub fn process(&mut self, frame: [f32; 2]) -> [f32; 2] {
        let peak = frame[0].abs().max(frame[1].abs());
        let need = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        // The lowest gain needed within the last `n` frames.
        while self.lowest.back().is_some_and(|b| b.1 >= need) {
            self.lowest.pop_back();
        }
        self.lowest.push_back((self.count, need));
        while self
            .lowest
            .front()
            .is_some_and(|f| f.0 + self.n as u64 <= self.count)
        {
            self.lowest.pop_front();
        }
        let lowest = self.lowest.front().map_or(1.0, |f| f.1);
        self.count += 1;
        // Down at once, back up slowly.
        self.held = if lowest < self.held {
            lowest
        } else {
            self.held + (lowest - self.held) * self.release
        };
        // Averaged over the look-ahead, so the volume glides instead of jumping.
        self.recent.push_back(self.held);
        self.sum += self.held;
        if self.recent.len() > self.n {
            self.sum -= self.recent.pop_front().unwrap_or(0.0);
        }
        let gain = self.sum / self.recent.len() as f32;
        self.delay.push_back(frame);
        if self.delay.len() >= self.n {
            let out = self.delay.pop_front().unwrap_or([0.0; 2]);
            [out[0] * gain, out[1] * gain]
        } else {
            [0.0; 2]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn run(l: &mut PeakLimiter, input: &[f32]) -> Vec<f32> {
        input.iter().map(|x| l.process([*x, *x])[0]).collect()
    }

    #[test]
    fn nothing_comes_out_louder_than_the_ceiling() {
        let mut l = PeakLimiter::new(SR, 0.89);
        // Quiet, then a loud burst (4x the ceiling), then quiet again.
        let mut x: Vec<f32> = (0..24_000).map(|i| 0.2 * (i as f32 * 0.05).sin()).collect();
        x.extend((0..4_800).map(|i| 3.5 * (i as f32 * 0.05).sin()));
        x.extend((0..24_000).map(|i| 0.2 * (i as f32 * 0.05).sin()));
        let y = run(&mut l, &x);
        let worst = y.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(worst <= 0.89 + 1e-4, "{worst}");
        // The loud burst really was turned down, not removed.
        let burst = &y[24_300..28_500];
        let peak = burst.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.8, "{peak}");
    }

    #[test]
    fn quiet_sound_passes_unchanged_apart_from_the_short_delay() {
        let mut l = PeakLimiter::new(SR, 0.89);
        let x: Vec<f32> = (0..10_000).map(|i| 0.3 * (i as f32 * 0.03).sin()).collect();
        let y = run(&mut l, &x);
        let delay = l.n - 1;
        for i in delay + 5..x.len() {
            assert!((y[i] - x[i - delay]).abs() < 1e-5, "frame {i}");
        }
        // Nothing is made louder.
        assert!(y.iter().all(|v| v.abs() <= 0.3 + 1e-6));
    }

    #[test]
    fn the_volume_comes_back_after_a_peak() {
        let mut l = PeakLimiter::new(SR, 0.5);
        let mut x = vec![2.0f32; 100];
        x.extend(vec![0.25f32; 48_000]);
        let y = run(&mut l, &x);
        // A quarter of a second later the quiet sound is back at its own level.
        let tail = y[y.len() - 100..].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((tail - 0.25).abs() < 0.01, "{tail}");
        // Right after the peak it was still turned down.
        assert!(y[400] < 0.25, "{}", y[400]);
    }

    #[test]
    fn the_volume_changes_smoothly_with_no_jumps() {
        let mut l = PeakLimiter::new(SR, 0.5);
        let mut x: Vec<f32> = vec![0.1; 2_000];
        x.extend(vec![3.0f32; 50]);
        x.extend(vec![0.1f32; 4_000]);
        let y = run(&mut l, &x);
        // Gain from the frames that were constant: the steps are small.
        let mut gains = Vec::new();
        for i in l.n..x.len() {
            let src = x[i - (l.n - 1)];
            if src.abs() > 1e-6 {
                gains.push(y[i] / src);
            }
        }
        let biggest_step = gains
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max);
        assert!(biggest_step < 0.1, "{biggest_step}");
    }
}
