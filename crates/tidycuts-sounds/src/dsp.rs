//! A small, dependency-free synthesis toolkit: seeded noise, band-limited
//! oscillators, envelopes, filters, physical models and time-based effects.
//!
//! Everything works on `f32` buffers at an arbitrary sample rate. Oscillator
//! phases and IIR filter states that need the headroom use `f64`.

use std::f32::consts::{PI, SQRT_2, TAU};
use std::f64::consts::TAU as TAU64;

// ------------------------------------------------------------------ helpers

/// MIDI note number to frequency in Hz (A4 = 69 = 440 Hz).
#[inline]
pub fn hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// Decibels to linear gain.
#[inline]
pub fn db(x: f32) -> f32 {
    10f32.powf(x / 20.0)
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Linear 0..1 ramp over `a` seconds.
#[inline]
pub fn ramp(t: f32, a: f32) -> f32 {
    if a <= 0.0 {
        if t >= 0.0 {
            1.0
        } else {
            0.0
        }
    } else {
        (t / a).clamp(0.0, 1.0)
    }
}

/// Exponential decay with time constant `tau` (zero before `t = 0`).
#[inline]
pub fn decay(t: f32, tau: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else {
        (-t / tau).exp()
    }
}

const EXP_M5: f32 = 0.006_737_947;

/// Exponential release that lands exactly on zero after five time constants.
#[inline]
pub fn release(t: f32, tau: f32) -> f32 {
    if t <= 0.0 {
        1.0
    } else {
        (((-t / tau).exp() - EXP_M5) / (1.0 - EXP_M5)).max(0.0)
    }
}

/// ADSR envelope as a pure function of time: linear attack `a`, exponential
/// decay (time constant `d`) to sustain `s`, exponential release (time
/// constant `r`, reaching zero at `gate + 5r`) after the note-on length `gate`.
pub fn adsr(t: f32, a: f32, d: f32, s: f32, r: f32, gate: f32) -> f32 {
    let held = |t: f32| {
        if t < a {
            t / a
        } else {
            s + (1.0 - s) * (-(t - a) / d).exp()
        }
    };
    if t < 0.0 {
        0.0
    } else if t < gate {
        held(t)
    } else {
        held(gate) * release(t - gate, r)
    }
}

/// Breakpoint envelope with geometric interpolation (for frequencies).
pub fn seg_exp(t: f32, pts: &[(f32, f32)]) -> f32 {
    if t <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            let x = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return v0 * (v1 / v0).powf(x);
        }
    }
    pts[pts.len() - 1].1
}

/// Soft clipper with unity small-signal gain.
#[inline]
pub fn sat(x: f32, drive: f32) -> f32 {
    (x * drive).tanh() / drive
}

/// Equal-power pan gains for `pan` in -1..=1 (centre = unity on both sides).
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos() * SQRT_2, a.sin() * SQRT_2)
}

/// 64-bit FNV-1a, used to derive per-sound seeds.
pub fn hash_str(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------- rng

/// SplitMix64: tiny, fast and fully deterministic.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn uni(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / 16_777_216.0)
    }
    /// Uniform in `[-1, 1)`.
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.uni() * 2.0 - 1.0
    }
    #[inline]
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.uni()
    }
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.uni() < p
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
    /// Exponentially distributed value with the given mean (Poisson gaps).
    pub fn exp(&mut self, mean: f32) -> f32 {
        -mean * (1.0 - self.uni()).max(1e-9).ln()
    }
    /// An independent generator derived from this one.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
}

// -------------------------------------------------------------------- noise

/// Pink noise (Paul Kellet's refined filter), roughly -1..1.
pub struct Pink {
    b: [f32; 7],
    rng: Rng,
}

impl Pink {
    pub fn new(rng: Rng) -> Self {
        Pink { b: [0.0; 7], rng }
    }
    #[inline]
    pub fn next(&mut self) -> f32 {
        let w = self.rng.bi();
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + w * 0.055_517_9;
        b[1] = 0.99332 * b[1] + w * 0.075_075_9;
        b[2] = 0.96900 * b[2] + w * 0.153_852;
        b[3] = 0.86650 * b[3] + w * 0.310_485_6;
        b[4] = 0.55000 * b[4] + w * 0.532_952_2;
        b[5] = -0.7616 * b[5] - w * 0.016_898;
        let out = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
        b[6] = w * 0.115_926;
        out * 0.18
    }
}

