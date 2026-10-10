//! Final processing shared by every sound: subsonic filter, level
//! normalisation (peak or EBU R128 loudness), a look-ahead peak limiter,
//! click-free edges and 16-bit WAV encoding.

use std::collections::VecDeque;

use crate::dsp::{db, frames, Biquad, Bq, Rng, Stereo};

/// Never let a sample get above this (-1.5 dBFS), leaving room for
/// inter-sample peaks and the test bound of 0.9.
pub const CEILING: f32 = 0.84;

/// How a sound's level is set.
#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// One-shot effect: bring the loudest 400 ms window (momentary
    /// loudness) to this many LUFS, but never let the peak exceed -3 dBFS.
    Fx(f32),
    /// Normalise integrated loudness to this many LUFS (then peak-limit).
    Lufs(f32),
}

/// How the start and end of the file are shaped.
#[derive(Clone, Copy, Debug)]
pub enum Edges {
    /// One-shot effect: 2 ms in, 15 ms out.
    Shot,
    /// Loopable ambience: short symmetric fades.
    Loop,
    /// Music bed: 10 ms in, 2.5 s out.
    Music,
}

/// Stage 1 of BS.1770 K-weighting (high shelf) and stage 2 (high-pass),
/// computed for any sample rate as libebur128 does.
fn k_filters(sr: f32) -> (Biquad, Biquad) {
    let sr = sr as f64;
    let mk = |b0: f64, b1: f64, b2: f64, a1: f64, a2: f64| Biquad::from_coeffs(b0, b1, b2, a1, a2);
    let f0 = 1_681.974_450_955_533;
    let g = 3.999_843_853_973_347;
    let q = 0.707_175_236_955_419_6;
    let k = (std::f64::consts::PI * f0 / sr).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = mk(
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    );
    let f0 = 38.135_470_876_024_44;
    let q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * f0 / sr).tan();
    let a0 = 1.0 + k / q + k * k;
    let hp = mk(
        1.0,
        -2.0,
        1.0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    );
    (shelf, hp)
}

/// Per-sample K-weighted power (left² + right²) as prefix sums.
fn k_power_prefix(st: &Stereo, sr: f32) -> Vec<f64> {
    let (s1, h1) = k_filters(sr);
    let (mut sl, mut hl, mut sr_, mut hr) = (s1, h1, s1, h1);
    let mut acc = 0.0f64;
    let mut pre = Vec::with_capacity(st.len() + 1);
    pre.push(0.0);
    for i in 0..st.len() {
        let l = hl.tick(sl.tick(st.l[i])) as f64;
        let r = hr.tick(sr_.tick(st.r[i])) as f64;
        acc += l * l + r * r;
        pre.push(acc);
    }
    pre
}

fn to_lufs(ms: f64) -> f32 {
    if ms <= 0.0 {
        f32::NEG_INFINITY
    } else {
        (-0.691 + 10.0 * ms.log10()) as f32
    }
}

/// Mean-square power of each 400 ms block (100 ms hop).
fn blocks(pre: &[f64], sr: f32) -> Vec<f64> {
    let n = pre.len() - 1;
    let block = frames(0.4, sr);
    let hop = frames(0.1, sr).max(1);
    if n < block {
        return vec![pre[n] / n.max(1) as f64];
    }
    (0..=(n - block) / hop)
        .map(|j| {
            let a = j * hop;
            (pre[a + block] - pre[a]) / block as f64
        })
        .collect()
}

/// Gated integrated loudness (EBU R128 / BS.1770-4) in LUFS.
pub fn integrated_lufs(st: &Stereo, sr: f32) -> f32 {
    let z = blocks(&k_power_prefix(st, sr), sr);
    let abs: Vec<f64> = z.iter().copied().filter(|&p| to_lufs(p) > -70.0).collect();
    if abs.is_empty() {
        return f32::NEG_INFINITY;
    }
    let rel = to_lufs(abs.iter().sum::<f64>() / abs.len() as f64) - 10.0;
    let gated: Vec<f64> = abs.into_iter().filter(|&p| to_lufs(p) > rel).collect();
    to_lufs(gated.iter().sum::<f64>() / gated.len().max(1) as f64)
}

/// Loudest 400 ms momentary loudness in LUFS (shorter sounds are measured
/// as if padded with silence to 400 ms).
pub fn max_momentary(st: &Stereo, sr: f32) -> f32 {
    let pre = k_power_prefix(st, sr);
    let n = pre.len() - 1;
    let block = frames(0.4, sr);
    if n < block {
        return to_lufs(pre[n] / block as f64);
    }
    to_lufs(blocks(&pre, sr).into_iter().fold(0.0, f64::max))
}

