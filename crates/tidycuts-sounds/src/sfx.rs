//! Sound effect generators. Each fills a stereo buffer of `c.n` frames.

use std::f32::consts::{PI, TAU};

use crate::drums::{self, Kick};
use crate::dsp::*;
use crate::inst;

/// Pre-roll before the first transient so the 2 ms edge fade never touches it.
const T0: f32 = 0.005;

// ------------------------------------------------------------------ helpers

fn room(room: f32, size: f32) -> Verb {
    Verb {
        room,
        size,
        damp: 0.5,
        width: 1.0,
        pre: 0.008,
        hp: 220.0,
        lp: 8000.0,
    }
}

fn add_verb(x: &mut Stereo, sr: f32, v: Verb, amt: f32) {
    let w = reverb(x, sr, &v);
    x.add(&w, 0, amt);
}

/// Rises smoothly to 1 at `pk` (fraction of the length), then falls to 0 at 1.
fn swell(x: f32, pk: f32, fall: f32) -> f32 {
    if x <= 0.0 || x >= 1.0 {
        0.0
    } else if x < pk {
        smoothstep(x / pk).powf(1.4)
    } else {
        (1.0 - (x - pk) / (1.0 - pk)).powf(fall)
    }
}

/// Stereo noise through a swept state-variable filter with amplitude and pan
/// envelopes. `white` blends pink (0) to white (1) noise.
#[allow(clippy::too_many_arguments)]
fn air(
    c: &mut Ctx,
    mode: Mode,
    q: f32,
    white: f32,
    fc: impl Fn(f32) -> f32,
    amp: impl Fn(f32) -> f32,
    pan: impl Fn(f32) -> f32,
) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut pl = Pink::new(c.rng.fork());
    let mut pr = Pink::new(c.rng.fork());
    let mut pc = Pink::new(c.rng.fork());
    let mut w = c.rng.fork();
    let mut fl = Svf::new(fc(0.0), q, sr);
    let mut fr = fl;
    let mut out = Stereo::new(n);
    let pick = |s: (f32, f32, f32)| match mode {
        Mode::Lp => s.0,
        Mode::Bp => s.1,
        Mode::Hp => s.2,
    };
    for i in 0..n {
        let t = i as f32 / sr;
        if i % 8 == 0 {
            let f = fc(t);
            fl.set(f, q, sr);
            fr.set(f, q, sr);
        }
        let a = amp(t);
        let common = pc.next() * (1.0 - white) + w.bi() * white * 0.6;
        let xl = common * 0.7 + (pl.next() * (1.0 - white) + w.bi() * white * 0.6) * 0.7;
        let xr = common * 0.7 + (pr.next() * (1.0 - white) + w.bi() * white * 0.6) * 0.7;
        let (gl, gr) = pan_gains(pan(t));
        out.l[i] = pick(fl.tick(xl)) * a * gl;
        out.r[i] = pick(fr.tick(xr)) * a * gr;
    }
    out
}

/// A struck object: decaying modes plus a short filtered noise transient.
fn strike(
    c: &mut Ctx,
    len: f32,
    list: &[(f32, f32, f32)],
    noise: f32,
    nfc: f32,
    ntau: f32,
) -> Vec<f32> {
    let sr = c.sr;
    let mut v = modes(sr, frames(len, sr), list, 0.0004);
    let mut f = Svf::new(nfc, 0.8, sr);
    let stop = frames(ntau * 10.0, sr).min(v.len());
    for (i, s) in v[..stop].iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s += f.bp(c.rng.bi()) * noise * ramp(t, 0.0003) * decay(t, ntau);
    }
    fade_out(&mut v, frames((len * 0.12).clamp(0.004, 0.08), sr));
    v
}

/// A one-shot buffer rendered from a function of time, with a short fade at
/// its end so a not-quite-finished decay never stops with a click.
fn shot(sr: f32, len: f32, f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let mut v = gen(sr, frames(len, sr), f);
    fade_out(&mut v, frames((len * 0.1).clamp(0.005, 0.06), sr));
    v
}

/// Simple voiced tone with time-varying pitch and amplitude.
fn tone(
    sr: f32,
    len: f32,
    f: impl Fn(f32) -> f32,
    amp: impl Fn(f32) -> f32,
    harm: &[f32],
) -> Vec<f32> {
    let mut ph = 0.0f64;
    shot(sr, len, |t| {
        ph = (ph + f(t) as f64 / sr as f64).fract();
        let p = (ph * std::f64::consts::TAU) as f32;
        let mut s = p.sin();
        for (k, h) in harm.iter().enumerate() {
            s += h * (p * (k as f32 + 2.0)).sin();
        }
        s * amp(t)
    })
}

/// Render `len` frames, then crossfade the overhang into the start so the
/// result loops seamlessly.
fn make_loop(c: &mut Ctx, overlap: f32, render: impl FnOnce(&mut Ctx, usize) -> Stereo) -> Stereo {
    let n = c.n;
    let ov = frames(overlap, c.sr).min(n / 2);
    // generators inside see the longer length through `c.n` too
    c.n = n + ov;
    let mut long = render(c, n + ov);
    c.n = n;
    long.resize(n + ov);
    let mut out = Stereo::new(n);
    out.l.copy_from_slice(&long.l[..n]);
    out.r.copy_from_slice(&long.r[..n]);
    for i in 0..ov {
        let x = i as f32 / ov as f32 * PI / 2.0;
        let (a, b) = (x.sin(), x.cos());
        out.l[i] = long.l[i] * a + long.l[n + i] * b;
        out.r[i] = long.r[i] * a + long.r[n + i] * b;
    }
    out
}

fn mono(v: &[f32], n: usize, at: usize) -> Stereo {
    let mut s = Stereo::new(n);
    s.add_mono(v, at, 1.0, 0.0);
    s
}

// --------------------------------------------------- whooshes & transitions

pub fn soft_whoosh(c: &mut Ctx) -> Stereo {
    let d = c.secs() - 0.2;
    let sr = c.sr;
    let fc = |t: f32| seg_exp(t / d, &[(0.0, 260.0), (0.55, 1500.0), (1.0, 450.0)]);
    let env = |t: f32| swell(t / d, 0.55, 2.2);
    let pan = |t: f32| lerp(-0.7, 0.7, smoothstep(t / d));
    let mut s = air(c, Mode::Bp, 1.1, 0.15, fc, env, pan);
    let body = air(c, Mode::Lp, 0.7, 0.0, |t| fc(t) * 0.35, env, pan);
    s.add(&body, 0, 0.25);
    s.eq(Bq::Hp, 90.0, 0.7, sr);
    add_verb(&mut s, sr, room(0.6, 0.8), 0.18);
    s
}

pub fn fast_whoosh(c: &mut Ctx) -> Stereo {
    let d = c.secs() - 0.12;
    let sr = c.sr;
    let fc = |t: f32| seg_exp(t / d, &[(0.0, 450.0), (0.5, 3200.0), (1.0, 800.0)]);
    let env = |t: f32| swell(t / d, 0.5, 2.6);
    let pan = |t: f32| lerp(-0.85, 0.85, smoothstep(t / d));
    let mut s = air(c, Mode::Bp, 1.5, 0.4, fc, env, pan);
    let body = air(c, Mode::Lp, 0.7, 0.0, |t| fc(t) * 0.3, env, pan);
    s.add(&body, 0, 0.25);
    s.eq(Bq::Hp, 90.0, 0.7, sr);
    add_verb(&mut s, sr, room(0.5, 0.6), 0.12);
    s
}

pub fn swoosh_up(c: &mut Ctx) -> Stereo {
    let d = c.secs() - 0.12;
    let sr = c.sr;
    let fc = |t: f32| 280.0 * (7000.0f32 / 280.0).powf((t / d).clamp(0.0, 1.0).powf(1.3));
    let env = |t: f32| swell(t / d, 0.82, 1.2);
    let pan = |t: f32| lerp(-0.3, 0.3, t / d);
    let mut s = air(c, Mode::Bp, 1.3, 0.3, fc, env, pan);
    // a resonant, slightly pitched layer that sells the upward motion
    let res = air(c, Mode::Bp, 7.0, 0.3, |t| fc(t) * 0.9, env, pan);
    s.add(&res, 0, 0.8);
    add_verb(&mut s, sr, room(0.6, 0.8), 0.15);
    s
}