/// Brown (red) noise: leaky-integrated white noise.
pub struct Brown {
    y: f32,
    rng: Rng,
}

impl Brown {
    pub fn new(rng: Rng) -> Self {
        Brown { y: 0.0, rng }
    }
    #[inline]
    pub fn next(&mut self) -> f32 {
        self.y = (self.y + 0.02 * self.rng.bi()) * 0.998;
        self.y * 3.0
    }
}

/// Smoothly wandering random value in -1..1 (low-passed noise), for slow
/// natural modulation such as wind gusts.
pub struct Wander {
    rng: Rng,
    a: f32,
    y1: f32,
    y2: f32,
    target: f32,
    count: usize,
    every: usize,
}

impl Wander {
    pub fn new(rng: Rng, rate_hz: f32, sr: f32) -> Self {
        let every = (sr / (rate_hz * 4.0)).max(1.0) as usize;
        let a = 1.0 - (-TAU * rate_hz / sr).exp();
        Wander {
            rng,
            a,
            y1: 0.0,
            y2: 0.0,
            target: 0.0,
            count: 0,
            every,
        }
    }
    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.count == 0 {
            self.target = self.rng.bi();
            self.count = self.every;
        }
        self.count -= 1;
        self.y1 += self.a * (self.target - self.y1);
        self.y2 += self.a * (self.y1 - self.y2);
        (self.y2 * 1.8).clamp(-1.0, 1.0)
    }
}

// -------------------------------------------------------------- oscillators

#[inline]
fn blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// Phase-accumulator oscillator with polyBLEP band-limited saw and pulse.
#[derive(Clone, Copy, Default)]
pub struct Osc {
    pub ph: f64,
}

impl Osc {
    pub fn new(phase: f32) -> Self {
        Osc {
            ph: phase.rem_euclid(1.0) as f64,
        }
    }
    #[inline]
    fn step(&mut self, f: f32, sr: f32) -> (f64, f64) {
        let t = self.ph;
        let dt = (f.abs() as f64 / sr as f64).min(0.5);
        self.ph += dt;
        if self.ph >= 1.0 {
            self.ph -= 1.0;
        }
        (t, dt.max(1e-9))
    }
    #[inline]
    pub fn sine(&mut self, f: f32, sr: f32) -> f32 {
        let (t, _) = self.step(f, sr);
        (t * TAU64).sin() as f32
    }
    #[inline]
    pub fn saw(&mut self, f: f32, sr: f32) -> f32 {
        let (t, dt) = self.step(f, sr);
        (2.0 * t - 1.0 - blep(t, dt)) as f32
    }
    #[inline]
    pub fn pulse(&mut self, f: f32, pw: f32, sr: f32) -> f32 {
        let (t, dt) = self.step(f, sr);
        let pw = pw.clamp(0.05, 0.95) as f64;
        let mut v = if t < pw { 1.0 } else { -1.0 };
        v += blep(t, dt);
        v -= blep((t + 1.0 - pw) % 1.0, dt);
        v as f32
    }
    /// Triangle (its harmonics fall at 12 dB/oct, so aliasing is negligible).
    #[inline]
    pub fn tri(&mut self, f: f32, sr: f32) -> f32 {
        let (t, _) = self.step(f, sr);
        (4.0 * (t - 0.5).abs() - 1.0) as f32
    }
}

// ------------------------------------------------------------------ filters

/// One-pole low-pass (and complementary high-pass).
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    a: f32,
    y: f32,
}

impl OnePole {
    pub fn new(fc: f32, sr: f32) -> Self {
        let mut f = OnePole::default();
        f.set(fc, sr);
        f
    }
    pub fn set(&mut self, fc: f32, sr: f32) {
        self.a = 1.0 - (-TAU * fc.max(1.0) / sr).exp();
    }
    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        x - self.lp(x)
    }
}