fn sliding_min(x: &[f32], r: usize) -> Vec<f32> {
    let n = x.len();
    let mut out = vec![1.0f32; n];
    let mut dq: VecDeque<usize> = VecDeque::new();
    let mut next = 0;
    for (i, o) in out.iter_mut().enumerate() {
        let hi = (i + r).min(n - 1);
        while next <= hi {
            while dq.back().is_some_and(|&b| x[b] >= x[next]) {
                dq.pop_back();
            }
            dq.push_back(next);
            next += 1;
        }
        let lo = i.saturating_sub(r);
        while dq.front().is_some_and(|&f| f < lo) {
            dq.pop_front();
        }
        *o = x[dq[0]];
    }
    out
}

/// Transparent look-ahead peak limiter. The gain curve is the windowed
/// minimum of the required gain, smoothed by a box filter of the same width
/// (so it is never above the required gain at any sample) with an 80 ms
/// release. Returns the deepest gain reduction in dB.
pub fn limit(st: &mut Stereo, sr: f32, ceiling: f32) -> f32 {
    let n = st.len();
    if n == 0 {
        return 0.0;
    }
    let req: Vec<f32> = (0..n)
        .map(|i| {
            let p = st.l[i].abs().max(st.r[i].abs());
            if p > ceiling {
                ceiling / p
            } else {
                1.0
            }
        })
        .collect();
    if req.iter().all(|&g| g >= 1.0) {
        return 0.0;
    }
    let r = frames(0.0015, sr).max(1);
    let m = sliding_min(&req, r);
    let mut pre = Vec::with_capacity(n + 1);
    pre.push(0.0f64);
    for v in &m {
        pre.push(pre[pre.len() - 1] + *v as f64);
    }
    let rel = (-1.0 / (0.08 * sr)).exp();
    let mut prev = 1.0f32;
    let mut deepest = 1.0f32;
    for i in 0..n {
        let lo = i.saturating_sub(r);
        let hi = (i + r + 1).min(n);
        let target = ((pre[hi] - pre[lo]) / (hi - lo) as f64) as f32;
        let g = if target < prev {
            target
        } else {
            target + (prev - target) * rel
        };
        prev = g;
        deepest = deepest.min(g);
        st.l[i] = (st.l[i] * g).clamp(-ceiling, ceiling);
        st.r[i] = (st.r[i] * g).clamp(-ceiling, ceiling);
    }
    -20.0 * deepest.log10()
}

/// Level, limit and fade a rendered sound; returns interleaved stereo.
pub fn finish(mut st: Stereo, sr: f32, level: Level, edges: Edges) -> Vec<f32> {
    // Remove DC and subsonic rumble that would only eat headroom.
    st.eq(Bq::Hp, 18.0, 0.707, sr);
    let gain = match level {
        Level::Fx(target) => {
            let p = st.peak().max(1e-9);
            let m = max_momentary(&st, sr);
            let by_peak = db(-3.0) / p;
            if m.is_finite() {
                by_peak.min(db(target - m))
            } else {
                by_peak
            }
        }
        Level::Lufs(target) => {
            let l = integrated_lufs(&st, sr);
            if l.is_finite() {
                db(target - l)
            } else {
                1.0
            }
        }
    };
    st.scale(gain);
    limit(&mut st, sr, CEILING);
    let (fi, fo) = match edges {
        Edges::Shot => (0.002, 0.015),
        Edges::Loop => (0.006, 0.006),
        Edges::Music => (0.01, 2.5),
    };
    st.fade_in(frames(fi, sr));
    st.fade_out(frames(fo, sr));
    let mut out = Vec::with_capacity(st.len() * 2);
    for i in 0..st.len() {
        out.push(st.l[i]);
        out.push(st.r[i]);
    }
    out
}

/// Encode interleaved stereo samples as a 16-bit PCM WAV file with
/// deterministic TPDF dither.
pub fn wav_bytes(samples: &[f32], sample_rate: u32, seed: u64) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&2u16.to_le_bytes()); // channels
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 4).to_le_bytes()); // byte rate
    b.extend_from_slice(&4u16.to_le_bytes()); // block align
    b.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    let mut rng = Rng::new(seed);
    for &s in samples {
        let dither = (rng.uni() - rng.uni()) / 32767.0;
        let v = ((s.clamp(-1.0, 1.0) + dither) * 32767.0).round();
        b.extend_from_slice(&(v.clamp(-32768.0, 32767.0) as i16).to_le_bytes());
    }
    b
}
