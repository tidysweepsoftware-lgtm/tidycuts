//! Synthesised drum voices. Each returns a mono hit starting at sample 0.

use crate::dsp::*;

/// Kick drum settings.
#[derive(Clone, Copy)]
pub struct Kick {
    /// Pitch at the start of the sweep and where it settles (Hz).
    pub f_hi: f32,
    pub f_lo: f32,
    /// Time constant of the pitch sweep.
    pub sweep: f32,
    /// Amplitude decay time constant.
    pub decay: f32,
    /// Level of the beater click.
    pub click: f32,
    /// Saturation drive.
    pub drive: f32,
}

impl Kick {
    pub const PUNCHY: Kick = Kick {
        f_hi: 170.0,
        f_lo: 52.0,
        sweep: 0.032,
        decay: 0.26,
        click: 0.35,
        drive: 1.6,
    };
    pub const DEEP: Kick = Kick {
        f_hi: 130.0,
        f_lo: 44.0,
        sweep: 0.05,
        decay: 0.45,
        click: 0.15,
        drive: 1.3,
    };
    pub const SOFT: Kick = Kick {
        f_hi: 115.0,
        f_lo: 52.0,
        sweep: 0.03,
        decay: 0.2,
        click: 0.08,
        drive: 1.2,
    };
}

pub fn kick(sr: f32, rng: &mut Rng, k: &Kick) -> Vec<f32> {
    let n = frames(k.decay * 6.0 + 0.03, sr);
    let mut ph = 0.0f64;
    let mut click_f = Svf::new(3500.0, 0.8, sr);
    let mut out = gen(sr, n, |t| {
        let f = k.f_lo + (k.f_hi - k.f_lo) * (-t / k.sweep).exp();
        ph = (ph + f as f64 / sr as f64).fract();
        let body = (ph * std::f64::consts::TAU).sin() as f32;
        // a touch of hold before the decay gives the kick its weight
        let amp = ramp(t, 0.0015)
            * if t < 0.02 {
                1.0
            } else {
                decay(t - 0.02, k.decay)
            };
        let click = click_f.bp(rng.bi()) * decay(t, 0.004) * k.click;
        sat(body * amp + click, k.drive)
    });
    fade_out(&mut out, frames(0.01, sr));
    out
}

/// Snare: two-mode drum head plus band-limited noise for the wires.
pub fn snare(sr: f32, rng: &mut Rng, tone: f32, snappy: f32, len: f32) -> Vec<f32> {
    let n = frames(len * 5.0 + 0.02, sr);
    let mut hp = Biquad::new(Bq::Hp, 1400.0, 0.7, sr);
    let mut lp = Biquad::new(Bq::Lp, 9500.0, 0.7, sr);
    let (mut a, mut b) = (Osc::default(), Osc::default());
    let mut out = gen(sr, n, |t| {
        let drop = 1.0 + 0.25 * decay(t, 0.012);
        let head =
            (a.sine(tone * drop, sr) + 0.6 * b.sine(tone * 1.62 * drop, sr)) * decay(t, 0.055);
        let wires = lp.tick(hp.tick(rng.bi())) * (decay(t, len) * 0.85 + decay(t, 0.012) * 0.6);
        ramp(t, 0.0008) * (head * 0.75 + wires * snappy * 1.6)
    });
    fade_out(&mut out, frames(0.01, sr));
    out
}

/// Hand clap: a few quick noise bursts followed by a short diffuse tail.
pub fn clap(sr: f32, rng: &mut Rng, len: f32) -> Vec<f32> {
    let n = frames(len * 5.0 + 0.05, sr);
    let mut bp = Biquad::new(Bq::Bp, 1150.0, 1.1, sr);
    let mut hp = Biquad::new(Bq::Hp, 500.0, 0.7, sr);
    let offs = [0.0, 0.0085, 0.0175, 0.029];
    let mut out = gen(sr, n, |t| {
        let mut e = 0.0f32;
        for (k, &o) in offs.iter().enumerate() {
            if t >= o {
                let g = if k == 3 { 1.0 } else { 0.8 };
                e = e.max(g * ramp(t - o, 0.0007) * decay(t - o, 0.0045));
            }
        }
        if t >= offs[3] {
            e = e.max(0.55 * decay(t - offs[3], len));
        }
        hp.tick(bp.tick(rng.bi())) * e * 2.2
    });
    fade_out(&mut out, frames(0.01, sr));
    out
}

const HAT_FREQS: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

/// Metallic noise from six detuned square waves (the classic 808 recipe),
/// plus white noise.
fn metal(sr: f32, rng: &mut Rng, n: usize, pitch: f32, noise: f32) -> Vec<f32> {
    let mut oscs: Vec<Osc> = HAT_FREQS.iter().map(|_| Osc::new(rng.uni())).collect();
    gen(sr, n, |_| {
        let mut s = 0.0;
        for (o, f) in oscs.iter_mut().zip(HAT_FREQS) {
            s += o.pulse(f * pitch, 0.5, sr);
        }
        s / 6.0 + rng.bi() * noise
    })
}