/// Biquad response types (RBJ audio-EQ cookbook).
#[derive(Clone, Copy, Debug)]
pub enum Bq {
    Lp,
    Hp,
    /// Band-pass with 0 dB peak gain.
    Bp,
    Peak(f32),
    HighShelf(f32),
}

/// Transposed direct form II biquad with `f64` state.
#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    /// Build from normalised coefficients (a0 = 1).
    pub fn from_coeffs(b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) -> Self {
        Biquad {
            b0,
            b1,
            b2,
            a1,
            a2,
            z1: 0.0,
            z2: 0.0,
        }
    }
    pub fn new(kind: Bq, fc: f32, q: f32, sr: f32) -> Self {
        let mut b = Biquad::default();
        b.set(kind, fc, q, sr);
        b
    }
    /// Recompute coefficients (state is kept, so this can be swept).
    pub fn set(&mut self, kind: Bq, fc: f32, q: f32, sr: f32) {
        let sr = sr as f64;
        let fc = (fc as f64).clamp(5.0, sr * 0.49);
        let w0 = TAU64 * fc / sr;
        let (sn, cs) = w0.sin_cos();
        let alpha = sn / (2.0 * q.max(0.05) as f64);
        let shelf = |g: f32| {
            let a = 10f64.powf(g as f64 / 40.0);
            (a, 2.0 * a.sqrt() * alpha)
        };
        let (b0, b1, b2, a0, a1, a2) = match kind {
            Bq::Lp => (
                (1.0 - cs) / 2.0,
                1.0 - cs,
                (1.0 - cs) / 2.0,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            Bq::Hp => (
                (1.0 + cs) / 2.0,
                -(1.0 + cs),
                (1.0 + cs) / 2.0,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            Bq::Bp => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
            Bq::Peak(g) => {
                let a = 10f64.powf(g as f64 / 40.0);
                (
                    1.0 + alpha * a,
                    -2.0 * cs,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cs,
                    1.0 - alpha / a,
                )
            }
            Bq::HighShelf(g) => {
                let (a, sq) = shelf(g);
                (
                    a * ((a + 1.0) + (a - 1.0) * cs + sq),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                    a * ((a + 1.0) + (a - 1.0) * cs - sq),
                    (a + 1.0) - (a - 1.0) * cs + sq,
                    2.0 * ((a - 1.0) - (a + 1.0) * cs),
                    (a + 1.0) - (a - 1.0) * cs - sq,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }
    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y as f32
    }
    pub fn run(&mut self, buf: &mut [f32]) {
        for v in buf {
            *v = self.tick(*v);
        }
    }
}

/// Apply a static biquad to a buffer in place.
pub fn eq(buf: &mut [f32], kind: Bq, fc: f32, q: f32, sr: f32) {
    Biquad::new(kind, fc, q, sr).run(buf);
}

/// Topology-preserving state-variable filter (Simper); stays well behaved
/// under fast cutoff modulation, so it is the one used for sweeps.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Svf {
    pub fn new(fc: f32, q: f32, sr: f32) -> Self {
        let mut f = Svf::default();
        f.set(fc, q, sr);
        f
    }
    #[inline]
    pub fn set(&mut self, fc: f32, q: f32, sr: f32) {
        let g = (PI * fc.clamp(10.0, sr * 0.45) / sr).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// Returns (low-pass, band-pass normalised to 0 dB peak, high-pass).
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, self.k * v1, x - self.k * v1 - v2)
    }
    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.tick(x).0
    }
    #[inline]
    pub fn bp(&mut self, x: f32) -> f32 {
        self.tick(x).1
    }
    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        self.tick(x).2
    }
}

/// Which output of the state-variable filter a sweep uses.
#[derive(Clone, Copy)]
pub enum Mode {
    Lp,
    Bp,
    Hp,
}

/// Filter `buf` in place with a state-variable filter whose cutoff follows
/// `fc(t)` (updated every 16 samples).
pub fn sweep(buf: &mut [f32], sr: f32, mode: Mode, q: f32, fc: impl Fn(f32) -> f32) {
    let mut f = Svf::new(fc(0.0), q, sr);
    for (i, v) in buf.iter_mut().enumerate() {
        if i % 16 == 0 {
            f.set(fc(i as f32 / sr), q, sr);
        }
        let (lp, bp, hp) = f.tick(*v);
        *v = match mode {
            Mode::Lp => lp,
            Mode::Bp => bp,
            Mode::Hp => hp,
        };
    }
}