pub fn swoosh_down(c: &mut Ctx) -> Stereo {
    let d = c.secs() - 0.15;
    let sr = c.sr;
    let fc = |t: f32| 7000.0 * (260.0f32 / 7000.0).powf((t / d).clamp(0.0, 1.0).powf(0.8));
    let env = |t: f32| swell(t / d, 0.18, 1.6);
    let pan = |t: f32| lerp(0.3, -0.3, t / d);
    let mut s = air(c, Mode::Bp, 1.3, 0.3, fc, env, pan);
    let res = air(c, Mode::Bp, 7.0, 0.3, |t| fc(t) * 1.1, env, pan);
    s.add(&res, 0, 0.8);
    add_verb(&mut s, sr, room(0.6, 0.8), 0.15);
    s
}

pub fn swish(c: &mut Ctx) -> Stereo {
    let d = 0.3;
    let sr = c.sr;
    let fc = |t: f32| seg_exp(t / d, &[(0.0, 1600.0), (0.4, 5600.0), (1.0, 2800.0)]);
    let env = |t: f32| swell(t / d, 0.38, 3.0);
    let mut s = air(c, Mode::Bp, 2.0, 0.7, fc, env, |t| lerp(0.45, -0.45, t / d));
    add_verb(&mut s, sr, room(0.4, 0.5), 0.1);
    s
}

pub fn riser(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let rise = 2.72;
    let up = |t: f32| (t / rise).clamp(0.0, 1.0);
    let env = |t: f32| {
        if t < rise {
            up(t).powf(1.8)
        } else {
            release(t - rise, 0.035)
        }
    };
    let mut s = air(
        c,
        Mode::Bp,
        1.4,
        0.8,
        |t| 250.0 * 36f32.powf(up(t)),
        env,
        |_| 0.0,
    );
    s.scale(0.7);
    // Supersaw chord sweeping up two octaves with an opening filter.
    let base = hz(45.0);
    let mut voices: Vec<(Osc, f32, f32)> = Vec::new();
    for (k, semi) in [0.0f32, 7.0, 12.0].iter().enumerate() {
        for j in 0..5 {
            let det = (j as f32 - 2.0) * 0.07;
            let pan = (j as f32 - 2.0) / 2.0 * 0.8 * if k % 2 == 0 { 1.0 } else { -1.0 };
            voices.push((Osc::new(c.rng.uni()), hz(45.0 + semi + det) / base, pan));
        }
    }
    let mut fl = Svf::new(300.0, 1.2, sr);
    let mut fr = fl;
    let mut trem = 0.0f32;
    for i in 0..n {
        let t = i as f32 / sr;
        let x = up(t);
        let f0 = base * 2f32.powf(2.0 * x.powf(1.5));
        if i % 16 == 0 {
            let fc = (300.0 * 2f32.powf(5.3 * x)).min(15000.0);
            fl.set(fc, 1.3, sr);
            fr.set(fc, 1.3, sr);
        }
        let (mut l, mut r) = (0.0, 0.0);
        for (o, ratio, pan) in voices.iter_mut() {
            let v = o.saw(f0 * *ratio, sr);
            let (gl, gr) = pan_gains(*pan);
            l += v * gl;
            r += v * gr;
        }
        trem += (3.0 + 20.0 * x * x) / sr;
        let depth = 0.45 * x;
        let tr = 1.0 - depth * (0.5 + 0.5 * (TAU * trem).sin());
        let a = env(t) * tr * 0.09;
        s.l[i] += fl.lp(l) * a;
        s.r[i] += fr.lp(r) * a;
    }
    add_verb(&mut s, sr, room(0.85, 1.1), 0.3);
    s
}

pub fn downlifter(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let d = 2.3;
    let fall = |t: f32| (1.0 - (t - T0) / d).clamp(0.0, 1.0);
    let mut s = air(
        c,
        Mode::Bp,
        1.0,
        0.5,
        |t| seg_exp(t, &[(0.0, 8000.0), (d, 160.0)]),
        |t| ramp(t - T0, 0.01) * fall(t).powf(2.0),
        |t| lerp(0.4, -0.4, t / d),
    );
    let sub = tone(
        sr,
        c.secs(),
        |t| seg_exp(t, &[(0.0, 240.0), (d, 34.0)]),
        |t| ramp(t - T0, 0.01) * fall(t).powf(1.4),
        &[0.25, 0.08],
    );
    s.add_mono(&sub, 0, 0.55, 0.0);
    s.resize(n);
    add_verb(&mut s, sr, room(0.85, 1.1), 0.3);
    s
}

pub fn reverse_cymbal(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let cym = drums::cymbal(sr, &mut c.rng, 0.7, 0.6);
    let cym2 = drums::cymbal(sr, &mut c.rng, 0.7, 0.6);
    let mut st = Stereo::new(cym.len());
    for i in 0..cym.len() {
        st.l[i] = cym[i] * 0.8 + cym2[i] * 0.2;
        st.r[i] = cym2[i] * 0.8 + cym[i] * 0.2;
    }
    add_verb(&mut st, sr, room(0.8, 1.0), 0.3);
    let keep = frames(1.92, sr).min(st.len());
    st.resize(keep);
    let mut rev = st.reversed();
    // soften the very end (the original attack) so it doesn't snap off
    rev.fade_out(frames(0.03, sr));
    let mut out = Stereo::new(n);
    out.add(&rev, n.saturating_sub(keep + frames(0.04, sr)), 1.0);
    out
}

pub fn glitch(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut out = Stereo::new(n);
    let mut t = T0;
    let mut prev: Vec<f32> = Vec::new();
    while t < 0.74 {
        let len = c.rng.range(0.02, 0.07).min(0.77 - t);
        let m = frames(len, sr);
        let kind = c.rng.below(6);
        let mut seg: Vec<f32> = match kind {
            0 | 1 => {
                let f = c.rng.range(180.0, 2400.0);
                let mut o = Osc::default();
                (0..m).map(|_| o.pulse(f, 0.5, sr) * 0.6).collect()
            }
            2 => {
                let hold = 3 + c.rng.below(18);
                let mut v = 0.0;
                (0..m)
                    .map(|i| {
                        if i % hold == 0 {
                            v = (c.rng.bi() * 2.0).round() / 2.0;
                        }
                        v * 0.7
                    })
                    .collect()
            }
            3 => {
                let f = c.rng.range(55.0, 120.0);
                let mut o = Osc::default();
                (0..m).map(|_| o.pulse(f, 0.12, sr) * 0.5).collect()
            }
            4 if !prev.is_empty() => {
                // stutter: repeat a slice of the previous segment
                let k = (prev.len() / 2).max(1);
                (0..m).map(|i| prev[i % k]).collect()
            }
            _ => vec![0.0; m],
        };
        let f = frames(0.001, sr);
        fade_in(&mut seg, f);
        fade_out(&mut seg, f);
        let g = c.rng.range(0.5, 1.0);
        out.add_mono(&seg, frames(t, sr), g, c.rng.range(-0.6, 0.6));
        prev = seg;
        t += len;
    }
    out.eq(Bq::Lp, 9000.0, 0.7, sr);
    out.eq(Bq::Hp, 90.0, 0.7, sr);
    out
}

pub fn magic_sparkle(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let scale = [
        84.0, 86.0, 88.0, 91.0, 93.0, 96.0, 98.0, 100.0, 103.0, 105.0,
    ];
    let mut out = Stereo::new(n);
    let mut t = T0;
    for k in 0..13 {
        let idx = (k * 7 / 10 + c.rng.below(3)).min(scale.len() - 1);
        let f = hz(scale[idx]);
        let v = modes(
            sr,
            frames(0.9, sr),
            &[(f, 1.0, 0.32), (f * 2.0, 0.3, 0.14), (f * 3.0, 0.12, 0.07)],
            0.001,
        );
        let g = c.rng.range(0.4, 0.75) * (1.0 - k as f32 / 20.0);
        out.add_mono(&v, frames(t, sr), g, c.rng.range(-0.8, 0.8));
        t += c.rng.range(0.055, 0.085);
    }
    let shimmer = air(
        c,
        Mode::Hp,
        0.7,
        1.0,
        |_| 6500.0,
        |t| swell(t / 1.6, 0.5, 2.0) * 0.25,
        |t| (TAU * 1.3 * t).sin() * 0.6,
    );
    out.add(&shimmer, 0, 1.0);
    let e = echo(&out, sr, 0.13, 0.35, 5000.0, true);
    out.add(&e, 0, 0.35);
    add_verb(&mut out, sr, room(0.85, 1.0), 0.35);
    out
}

// ------------------------------------------------------------------ impacts