/// Hi-hat; `len` is the decay time constant (0.04 closed .. 0.35 open).
pub fn hat(sr: f32, rng: &mut Rng, len: f32) -> Vec<f32> {
    let n = frames(len * 6.0 + 0.01, sr);
    let mut m = metal(sr, rng, n, 1.6, 0.5);
    eq(&mut m, Bq::Bp, 9500.0, 0.9, sr);
    eq(&mut m, Bq::Hp, 6500.0, 0.7, sr);
    eq(&mut m, Bq::Hp, 6500.0, 0.7, sr);
    for (i, v) in m.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *v *= ramp(t, 0.0006) * (decay(t, len) * 0.8 + decay(t, 0.006) * 0.4) * 2.5;
    }
    fade_out(&mut m, frames(0.01, sr));
    m
}

/// Crash cymbal: dense noise plus ring-modulated metallic partials, split
/// into a faster-decaying body band and a long high "sizzle" band.
pub fn cymbal(sr: f32, rng: &mut Rng, len: f32, bright: f32) -> Vec<f32> {
    let n = frames(len * 4.0 + 0.05, sr);
    let a = metal(sr, rng, n, 2.3, 0.0);
    let b = metal(sr, rng, n, 3.17, 0.0);
    let src: Vec<f32> = a
        .iter()
        .zip(&b)
        .map(|(x, y)| rng.bi() * 0.9 + x * y * 1.6 + x * 0.12)
        .collect();
    let mut body = src.clone();
    eq(&mut body, Bq::Bp, 4200.0 + bright * 1200.0, 0.7, sr);
    let mut sizzle = src.clone();
    let hp = 6500.0 + bright * 1500.0;
    eq(&mut sizzle, Bq::Hp, hp, 0.7, sr);
    eq(&mut sizzle, Bq::Hp, hp, 0.7, sr);
    eq(&mut sizzle, Bq::Lp, 15000.0, 0.7, sr);
    let mut crack = src;
    eq(&mut crack, Bq::Hp, 1500.0, 0.7, sr);
    let mut shimmer = Wander::new(rng.fork(), 5.0, sr);
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let s = 1.0 + 0.1 * shimmer.next();
            let v = body[i] * decay(t, len * 0.45) * 0.8
                + sizzle[i] * decay(t, len) * s
                + crack[i] * decay(t, 0.012) * 0.6;
            v * ramp(t, 0.001) * 1.4
        })
        .collect();
    fade_out(&mut out, frames(0.05, sr));
    out
}

/// Tom: a pitched membrane with a quick downward bend.
pub fn tom(sr: f32, rng: &mut Rng, f: f32, len: f32) -> Vec<f32> {
    let n = frames(len * 6.0 + 0.02, sr);
    let mut o = Osc::default();
    let mut o2 = Osc::default();
    let mut lp = Svf::new(1800.0, 0.7, sr);
    let mut out = gen(sr, n, |t| {
        let ff = f * (1.0 + 0.45 * decay(t, 0.04));
        let body = o.sine(ff, sr) + 0.25 * o2.sine(ff * 1.5, sr) * decay(t, 0.06);
        let stick = lp.lp(rng.bi()) * decay(t, 0.008) * 0.6;
        ramp(t, 0.001) * (body * decay(t, len) + stick)
    });
    fade_out(&mut out, frames(0.01, sr));
    out
}

/// Shaker: a soft, band-passed noise grain.
pub fn shaker(sr: f32, rng: &mut Rng, len: f32) -> Vec<f32> {
    let n = frames(len * 6.0 + 0.03, sr);
    let mut bp = Biquad::new(Bq::Bp, 7200.0, 1.3, sr);
    let mut out = gen(sr, n, |t| {
        let e = ramp(t, 0.012)
            * if t < 0.012 {
                1.0
            } else {
                decay(t - 0.012, len)
            };
        bp.tick(rng.bi()) * e * 1.5
    });
    fade_out(&mut out, frames(0.005, sr));
    out
}

/// Rim click / finger snap style tick.
pub fn rim(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let n = frames(0.12, sr);
    let mut out = modes(
        sr,
        n,
        &[
            (1650.0, 0.8, 0.012),
            (3300.0, 0.4, 0.008),
            (520.0, 0.5, 0.02),
        ],
        0.0004,
    );
    let mut hp = Svf::new(2500.0, 0.7, sr);
    for (i, v) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *v += hp.hp(rng.bi()) * decay(t, 0.003) * 0.6;
    }
    fade_out(&mut out, frames(0.01, sr));
    out
}

/// Big, low "taiko" drum for cinematic tracks.
pub fn taiko(sr: f32, rng: &mut Rng, f: f32) -> Vec<f32> {
    let n = frames(1.4, sr);
    let mut o = Osc::default();
    let mut lp = Svf::new(900.0, 0.8, sr);
    let mut out = gen(sr, n, |t| {
        let ff = f * (1.0 + 0.6 * decay(t, 0.03));
        let body = o.sine(ff, sr) * decay(t, 0.28);
        let skin = lp.lp(rng.bi()) * decay(t, 0.05) * 0.9;
        sat(ramp(t, 0.002) * (body + skin), 1.5)
    });
    fade_out(&mut out, frames(0.02, sr));
    out
}