// ------------------------------------------------------------------ context

/// Everything a generator needs: sample rate, output length and a seeded RNG.
pub struct Ctx {
    pub sr: f32,
    pub n: usize,
    pub rng: Rng,
}

impl Ctx {
    /// Output length in seconds.
    pub fn secs(&self) -> f32 {
        self.n as f32 / self.sr
    }
}

// ------------------------------------------------------------ stereo buffer

/// A stereo buffer with non-interleaved channels.
#[derive(Clone, Debug, Default)]
pub struct Stereo {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Stereo {
    pub fn new(n: usize) -> Self {
        Stereo {
            l: vec![0.0; n],
            r: vec![0.0; n],
        }
    }
    pub fn len(&self) -> usize {
        self.l.len()
    }
    pub fn resize(&mut self, n: usize) {
        self.l.resize(n, 0.0);
        self.r.resize(n, 0.0);
    }
    /// Mix a mono signal in at frame `at` with gain and equal-power pan.
    pub fn add_mono(&mut self, src: &[f32], at: usize, gain: f32, pan: f32) {
        if at >= self.len() {
            return;
        }
        let (gl, gr) = pan_gains(pan);
        let (gl, gr) = (gl * gain, gr * gain);
        let end = (at + src.len()).min(self.len());
        for (i, s) in src[..end - at].iter().enumerate() {
            self.l[at + i] += s * gl;
            self.r[at + i] += s * gr;
        }
    }
    /// Mix another stereo buffer in at frame `at`.
    pub fn add(&mut self, src: &Stereo, at: usize, gain: f32) {
        if at >= self.len() {
            return;
        }
        let end = (at + src.len()).min(self.len());
        for i in 0..end - at {
            self.l[at + i] += src.l[i] * gain;
            self.r[at + i] += src.r[i] * gain;
        }
    }
    pub fn scale(&mut self, g: f32) {
        for v in self.l.iter_mut().chain(self.r.iter_mut()) {
            *v *= g;
        }
    }
    pub fn peak(&self) -> f32 {
        self.l
            .iter()
            .chain(self.r.iter())
            .fold(0.0f32, |m, v| m.max(v.abs()))
    }
    /// Apply the same static biquad to both channels.
    pub fn eq(&mut self, kind: Bq, fc: f32, q: f32, sr: f32) {
        eq(&mut self.l, kind, fc, q, sr);
        eq(&mut self.r, kind, fc, q, sr);
    }
    /// Swept state-variable filter on both channels.
    pub fn sweep(&mut self, sr: f32, mode: Mode, q: f32, fc: impl Fn(f32) -> f32) {
        sweep(&mut self.l, sr, mode, q, &fc);
        sweep(&mut self.r, sr, mode, q, &fc);
    }
    /// Mid/side width: 0 = mono, 1 = unchanged, >1 = wider.
    pub fn width(&mut self, w: f32) {
        for i in 0..self.len() {
            let m = (self.l[i] + self.r[i]) * 0.5;
            let s = (self.l[i] - self.r[i]) * 0.5 * w;
            self.l[i] = m + s;
            self.r[i] = m - s;
        }
    }
    /// Multiply by a gain envelope given per frame time.
    pub fn gain_env(&mut self, sr: f32, g: impl Fn(f32) -> f32) {
        for i in 0..self.len() {
            let v = g(i as f32 / sr);
            self.l[i] *= v;
            self.r[i] *= v;
        }
    }
    pub fn map(&mut self, f: impl Fn(f32) -> f32) {
        for v in self.l.iter_mut().chain(self.r.iter_mut()) {
            *v = f(*v);
        }
    }
    pub fn reversed(&self) -> Stereo {
        let mut s = self.clone();
        s.l.reverse();
        s.r.reverse();
        s
    }
    pub fn fade_in(&mut self, n: usize) {
        fade_in(&mut self.l, n);
        fade_in(&mut self.r, n);
    }
    pub fn fade_out(&mut self, n: usize) {
        fade_out(&mut self.l, n);
        fade_out(&mut self.r, n);
    }
}

/// Raised-cosine fade-in over the first `n` samples.
pub fn fade_in(v: &mut [f32], n: usize) {
    let n = n.min(v.len());
    for (i, s) in v[..n].iter_mut().enumerate() {
        *s *= 0.5 - 0.5 * (PI * i as f32 / n as f32).cos();
    }
}

/// Raised-cosine fade-out over the last `n` samples (last sample is zero).
pub fn fade_out(v: &mut [f32], n: usize) {
    let len = v.len();
    let n = n.min(len);
    for i in 0..n {
        let x = (i as f32 + 1.0) / n as f32;
        v[len - n + i] *= 0.5 + 0.5 * (PI * x).cos();
    }
}

/// Render a mono signal sample by sample from a function of time.
pub fn gen(sr: f32, n: usize, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..n).map(|i| f(i as f32 / sr)).collect()
}