pub fn cinematic_boom(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut ph = 0.0f64;
    let mut body_f = Svf::new(1000.0, 0.7, sr);
    let mut crack_f = Svf::new(2500.0, 0.7, sr);
    let mut brown = Brown::new(c.rng.fork());
    let rng = &mut c.rng;
    let m = gen(sr, n, |t| {
        let t = t - T0;
        if t < 0.0 {
            return 0.0;
        }
        let f = 36.0 + 64.0 * decay(t, 0.12);
        ph = (ph + f as f64 / sr as f64).fract();
        let sub = (ph * std::f64::consts::TAU).sin() as f32 * ramp(t, 0.003) * decay(t, 1.0);
        body_f.set(120.0 + 1600.0 * decay(t, 0.05), 0.7, sr);
        let body = body_f.lp(brown.next() * 0.5 + rng.bi() * 0.5) * decay(t, 0.32) * 1.4;
        let crack = crack_f.hp(rng.bi()) * decay(t, 0.006) * 0.5;
        sat(sub + body + crack, 1.8)
    });
    let mut s = mono(&m, n, 0);
    let mut big = room(0.93, 1.3);
    big.hp = 120.0;
    big.lp = 5000.0;
    add_verb(&mut s, sr, big, 0.5);
    s.gain_env(sr, |t| release((t - 3.2).max(0.0), 0.16));
    s
}

pub fn sub_drop(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let len = c.secs();
    let mut ph = 0.0f64;
    let m = gen(sr, n, |t| {
        let t = t - T0;
        if t < 0.0 {
            return 0.0;
        }
        let f = 31.0 + 120.0 * decay(t, 0.42);
        ph = (ph + f as f64 / sr as f64).fract();
        let s = (ph * std::f64::consts::TAU).sin() as f32;
        let a = ramp(t, 0.006) * decay(t, 1.1) * (1.0 - t / (len - T0)).max(0.0).sqrt();
        sat(s * a * 1.4, 2.2)
    });
    mono(&m, n, 0)
}

pub fn hit(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut ph = 0.0f64;
    let mut snap = Svf::new(1900.0, 0.8, sr);
    let rng = &mut c.rng;
    let mut m = shot(sr, 1.4, |t| {
        let f = 48.0 + 150.0 * decay(t, 0.035);
        ph = (ph + f as f64 / sr as f64).fract();
        let body = (ph * std::f64::consts::TAU).sin() as f32
            * ramp(t, 0.0015)
            * if t < 0.015 {
                1.0
            } else {
                decay(t - 0.015, 0.22)
            };
        let sn = snap.bp(rng.bi()) * decay(t, 0.05) * 1.2 * ramp(t, 0.0005);
        sat(body + sn, 1.7)
    });
    let ring = modes(
        sr,
        m.len(),
        &[
            (380.0, 0.12, 0.15),
            (920.0, 0.08, 0.1),
            (1730.0, 0.05, 0.07),
        ],
        0.001,
    );
    for (a, b) in m.iter_mut().zip(ring) {
        *a += b;
    }
    let mut s = mono(&m, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.8, 1.0), 0.35);
    s.gain_env(sr, |t| release((t - 1.25).max(0.0), 0.04));
    s
}

pub fn punch(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let hit_t = 0.075;
    // short whoosh into the impact
    let mut s = air(
        c,
        Mode::Bp,
        1.2,
        0.4,
        |t| seg_exp(t, &[(0.0, 600.0), (hit_t, 2600.0)]),
        |t| {
            if t < hit_t {
                smoothstep(t / hit_t).powf(2.0) * 0.35
            } else {
                0.35 * decay(t - hit_t, 0.008)
            }
        },
        |t| lerp(-0.5, 0.0, (t / hit_t).min(1.0)),
    );
    let mut ph = 0.0f64;
    let mut slap = Svf::new(1200.0, 1.2, sr);
    let mut thump = Svf::new(300.0, 0.7, sr);
    let rng = &mut c.rng;
    let m = shot(sr, 0.5, |t| {
        let f = 55.0 + 110.0 * decay(t, 0.02);
        ph = (ph + f as f64 / sr as f64).fract();
        let body = (ph * std::f64::consts::TAU).sin() as f32 * ramp(t, 0.001) * decay(t, 0.09);
        let x = rng.bi();
        let sl = slap.bp(x) * decay(t, 0.02) * 1.3 * ramp(t, 0.0004);
        let th = thump.lp(x) * decay(t, 0.03) * 1.2;
        sat(body + sl + th, 2.0)
    });
    s.add_mono(&m, frames(hit_t, sr), 1.0, 0.0);
    s.resize(n);
    add_verb(&mut s, sr, room(0.5, 0.6), 0.15);
    s
}

pub fn thud(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut ph = 0.0f64;
    let mut lp = Svf::new(350.0, 0.7, sr);
    let rng = &mut c.rng;
    let m = shot(sr, 0.75, |t| {
        let f = 45.0 + 35.0 * decay(t, 0.03);
        ph = (ph + f as f64 / sr as f64).fract();
        let body = (ph * std::f64::consts::TAU).sin() as f32 * decay(t, 0.08);
        lp.set(150.0 + 250.0 * decay(t, 0.03), 0.7, sr);
        let dull = lp.lp(rng.bi()) * decay(t, 0.09) * 2.0;
        let wood = (TAU * 185.0 * t).sin() * decay(t, 0.05) * 0.3;
        sat(ramp(t, 0.002) * (body + dull + wood), 1.5)
    });
    let mut s = mono(&m, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.4, 0.5), 0.12);
    s
}

pub fn glass_tink(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let f = 2350.0;
    let mut list = Vec::new();
    for (r, a, d) in [
        (1.0, 1.0, 0.36),
        (2.32, 0.55, 0.22),
        (4.25, 0.3, 0.13),
        (6.63, 0.16, 0.08),
        (9.38, 0.08, 0.05),
    ] {
        list.push((f * r, a, d));
        list.push((f * r * 1.0017, a * 0.4, d * 0.9));
    }
    let v = strike(c, 1.4, &list, 0.25, 7000.0, 0.0015);
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.6, 0.6), 0.2);
    s
}

pub fn metal_clang(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let base = 340.0;
    let ratios = [
        1.0, 1.59, 2.14, 2.30, 2.65, 2.92, 3.16, 3.50, 3.60, 4.06, 4.15, 5.13, 5.40, 6.2, 7.0,
    ];
    let mut list = Vec::new();
    for (k, r) in ratios.iter().enumerate() {
        let a = c.rng.range(0.7, 1.3) / (1.0 + 0.3 * k as f32);
        let d = c.rng.range(0.25, 0.65) / (1.0 + 0.1 * k as f32);
        list.push((base * r, a, d));
        list.push((base * r * 1.003, a * 0.3, d));
    }
    let mut v = strike(c, 2.4, &list, 0.5, 3000.0, 0.003);
    for x in v.iter_mut() {
        *x = sat(*x * 0.6, 1.3);
    }
    let mut s = Stereo::new(n);
    s.add_mono(&v, frames(T0, sr), 1.0, -0.1);
    add_verb(&mut s, sr, room(0.75, 0.9), 0.3);
    s
}

pub fn braam(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let b = inst::braam(sr, &mut c.rng, &[36.0, 43.0, 48.0], 2.0);
    let mut s = Stereo::new(n);
    s.add(&b, frames(T0, sr), 1.0);
    let mut big = room(0.9, 1.2);
    big.hp = 150.0;
    add_verb(&mut s, sr, big, 0.4);
    s.gain_env(sr, |t| release((t - 3.0).max(0.0), 0.08));
    s
}

// ----------------------------------------------------------- pops & clicks

pub fn pop(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut v = tone(
        sr,
        0.25,
        |t| 340.0 + 760.0 * (1.0 - decay(t, 0.012)),
        |t| ramp(t, 0.001) * decay(t, 0.03),
        &[0.18],
    );
    let mut lp = Svf::new(1200.0, 0.7, sr);
    for (i, x) in v.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *x += lp.lp(c.rng.bi()) * decay(t, 0.004) * ramp(t, 0.0005) * 0.5;
    }
    mono(&v, n, frames(T0, sr))
}

pub fn bubble_pop(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for (at, f0, g, pan) in [(T0, 520.0, 1.0, -0.15), (0.17, 780.0, 0.65, 0.2)] {
        let v = tone(
            sr,
            0.22,
            |t| f0 * (1.0 + 2.2 * (t / 0.06).min(1.4)),
            |t| ramp(t, 0.0015) * decay(t, 0.035),
            &[0.1],
        );
        s.add_mono(&v, frames(at, sr), g, pan);
    }
    add_verb(&mut s, sr, room(0.4, 0.4), 0.1);
    s
}

