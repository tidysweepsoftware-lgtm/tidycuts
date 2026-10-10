//! Melodic instruments for the music beds. Unless noted, each renders one
//! mono note starting at sample 0; `gate` is how long the key is held (s).

use std::f32::consts::{PI, TAU};
use std::f64::consts::TAU as TAU64;

use crate::dsp::*;

/// Multiply a note by a hold/release gate so it ends cleanly.
fn gate_env(v: &mut [f32], sr: f32, gate: f32, rel: f32) {
    for (i, x) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        if t > gate {
            *x *= release(t - gate, rel);
        }
    }
}

/// FM electric piano: a ratio-1 operator pair for the warm body plus a
/// ratio-14 pair with a very short envelope for the tine's bell-like attack.
pub fn epiano(sr: f32, f: f32, vel: f32, gate: f32) -> Vec<f32> {
    let rel = 0.12;
    let tau = (1.7 * (220.0 / f).powf(0.45)).clamp(0.5, 3.0);
    let n = frames(gate + rel * 5.0 + 0.01, sr);
    let df = f as f64 / sr as f64;
    // fade the tine out for high notes so its sidebands never alias
    let tine = (1.0 - f * 14.0 * 3.0 / (sr * 0.5)).clamp(0.0, 1.0);
    let (mut pc, mut pm, mut pt) = (0.0f64, 0.0f64, 0.0f64);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let env = ramp(t, 0.0015) * (-t / tau).exp();
        let i1 = vel * (0.25 + 1.2 * (-t / 0.4).exp());
        let i2 = vel * vel * 1.8 * (-t / 0.015).exp() * tine;
        let car = (pc * TAU64) as f32;
        let body = (car + i1 * (pm * TAU64).sin() as f32).sin();
        let ding = (car + i2 * (pt * TAU64).sin() as f32).sin();
        out.push(env * (body * 0.7 + ding * 0.3) * (0.35 + 0.65 * vel));
        pc = (pc + df).fract();
        pm = (pm + df).fract();
        pt = (pt + df * 14.0).fract();
    }
    gate_env(&mut out, sr, gate, rel);
    out
}

/// Settings for [`pad`].
#[derive(Clone, Copy)]
pub struct Pad {
    pub attack: f32,
    pub release: f32,
    pub cutoff: f32,
    pub q: f32,
    /// Detune spread across the unison voices, in semitones.
    pub detune: f32,
    pub voices: usize,
    /// Filter LFO rate (Hz).
    pub lfo: f32,
    /// Delayed vibrato depth (fraction of pitch).
    pub vib: f32,
    pub hp: f32,
}

impl Pad {
    pub const WARM: Pad = Pad {
        attack: 0.6,
        release: 0.5,
        cutoff: 1800.0,
        q: 0.7,
        detune: 0.12,
        voices: 3,
        lfo: 0.15,
        vib: 0.0,
        hp: 150.0,
    };
    pub const STRINGS: Pad = Pad {
        attack: 0.5,
        release: 0.45,
        cutoff: 2600.0,
        q: 0.6,
        detune: 0.08,
        voices: 3,
        lfo: 0.2,
        vib: 0.003,
        hp: 180.0,
    };
}