/// Number of frames for a duration.
#[inline]
pub fn frames(secs: f32, sr: f32) -> usize {
    (secs.max(0.0) * sr).round() as usize
}

// ---------------------------------------------------------- physical models

/// A bank of exponentially decaying sinusoids `(freq, amp, tau)`, i.e. modal
/// synthesis of struck objects. Uses rotating phasors, so it is cheap.
pub fn modes(sr: f32, n: usize, list: &[(f32, f32, f32)], attack: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    for &(f, a, tau) in list {
        if f <= 0.0 || f >= sr * 0.47 || a == 0.0 {
            continue;
        }
        let w = TAU64 * f as f64 / sr as f64;
        let (sw, cw) = w.sin_cos();
        let r = (-1.0 / (tau as f64 * sr as f64)).exp();
        let (mut re, mut im) = (a as f64, 0.0f64);
        let len = ((tau * 9.2 * sr) as usize).min(n);
        for (i, o) in out[..len].iter_mut().enumerate() {
            *o += im as f32;
            let nre = (re * cw - im * sw) * r;
            im = (re * sw + im * cw) * r;
            re = nre;
            if i & 1023 == 1023 {
                // keep the phasor's magnitude from drifting
                let mag = (re * re + im * im).sqrt();
                let want = a as f64 * r.powi(i as i32 + 1);
                if mag > 0.0 {
                    re *= want / mag;
                    im *= want / mag;
                }
            }
        }
    }
    let na = (attack * sr) as usize;
    for (i, v) in out.iter_mut().take(na).enumerate() {
        *v *= i as f32 / na as f32;
    }
    out
}

/// Parameters for the Karplus–Strong plucked string.
#[derive(Clone, Copy)]
pub struct Pluck {
    /// Time for the fundamental to fall by 60 dB while held.
    pub decay: f32,
    /// Loop low-pass weight (0.05 bright/metallic .. 0.5 dark/classic).
    pub damp: f32,
    /// Excitation brightness 0..1.
    pub bright: f32,
    /// Pluck position along the string, 0..0.5.
    pub pick: f32,
    /// Decay time after note-off (finger damping).
    pub mute: f32,
}