pub fn click(c: &mut Ctx) -> Stereo {
    let n = c.n;
    let v = strike(
        c,
        0.1,
        &[
            (1900.0, 0.6, 0.006),
            (4100.0, 0.5, 0.004),
            (6800.0, 0.25, 0.002),
        ],
        0.8,
        3000.0,
        0.0015,
    );
    mono(&v, n, frames(T0, c.sr))
}

pub fn mouse_click(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let press = strike(
        c,
        0.08,
        &[
            (3200.0, 0.7, 0.006),
            (5400.0, 0.4, 0.004),
            (1200.0, 0.3, 0.008),
        ],
        0.7,
        4500.0,
        0.001,
    );
    let rel = strike(
        c,
        0.08,
        &[
            (3450.0, 0.7, 0.005),
            (5800.0, 0.4, 0.003),
            (1300.0, 0.25, 0.007),
        ],
        0.6,
        5000.0,
        0.0008,
    );
    let mut s = mono(&press, n, frames(T0, sr));
    s.add_mono(&rel, frames(T0 + 0.085, sr), 0.5, 0.0);
    s
}

pub fn tick(c: &mut Ctx) -> Stereo {
    let n = c.n;
    let v = strike(
        c,
        0.09,
        &[
            (2600.0, 1.0, 0.009),
            (4100.0, 0.5, 0.006),
            (6900.0, 0.25, 0.003),
        ],
        0.5,
        5000.0,
        0.0006,
    );
    mono(&v, n, frames(T0, c.sr))
}

pub fn toggle(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let a = strike(
        c,
        0.12,
        &[
            (1400.0, 0.8, 0.012),
            (2900.0, 0.5, 0.008),
            (650.0, 0.4, 0.015),
        ],
        0.6,
        2000.0,
        0.0015,
    );
    let b = strike(
        c,
        0.08,
        &[(3800.0, 0.5, 0.005), (2100.0, 0.3, 0.006)],
        0.4,
        5000.0,
        0.0008,
    );
    let mut s = mono(&a, n, frames(T0, sr));
    s.add_mono(&b, frames(T0 + 0.032, sr), 0.6, 0.0);
    add_verb(&mut s, sr, room(0.3, 0.35), 0.08);
    s
}

pub fn swipe(c: &mut Ctx) -> Stereo {
    let d = 0.28;
    air(
        c,
        Mode::Bp,
        1.4,
        0.6,
        |t| seg_exp(t / d, &[(0.0, 1200.0), (1.0, 5200.0)]),
        |t| swell(t / d, 0.4, 2.0),
        |t| lerp(0.5, -0.5, t / d),
    )
}

/// One computer-keyboard keystroke: press "thock" and a lighter release.
fn keystroke(c: &mut Ctx, space: bool) -> (Vec<f32>, Vec<f32>) {
    let f1 = if space {
        c.rng.range(430.0, 560.0)
    } else {
        c.rng.range(900.0, 1500.0)
    };
    let f2 = c.rng.range(2400.0, 3600.0);
    let down = strike(
        c,
        0.09,
        &[
            (f1, 0.6, if space { 0.02 } else { 0.012 }),
            (f2, 0.4, 0.006),
            (f2 * 1.7, 0.2, 0.003),
            (if space { 160.0 } else { 230.0 }, 0.3, 0.02),
        ],
        0.8,
        2500.0,
        0.002,
    );
    let up = strike(
        c,
        0.05,
        &[(f2 * 1.2, 0.35, 0.004), (f1 * 1.5, 0.2, 0.006)],
        0.3,
        4000.0,
        0.001,
    );
    (down, up)
}

pub fn keyboard_typing(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    let mut t = 0.03;
    let mut count = 0;
    while t < 2.85 {
        let space = count % 7 == 6 && c.rng.chance(0.75);
        let (down, up) = keystroke(c, space);
        let g = c.rng.range(0.6, 1.0);
        let pan = if space { 0.0 } else { c.rng.range(-0.35, 0.35) };
        s.add_mono(&down, frames(t, sr), g, pan);
        s.add_mono(&up, frames(t + c.rng.range(0.05, 0.09), sr), g * 0.6, pan);
        count += 1;
        t += if c.rng.chance(0.08) {
            c.rng.range(0.25, 0.4)
        } else {
            c.rng.range(0.07, 0.17)
        };
    }
    add_verb(&mut s, sr, room(0.3, 0.35), 0.08);
    s
}

pub fn typewriter(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    let mut t = 0.03;
    while t < 2.95 {
        let j = c.rng.range(0.92, 1.08);
        let slap = strike(
            c,
            0.12,
            &[
                (1700.0 * j, 0.7, 0.02),
                (3400.0 * j, 0.5, 0.012),
                (5600.0 * j, 0.3, 0.006),
                (260.0, 0.5, 0.03),
            ],
            1.2,
            3500.0,
            0.001,
        );
        let mut lp = Svf::new(400.0, 0.7, sr);
        let thump = gen(sr, frames(0.05, sr), |tt| {
            lp.lp(c.rng.bi()) * decay(tt, 0.008) * 2.0
        });
        let g = c.rng.range(0.7, 1.0);
        let pan = c.rng.range(-0.25, 0.25);
        s.add_mono(&slap, frames(t, sr), g, pan);
        s.add_mono(&thump, frames(t, sr), g, pan);
        for k in 0..2 {
            let r = strike(c, 0.03, &[(2200.0 * j, 0.2, 0.004)], 0.15, 4000.0, 0.0005);
            s.add_mono(&r, frames(t + 0.018 + 0.014 * k as f32, sr), g * 0.5, pan);
        }
        t += c.rng.range(0.11, 0.22);
    }
    // end-of-line bell
    let fb = 2100.0;
    let mut list = Vec::new();
    for (r, a, d) in [
        (1.0, 1.0, 0.3),
        (2.4, 0.5, 0.18),
        (3.95, 0.25, 0.1),
        (5.5, 0.12, 0.06),
    ] {
        list.push((fb * r, a, d));
        list.push((fb * r * 1.002, a * 0.4, d));
    }
    let bell = strike(c, 0.9, &list, 0.3, 6000.0, 0.001);
    s.add_mono(&bell, frames(3.05, sr), 0.9, 0.4);
    // carriage-return ratchet sweeping across, then the clunk at the stop
    let clicks = 34;
    for k in 0..clicks {
        let x = k as f32 / clicks as f32;
        let f = c.rng.range(2700.0, 3300.0);
        let r = strike(c, 0.02, &[(f, 0.3, 0.003)], 0.3, 5000.0, 0.0006);
        let g = swell(x, 0.3, 0.6) * 0.55 + 0.05;
        s.add_mono(&r, frames(3.2 + x * 0.42, sr), g, lerp(0.6, -0.6, x));
    }
    let clunk = strike(
        c,
        0.2,
        &[(300.0, 0.6, 0.04), (900.0, 0.4, 0.02)],
        0.6,
        1500.0,
        0.003,
    );
    s.add_mono(&clunk, frames(3.66, sr), 0.9, -0.6);
    add_verb(&mut s, sr, room(0.45, 0.5), 0.12);
    s
}

pub fn camera_shutter(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let a = strike(
        c,
        0.15,
        &[
            (1100.0, 0.8, 0.015),
            (2300.0, 0.6, 0.01),
            (4700.0, 0.4, 0.006),
            (180.0, 0.5, 0.02),
        ],
        1.0,
        2500.0,
        0.003,
    );
    let b = strike(
        c,
        0.2,
        &[
            (950.0, 0.7, 0.02),
            (2050.0, 0.5, 0.012),
            (5200.0, 0.35, 0.006),
            (150.0, 0.6, 0.03),
        ],
        1.0,
        2200.0,
        0.003,
    );
    let mut s = mono(&a, n, frames(T0, sr));
    for k in 0..3 {
        let f = c.rng.range(3000.0, 4500.0);
        let r = strike(c, 0.02, &[(f, 0.25, 0.003)], 0.2, 6000.0, 0.0005);
        s.add_mono(&r, frames(T0 + 0.02 + k as f32 * 0.014, sr), 0.5, 0.0);
    }
    s.add_mono(&b, frames(T0 + 0.075, sr), 0.9, 0.0);
    add_verb(&mut s, sr, room(0.35, 0.4), 0.1);
    s
}

// ------------------------------------------------------------ notifications

fn bell_modes(f: f32, tau: f32) -> Vec<(f32, f32, f32)> {
    vec![
        (f, 1.0, tau),
        (f * 1.0015, 0.3, tau),
        (f * 2.005, 0.25, tau * 0.4),
        (f * 3.01, 0.08, tau * 0.2),
        (f * 5.4, 0.05, tau * 0.07),
    ]
}