/// Unison detuned-saw chord through a slowly moving low-pass. Stereo.
pub fn pad(sr: f32, rng: &mut Rng, notes: &[f32], gate: f32, p: &Pad) -> Stereo {
    let n = frames(gate + p.release * 5.0 + 0.01, sr);
    let mut vs: Vec<(Osc, f32, f32, f32)> = Vec::new();
    for (k, &note) in notes.iter().enumerate() {
        for j in 0..p.voices {
            let x = if p.voices > 1 {
                j as f32 / (p.voices - 1) as f32 * 2.0 - 1.0
            } else {
                0.0
            };
            let f = hz(note + x * p.detune + rng.bi() * 0.015);
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let (gl, gr) = pan_gains((x * 0.8 + side * 0.15).clamp(-1.0, 1.0));
            vs.push((Osc::new(rng.uni()), f, gl, gr));
        }
    }
    let norm = 0.5 / (vs.len() as f32).sqrt();
    let lfo_ph = rng.uni();
    let vib_ph = rng.uni();
    let mut fl = Svf::new(p.cutoff, p.q, sr);
    let mut fr = fl;
    let mut out = Stereo::new(n);
    for i in 0..n {
        let t = i as f32 / sr;
        if i % 32 == 0 {
            let fc = p.cutoff * (1.0 + 0.3 * (TAU * (p.lfo * t + lfo_ph)).sin());
            fl.set(fc, p.q, sr);
            fr.set(fc * 1.04, p.q, sr);
        }
        let vib = 1.0 + p.vib * (TAU * (5.0 * t + vib_ph)).sin() * ramp(t - 0.3, 0.6);
        let (mut l, mut r) = (0.0, 0.0);
        for (o, f, gl, gr) in vs.iter_mut() {
            let v = o.saw(*f * vib, sr);
            l += v * *gl;
            r += v * *gr;
        }
        let a = adsr(t, p.attack, 1.0, 1.0, p.release, gate) * norm;
        out.l[i] = fl.lp(l) * a;
        out.r[i] = fr.lp(r) * a;
    }
    out.eq(Bq::Hp, p.hp, 0.7, sr);
    out
}

/// Additive piano: slightly inharmonic partials, two detuned strings per
/// low partial (natural beating), two-stage decay, hammer thump, damper.
pub fn piano(sr: f32, f: f32, vel: f32, gate: f32, rng: &mut Rng) -> Vec<f32> {
    let damper = 0.09;
    let n = frames(gate + damper * 5.0 + 0.01, sr);
    let base_tau = (3.0 * (261.6 / f).powf(0.6)).clamp(0.6, 7.0);
    let b = 0.00015 + 0.0002 * (f / 500.0);
    let slope = 2.0 - 0.9 * vel;
    let mut out = vec![0.0f32; n];
    for k in 1..=18usize {
        let kf = k as f32;
        let fk = kf * f * (1.0 + b * kf * kf).sqrt();
        if fk > sr * 0.42 {
            break;
        }
        let hammer = 0.3 + (PI * kf * 0.12).sin().abs();
        let amp = hammer / kf.powf(slope);
        if amp < 0.003 {
            continue;
        }
        let tau = base_tau / (1.0 + 0.5 * (kf - 1.0));
        let r1 = (-1.0 / (tau as f64 * 0.2 * sr as f64)).exp();
        let r2 = (-1.0 / (tau as f64 * sr as f64)).exp();
        let strings: &[f32] = if k <= 6 { &[-1.0, 1.0] } else { &[0.0] };
        for &st in strings {
            let det = 1.0 + st * 0.0004 * (1.0 + 0.15 * kf);
            let w = TAU64 * (fk * det) as f64 / sr as f64;
            let (sw, cw) = w.sin_cos();
            let (mut re, mut im) = (1.0f64, 0.0f64);
            let share = amp as f64 / strings.len() as f64;
            let (mut a1, mut a2) = (0.55 * share, 0.45 * share);
            for (i, o) in out.iter_mut().enumerate() {
                *o += (im * (a1 + a2)) as f32;
                let nre = re * cw - im * sw;
                im = re * sw + im * cw;
                re = nre;
                a1 *= r1;
                a2 *= r2;
                if i & 1023 == 1023 {
                    let m = (re * re + im * im).sqrt();
                    re /= m;
                    im /= m;
                    if a1 + a2 < 1e-5 {
                        break;
                    }
                }
            }
        }
    }
    let mut lp = OnePole::new(600.0 + 2500.0 * vel, sr);
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let thump = if t < 0.04 {
            lp.lp(rng.bi()) * decay(t, 0.004) * 0.12 * vel
        } else {
            0.0
        };
        *o = (*o + thump) * ramp(t, 0.001) * (0.25 + 0.75 * vel);
    }
    gate_env(&mut out, sr, gate, damper);
    out
}

