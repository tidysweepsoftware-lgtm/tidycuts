//! Lining up clips by their sound: two cameras (or a camera and a recorder)
//! that heard the same thing are shifted until that sound happens at the
//! same moment on the timeline.
//!
//! The offset comes from GCC-PHAT, a cross-correlation that keeps only the
//! phase of the two sounds. That makes the peak sharp and keeps it
//! reliable when the microphones differ or the room echoes.

use crate::detect::decode_mono;
use std::path::Path;
use tidycuts_media::Tools;

/// Samples a second used for the comparison.
const RATE: u32 = 4000;
/// Longest stretch of each clip that is compared.
const MAX_SECS: f64 = 150.0;

/// How the second sound sits against the first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    /// Seconds: an event `t` seconds into `y` happens `t + lag` seconds into `x`.
    pub lag: f64,
    /// Peak height against the noise around it; above about 5 is a safe match.
    pub confidence: f64,
}

/// The lag of `y` against `x` (both mono at `rate`), within `max_secs` either way.
pub fn lag_between(x: &[f32], y: &[f32], rate: u32, max_secs: f64) -> Option<Offset> {
    if x.len() < 256 || y.len() < 256 {
        return None;
    }
    let n = (x.len() + y.len()).next_power_of_two();
    let spectrum = |s: &[f32]| {
        let mut re = vec![0f32; n];
        let mut im = vec![0f32; n];
        // Take the average away so a steady hum doesn't dominate.
        let mean = s.iter().sum::<f32>() / s.len() as f32;
        for (i, v) in s.iter().enumerate() {
            re[i] = v - mean;
        }
        crate::levels::fft(&mut re, &mut im);
        (re, im)
    };
    let (xr, xi) = spectrum(x);
    let (yr, yi) = spectrum(y);
    // X * conj(Y), with every bin brought to the same size (the PHAT weight).
    let mut re = vec![0f32; n];
    let mut im = vec![0f32; n];
    for k in 0..n {
        let (a, b) = (xr[k], xi[k]);
        let (c, d) = (yr[k], -yi[k]);
        let (pr, pi) = (a * c - b * d, a * d + b * c);
        let mag = (pr * pr + pi * pi).sqrt() + 1e-9;
        re[k] = pr / mag;
        im[k] = -pi / mag; // conjugated, so a forward FFT acts as the inverse
    }
    crate::levels::fft(&mut re, &mut im);
    // The result is the correlation: index j holds lag j (negative lags wrap round).
    let at = |lag: i64| -> f32 { re[lag.rem_euclid(n as i64) as usize] / n as f32 };
    let max_lag = ((max_secs * f64::from(rate)) as i64).min(n as i64 / 2 - 1);
    let (mut best, mut best_lag) = (f32::MIN, 0i64);
    for lag in -max_lag..=max_lag {
        let v = at(lag);
        if v > best {
            best = v;
            best_lag = lag;
        }
    }
    // Noise level: the typical size of the correlation away from the peak.
    let guard = (f64::from(rate) * 0.05) as i64;
    let (mut sum, mut count) = (0f64, 0u64);
    for lag in (-max_lag..=max_lag).step_by(7) {
        if (lag - best_lag).abs() > guard {
            sum += f64::from(at(lag).abs());
            count += 1;
        }
    }
    let floor = (sum / count.max(1) as f64).max(1e-9);
    Some(Offset {
        lag: best_lag as f64 / f64::from(rate),
        confidence: f64::from(best) / floor,
    })
}

/// How the sound of `other` (a stretch of a file) sits against `reference`'s.
pub fn offset_between(
    tools: &Tools,
    reference: (&Path, (f64, f64)),
    other: (&Path, (f64, f64)),
    max_secs: f64,
) -> Result<Offset, String> {
    let cap = |r: (f64, f64)| (r.0, r.1.min(r.0 + MAX_SECS));
    let x = decode_mono(tools, reference.0, cap(reference.1), RATE)?;
    let y = decode_mono(tools, other.0, cap(other.1), RATE)?;
    if x.is_empty() || y.is_empty() {
        return Err("One of the clips has no sound to line up.".into());
    }
    lag_between(&x, &y, RATE, max_secs).ok_or_else(|| "The sounds are too short to compare.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproducible noise with a rhythm, like speech or claps.
    fn signal(len: usize, seed: u32) -> Vec<f32> {
        let mut state = seed;
        let mut rnd = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        };
        (0..len)
            .map(|i| {
                let gate = if (i / 700) % 3 == 0 { 1.0 } else { 0.25 };
                rnd() * gate
            })
            .collect()
    }

    #[test]
    fn a_known_shift_is_found_either_way() {
        let rate = 4000;
        let long = signal(rate as usize * 30, 7);
        for shift in [3.25f64, -2.5, 0.0, 12.0] {
            let cut = (shift.abs() * f64::from(rate)) as usize;
            // y holds the same sound as x, started `shift` seconds later or earlier.
            let (x, y): (Vec<f32>, Vec<f32>) = if shift >= 0.0 {
                (long.clone(), long[cut..].to_vec())
            } else {
                (long[cut..].to_vec(), long.clone())
            };
            let found = lag_between(&x, &y, rate, 30.0).unwrap();
            assert!(
                (found.lag - shift).abs() < 0.002,
                "shift {shift}: found {found:?}"
            );
            assert!(found.confidence > 5.0, "shift {shift}: {found:?}");
        }
    }

    #[test]
    fn a_noisy_copy_still_matches_and_unrelated_sound_does_not() {
        let rate = 4000;
        let a = signal(rate as usize * 20, 3);
        let noise = signal(a.len(), 99);
        let y: Vec<f32> = a[8000..]
            .iter()
            .zip(&noise)
            .map(|(s, n)| s * 0.6 + n * 0.5)
            .collect();
        let found = lag_between(&a, &y, rate, 30.0).unwrap();
        assert!((found.lag - 2.0).abs() < 0.005, "{found:?}");
        assert!(found.confidence > 5.0, "{found:?}");
        let other = signal(a.len(), 12345);
        let none = lag_between(&a, &other, rate, 30.0).unwrap();
        assert!(
            none.confidence < found.confidence / 2.0,
            "{none:?} vs {found:?}"
        );
    }

    #[test]
    fn too_little_sound_gives_no_answer() {
        assert!(lag_between(&[0.0; 10], &[0.0; 1000], 4000, 5.0).is_none());
    }
}