pub fn ding(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let v = strike(c, 1.45, &bell_modes(hz(88.0), 0.38), 0.15, 6000.0, 0.001);
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.6, 0.7), 0.2);
    s
}

fn glock(sr: f32, f: f32, tau: f32) -> Vec<f32> {
    modes(
        sr,
        frames(tau * 5.0, sr),
        &[
            (f, 1.0, tau),
            (f * 2.76, 0.3, tau * 0.33),
            (f * 5.4, 0.12, tau * 0.12),
            (f * 8.93, 0.05, tau * 0.06),
        ],
        0.0008,
    )
}

pub fn chime(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for (k, note) in [84.0f32, 88.0, 91.0, 96.0].iter().enumerate() {
        let v = glock(sr, hz(*note), 0.9);
        let pan = -0.45 + 0.3 * k as f32;
        s.add_mono(
            &v,
            frames(T0 + 0.085 * k as f32, sr),
            0.8 + 0.05 * k as f32,
            pan,
        );
    }
    let e = echo(&s, sr, 0.17, 0.3, 4000.0, true);
    s.add(&e, 0, 0.25);
    add_verb(&mut s, sr, room(0.8, 0.9), 0.3);
    s.gain_env(sr, |t| release((t - 2.1).max(0.0), 0.08));
    s
}

pub fn bell(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let p = 330.0;
    let mut list = Vec::new();
    for (r, a, d) in [
        (0.5, 0.5, 0.95),
        (1.0, 0.6, 0.7),
        (1.2, 0.45, 0.55),
        (1.5, 0.3, 0.45),
        (2.0, 1.0, 0.5),
        (2.5, 0.3, 0.3),
        (2.66, 0.25, 0.26),
        (3.0, 0.2, 0.2),
        (4.0, 0.12, 0.13),
        (5.2, 0.08, 0.08),
    ] {
        list.push((p * r, a, d));
        list.push((p * r * 1.0021, a * 0.5, d));
    }
    let v = strike(c, 2.95, &list, 0.25, 2000.0, 0.004);
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.85, 1.0), 0.3);
    s.gain_env(sr, |t| release((t - 2.4).max(0.0), 0.11));
    s
}

/// Soft mallet tone (kalimba / marimba blend).
fn mallet(sr: f32, f: f32, tau: f32) -> Vec<f32> {
    modes(
        sr,
        frames(tau * 5.0 + 0.05, sr),
        &[
            (f, 1.0, tau),
            (f * 2.0, 0.2, tau * 0.45),
            (f * 3.93, 0.25, tau * 0.08),
            (f * 9.2, 0.06, tau * 0.03),
        ],
        0.0015,
    )
}

pub fn success(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    let notes = [72.0f32, 76.0, 79.0, 84.0];
    for (k, note) in notes.iter().enumerate() {
        let last = k == notes.len() - 1;
        let tau = if last { 0.5 } else { 0.22 };
        let v = mallet(sr, hz(*note), tau);
        let at = frames(T0 + 0.075 * k as f32, sr);
        s.add_mono(&v, at, 0.8, -0.3 + 0.2 * k as f32);
        if last {
            let o = glock(sr, hz(note + 12.0), 0.5);
            s.add_mono(&o, at, 0.3, 0.2);
        }
    }
    let e = echo(&s, sr, 0.15, 0.25, 4000.0, true);
    s.add(&e, 0, 0.2);
    add_verb(&mut s, sr, room(0.7, 0.8), 0.25);
    s
}

pub fn error_buzz(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for (at, f) in [(T0, 185.0f32), (0.27, 147.0)] {
        let mut a = Osc::default();
        let mut b = Osc::new(0.3);
        let mut lp = Svf::new(1500.0, 0.9, sr);
        let v = gen(sr, frames(0.3, sr), |t| {
            let x = a.saw(f, sr) * 0.6 + b.pulse(f * 1.012, 0.5, sr) * 0.4;
            lp.lp(x) * adsr(t, 0.008, 0.2, 0.8, 0.012, 0.19)
        });
        s.add_mono(&v, frames(at, sr), 1.0, 0.0);
    }
    add_verb(&mut s, sr, room(0.3, 0.4), 0.06);
    s
}

pub fn notification(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for (k, note) in [79.0f32, 86.0].iter().enumerate() {
        let v = mallet(sr, hz(*note), 0.32);
        s.add_mono(
            &v,
            frames(T0 + 0.13 * k as f32, sr),
            0.9,
            -0.1 + 0.2 * k as f32,
        );
        let sub = mallet(sr, hz(note - 12.0), 0.25);
        s.add_mono(&sub, frames(T0 + 0.13 * k as f32, sr), 0.25, 0.0);
    }
    add_verb(&mut s, sr, room(0.6, 0.7), 0.25);
    s
}

pub fn message_pop(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let blip = tone(
        sr,
        0.2,
        |t| 420.0 * 2f32.powf(1.4 * (t / 0.05).min(1.0)),
        |t| ramp(t, 0.002) * decay(t, 0.05),
        &[0.15],
    );
    let ping = tone(
        sr,
        0.4,
        |_| 1660.0,
        |t| ramp(t, 0.002) * decay(t, 0.08),
        &[0.1, 0.04],
    );
    let mut s = mono(&blip, n, frames(T0, sr));
    s.add_mono(&ping, frames(T0 + 0.065, sr), 0.45, 0.0);
    add_verb(&mut s, sr, room(0.5, 0.5), 0.12);
    s
}

pub fn coin(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut o = Osc::default();
    let mut lp = Svf::new(7000.0, 0.7, sr);
    let v = shot(sr, 0.75, |t| {
        let (f, a) = if t < 0.075 {
            (hz(83.0), ramp(t, 0.002))
        } else {
            (hz(88.0), decay(t - 0.075, 0.15))
        };
        lp.lp(o.pulse(f, 0.5, sr)) * a * 0.5
    });
    let mut s = mono(&v, n, frames(T0, sr));
    let e = echo(&s, sr, 0.09, 0.2, 5000.0, true);
    s.add(&e, 0, 0.2);
    s
}

pub fn level_up(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let notes = [72.0f32, 76.0, 79.0, 84.0, 88.0, 91.0, 96.0];
    let step = 0.055;
    let held = 0.65;
    let total = step * (notes.len() - 1) as f32 + held;
    let mut o = Osc::default();
    let mut sub = Osc::default();
    let mut lp = Svf::new(8000.0, 0.7, sr);
    let v = shot(sr, total + 0.25, |t| {
        let k = ((t / step) as usize).min(notes.len() - 1);
        let last = k == notes.len() - 1;
        let tn = t - k as f32 * step;
        let vib = if last {
            1.0 + 0.008 * (TAU * 6.0 * tn).sin() * ramp(tn, 0.15)
        } else {
            1.0
        };
        let f = hz(notes[k]) * vib;
        let a = if last {
            ramp(tn, 0.003) * decay(tn, 0.22)
        } else {
            1.0 - 0.3 * (tn / step)
        };
        let x = o.pulse(f, 0.25, sr) * 0.35 + sub.tri(f * 0.5, sr) * 0.4;
        lp.lp(x) * a * ramp(t, 0.002)
    });
    let mut s = mono(&v, n, frames(T0, sr));
    let e = echo(&s, sr, 0.11, 0.3, 4000.0, true);
    s.add(&e, 0, 0.3);
    add_verb(&mut s, sr, room(0.5, 0.6), 0.12);
    s
}

// ------------------------------------------------------- countdown & timers

fn beep_tone(sr: f32, f: f32, gate: f32, tail: f32) -> Vec<f32> {
    tone(
        sr,
        gate + tail * 5.0 + 0.02,
        |_| f,
        |t| adsr(t, 0.004, 0.4, 0.85, tail, gate),
        &[0.12, 0.04],
    )
}

pub fn beep(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    mono(&beep_tone(sr, 1000.0, 0.25, 0.008), n, frames(T0, sr))
}

pub fn countdown(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for k in 0..3 {
        let v = beep_tone(sr, 880.0, 0.16, 0.01);
        s.add_mono(&v, frames(0.02 + k as f32, sr), 0.8, 0.0);
    }
    let fin = tone(
        sr,
        0.95,
        |_| 1760.0,
        |t| ramp(t, 0.004) * decay(t, 0.3) * release((t - 0.55).max(0.0), 0.05),
        &[0.1, 0.03],
    );
    s.add_mono(&fin, frames(3.02, sr), 1.0, 0.0);
    add_verb(&mut s, sr, room(0.4, 0.5), 0.1);
    s
}