/// Glockenspiel / music-box bar.
pub fn glock(sr: f32, f: f32, vel: f32, tau: f32) -> Vec<f32> {
    let mut v = modes(
        sr,
        frames(tau * 5.0 + 0.02, sr),
        &[
            (f, 1.0, tau),
            (f * 2.76, 0.3 * vel, tau * 0.3),
            (f * 5.4, 0.12 * vel, tau * 0.1),
            (f * 8.93, 0.05 * vel, tau * 0.05),
        ],
        0.0008,
    );
    for x in v.iter_mut() {
        *x *= vel;
    }
    fade_out(&mut v, frames(0.01, sr));
    v
}

/// Two-operator FM bell (inharmonic ratio 3.5).
pub fn fm_bell(sr: f32, f: f32, vel: f32, tau: f32) -> Vec<f32> {
    let n = frames(tau * 5.0 + 0.02, sr);
    let (mut pc, mut pm) = (0.0f64, 0.0f64);
    let df = f as f64 / sr as f64;
    let mut v = gen(sr, n, |t| {
        let idx = vel * 2.0 * decay(t, tau * 0.35) + 0.15;
        let y = ((pc * TAU64) as f32 + idx * (pm * TAU64).sin() as f32).sin();
        pc = (pc + df).fract();
        pm = (pm + df * 3.5).fract();
        y * ramp(t, 0.002) * decay(t, tau) * vel
    });
    fade_out(&mut v, frames(0.01, sr));
    v
}

/// Karplus–Strong guitar / ukulele string.
pub fn string(
    sr: f32,
    f: f32,
    vel: f32,
    gate: f32,
    bright: f32,
    sustain: f32,
    rng: &mut Rng,
) -> Vec<f32> {
    let n = frames(gate + 0.35, sr);
    let p = Pluck {
        decay: sustain * (220.0 / f).powf(0.3),
        damp: 0.5 - 0.35 * bright,
        bright: (0.25 + 0.65 * vel * bright).min(1.0),
        pick: 0.13,
        mute: 0.07,
    };
    let mut v = karplus(sr, f, n, frames(gate, sr), &p, rng);
    for x in v.iter_mut() {
        *x *= 0.3 + 0.7 * vel;
    }
    fade_in(&mut v, 4);
    fade_out(&mut v, frames(0.02, sr));
    v
}

/// Round sine bass with a little second and third harmonic.
pub fn sub_bass(sr: f32, f: f32, vel: f32, gate: f32) -> Vec<f32> {
    let mut ph = 0.0f64;
    gen(sr, frames(gate + 0.3, sr), |t| {
        let p = (ph * TAU64) as f32;
        ph = (ph + f as f64 / sr as f64).fract();
        let x = p.sin() + 0.22 * (2.0 * p).sin() + 0.08 * (3.0 * p).sin();
        x * adsr(t, 0.008, 0.6, 0.75, 0.05, gate) * vel
    })
}

/// Plucked "upright" style bass: sine/triangle body with a finger thump.
pub fn round_bass(sr: f32, f: f32, vel: f32, gate: f32, rng: &mut Rng) -> Vec<f32> {
    let mut o = Osc::default();
    let mut s = Osc::default();
    let mut lp = OnePole::new(250.0, sr);
    let mut v = gen(sr, frames(gate + 0.3, sr), |t| {
        let x = o.tri(f, sr) * 0.5 + s.sine(f, sr) * 0.7;
        let thump = lp.lp(rng.bi()) * decay(t, 0.012) * 0.6;
        (x * ramp(t, 0.004) * decay(t, 0.9) + thump) * vel
    });
    gate_env(&mut v, sr, gate, 0.04);
    v
}

/// Analog-style bass: saw + square through an enveloped low-pass, plus sub.
pub fn synth_bass(sr: f32, f: f32, vel: f32, gate: f32, cutoff: f32, env: f32, q: f32) -> Vec<f32> {
    let (mut a, mut b, mut s) = (Osc::default(), Osc::new(0.5), Osc::default());
    let mut lp = Svf::new(cutoff, q, sr);
    gen(sr, frames(gate + 0.25, sr), |t| {
        lp.set(cutoff * (1.0 + env * vel * decay(t, 0.09)), q, sr);
        let x = a.saw(f, sr) * 0.55 + b.pulse(f, 0.5, sr) * 0.35;
        let y = lp.lp(x) + s.sine(f, sr) * 0.45;
        y * adsr(t, 0.003, 0.25, 0.8, 0.03, gate) * vel
    })
}