/// Extended Karplus–Strong: noise-burst excitation (low-passed and comb
/// filtered for pluck position), averaging loop filter, allpass fine tuning.
pub fn karplus(sr: f32, freq: f32, n: usize, gate: usize, p: &Pluck, rng: &mut Rng) -> Vec<f32> {
    let period = sr / freq.max(20.0);
    let s = p.damp.clamp(0.02, 0.5);
    let total = period - s;
    let nint = ((total - 0.2).floor() as usize).max(2);
    let frac = total - nint as f32;
    let c = (1.0 - frac) / (1.0 + frac);

    // Excitation: one period of zero-mean noise, low-passed and comb filtered.
    let mut exc: Vec<f32> = (0..nint).map(|_| rng.bi()).collect();
    let mean = exc.iter().sum::<f32>() / nint as f32;
    let mut lp = OnePole::new(400.0 + p.bright * p.bright * 14000.0, sr);
    for v in exc.iter_mut() {
        *v = lp.lp(*v - mean);
    }
    let pick = ((p.pick * period).round() as usize).clamp(1, nint - 1);
    let orig = exc.clone();
    for i in pick..nint {
        exc[i] -= orig[i - pick];
    }
    let peak = exc.iter().fold(1e-6f32, |m, v| m.max(v.abs()));
    let mut line: Vec<f32> = exc.iter().map(|v| v / peak).collect();

    let rho_on = 10f32.powf(-3.0 / (p.decay.max(0.05) * freq));
    let rho_off = 10f32.powf(-3.0 / (p.mute.max(0.01) * freq));
    let mut out = vec![0.0f32; n];
    let (mut prev, mut ax, mut ay) = (0.0f32, 0.0f32, 0.0f32);
    let mut idx = 0;
    for (i, o) in out.iter_mut().enumerate() {
        let y = line[idx];
        let f = (1.0 - s) * y + s * prev;
        prev = y;
        let a = c * f + ax - c * ay;
        ax = f;
        ay = a;
        line[idx] = a * if i < gate { rho_on } else { rho_off };
        idx += 1;
        if idx == nint {
            idx = 0;
        }
        *o = y;
    }
    out
}

// ------------------------------------------------------------------ effects

struct Comb {
    buf: Vec<f32>,
    i: usize,
    store: f32,
}

impl Comb {
    #[inline]
    fn tick(&mut self, x: f32, fb: f32, damp: f32) -> f32 {
        let out = self.buf[self.i];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.i] = x + self.store * fb;
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    i: usize,
}

impl Allpass {
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i += 1;
        if self.i == self.buf.len() {
            self.i = 0;
        }
        b - x
    }
}

/// Freeverb-style reverb settings.
#[derive(Clone, Copy)]
pub struct Verb {
    /// Decay, 0..1.
    pub room: f32,
    /// High-frequency damping, 0..1.
    pub damp: f32,
    /// Stereo width, 0..1.
    pub width: f32,
    /// Pre-delay in seconds.
    pub pre: f32,
    /// Input high-pass / low-pass (keeps the tail clean).
    pub hp: f32,
    pub lp: f32,
    /// Scales the delay-line lengths (smaller = smaller room).
    pub size: f32,
}

impl Default for Verb {
    fn default() -> Self {
        Verb {
            room: 0.75,
            damp: 0.5,
            width: 1.0,
            pre: 0.02,
            hp: 180.0,
            lp: 9000.0,
            size: 1.0,
        }
    }
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];

/// Schroeder/Freeverb reverb: 8 damped combs into 4 allpasses per channel.
/// Returns only the wet signal.
pub fn reverb(x: &Stereo, sr: f32, p: &Verb) -> Stereo {
    let scale = sr / 44100.0 * p.size;
    let len = |t: usize| ((t as f32 * scale) as usize).max(4);
    let mk = |off: usize| {
        let combs: Vec<Comb> = COMBS
            .iter()
            .map(|&t| Comb {
                buf: vec![0.0; len(t + off)],
                i: 0,
                store: 0.0,
            })
            .collect();
        let aps: Vec<Allpass> = ALLPASSES
            .iter()
            .map(|&t| Allpass {
                buf: vec![0.0; len(t + off)],
                i: 0,
            })
            .collect();
        (combs, aps)
    };
    let (mut cl, mut al) = mk(0);
    let (mut cr, mut ar) = mk(23);
    let fb = p.room.clamp(0.0, 1.0) * 0.28 + 0.7;
    let damp = p.damp.clamp(0.0, 1.0) * 0.4;
    let wet1 = 3.0 * (p.width / 2.0 + 0.5);
    let wet2 = 3.0 * ((1.0 - p.width) / 2.0);
    let pre = frames(p.pre, sr);
    let mut hp = Biquad::new(Bq::Hp, p.hp, 0.7, sr);
    let mut lp = Biquad::new(Bq::Lp, p.lp, 0.7, sr);
    let n = x.len();
    let mut out = Stereo::new(n);
    for i in 0..n {
        let dry = if i >= pre {
            x.l[i - pre] + x.r[i - pre]
        } else {
            0.0
        };
        let inp = lp.tick(hp.tick(dry)) * 0.015 + 1e-20;
        let mut ol = 0.0;
        let mut or = 0.0;
        for c in cl.iter_mut() {
            ol += c.tick(inp, fb, damp);
        }
        for c in cr.iter_mut() {
            or += c.tick(inp, fb, damp);
        }
        for a in al.iter_mut() {
            ol = a.tick(ol);
        }
        for a in ar.iter_mut() {
            or = a.tick(or);
        }
        out.l[i] = ol * wet1 + or * wet2;
        out.r[i] = or * wet1 + ol * wet2;
    }
    out
}