pub fn clock_ticking(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    for k in 0..10 {
        let m = if k % 2 == 0 { 1.0 } else { 0.82 };
        let v = strike(
            c,
            0.08,
            &[
                (2900.0 * m, 0.6, 0.01),
                (4400.0 * m, 0.4, 0.006),
                (1500.0 * m, 0.3, 0.012),
                (6000.0 * m, 0.15, 0.003),
                (800.0, 0.2, 0.03),
            ],
            0.4,
            5000.0,
            0.0008,
        );
        s.add_mono(&v, frames(0.012 + 0.5 * k as f32, sr), 0.9, 0.0);
    }
    add_verb(&mut s, sr, room(0.3, 0.35), 0.1);
    s
}

pub fn alarm(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    let mut o = Osc::default();
    let mut lp = Svf::new(5000.0, 0.7, sr);
    let beep: Vec<f32> = gen(sr, frames(0.1, sr), |t| {
        lp.lp(o.pulse(2050.0, 0.5, sr)) * adsr(t, 0.003, 1.0, 1.0, 0.004, 0.065) * 0.5
    });
    for g in 0..3 {
        for b in 0..4 {
            let t = 0.02 + g as f32 + b as f32 * 0.125;
            s.add_mono(&beep, frames(t, sr), 1.0, 0.0);
        }
    }
    add_verb(&mut s, sr, room(0.3, 0.4), 0.05);
    s
}

// --------------------------------------------------------- cartoon & comedy

pub fn boing(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut saw = Osc::default();
    let mut sine = Osc::default();
    let mut bp = Svf::new(700.0, 3.0, sr);
    let v = shot(sr, 0.95, |t| {
        // a doorstop spring: pitch and "wah" wobble together and die away
        let wob = (TAU * 10.0 * t).sin() * decay(t, 0.4);
        let f = 150.0 * 2f32.powf(0.75 * (1.0 - decay(t, 0.05)) + 0.2 * wob);
        bp.set(650.0 * 2f32.powf(1.2 * wob), 4.0, sr);
        let x = bp.bp(saw.saw(f, sr)) * 0.9 + sine.sine(f, sr) * 0.5;
        x * ramp(t, 0.003) * decay(t, 0.28)
    });
    mono(&v, n, frames(T0, sr))
}

fn slide_whistle(c: &mut Ctx, from: f32, to: f32) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let gate = 0.86;
    let mut ph = 0.0f64;
    let mut bp = Svf::new(from, 6.0, sr);
    let rng = &mut c.rng;
    let v = shot(sr, 0.95, |t| {
        let x = smoothstep(t / gate);
        let f = from * (to / from).powf(x) * (1.0 + 0.012 * (TAU * 6.0 * t).sin());
        ph = (ph + f as f64 / sr as f64).fract();
        let p = (ph * std::f64::consts::TAU) as f32;
        bp.set(f, 6.0, sr);
        let breath = bp.bp(rng.bi()) * 0.25;
        (p.sin() + 0.08 * (2.0 * p).sin() + breath) * adsr(t, 0.05, 1.0, 1.0, 0.012, gate)
    });
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.5, 0.6), 0.12);
    s
}

pub fn slide_whistle_up(c: &mut Ctx) -> Stereo {
    slide_whistle(c, 480.0, 1900.0)
}

pub fn slide_whistle_down(c: &mut Ctx) -> Stereo {
    slide_whistle(c, 1900.0, 420.0)
}

pub fn rimshot(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut s = Stereo::new(n);
    let sn = drums::snare(sr, &mut c.rng, 210.0, 0.6, 0.1);
    s.add_mono(&sn, frames(T0, sr), 0.55, 0.1);
    let tm = drums::tom(sr, &mut c.rng, 110.0, 0.2);
    s.add_mono(&tm, frames(0.25, sr), 0.8, -0.2);
    let k = drums::kick(sr, &mut c.rng, &Kick::PUNCHY);
    s.add_mono(&k, frames(0.25, sr), 0.5, 0.0);
    s.add_mono(&k, frames(0.52, sr), 0.7, 0.0);
    let sn2 = drums::snare(sr, &mut c.rng, 200.0, 0.8, 0.12);
    s.add_mono(&sn2, frames(0.52, sr), 0.6, 0.1);
    let cym = drums::cymbal(sr, &mut c.rng, 0.4, 0.8);
    s.add_mono(&cym, frames(0.52, sr), 0.55, 0.35);
    add_verb(&mut s, sr, room(0.55, 0.7), 0.15);
    s.gain_env(sr, |t| release((t - 1.6).max(0.0), 0.07));
    s
}

pub fn record_scratch(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    // (start, duration, peak speed) of each hand stroke; sign = direction
    let strokes = [
        (0.0f32, 0.11f32, -3.6f32),
        (0.11, 0.1, 3.4),
        (0.21, 0.09, -3.0),
        (0.30, 0.16, 2.6),
        (0.46, 0.2, -1.8),
    ];
    let rate = |t: f32| {
        for &(s0, d, v) in &strokes {
            if t >= s0 && t < s0 + d {
                return v * (PI * (t - s0) / d).sin();
            }
        }
        0.0
    };
    // the "record": a vowel-like buzz plus groove noise, evaluated at any position
    let f0 = 165.0;
    let formants = [(750.0f32, 1.0f32), (1200.0, 0.6), (2600.0, 0.25)];
    let mut pos = 0.4f32;
    let mut bp = Svf::new(2000.0, 1.0, sr);
    let rng = &mut c.rng;
    let v = gen(sr, frames(0.7, sr), |t| {
        let r = rate(t);
        pos += r / sr;
        let mut s = 0.0;
        for k in 1..40 {
            let fk = f0 * k as f32;
            if fk * r.abs() > sr * 0.45 {
                break;
            }
            let w: f32 = formants
                .iter()
                .map(|&(ff, g)| g / (1.0 + ((fk - ff) / 220.0).powi(2)))
                .sum();
            s += w * (TAU * fk * pos).sin();
        }
        bp.set(800.0 + 1500.0 * r.abs(), 1.0, sr);
        let grit = bp.bp(rng.bi()) * 0.6;
        (s * 0.25 + grit) * r.abs().min(1.0)
    });
    let mut s = mono(&v, n, frames(T0, sr));
    s.eq(Bq::Hp, 120.0, 0.7, sr);
    add_verb(&mut s, sr, room(0.4, 0.5), 0.08);
    s
}

pub fn laser(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let mut sq = Osc::default();
    let mut si = Osc::default();
    let mut lp = Svf::new(6000.0, 0.7, sr);
    let v = shot(sr, 0.4, |t| {
        let f = 180.0 + 2600.0 * decay(t, 0.05);
        let x = sq.pulse(f, 0.5, sr) * 0.35 + si.sine(f, sr) * 0.6;
        lp.lp(x) * ramp(t, 0.002) * decay(t, 0.09)
    });
    let mut s = mono(&v, n, frames(T0, sr));
    let e = echo(&s, sr, 0.085, 0.35, 3500.0, true);
    s.add(&e, 0, 0.35);
    s
}

pub fn sad_trombone(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let notes = [
        (0.0f32, 0.38f32, 58.0f32),
        (0.45, 0.38, 57.0),
        (0.9, 0.38, 56.0),
        (1.35, 1.35, 55.0),
    ];
    let mut s = Stereo::new(n);
    for (k, &(at, gate, note)) in notes.iter().enumerate() {
        let last = k == notes.len() - 1;
        let mut saw = Osc::default();
        let mut pul = Osc::new(0.25);
        let mut wah = Svf::new(500.0, 2.5, sr);
        let mut lp = Svf::new(2500.0, 0.7, sr);
        let v = gen(sr, frames(gate + 0.3, sr), |t| {
            let mut f = hz(note);
            if last {
                f *= 1.0 + 0.025 * (TAU * 5.0 * t).sin() * ramp(t - 0.25, 0.4);
                f *= 1.0 - 0.04 * smoothstep((t - gate + 0.35) / 0.35);
            }
            let x = saw.saw(f, sr) * 0.6 + pul.pulse(f, 0.3, sr) * 0.4;
            let open = if last {
                900.0 + 450.0 * (TAU * 3.0 * t).sin()
            } else {
                500.0 + 850.0 * smoothstep(t / 0.15)
            };
            wah.set(open, 2.5, sr);
            let a = adsr(t, 0.04, 0.3, 0.85, 0.03, gate);
            lp.lp(wah.bp(x) * 0.8 + x * 0.25) * a
        });
        s.add_mono(&v, frames(T0 + at, sr), 1.0, 0.0);
    }
    add_verb(&mut s, sr, room(0.5, 0.6), 0.12);
    s
}