/// Clavinet-like funk keys: narrow pulse with a snappy resonant filter.
pub fn clav(sr: f32, f: f32, vel: f32, gate: f32) -> Vec<f32> {
    let (mut a, mut b) = (Osc::default(), Osc::new(0.3));
    let mut hp = OnePole::new(180.0, sr);
    let mut lp = Svf::new(1000.0, 1.8, sr);
    gen(sr, frames(gate + 0.15, sr), |t| {
        lp.set(450.0 + 5000.0 * vel * decay(t, 0.035), 1.8, sr);
        let x = a.pulse(f, 0.22, sr) * 0.7 + b.pulse(f * 2.0, 0.4, sr) * 0.25;
        lp.lp(hp.hp(x)) * adsr(t, 0.001, 0.12, 0.5, 0.02, gate) * vel
    })
}

/// Brass section stab: detuned saws with a scooped pitch and a filter swell.
pub fn brass(sr: f32, f: f32, vel: f32, gate: f32, rng: &mut Rng) -> Vec<f32> {
    let mut oscs: Vec<(Osc, f32)> = (0..3)
        .map(|j| (Osc::new(rng.uni()), 1.0 + (j as f32 - 1.0) * 0.006))
        .collect();
    let mut lp = Svf::new(500.0, 0.9, sr);
    gen(sr, frames(gate + 0.3, sr), |t| {
        let scoop = 1.0 - 0.03 * decay(t, 0.03);
        let open = adsr(t, 0.035, 0.18, 0.5, 0.05, gate);
        lp.set(380.0 + 3200.0 * vel * open, 0.9, sr);
        let mut x = 0.0;
        for (o, d) in oscs.iter_mut() {
            x += o.saw(f * *d * scoop, sr);
        }
        lp.lp(x / 3.0) * adsr(t, 0.015, 0.25, 0.75, 0.06, gate) * vel
    })
}

/// Bright synth lead: two detuned saws, square sub, delayed vibrato.
pub fn saw_lead(sr: f32, f: f32, vel: f32, gate: f32) -> Vec<f32> {
    let (mut a, mut b, mut c) = (Osc::default(), Osc::new(0.37), Osc::new(0.1));
    let mut lp = Svf::new(3000.0, 0.8, sr);
    gen(sr, frames(gate + 0.4, sr), |t| {
        let vib = 1.0 + 0.006 * (TAU * 5.5 * t).sin() * ramp(t - 0.2, 0.3);
        let ff = f * vib;
        lp.set(1800.0 + 2500.0 * decay(t, 0.15) * vel, 0.8, sr);
        let x = a.saw(ff * 1.004, sr) * 0.4
            + b.saw(ff * 0.996, sr) * 0.4
            + c.pulse(ff * 0.5, 0.5, sr) * 0.2;
        lp.lp(x) * adsr(t, 0.01, 0.3, 0.8, 0.07, gate) * vel
    })
}

/// Short square pluck for arpeggios.
pub fn square_pluck(sr: f32, f: f32, vel: f32, gate: f32) -> Vec<f32> {
    let mut a = Osc::default();
    let mut lp = Svf::new(2000.0, 1.0, sr);
    gen(sr, frames(gate + 0.2, sr), |t| {
        lp.set(700.0 + 4000.0 * decay(t, 0.05) * vel, 1.0, sr);
        lp.lp(a.pulse(f, 0.5, sr)) * adsr(t, 0.002, 0.12, 0.35, 0.03, gate) * vel
    })
}

/// Pop-style synth pluck (detuned saws, fast filter envelope).
pub fn pluck_synth(sr: f32, f: f32, vel: f32, gate: f32, bright: f32) -> Vec<f32> {
    let (mut a, mut b) = (Osc::default(), Osc::new(0.5));
    let mut lp = Svf::new(2000.0, 0.9, sr);
    gen(sr, frames(gate + 0.25, sr), |t| {
        lp.set(350.0 + bright * 5500.0 * vel * decay(t, 0.08), 0.9, sr);
        let x = a.saw(f * 1.003, sr) * 0.5 + b.saw(f * 0.997, sr) * 0.5;
        lp.lp(x)
            * ramp(t, 0.002)
            * decay(t, 0.35)
            * if t > gate {
                release(t - gate, 0.04)
            } else {
                1.0
            }
            * vel
    })
}