/// Feedback delay with a low-pass in the loop; `pingpong` bounces the
/// repeats between the channels. Returns only the wet signal.
pub fn echo(x: &Stereo, sr: f32, time: f32, fb: f32, lp: f32, pingpong: bool) -> Stereo {
    let d = frames(time, sr).max(1);
    let n = x.len();
    let mut bl = vec![0.0f32; d];
    let mut br = vec![0.0f32; d];
    let mut fl = OnePole::new(lp, sr);
    let mut fr = OnePole::new(lp, sr);
    let mut out = Stereo::new(n);
    let mut k = 0;
    for i in 0..n {
        let dl = bl[k];
        let dr = br[k];
        let (l, r) = (fl.lp(dl), fr.lp(dr));
        if pingpong {
            bl[k] = (x.l[i] + x.r[i]) * 0.5 + r * fb;
            br[k] = l * fb;
        } else {
            bl[k] = x.l[i] + l * fb;
            br[k] = x.r[i] + r * fb;
        }
        out.l[i] = dl;
        out.r[i] = dr;
        k += 1;
        if k == d {
            k = 0;
        }
    }
    out
}

/// Read `x` through a time-varying delay `d(t)` seconds (linear interpolation).
pub fn mod_delay(x: &[f32], sr: f32, d: impl Fn(f32) -> f32) -> Vec<f32> {
    let n = x.len();
    let mut y = vec![0.0f32; n];
    for (i, o) in y.iter_mut().enumerate() {
        let pos = i as f32 - d(i as f32 / sr) * sr;
        if pos < 0.0 {
            continue;
        }
        let i0 = pos as usize;
        if i0 >= n {
            continue;
        }
        let fr = pos - i0 as f32;
        let b = if i0 + 1 < n { x[i0 + 1] } else { 0.0 };
        *o = x[i0] + (b - x[i0]) * fr;
    }
    y
}

/// Stereo chorus: one modulated delay per channel, LFOs in quadrature.
pub fn chorus(x: &mut Stereo, sr: f32, base: f32, depth: f32, rate: f32, mix: f32) {
    let wl = mod_delay(&x.l, sr, |t| base + depth * (TAU * rate * t).sin());
    let wr = mod_delay(&x.r, sr, |t| {
        base + depth * (TAU * rate * t + PI / 2.0).sin()
    });
    for i in 0..x.len() {
        x.l[i] = x.l[i] * (1.0 - mix) + wl[i] * mix;
        x.r[i] = x.r[i] * (1.0 - mix) + wr[i] * mix;
    }
}

/// Sidechain-style ducking: dip the gain by `depth` at each hit time and
/// recover with time constant `rel`.
pub fn duck(x: &mut Stereo, sr: f32, hits: &[f32], depth: f32, rel: f32) {
    let n = x.len();
    let mut env = vec![0.0f32; n];
    let att = frames(0.004, sr).max(1);
    let tail = frames(rel * 6.0, sr);
    for &h in hits {
        let c = frames(h, sr);
        let start = c.saturating_sub(att);
        for (j, e) in env
            .iter_mut()
            .enumerate()
            .take((c + tail).min(n))
            .skip(start)
        {
            let v = if j < c {
                (j - start) as f32 / att as f32
            } else {
                ((c as f32 - j as f32) / (rel * sr)).exp()
            };
            *e = e.max(v);
        }
    }
    for (i, e) in env.iter().enumerate() {
        let g = 1.0 - depth * e;
        x.l[i] *= g;
        x.r[i] *= g;
    }
}