// -------------------------------------------------------------------- drums

pub fn kick(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let tight = Kick {
        decay: 0.16,
        ..Kick::PUNCHY
    };
    let k = drums::kick(sr, &mut c.rng, &tight);
    mono(&k, n, frames(T0, sr))
}

pub fn snare(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let v = drums::snare(sr, &mut c.rng, 190.0, 0.85, 0.13);
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.55, 0.6), 0.15);
    s
}

pub fn clap(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let v = drums::clap(sr, &mut c.rng, 0.09);
    let mut s = mono(&v, n, frames(T0, sr));
    add_verb(&mut s, sr, room(0.6, 0.7), 0.2);
    s
}

pub fn hihat(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let v = drums::hat(sr, &mut c.rng, 0.04);
    mono(&v, n, frames(T0, sr))
}

pub fn open_hihat(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let v = drums::hat(sr, &mut c.rng, 0.2);
    mono(&v, n, frames(T0, sr))
}

pub fn crash(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let a = drums::cymbal(sr, &mut c.rng, 0.75, 0.7);
    let b = drums::cymbal(sr, &mut c.rng, 0.75, 0.7);
    let mut s = Stereo::new(n);
    let at = frames(T0, sr);
    s.add_mono(&a, at, 0.7, -0.5);
    s.add_mono(&b, at, 0.7, 0.5);
    s.gain_env(sr, |t| release((t - 2.6).max(0.0), 0.08));
    s
}

// ----------------------------------------------------------------- ambience

pub fn rain(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.0, |c, n| {
        let sr = c.sr;
        let mut s = Stereo::new(n);
        let mut pl = Pink::new(c.rng.fork());
        let mut pr = Pink::new(c.rng.fork());
        let mut wl = c.rng.fork();
        let mut mod_ = Wander::new(c.rng.fork(), 0.3, sr);
        for i in 0..n {
            let g = 1.0 + 0.12 * mod_.next();
            s.l[i] = (pl.next() * 0.6 + wl.bi() * 0.25) * g;
            s.r[i] = (pr.next() * 0.6 + wl.bi() * 0.25) * g;
        }
        s.eq(Bq::Hp, 350.0, 0.7, sr);
        s.eq(Bq::Lp, 7500.0, 0.7, sr);
        s.eq(Bq::Peak(3.0), 3000.0, 0.8, sr);
        s.scale(0.35);
        // individual drops: mostly light ticks, a few heavier plinks
        let secs = n as f32 / sr;
        let mut t = 0.0;
        while t < secs {
            let heavy = c.rng.chance(0.03);
            let (f, tau) = if heavy {
                (c.rng.range(800.0, 2000.0), c.rng.range(0.006, 0.012))
            } else {
                (c.rng.range(2500.0, 7000.0), c.rng.range(0.0015, 0.004))
            };
            let a = c.rng.exp(0.25).min(1.0) * if heavy { 0.6 } else { 0.25 };
            let drop = strike(c, tau * 6.0, &[(f, 1.0, tau)], 0.6, f, tau * 0.5);
            s.add_mono(&drop, frames(t, sr), a, c.rng.range(-0.9, 0.9));
            t += c.rng.exp(1.0 / 160.0);
        }
        let mut b = Brown::new(c.rng.fork());
        let mut lp = Svf::new(120.0, 0.7, sr);
        for i in 0..n {
            let r = lp.lp(b.next()) * 0.06;
            s.l[i] += r;
            s.r[i] += r;
        }
        s
    })
}

pub fn wind(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.5, |c, n| {
        let sr = c.sr;
        let mut s = Stereo::new(n);
        let mut gust = Wander::new(c.rng.fork(), 0.3, sr);
        let mut swell_w = Wander::new(c.rng.fork(), 0.07, sr);
        let mut tone_w = Wander::new(c.rng.fork(), 0.08, sr);
        let mut whistle_w = Wander::new(c.rng.fork(), 0.05, sr);
        let mut chans: Vec<(Brown, Pink, Svf, Svf, Wander)> = (0..2)
            .map(|_| {
                (
                    Brown::new(c.rng.fork()),
                    Pink::new(c.rng.fork()),
                    Svf::new(500.0, 0.8, sr),
                    Svf::new(900.0, 12.0, sr),
                    Wander::new(c.rng.fork(), 0.2, sr),
                )
            })
            .collect();
        for i in 0..n {
            let g = (0.62 + 0.45 * gust.next() + 0.3 * swell_w.next())
                .clamp(0.25, 1.3)
                .powf(1.4);
            let tw = tone_w.next();
            let ww = whistle_w.next();
            for (ch, (b, p, bp, wh, own)) in chans.iter_mut().enumerate() {
                let o = own.next();
                if i % 16 == 0 {
                    bp.set(420.0 * 2f32.powf(1.1 * tw + 0.3 * o + g), 0.8, sr);
                    wh.set(950.0 * 2f32.powf(0.5 * ww + 0.1 * o), 14.0, sr);
                }
                let x = b.next() * 0.6 + p.next() * 0.4;
                let gg = (g + 0.15 * o).max(0.05);
                let v = bp.bp(x) * gg + wh.bp(p.next()) * gg * gg * 0.35;
                if ch == 0 {
                    s.l[i] = v;
                } else {
                    s.r[i] = v;
                }
            }
        }
        s
    })
}

pub fn ocean_waves(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.5, |c, n| {
        let sr = c.sr;
        let secs = n as f32 / sr;
        // wave crests (time of crash, size, pan)
        let mut waves = Vec::new();
        let mut t = -2.0;
        while t < secs + 1.0 {
            waves.push((t, c.rng.range(0.7, 1.0), c.rng.range(-0.4, 0.4)));
            t += c.rng.range(3.6, 4.6);
        }
        let env = |t: f32| -> (f32, f32, f32) {
            let (mut e, mut wash, mut pan) = (0.0f32, 0.0f32, 0.0f32);
            for &(tc, size, p) in &waves {
                let dt = t - tc;
                let v = if dt < -1.8 {
                    0.0
                } else if dt < 0.0 {
                    smoothstep((dt + 1.8) / 1.8).powf(2.0)
                } else {
                    decay(dt, 1.6)
                } * size;
                if v > e {
                    pan = p;
                }
                e = e.max(v);
                if dt > 0.0 {
                    wash += decay(dt, 1.2) * smoothstep(dt / 0.3) * size;
                }
            }
            (e, wash.min(1.2), pan)
        };
        let mut s = Stereo::new(n);
        let mut pl = Pink::new(c.rng.fork());
        let mut pr = Pink::new(c.rng.fork());
        let mut wl = c.rng.fork();
        let mut fl = Svf::new(400.0, 0.6, sr);
        let mut fr = fl;
        let mut hl = Svf::new(3500.0, 0.7, sr);
        let mut hr = hl;
        let mut brown = Brown::new(c.rng.fork());
        let mut rl = Svf::new(180.0, 0.7, sr);
        for i in 0..n {
            let t = i as f32 / sr;
            let (e, wash, pan) = env(t);
            if i % 16 == 0 {
                let fc = 300.0 + 3200.0 * e.powf(1.5);
                fl.set(fc, 0.6, sr);
                fr.set(fc * 1.08, 0.6, sr);
            }
            let body = 0.15 + 0.85 * e;
            let (bl, br) = (body * (1.0 - 0.3 * pan), body * (1.0 + 0.3 * pan));
            let fizz = wash * 0.22 * (0.6 + 0.4 * wl.uni());
            let rumble = rl.lp(brown.next()) * 0.3;
            s.l[i] = fl.lp(pl.next()) * bl + hl.hp(wl.bi()) * fizz + rumble;
            s.r[i] = fr.lp(pr.next()) * br + hr.hp(wl.bi()) * fizz + rumble;
        }
        let w = reverb(&s, sr, &room(0.7, 1.0));
        s.add(&w, 0, 0.15);
        s
    })
}