/// Whistled melody: sine with a pitch scoop, delayed vibrato and breath.
pub fn whistle(sr: f32, f: f32, vel: f32, gate: f32, rng: &mut Rng) -> Vec<f32> {
    let mut ph = 0.0f64;
    let mut bp = Svf::new(f, 8.0, sr);
    gen(sr, frames(gate + 0.3, sr), |t| {
        let scoop = 2f32.powf(-0.5 / 12.0 * decay(t, 0.03));
        let vib = 1.0 + 0.005 * (TAU * 5.5 * t).sin() * ramp(t - 0.15, 0.25);
        let ff = f * scoop * vib;
        let p = (ph * TAU64) as f32;
        ph = (ph + ff as f64 / sr as f64).fract();
        let breath = bp.bp(rng.bi()) * 0.12;
        (p.sin() + 0.04 * (2.0 * p).sin() + breath) * adsr(t, 0.04, 0.2, 0.85, 0.05, gate) * vel
    })
}

/// Short, punchy string-section staccato for ostinatos.
pub fn staccato(sr: f32, f: f32, vel: f32, gate: f32, rng: &mut Rng) -> Vec<f32> {
    let mut oscs: Vec<(Osc, f32)> = (0..3)
        .map(|j| (Osc::new(rng.uni()), 1.0 + (j as f32 - 1.0) * 0.004))
        .collect();
    let mut lp = Svf::new(2000.0, 0.7, sr);
    gen(sr, frames(gate + 0.2, sr), |t| {
        lp.set(900.0 + 2400.0 * vel * decay(t, 0.06), 0.7, sr);
        let mut x = 0.0;
        for (o, d) in oscs.iter_mut() {
            x += o.saw(f * *d, sr);
        }
        lp.lp(x / 3.0) * adsr(t, 0.004, 0.06, 0.35, 0.02, gate) * vel
    })
}

/// Cinematic "braam": a stack of low detuned brass saws with a filter
/// swell and grit. Stereo; `notes` are MIDI numbers.
pub fn braam(sr: f32, rng: &mut Rng, notes: &[f32], gate: f32) -> Stereo {
    let n = frames(gate + 1.2, sr);
    let mut voices: Vec<(Osc, f32, f32, f32)> = Vec::new();
    for &note in notes {
        for j in 0..5 {
            let det = (j as f32 - 2.0) * 0.08;
            let (gl, gr) = pan_gains((j as f32 - 2.0) / 2.0 * 0.7);
            voices.push((Osc::new(rng.uni()), hz(note + det), gl, gr));
        }
    }
    let norm = 0.35 / (voices.len() as f32).sqrt();
    let mut fl = Svf::new(200.0, 1.2, sr);
    let mut fr = fl;
    let mut sub = Osc::default();
    let sub_f = hz(notes[0]);
    let mut s = Stereo::new(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let scoop = 1.0 - 0.035 * decay(t, 0.06);
        if i % 16 == 0 {
            let fc = seg_exp(
                t,
                &[
                    (0.0, 150.0),
                    (0.12, 2600.0),
                    (0.6, 950.0),
                    (gate + 1.2, 260.0),
                ],
            );
            fl.set(fc, 1.2, sr);
            fr.set(fc, 1.2, sr);
        }
        let (mut l, mut r) = (0.0, 0.0);
        for (o, f, gl, gr) in voices.iter_mut() {
            let v = o.saw(*f * scoop, sr);
            l += v * *gl;
            r += v * *gr;
        }
        let a = ramp(t, 0.04) * if t < 0.25 { 1.0 } else { decay(t - 0.25, 1.4) };
        let a = a * if t > gate {
            release(t - gate, 0.2)
        } else {
            1.0
        };
        let sb = sub.sine(sub_f * scoop, sr) * 0.5;
        s.l[i] = sat((fl.lp(l) * norm * 3.0 + sb) * a, 2.2);
        s.r[i] = sat((fr.lp(r) * norm * 3.0 + sb) * a, 2.2);
    }
    s
}