pub fn fire_crackle(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.0, |c, n| {
        let sr = c.sr;
        let secs = n as f32 / sr;
        let mut s = Stereo::new(n);
        let mut b = Brown::new(c.rng.fork());
        let mut p = Pink::new(c.rng.fork());
        let mut lp = Svf::new(250.0, 0.7, sr);
        let mut lp2 = Svf::new(900.0, 0.7, sr);
        let mut hp = Svf::new(3000.0, 0.7, sr);
        let mut flick = Wander::new(c.rng.fork(), 0.6, sr);
        for i in 0..n {
            let f = 0.8 + 0.2 * flick.next();
            let x = p.next();
            let roar = lp.lp(b.next()) * 0.8 + lp2.lp(x) * 0.25;
            let hiss = hp.hp(x) * 0.05;
            let v = (roar + hiss) * f;
            s.l[i] = v;
            s.r[i] = v;
        }
        // crackles come in little clusters; a few bigger pops
        let mut t = 0.0;
        while t < secs {
            let count = 1 + c.rng.below(4);
            let pan = c.rng.range(-0.6, 0.6);
            let mut tt = t;
            for _ in 0..count {
                let f = c.rng.range(2000.0, 6000.0);
                let tau = c.rng.range(0.0008, 0.003);
                let a = (c.rng.uni().powf(3.0) * 0.9 + 0.1) * 0.6;
                let k = strike(c, tau * 8.0, &[(f, 0.4, tau)], 1.0, f, tau * 0.6);
                s.add_mono(&k, frames(tt, sr), a, pan);
                tt += c.rng.range(0.003, 0.015);
            }
            if c.rng.chance(0.07) {
                let f = c.rng.range(600.0, 1800.0);
                let tau = c.rng.range(0.008, 0.015);
                let k = strike(c, tau * 6.0, &[(f, 0.6, tau)], 1.0, f * 1.5, 0.002);
                s.add_mono(&k, frames(t, sr), 0.8, pan);
            }
            t += c.rng.exp(1.0 / 12.0);
        }
        s
    })
}

pub fn room_tone(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.0, |c, n| {
        let sr = c.sr;
        let mut s = Stereo::new(n);
        let mut pc = Pink::new(c.rng.fork());
        let mut pl = Pink::new(c.rng.fork());
        let mut pr = Pink::new(c.rng.fork());
        let mut hum = Osc::default();
        for i in 0..n {
            let common = pc.next();
            let h = hum.sine(120.0, sr) * 0.015;
            s.l[i] = common * 0.7 + pl.next() * 0.3 + h;
            s.r[i] = common * 0.7 + pr.next() * 0.3 + h;
        }
        s.eq(Bq::Lp, 1500.0, 0.7, sr);
        s.eq(Bq::Hp, 60.0, 0.7, sr);
        s
    })
}

pub fn heartbeat(c: &mut Ctx) -> Stereo {
    let (sr, n) = (c.sr, c.n);
    let period = c.secs() / 12.0; // 72 bpm, exactly 12 beats so it loops
    let mut lub_lp = Svf::new(150.0, 0.7, sr);
    let mut ph = 0.0f64;
    let rng = &mut c.rng;
    let mut beat = |f0: f32, tau: f32, amp: f32| -> Vec<f32> {
        ph = 0.0;
        shot(sr, 0.3, |t| {
            let f = f0 * (1.0 + 0.4 * decay(t, 0.02));
            ph = (ph + f as f64 / sr as f64).fract();
            let body = (ph * std::f64::consts::TAU).sin() as f32;
            let thump = lub_lp.lp(rng.bi()) * decay(t, 0.03) * 1.5;
            sat(
                (body + thump) * amp * smoothstep(t / 0.008) * decay(t, tau),
                1.8,
            )
        })
    };
    let lub = beat(52.0, 0.07, 1.0);
    let dub = beat(68.0, 0.05, 0.75);
    let mut buf = Stereo::new(n + frames(1.0, sr));
    for k in 0..12 {
        let t = 0.05 + k as f32 * period;
        buf.add_mono(&lub, frames(t, sr), 1.0, 0.0);
        buf.add_mono(&dub, frames(t + 0.28, sr), 1.0, 0.0);
    }
    // fold the overhang back onto the start so the loop is seamless
    let mut s = Stereo::new(n);
    for i in 0..buf.len() {
        s.l[i % n] += buf.l[i];
        s.r[i % n] += buf.r[i];
    }
    s.eq(Bq::Lp, 320.0, 0.7, sr);
    s.map(|x| sat(x, 1.4));
    s
}

/// One bird call: returns a mono vector.
fn bird(c: &mut Ctx) -> Vec<f32> {
    let sr = c.sr;
    let kind = c.rng.below(4);
    let mut ph = 0.0f64;
    let mut voice = |len: f32, f: &dyn Fn(f32) -> f32, a: &dyn Fn(f32) -> f32| -> Vec<f32> {
        ph = 0.0;
        gen(sr, frames(len, sr), |t| {
            ph = (ph + f(t) as f64 / sr as f64).fract();
            let p = (ph * std::f64::consts::TAU) as f32;
            (p.sin() + 0.08 * (2.0 * p).sin()) * a(t)
        })
    };
    match kind {
        0 => {
            // series of quick down-chirps
            let hi = c.rng.range(4500.0, 6200.0);
            let lo = c.rng.range(2500.0, 3500.0);
            let count = 3 + c.rng.below(4);
            let gap = c.rng.range(0.07, 0.11);
            let d = c.rng.range(0.035, 0.06);
            let chirp = voice(d, &|t| hi * (lo / hi).powf(t / d), &|t| {
                (PI * t / d).sin().max(0.0)
            });
            let mut out = vec![0.0; frames(gap * count as f32 + d, sr)];
            for k in 0..count {
                let at = frames(gap * k as f32, sr);
                let g = 1.0 - 0.1 * k as f32;
                for (i, v) in chirp.iter().enumerate() {
                    if at + i < out.len() {
                        out[at + i] += v * g;
                    }
                }
            }
            out
        }
        1 => {
            // trill
            let f0 = c.rng.range(3500.0, 5000.0);
            let rate = c.rng.range(24.0, 34.0);
            let d = c.rng.range(0.25, 0.45);
            voice(d, &|t| f0 * (1.0 + 0.04 * (TAU * rate * t).sin()), &|t| {
                (0.5 - 0.5 * (TAU * rate * t).cos()) * (PI * t / d).sin().max(0.0)
            })
        }
        2 => {
            // two-note whistle, "fee-bee"
            let f1 = c.rng.range(2800.0, 3600.0);
            let f2 = f1 * c.rng.range(0.78, 0.86);
            voice(
                0.6,
                &|t| {
                    let x = smoothstep((t - 0.22) / 0.04);
                    lerp(f1, f2, x) * (1.0 + 0.006 * (TAU * 9.0 * t).sin())
                },
                &|t| {
                    let a1 = (PI * t / 0.22).sin().max(0.0) * if t < 0.22 { 1.0 } else { 0.0 };
                    let a2 = if t > 0.24 {
                        (PI * (t - 0.24) / 0.33).sin().max(0.0) * 0.8
                    } else {
                        0.0
                    };
                    a1 + a2
                },
            )
        }
        _ => {
            // quick upward tweets
            let lo = c.rng.range(2300.0, 3000.0);
            let hi = lo * c.rng.range(1.7, 2.1);
            let d = 0.065;
            voice(
                0.22,
                &|t| {
                    let tt = t % 0.12;
                    lo * (hi / lo).powf((tt / d).min(1.0))
                },
                &|t| {
                    let tt = t % 0.12;
                    if tt < d {
                        (PI * tt / d).sin()
                    } else {
                        0.0
                    }
                },
            )
        }
    }
}

pub fn forest_birds(c: &mut Ctx) -> Stereo {
    make_loop(c, 1.0, |c, n| {
        let sr = c.sr;
        let secs = n as f32 / sr;
        let mut s = air(c, Mode::Bp, 0.5, 0.2, |_| 3000.0, |_| 0.06, |_| 0.0);
        s.resize(n);
        let mut leaves = Wander::new(c.rng.fork(), 0.3, sr);
        for i in 0..n {
            let g = 0.7 + 0.3 * leaves.next();
            s.l[i] *= g;
            s.r[i] *= g;
        }
        let mut near = Stereo::new(n);
        let mut far = Stereo::new(n);
        let mut t = 0.2;
        while t < secs {
            let call = bird(c);
            let dist = c.rng.uni();
            let pan = c.rng.range(-0.85, 0.85);
            let g = lerp(0.5, 0.15, dist);
            if dist < 0.5 {
                near.add_mono(&call, frames(t, sr), g, pan);
            } else {
                far.add_mono(&call, frames(t, sr), g, pan);
            }
            t += c.rng.range(0.25, 1.4);
        }
        far.eq(Bq::Lp, 4500.0, 0.7, sr);
        s.add(&near, 0, 1.0);
        s.add(&far, 0, 1.0);
        let mut v = room(0.75, 1.0);
        v.damp = 0.6;
        let mut send = near.clone();
        send.add(&far, 0, 2.0);
        let w = reverb(&send, sr, &v);
        s.add(&w, 0, 0.3);
        s
    })
}
