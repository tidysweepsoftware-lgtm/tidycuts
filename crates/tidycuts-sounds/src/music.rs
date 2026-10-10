//! Music beds: short arrangements with a tempo, a chord progression, an
//! intro, a groove and a proper ending (a held final chord that rings out).

use std::f32::consts::TAU;

use crate::drums::{self, Kick};
use crate::dsp::*;
use crate::inst::{self, Pad};

/// Music starts after this pre-roll.
const T0: f32 = 0.05;

/// Arrangement clock and a few helpers shared by every track.
struct Song {
    sr: f32,
    n: usize,
    /// Seconds per beat.
    spb: f32,
    /// Swing offsets for the four 16ths of a beat, in 16ths.
    sw: [f32; 4],
    /// Ritardando from this beat on: extra seconds = k * (beats past it)².
    rit: (f32, f32),
    rng: Rng,
}

impl Song {
    fn new(c: &mut Ctx, bpm: f32) -> Self {
        Song {
            sr: c.sr,
            n: c.n,
            spb: 60.0 / bpm,
            sw: [0.0; 4],
            rit: (f32::MAX, 0.0),
            rng: c.rng.fork(),
        }
    }
    /// Time in seconds of beat `b` (quarter notes from the top).
    fn tb(&self, b: f32) -> f32 {
        let extra = if b > self.rit.0 {
            (b - self.rit.0).powi(2) * self.rit.1
        } else {
            0.0
        };
        T0 + b * self.spb + extra
    }
    /// Time of 16th step `s` (may run past 15) of bar `bar`, with swing.
    fn ts(&self, bar: usize, s: usize) -> f32 {
        self.tb(((bar * 16 + s) as f32 + self.sw[s % 4]) / 4.0)
    }
    /// Time of beat `b` within bar `bar`.
    fn tbar(&self, bar: usize, b: f32) -> f32 {
        self.tb(bar as f32 * 4.0 + b)
    }
    fn bus(&self) -> Stereo {
        Stereo::new(self.n)
    }
    /// Humanising timing offset of up to `secs`.
    fn hum(&mut self, secs: f32) -> f32 {
        self.rng.bi() * secs
    }
    /// Humanised velocity.
    fn vel(&mut self, v: f32, spread: f32) -> f32 {
        (v * (1.0 + self.rng.bi() * spread)).clamp(0.05, 1.0)
    }
}

/// Mix a mono note into a bus at time `t`.
fn put(bus: &mut Stereo, sr: f32, v: &[f32], t: f32, gain: f32, pan: f32) {
    bus.add_mono(v, frames(t, sr), gain, pan);
}

/// Mix a stereo part into a bus at time `t`.
fn put_st(bus: &mut Stereo, sr: f32, v: &Stereo, t: f32, gain: f32) {
    bus.add(v, frames(t, sr), gain);
}

/// Parse a step pattern: `X` accent, `x` normal, `o` soft, `g` ghost, `.`
/// rest; spaces and `|` are ignored. Returns (step, velocity).
fn steps(p: &str) -> Vec<(usize, f32)> {
    p.chars()
        .filter(|c| !c.is_whitespace() && *c != '|')
        .enumerate()
        .filter_map(|(i, c)| match c {
            'X' => Some((i, 1.0)),
            'x' => Some((i, 0.8)),
            'o' => Some((i, 0.55)),
            'g' => Some((i, 0.3)),
            _ => None,
        })
        .collect()
}

fn hall(room: f32, size: f32) -> Verb {
    Verb {
        room,
        size,
        damp: 0.55,
        width: 1.0,
        pre: 0.025,
        hp: 250.0,
        lp: 7000.0,
    }
}

/// Add a reverb of `src` to `mix`.
fn send(mix: &mut Stereo, src: &Stereo, sr: f32, v: Verb, amt: f32) {
    let w = reverb(src, sr, &v);
    mix.add(&w, 0, amt);
}

/// Add a tempo-synced ping-pong echo of `src` to `mix`.
fn delay_send(mix: &mut Stereo, src: &Stereo, sr: f32, time: f32, fb: f32, amt: f32) {
    let w = echo(src, sr, time, fb, 3500.0, true);
    mix.add(&w, 0, amt);
}

/// Normalise the peak and round off transients with gentle saturation.
fn glue(mut mix: Stereo, drive: f32) -> Stereo {
    let p = mix.peak().max(1e-9);
    mix.scale(1.0 / p);
    mix.map(|x| sat(x, drive));
    mix
}

/// Vinyl surface noise: soft hiss plus sparse band-limited crackles.
fn vinyl(sr: f32, n: usize, rng: &mut Rng, per_sec: f32) -> Stereo {
    let mut s = Stereo::new(n);
    let mut t = 0.0;
    let secs = n as f32 / sr;
    while t < secs {
        let i = frames(t, sr);
        if i < n {
            let a = rng.uni().powf(2.5) * 0.8 + 0.1;
            let (gl, gr) = pan_gains(rng.range(-0.5, 0.5));
            s.l[i] += a * gl;
            s.r[i] += a * gr;
        }
        t += rng.exp(1.0 / per_sec);
    }
    s.eq(Bq::Bp, 2500.0, 0.8, sr);
    s.eq(Bq::Lp, 6000.0, 0.7, sr);
    let mut pl = Pink::new(rng.fork());
    let mut pr = Pink::new(rng.fork());
    let mut hl = Svf::new(3000.0, 0.7, sr);
    let mut hr = hl;
    for i in 0..n {
        s.l[i] += hl.hp(pl.next()) * 0.02;
        s.r[i] += hr.hp(pr.next()) * 0.02;
    }
    s
}

/// White-noise riser sweeping up over `len` seconds (stereo).
fn noise_riser(sr: f32, rng: &mut Rng, len: f32) -> Stereo {
    let n = frames(len, sr);
    let mut s = Stereo::new(n);
    let mut fl = Svf::new(300.0, 1.2, sr);
    let mut fr = fl;
    for i in 0..n {
        let x = i as f32 / n as f32;
        if i % 16 == 0 {
            let fc = 300.0 * 30f32.powf(x);
            fl.set(fc, 1.2, sr);
            fr.set(fc * 1.05, 1.2, sr);
        }
        let a = x.powf(2.0) * (1.0 - smoothstep((x - 0.97) / 0.03));
        s.l[i] = fl.bp(rng.bi()) * a;
        s.r[i] = fr.bp(rng.bi()) * a;
    }
    s
}

/// Deep cinematic boom for downbeats.
fn boom(sr: f32, rng: &mut Rng) -> Vec<f32> {
    let mut ph = 0.0f64;
    let mut lp = Svf::new(400.0, 0.7, sr);
    let mut v = gen(sr, frames(2.5, sr), |t| {
        let f = 38.0 + 55.0 * decay(t, 0.09);
        ph = (ph + f as f64 / sr as f64).fract();
        let body = (ph * std::f64::consts::TAU).sin() as f32 * decay(t, 0.7);
        let rumble = lp.lp(rng.bi()) * decay(t, 0.15);
        sat(ramp(t, 0.003) * (body + rumble), 1.6)
    });
    fade_out(&mut v, frames(0.05, sr));
    v
}

/// Gate a reverb return so each hit's tail stops after `hold` seconds (the
/// classic 80s gated snare).
fn gate_tail(wet: &mut Stereo, sr: f32, hits: &[f32], hold: f32) {
    let n = wet.len();
    let mut g = vec![0.0f32; n];
    for &h in hits {
        let a = frames(h, sr);
        let a2 = frames(h + 0.005, sr);
        let b = frames(h + hold, sr);
        let c = frames(h + hold + 0.05, sr);
        for (i, v) in g.iter_mut().enumerate().take(c.min(n)).skip(a) {
            let x = if i < a2 {
                (i - a) as f32 / (a2 - a) as f32
            } else if i < b {
                1.0
            } else {
                1.0 - (i - b) as f32 / (c - b) as f32
            };
            *v = v.max(x);
        }
    }
    for (i, gi) in g.iter().enumerate() {
        wet.l[i] *= gi;
        wet.r[i] *= gi;
    }
}

// ------------------------------------------------------------------- chill

/// Lo-fi hip-hop: swung soft drums, FM e-piano with tape wow, sub bass and
/// vinyl crackle. 80 bpm, Fmaj9 – Em7 – Dm9 – Cmaj9.
pub fn lofi_chill(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 80.0);
    s.sw = [0.0, 0.3, 0.0, 0.3];
    let sr = s.sr;
    let chords = [
        [57.0, 60.0, 64.0, 67.0],
        [55.0, 59.0, 62.0, 64.0],
        [53.0, 57.0, 60.0, 64.0],
        [52.0, 55.0, 59.0, 62.0],
    ];
    let roots = [41.0, 40.0, 38.0, 36.0];
    let end = 12;
    let mut keys = s.bus();
    let mut lead = s.bus();
    let mut bass = s.bus();
    let mut drums = s.bus();

    for bar in 0..=end {
        let ch = chords[bar % 4];
        let hits: &[(f32, f32, f32)] = if bar == end {
            &[(0.0, 7.0, 0.7)]
        } else {
            &[(0.0, 1.6, 0.68), (2.5, 1.3, 0.5)]
        };
        for &(b, d, v) in hits {
            let roll = if bar == end { 0.07 } else { 0.02 };
            let t0 = s.tbar(bar, b) + s.hum(0.008);
            for (k, &note) in ch.iter().enumerate() {
                let vel = s.vel(v, 0.12);
                let x = inst::epiano(sr, hz(note), vel, d * s.spb);
                put(
                    &mut keys,
                    sr,
                    &x,
                    t0 + k as f32 * roll,
                    0.5,
                    -0.3 + 0.2 * k as f32,
                );
            }
        }
    }

    for bar in 2..=end {
        let r = roots[bar % 4];
        let pat: &[(f32, f32, f32)] = if bar == end {
            &[(0.0, 5.0, 0.0)]
        } else {
            &[(0.0, 1.5, 0.0), (2.5, 0.9, 0.0), (3.5, 0.4, 7.0)]
        };
        for &(b, d, iv) in pat {
            let t = s.tbar(bar, b) + s.hum(0.005);
            let x = inst::sub_bass(sr, hz(r + iv), s.vel(0.9, 0.08), d * s.spb);
            put(&mut bass, sr, &x, t, 1.0, 0.0);
        }
    }

    let kick = drums::kick(sr, &mut s.rng, &Kick::SOFT);
    let snare = drums::snare(sr, &mut s.rng, 185.0, 0.55, 0.11);
    let rim = drums::rim(sr, &mut s.rng);
    let hats: Vec<Vec<f32>> = (0..4).map(|_| drums::hat(sr, &mut s.rng, 0.03)).collect();
    for bar in 2..end {
        let breakdown = bar == 10;
        let kp = if bar % 4 == 1 {
            "X.....x...X..x.."
        } else {
            "X.....x...X....."
        };
        if !breakdown {
            for (st, v) in steps(kp) {
                let t = s.ts(bar, st) + s.hum(0.004);
                put(&mut drums, sr, &kick, t, v, 0.0);
            }
        }
        for (st, v) in steps("....X.......X...") {
            let t = s.ts(bar, st) + 0.015 + s.hum(0.005);
            if breakdown {
                put(&mut drums, sr, &rim, t, v * 0.3, 0.1);
            } else {
                put(&mut drums, sr, &snare, t, v * 0.8, 0.05);
            }
        }
        let hp = if bar % 2 == 0 {
            "x.ox.ox.x.oxx.o."
        } else {
            "x.oxx.o.x.ox.xog"
        };
        for (st, v) in steps(hp) {
            let h = &hats[s.rng.below(4)];
            let t = s.ts(bar, st) + s.hum(0.005);
            let g = s.vel(v * 0.6, 0.2);
            put(&mut drums, sr, h, t, g, 0.3);
        }
    }
    let t = s.tbar(end, 0.0);
    put(&mut drums, sr, &kick, t, 0.8, 0.0);

    // a lazy e-piano melody (bars 6–9), answered once more in bars 10–11
    let mel = [
        (0.5f32, 1.0f32, 76.0f32),
        (1.5, 0.5, 79.0),
        (2.0, 1.5, 81.0),
        (3.5, 0.5, 79.0),
        (4.5, 1.0, 76.0),
        (5.5, 0.5, 74.0),
        (6.0, 1.75, 71.0),
        (8.5, 0.5, 69.0),
        (9.0, 0.5, 72.0),
        (9.5, 0.5, 74.0),
        (10.0, 1.5, 76.0),
        (11.5, 0.5, 74.0),
        (12.0, 0.5, 72.0),
        (12.5, 0.5, 74.0),
        (13.0, 2.5, 71.0),
    ];
    for (start, upto) in [(6usize, 16.0f32), (10, 8.0)] {
        for &(b, d, note) in mel.iter().filter(|m| m.0 < upto) {
            let t = s.tbar(start, b) + s.hum(0.01);
            let x = inst::epiano(sr, hz(note), s.vel(0.55, 0.1), d * s.spb);
            put(&mut lead, sr, &x, t, 0.6, 0.15);
        }
    }

    // lo-fi treatment: filtered intro, tape wow, dull drums
    let open = s.tb(8.0);
    keys.sweep(sr, Mode::Lp, 0.8, |t| {
        seg_exp(t, &[(0.0, 700.0), (open, 3400.0)])
    });
    lead.eq(Bq::Lp, 3800.0, 0.7, sr);
    let wow = |t: f32| 0.005 + 0.0011 * (TAU * 0.5 * t).sin() + 0.00025 * (TAU * 4.7 * t).sin();
    for b in [&mut keys, &mut lead] {
        b.l = mod_delay(&b.l, sr, wow);
        b.r = mod_delay(&b.r, sr, wow);
    }
    drums.eq(Bq::Lp, 6500.0, 0.7, sr);
    bass.eq(Bq::Lp, 900.0, 0.7, sr);

    let mut mix = s.bus();
    mix.add(&keys, 0, 0.75);
    mix.add(&lead, 0, 0.9);
    mix.add(&bass, 0, 0.4);
    mix.add(&drums, 0, 1.45);
    delay_send(&mut mix, &lead, sr, s.spb * 0.75, 0.35, 0.25);
    let mut verb_in = keys.clone();
    verb_in.add(&lead, 0, 1.0);
    verb_in.add(&drums, 0, 0.25);
    send(&mut mix, &verb_in, sr, hall(0.8, 1.0), 0.3);
    let p = mix.peak();
    let vin = vinyl(sr, s.n, &mut s.rng, 6.0);
    mix.add(&vin, 0, p * 0.08);
    mix.eq(Bq::Lp, 12000.0, 0.7, sr);
    glue(mix, 1.3)
}

/// Acoustic fingerpicked guitar, strummed ukulele, shaker and a whistled
/// tune. 96 bpm, G – Em – C – D.
pub fn acoustic_sunshine(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 96.0);
    let sr = s.sr;
    let gtr: [&[f32]; 4] = [
        &[43.0, 47.0, 50.0, 55.0, 59.0, 67.0],
        &[40.0, 47.0, 52.0, 55.0, 59.0, 64.0],
        &[48.0, 52.0, 55.0, 60.0, 64.0],
        &[50.0, 57.0, 62.0, 66.0],
    ];
    // ukulele, re-entrant G-C-E-A strings in string order
    let uke = [
        [67.0, 62.0, 67.0, 71.0],
        [67.0, 64.0, 67.0, 71.0],
        [67.0, 60.0, 64.0, 72.0],
        [69.0, 62.0, 66.0, 69.0],
    ];
    let bass_roots = [43.0, 40.0, 36.0, 38.0];
    let end = 18;
    let mut guitar = s.bus();
    let mut ukes = s.bus();
    let mut bass = s.bus();
    let mut perc = s.bus();
    let mut lead = s.bus();

    // fingerpicking: alternating thumb bass with treble notes in between
    for bar in 0..end {
        let v = gtr[bar % 4];
        let m = v.len();
        let (b0, b1) = (0, if m >= 5 { 2 } else { 1 });
        let (t1, t2, t3) = (m - 3, m - 2, m - 1);
        let pattern: [&[usize]; 8] = [&[b0, t3], &[t1], &[b1], &[t2], &[b0], &[t3], &[b1], &[t1]];
        for (k, strings) in pattern.iter().enumerate() {
            for &st in strings.iter() {
                let t = s.tbar(bar, k as f32 * 0.5) + s.hum(0.006);
                let thumb = st == b0 || st == b1;
                let vel = s.vel(if thumb { 0.7 } else { 0.55 }, 0.1);
                let x = inst::string(sr, hz(v[st]), vel, 1.6 * s.spb, 0.55, 2.5, &mut s.rng);
                put(&mut guitar, sr, &x, t, 0.5, -0.35 + 0.12 * st as f32);
            }
        }
    }
    // final strum, slow and full
    let last = gtr[0];
    for (k, &note) in last.iter().enumerate() {
        let t = s.tbar(end, 0.0) + k as f32 * 0.03;
        let x = inst::string(sr, hz(note), 0.8, 4.0, 0.55, 3.0, &mut s.rng);
        put(&mut guitar, sr, &x, t, 0.5, -0.35 + 0.12 * k as f32);
    }

    // ukulele strums "D . D U . U D U" from bar 2
    let strum = [
        (0usize, true),
        (2, true),
        (3, false),
        (5, false),
        (6, true),
        (7, false),
    ];
    for bar in 2..=end {
        let ch = uke[bar % 4];
        let hits: Vec<(usize, bool)> = if bar == end {
            vec![(0, true)]
        } else {
            strum.to_vec()
        };
        for (i, &(e, down)) in hits.iter().enumerate() {
            let next = hits.get(i + 1).map(|h| h.0).unwrap_or(8);
            let gate = if bar == end {
                3.5
            } else {
                (next - e) as f32 * 0.5 * s.spb
            };
            let t0 = s.tbar(bar, e as f32 * 0.5) + s.hum(0.004);
            let vel = s.vel(if down { 0.75 } else { 0.5 }, 0.1);
            for k in 0..4 {
                let st = if down { k } else { 3 - k };
                let x = inst::string(sr, hz(ch[st]), vel, gate, 0.75, 1.3, &mut s.rng);
                put(
                    &mut ukes,
                    sr,
                    &x,
                    t0 + k as f32 * 0.011,
                    0.33,
                    0.3 - 0.06 * st as f32,
                );
            }
        }
    }

    // bass and percussion
    let kick = drums::kick(sr, &mut s.rng, &Kick::SOFT);
    let clap = drums::clap(sr, &mut s.rng, 0.07);
    let shakers: Vec<Vec<f32>> = (0..3)
        .map(|_| drums::shaker(sr, &mut s.rng, 0.03))
        .collect();
    for bar in 2..end {
        let r = bass_roots[bar % 4];
        let next = bass_roots[(bar + 1) % 4];
        let walk = if next > r { next - 1.0 } else { next + 2.0 };
        for &(b, d, note) in &[
            (0.0f32, 1.4f32, r),
            (1.5, 0.4, r),
            (2.0, 1.4, r + 7.0),
            (3.5, 0.4, walk),
        ] {
            let t = s.tbar(bar, b) + s.hum(0.004);
            let x = inst::round_bass(sr, hz(note), s.vel(0.85, 0.1), d * s.spb, &mut s.rng);
            put(&mut bass, sr, &x, t, 1.0, 0.0);
        }
        for (st, v) in steps("X.......x.......") {
            put(&mut perc, sr, &kick, s.ts(bar, st), v * 0.8, 0.0);
        }
        for (st, v) in steps("xoXoxoxoxoXoxoxo") {
            let sh = &shakers[s.rng.below(3)];
            let t = s.ts(bar, st) + s.hum(0.004);
            let g = s.vel(v * 0.35, 0.2);
            put(&mut perc, sr, sh, t, g, 0.35);
        }
        if bar >= 10 {
            for (st, v) in steps("....X.......X...") {
                let t = s.ts(bar, st) + s.hum(0.003);
                put(&mut perc, sr, &clap, t, v * 1.0, -0.1);
            }
        }
    }
    let x = inst::round_bass(sr, hz(43.0), 0.9, 4.0, &mut s.rng);
    put(&mut bass, sr, &x, s.tbar(end, 0.0), 1.0, 0.0);
    put(&mut perc, sr, &kick, s.tbar(end, 0.0), 0.8, 0.0);

    // whistled tune in bars 10–17
    let tune = [
        (0.0f32, 0.5f32, 74.0f32),
        (0.5, 0.5, 79.0),
        (1.0, 1.0, 83.0),
        (2.0, 1.0, 81.0),
        (3.0, 1.0, 79.0),
        (4.0, 1.5, 76.0),
        (5.5, 0.5, 79.0),
        (6.0, 2.0, 83.0),
        (8.0, 1.0, 84.0),
        (9.0, 0.5, 83.0),
        (9.5, 0.5, 81.0),
        (10.0, 1.0, 79.0),
        (11.0, 1.0, 76.0),
        (12.0, 1.5, 78.0),
        (13.5, 0.5, 81.0),
        (14.0, 2.0, 74.0),
    ];
    for start in [10usize, 14] {
        for &(b, d, note) in &tune {
            let t = s.tbar(start, b) + s.hum(0.008);
            let x = inst::whistle(sr, hz(note), s.vel(0.7, 0.08), d * s.spb * 0.92, &mut s.rng);
            put(&mut lead, sr, &x, t, 0.45, 0.1);
        }
    }

    // a touch of guitar body and air
    for b in [&mut guitar, &mut ukes] {
        b.eq(Bq::Hp, 80.0, 0.7, sr);
        b.eq(Bq::Peak(2.5), 180.0, 1.0, sr);
        b.eq(Bq::HighShelf(2.0), 5000.0, 0.7, sr);
    }
    bass.eq(Bq::Lp, 1200.0, 0.7, sr);

    let mut mix = s.bus();
    mix.add(&guitar, 0, 1.4);
    mix.add(&ukes, 0, 1.1);
    mix.add(&bass, 0, 0.38);
    mix.add(&perc, 0, 1.0);
    mix.add(&lead, 0, 0.66);
    delay_send(&mut mix, &lead, sr, s.spb * 0.75, 0.3, 0.2);
    let mut verb_in = guitar.clone();
    verb_in.add(&ukes, 0, 0.6);
    verb_in.add(&lead, 0, 1.0);
    verb_in.add(&perc, 0, 0.2);
    send(&mut mix, &verb_in, sr, hall(0.7, 0.9), 0.25);
    glue(mix, 1.2)
}

// ------------------------------------------------------------------ upbeat

/// Bright pop: four-on-the-floor, sidechained pad, pumping octave bass,
/// syncopated plucks and a saw-lead hook. 120 bpm, C – G – Am – F.
pub fn upbeat_pop(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 120.0);
    let sr = s.sr;
    let plk = [
        [60.0, 64.0, 67.0, 72.0],
        [59.0, 62.0, 67.0, 71.0],
        [60.0, 64.0, 69.0, 72.0],
        [60.0, 65.0, 69.0, 72.0],
    ];
    let pads = [
        [48.0, 55.0, 64.0, 67.0],
        [47.0, 55.0, 62.0, 67.0],
        [45.0, 57.0, 60.0, 64.0],
        [41.0, 57.0, 60.0, 65.0],
    ];
    let roots = [36.0, 31.0, 33.0, 29.0];
    let end = 20;
    let mut drums = s.bus();
    let mut bass = s.bus();
    let mut plucks = s.bus();
    let mut pad = s.bus();
    let mut lead = s.bus();
    let mut fx = s.bus();
    let mut kicks = Vec::new();

    for bar in 0..=end {
        let ch = plk[bar % 4];
        let pat = if bar == end {
            "x..............."
        } else {
            "x..x..x...x..x.."
        };
        for (st, v) in steps(pat) {
            let t = s.ts(bar, st) + s.hum(0.003);
            let gate = if bar == end { 3.0 } else { s.spb * 0.4 };
            for (k, &note) in ch.iter().enumerate() {
                let vel = s.vel(v * 0.85, 0.08);
                let x = inst::pluck_synth(sr, hz(note), vel, gate, 0.8);
                put(&mut plucks, sr, &x, t, 0.35, -0.4 + 0.27 * k as f32);
            }
        }
    }
    for bar in 4..=end {
        let gate = if bar == end { 6.0 * s.spb } else { 4.0 * s.spb };
        let p = inst::pad(sr, &mut s.rng, &pads[bar % 4], gate, &Pad::WARM);
        put_st(&mut pad, sr, &p, s.tbar(bar, 0.0), 0.7);
    }
    for bar in 4..end {
        let r = roots[bar % 4];
        for k in 0..8 {
            let note = if k % 2 == 1 { r + 12.0 } else { r };
            let t = s.tbar(bar, k as f32 * 0.5);
            let vel = if k % 2 == 0 { 0.95 } else { 0.75 };
            let x = inst::synth_bass(sr, hz(note), vel, s.spb * 0.4, 380.0, 3.0, 1.0);
            put(&mut bass, sr, &x, t, 0.8, 0.0);
        }
    }
    let x = inst::synth_bass(sr, hz(36.0), 1.0, 3.0, 380.0, 3.0, 1.0);
    put(&mut bass, sr, &x, s.tbar(end, 0.0), 0.8, 0.0);

    let kick = drums::kick(sr, &mut s.rng, &Kick::PUNCHY);
    let clap = drums::clap(sr, &mut s.rng, 0.09);
    let hats: Vec<Vec<f32>> = (0..3).map(|_| drums::hat(sr, &mut s.rng, 0.035)).collect();
    let ohat = drums::hat(sr, &mut s.rng, 0.16);
    let crash = drums::cymbal(sr, &mut s.rng, 0.9, 0.7);
    for bar in 0..end {
        let main = bar >= 4;
        let chorus = bar >= 12;
        if main {
            for (st, v) in steps("X...X...X...X...") {
                let t = s.ts(bar, st);
                kicks.push(t);
                put(&mut drums, sr, &kick, t, v, 0.0);
            }
        }
        if bar >= 2 {
            for (st, v) in steps("....X.......X...") {
                let t = s.ts(bar, st) + s.hum(0.002);
                put(&mut drums, sr, &clap, t, v * 1.2, 0.0);
            }
        }
        let hp = if chorus {
            "x.x.x.x.x.x.x.x."
        } else {
            "g.x.g.x.g.x.g.xo"
        };
        for (st, v) in steps(hp) {
            let h = &hats[s.rng.below(3)];
            let t = s.ts(bar, st) + s.hum(0.003);
            let g = s.vel(v * 0.6, 0.15);
            put(&mut drums, sr, h, t, g, 0.25);
        }
        if chorus {
            for (st, v) in steps("..x...x...x...x.") {
                put(&mut drums, sr, &ohat, s.ts(bar, st), v * 0.3, -0.2);
            }
        }
        if bar == 3 || bar == 11 {
            let r = noise_riser(sr, &mut s.rng, 4.0 * s.spb);
            put_st(&mut fx, sr, &r, s.tbar(bar, 0.0), 0.5);
        }
    }
    for bar in [4usize, 12, 16, end] {
        put(&mut drums, sr, &crash, s.tbar(bar, 0.0), 0.35, 0.3);
    }
    let t = s.tbar(end, 0.0);
    put(&mut drums, sr, &kick, t, 1.0, 0.0);
    put(&mut drums, sr, &clap, t, 0.5, 0.0);

    let hook = [
        (0.0f32, 0.5f32, 76.0f32),
        (0.5, 0.5, 79.0),
        (1.0, 0.75, 81.0),
        (1.75, 0.75, 79.0),
        (2.5, 0.5, 76.0),
        (3.0, 1.0, 74.0),
        (4.0, 0.5, 74.0),
        (4.5, 0.5, 76.0),
        (5.0, 0.75, 79.0),
        (5.75, 0.75, 76.0),
        (6.5, 1.5, 74.0),
        (8.0, 0.5, 72.0),
        (8.5, 0.5, 76.0),
        (9.0, 0.75, 81.0),
        (9.75, 0.75, 79.0),
        (10.5, 0.5, 76.0),
        (11.0, 1.0, 74.0),
        (12.0, 0.5, 77.0),
        (12.5, 0.5, 76.0),
        (13.0, 1.0, 72.0),
        (14.0, 1.5, 74.0),
    ];
    for start in [12usize, 16] {
        for &(b, d, note) in &hook {
            let t = s.tbar(start, b);
            let x = inst::saw_lead(sr, hz(note), 0.85, d * s.spb * 0.9);
            put(&mut lead, sr, &x, t, 0.4, 0.0);
        }
    }

    let drop = s.tb(16.0);
    plucks.sweep(sr, Mode::Lp, 1.0, |t| {
        seg_exp(t, &[(0.0, 450.0), (drop, 9000.0)])
    });
    duck(&mut pad, sr, &kicks, 0.65, 0.12);
    duck(&mut plucks, sr, &kicks, 0.35, 0.1);
    duck(&mut bass, sr, &kicks, 0.55, 0.08);

    let mut mix = s.bus();
    mix.add(&drums, 0, 0.78);
    mix.add(&bass, 0, 0.75);
    mix.add(&plucks, 0, 1.27);
    mix.add(&pad, 0, 0.6);
    mix.add(&lead, 0, 1.25);
    mix.add(&fx, 0, 0.7);
    delay_send(&mut mix, &lead, sr, s.spb * 0.75, 0.35, 0.3);
    delay_send(&mut mix, &plucks, sr, s.spb * 0.75, 0.25, 0.15);
    let mut verb_in = lead.clone();
    verb_in.add(&plucks, 0, 0.6);
    verb_in.add(&pad, 0, 0.5);
    verb_in.add(&drums, 0, 0.15);
    send(&mut mix, &verb_in, sr, hall(0.75, 0.9), 0.25);
    glue(mix, 1.5)
}

/// Uplifting corporate: pedalled piano arpeggios, strings, claps and a
/// glockenspiel tune. 112 bpm, G – D – Em – C.
pub fn corporate_inspire(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 112.0);
    let sr = s.sr;
    let rh = [
        [67.0, 71.0, 74.0, 79.0],
        [66.0, 69.0, 74.0, 78.0],
        [64.0, 67.0, 71.0, 76.0],
        [64.0, 67.0, 72.0, 76.0],
    ];
    let lh = [43.0, 38.0, 40.0, 36.0];
    let strings = [
        [55.0, 59.0, 62.0, 67.0],
        [54.0, 57.0, 62.0, 66.0],
        [55.0, 59.0, 64.0, 67.0],
        [55.0, 60.0, 64.0, 67.0],
    ];
    let bass_roots = [31.0, 38.0, 40.0, 36.0];
    let end = 20;
    let mut piano = s.bus();
    let mut strs = s.bus();
    let mut bass = s.bus();
    let mut drums = s.bus();
    let mut glock = s.bus();

    let arp = [0usize, 1, 2, 3, 2, 1, 2, 3];
    for bar in 0..end {
        let ch = rh[bar % 4];
        for (k, &ix) in arp.iter().enumerate() {
            let t = s.tbar(bar, k as f32 * 0.5) + s.hum(0.004);
            let gate = (4.0 - k as f32 * 0.5) * s.spb + 0.05;
            let vel = s.vel(if k % 2 == 0 { 0.6 } else { 0.47 }, 0.1);
            let x = inst::piano(sr, hz(ch[ix]), vel, gate, &mut s.rng);
            put(&mut piano, sr, &x, t, 0.5, 0.15);
        }
        for &(b, iv) in &[(0.0f32, 0.0f32), (2.0, 7.0)] {
            let t = s.tbar(bar, b);
            let x = inst::piano(sr, hz(lh[bar % 4] + iv), 0.55, 2.0 * s.spb, &mut s.rng);
            put(&mut piano, sr, &x, t, 0.55, -0.2);
        }
    }
    // final chord: rolled, with the low octave
    let t_end = s.tbar(end, 0.0);
    for (k, &note) in [31.0f32, 43.0, 55.0, 62.0, 67.0, 71.0, 74.0, 79.0]
        .iter()
        .enumerate()
    {
        let x = inst::piano(sr, hz(note), 0.62, 4.0, &mut s.rng);
        put(
            &mut piano,
            sr,
            &x,
            t_end + k as f32 * 0.025,
            0.5,
            -0.3 + 0.08 * k as f32,
        );
    }

    let str_pad = Pad {
        attack: 0.8,
        ..Pad::STRINGS
    };
    for bar in 4..=end {
        let gate = if bar == end { 3.5 } else { 4.0 * s.spb };
        let p = inst::pad(sr, &mut s.rng, &strings[bar % 4], gate, &str_pad);
        put_st(&mut strs, sr, &p, s.tbar(bar, 0.0), 0.8);
    }
    for bar in 4..=end {
        let r = bass_roots[bar % 4];
        let pat: &[(f32, f32)] = if bar == end {
            &[(0.0, 3.0)]
        } else {
            &[(0.0, 1.4), (1.5, 0.4), (2.0, 1.4), (3.5, 0.4)]
        };
        for &(b, d) in pat {
            let x = inst::sub_bass(sr, hz(r), 0.85, d * s.spb);
            put(&mut bass, sr, &x, s.tbar(bar, b), 1.0, 0.0);
        }
    }

    let kick = drums::kick(sr, &mut s.rng, &Kick::DEEP);
    let clap = drums::clap(sr, &mut s.rng, 0.1);
    let rim = drums::rim(sr, &mut s.rng);
    let shakers: Vec<Vec<f32>> = (0..3)
        .map(|_| drums::shaker(sr, &mut s.rng, 0.03))
        .collect();
    let hat = drums::hat(sr, &mut s.rng, 0.04);
    let crash = drums::cymbal(sr, &mut s.rng, 0.9, 0.6);
    for bar in 4..end {
        let b_sec = bar >= 12;
        for (st, v) in steps("X.......X.x.....") {
            put(&mut drums, sr, &kick, s.ts(bar, st), v * 0.9, 0.0);
        }
        for (st, v) in steps("xoxoXoxoxoxoXoxo") {
            let sh = &shakers[s.rng.below(3)];
            let t = s.ts(bar, st) + s.hum(0.003);
            let g = s.vel(v * 0.3, 0.15);
            put(&mut drums, sr, sh, t, g, 0.4);
        }
        for (st, v) in steps("....X.......X...") {
            let t = s.ts(bar, st) + s.hum(0.002);
            if b_sec {
                put(&mut drums, sr, &clap, t, v * 1.1, 0.0);
            } else {
                put(&mut drums, sr, &rim, t, v * 0.6, -0.1);
            }
        }
        if b_sec {
            for (st, v) in steps("x.x.x.x.x.x.x.x.") {
                put(&mut drums, sr, &hat, s.ts(bar, st), v * 0.35, 0.25);
            }
        }
    }
    for bar in [4usize, 12, 16, end] {
        put(&mut drums, sr, &crash, s.tbar(bar, 0.0), 0.3, 0.3);
    }
    put(&mut drums, sr, &kick, t_end, 0.9, 0.0);

    let tune = [
        (0.0f32, 1.0f32, 83.0f32),
        (1.0, 1.0, 81.0),
        (2.0, 1.5, 79.0),
        (3.5, 0.5, 81.0),
        (4.0, 1.0, 81.0),
        (5.0, 1.0, 78.0),
        (6.0, 2.0, 74.0),
        (8.0, 1.0, 79.0),
        (9.0, 1.0, 83.0),
        (10.0, 1.5, 86.0),
        (11.5, 0.5, 83.0),
        (12.0, 1.0, 84.0),
        (13.0, 1.0, 83.0),
        (14.0, 2.0, 79.0),
    ];
    for start in [12usize, 16] {
        for &(b, _d, note) in &tune {
            let t = s.tbar(start, b);
            let x = inst::glock(sr, hz(note), 0.8, 0.6);
            put(&mut glock, sr, &x, t, 0.35, 0.2);
        }
    }
    let x = inst::glock(sr, hz(91.0), 0.8, 0.9);
    put(&mut glock, sr, &x, t_end, 0.3, 0.2);

    let mut mix = s.bus();
    mix.add(&piano, 0, 0.85);
    mix.add(&strs, 0, 0.6);
    mix.add(&bass, 0, 0.3);
    mix.add(&drums, 0, 0.8);
    mix.add(&glock, 0, 0.9);
    delay_send(&mut mix, &glock, sr, s.spb * 0.75, 0.3, 0.25);
    let mut verb_in = piano.clone();
    verb_in.add(&strs, 0, 0.6);
    verb_in.add(&glock, 0, 1.0);
    verb_in.add(&drums, 0, 0.15);
    send(&mut mix, &verb_in, sr, hall(0.8, 1.0), 0.28);
    glue(mix, 1.3)
}

/// Funk: syncopated drums with ghost notes, slap-style synth bass, clav and
/// a horn section. 105 bpm, Em9 – A13 vamp with a Cmaj7 – B7 turnaround.
pub fn funky_groove(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 105.0);
    s.sw = [0.0, 0.12, 0.0, 0.12];
    let sr = s.sr;
    // per bar: (bass root, clav voicing, horn voicing, minor?)
    let em = (
        40.0,
        [64.0, 67.0, 71.0, 74.0],
        [59.0, 62.0, 66.0, 67.0],
        true,
    );
    let a = (
        33.0,
        [61.0, 66.0, 67.0, 71.0],
        [61.0, 66.0, 67.0, 71.0],
        false,
    );
    let cm = (
        36.0,
        [64.0, 67.0, 71.0, 72.0],
        [59.0, 64.0, 67.0, 71.0],
        false,
    );
    let b7 = (
        35.0,
        [63.0, 66.0, 69.0, 71.0],
        [63.0, 66.0, 69.0, 71.0],
        false,
    );
    let end = 18;
    let prog = |bar: usize| {
        if bar == 16 {
            cm
        } else if bar == 17 {
            b7
        } else if bar.is_multiple_of(2) {
            em
        } else {
            a
        }
    };
    let mut drums = s.bus();
    let mut bass = s.bus();
    let mut keys = s.bus();
    let mut horns = s.bus();

    let kick = drums::kick(sr, &mut s.rng, &Kick::PUNCHY);
    let snare = drums::snare(sr, &mut s.rng, 200.0, 0.8, 0.1);
    let hats: Vec<Vec<f32>> = (0..4).map(|_| drums::hat(sr, &mut s.rng, 0.028)).collect();
    let ohat = drums::hat(sr, &mut s.rng, 0.14);
    let crash = drums::cymbal(sr, &mut s.rng, 0.9, 0.7);
    for bar in 0..end {
        let b_sec = bar >= 10;
        let kp = if bar % 2 == 1 {
            "X.x.......X..x.."
        } else {
            "X.x.......X....x"
        };
        for (st, v) in steps(kp) {
            put(&mut drums, sr, &kick, s.ts(bar, st), v * 0.9, 0.0);
        }
        for (st, v) in steps("....X..g.g..X..g") {
            let t = s.ts(bar, st) + s.hum(0.003);
            put(&mut drums, sr, &snare, t, v * 0.6, 0.08);
        }
        let hp = if b_sec {
            "xoxoxoxoxoxoxo.o"
        } else {
            "xoxoxoxoxoxoxoxo"
        };
        for (st, v) in steps(hp) {
            let h = &hats[s.rng.below(4)];
            let t = s.ts(bar, st) + s.hum(0.003);
            let g = s.vel(v * 0.5, 0.15);
            put(&mut drums, sr, h, t, g, 0.3);
        }
        if b_sec {
            put(&mut drums, sr, &ohat, s.ts(bar, 14), 0.22, 0.3);
        }
    }
    for bar in [2usize, 10, end] {
        put(&mut drums, sr, &crash, s.tbar(bar, 0.0), 0.3, -0.3);
    }

    // bass: (step, interval, length in 16ths, velocity)
    let line = [
        (0usize, 0.0f32, 3.0f32, 1.0f32),
        (3, 0.0, 1.0, 0.5),
        (6, 12.0, 1.0, 0.95),
        (7, 10.0, 1.0, 0.7),
        (10, 0.0, 2.0, 0.9),
        (11, 3.0, 1.0, 0.6),
        (13, 5.0, 1.0, 0.8),
        (14, 7.0, 1.5, 0.85),
    ];
    for bar in 1..end {
        let (root, _, _, minor) = prog(bar);
        for &(st, iv, len, v) in &line {
            if bar == 1 && st < 10 {
                continue; // pickup into the groove
            }
            let iv = if iv == 3.0 && !minor { 4.0 } else { iv };
            let t = s.ts(bar, st) + s.hum(0.003);
            let gate = len * s.spb / 4.0 * 0.85;
            let x = inst::synth_bass(sr, hz(root + iv), v, gate, 300.0, 6.0, 2.2);
            put(&mut bass, sr, &x, t, 0.8, 0.0);
        }
    }

    for bar in 2..end {
        let (_, v, _, _) = prog(bar);
        for (st, vel) in steps(".x.xo..x.x.xo..x") {
            let t = s.ts(bar, st) + s.hum(0.003);
            let gate = if vel < 0.6 { 0.04 } else { s.spb * 0.2 };
            for (k, &note) in v.iter().enumerate() {
                let x = inst::clav(sr, hz(note), vel, gate);
                put(
                    &mut keys,
                    sr,
                    &x,
                    t + k as f32 * 0.003,
                    0.3,
                    -0.35 + 0.1 * k as f32,
                );
            }
        }
    }
    for bar in 10..end {
        let (_, _, hv, _) = prog(bar);
        let hits: &[(usize, f32)] = if bar % 2 == 1 {
            &[(6, 0.15), (14, 0.15)]
        } else {
            &[(0, 0.2), (6, 0.15), (10, 0.3)]
        };
        for &(st, beats) in hits {
            let t = s.ts(bar, st);
            for (k, &note) in hv.iter().enumerate() {
                let x = inst::brass(sr, hz(note), 0.85, beats * s.spb + 0.08, &mut s.rng);
                put(&mut horns, sr, &x, t, 0.35, -0.3 + 0.2 * k as f32);
            }
        }
    }

    // ending: big E9 hit with horns, clav, bass and crash
    let t_end = s.tbar(end, 0.0);
    for (k, &note) in [52.0f32, 56.0, 62.0, 66.0, 71.0].iter().enumerate() {
        let x = inst::brass(sr, hz(note), 0.95, 1.6, &mut s.rng);
        put(&mut horns, sr, &x, t_end, 0.35, -0.4 + 0.2 * k as f32);
        let y = inst::clav(sr, hz(note + 12.0), 0.8, 0.6);
        put(
            &mut keys,
            sr,
            &y,
            t_end + k as f32 * 0.004,
            0.25,
            -0.3 + 0.15 * k as f32,
        );
    }
    let x = inst::synth_bass(sr, hz(28.0), 1.0, 1.8, 300.0, 6.0, 2.2);
    put(&mut bass, sr, &x, t_end, 0.8, 0.0);
    put(&mut drums, sr, &kick, t_end, 1.0, 0.0);
    put(&mut drums, sr, &snare, t_end, 0.6, 0.08);

    keys.eq(Bq::Hp, 200.0, 0.7, sr);
    let mut mix = s.bus();
    mix.add(&drums, 0, 1.0);
    mix.add(&bass, 0, 0.75);
    mix.add(&keys, 0, 0.8);
    mix.add(&horns, 0, 1.2);
    let mut verb_in = horns.clone();
    verb_in.add(&keys, 0, 0.4);
    verb_in.add(&drums, 0, 0.15);
    send(&mut mix, &verb_in, sr, hall(0.6, 0.7), 0.22);
    glue(mix, 1.5)
}

// --------------------------------------------------------------- cinematic

/// Driving cinematic ostinato with strings, taikos, braams and risers.
/// 96 bpm, Dm – Bb – Gm – A.
pub fn cinematic_pulse(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 96.0);
    let sr = s.sr;
    let roots = [50.0, 46.0, 43.0, 45.0];
    let pads = [
        [50.0, 57.0, 62.0, 65.0],
        [46.0, 53.0, 58.0, 62.0],
        [43.0, 50.0, 58.0, 62.0],
        [45.0, 52.0, 57.0, 61.0],
    ];
    let end = 16;
    let mut ost = s.bus();
    let mut strs = s.bus();
    let mut hi = s.bus();
    let mut perc = s.bus();
    let mut fx = s.bus();

    let accents = [0usize, 3, 6, 10, 13];
    for bar in 0..end {
        let r = roots[bar % 4];
        for st in 0..16 {
            let acc = accents.contains(&st);
            let vel = s.vel(if acc { 1.0 } else { 0.55 }, 0.06);
            let note = r + if acc && st != 0 { 12.0 } else { 0.0 };
            let t = s.ts(bar, st);
            let x = inst::staccato(sr, hz(note), vel, s.spb * 0.15, &mut s.rng);
            put(
                &mut ost,
                sr,
                &x,
                t,
                0.5,
                if st % 2 == 0 { -0.25 } else { 0.25 },
            );
            if bar >= 8 && acc {
                let y = inst::staccato(sr, hz(note + 12.0), vel * 0.8, s.spb * 0.15, &mut s.rng);
                put(&mut ost, sr, &y, t, 0.3, 0.0);
            }
        }
    }
    let lows = Pad {
        attack: 1.2,
        ..Pad::STRINGS
    };
    for bar in 4..=end {
        let gate = if bar == end { 4.0 } else { 4.0 * s.spb };
        let p = inst::pad(sr, &mut s.rng, &pads[bar % 4], gate, &lows);
        put_st(&mut strs, sr, &p, s.tbar(bar, 0.0), 0.9);
    }
    let line = [
        [(0.0f32, 4.0f32, 74.0f32), (0.0, 0.0, 0.0)],
        [(0.0, 2.0, 77.0), (2.0, 2.0, 74.0)],
        [(0.0, 4.0, 70.0), (0.0, 0.0, 0.0)],
        [(0.0, 2.0, 73.0), (2.0, 2.0, 76.0)],
        [(0.0, 4.0, 77.0), (0.0, 0.0, 0.0)],
        [(0.0, 2.0, 74.0), (2.0, 2.0, 77.0)],
        [(0.0, 2.0, 79.0), (2.0, 2.0, 77.0)],
        [(0.0, 4.0, 76.0), (0.0, 0.0, 0.0)],
    ];
    let violin = Pad {
        attack: 0.35,
        release: 0.4,
        cutoff: 3800.0,
        q: 0.6,
        detune: 0.06,
        voices: 3,
        lfo: 0.3,
        vib: 0.005,
        hp: 300.0,
    };
    for (k, notes) in line.iter().enumerate() {
        for &(b, d, note) in notes.iter().filter(|n| n.1 > 0.0) {
            let p = inst::pad(sr, &mut s.rng, &[note], d * s.spb, &violin);
            put_st(&mut hi, sr, &p, s.tbar(8 + k, b), 0.9);
        }
    }
    let p = inst::pad(sr, &mut s.rng, &[74.0, 77.0], 4.0, &violin);
    put_st(&mut hi, sr, &p, s.tbar(end, 0.0), 0.8);

    let taiko_lo: Vec<Vec<f32>> = (0..3).map(|_| drums::taiko(sr, &mut s.rng, 68.0)).collect();
    let tom_hi = drums::tom(sr, &mut s.rng, 150.0, 0.2);
    let tom_lo = drums::tom(sr, &mut s.rng, 100.0, 0.25);
    let snare = drums::snare(sr, &mut s.rng, 180.0, 0.9, 0.16);
    let crash = drums::cymbal(sr, &mut s.rng, 1.0, 0.5);
    let bm = boom(sr, &mut s.rng);
    for bar in 4..end {
        let pat = if bar < 8 {
            "X.......X......."
        } else {
            "X..x..X...X.x..."
        };
        for (st, v) in steps(pat) {
            let tk = &taiko_lo[s.rng.below(3)];
            put(
                &mut perc,
                sr,
                tk,
                s.ts(bar, st),
                v * 0.8,
                s.rng.range(-0.2, 0.2),
            );
        }
        if bar >= 12 {
            for (st, v) in steps("....X.......X...") {
                put(&mut perc, sr, &snare, s.ts(bar, st), v * 0.5, 0.0);
            }
        }
        if bar == 7 || bar == 15 {
            for st in 8..16 {
                let tom = if st % 2 == 0 { &tom_hi } else { &tom_lo };
                let v = 0.4 + 0.5 * (st - 8) as f32 / 8.0;
                put(
                    &mut perc,
                    sr,
                    tom,
                    s.ts(bar, st),
                    v,
                    if st % 2 == 0 { -0.3 } else { 0.3 },
                );
            }
            let r = noise_riser(sr, &mut s.rng, 4.0 * s.spb);
            put_st(&mut fx, sr, &r, s.tbar(bar, 0.0), 0.6);
        }
        if bar >= 8 && bar % 2 == 0 {
            put(&mut perc, sr, &bm, s.tbar(bar, 0.0), 0.7, 0.0);
        }
    }
    for bar in [8usize, 12] {
        let b = inst::braam(sr, &mut s.rng, &[38.0, 45.0, 50.0], 3.0 * s.spb);
        put_st(&mut fx, sr, &b, s.tbar(bar, 0.0), 0.45);
        put(&mut perc, sr, &crash, s.tbar(bar, 0.0), 0.35, 0.3);
    }
    let t_end = s.tbar(end, 0.0);
    let b = inst::braam(sr, &mut s.rng, &[38.0, 45.0, 50.0, 53.0], 2.0);
    put_st(&mut fx, sr, &b, t_end, 0.5);
    put(&mut perc, sr, &bm, t_end, 1.0, 0.0);
    put(&mut perc, sr, &taiko_lo[0], t_end, 1.0, 0.0);
    put(&mut perc, sr, &crash, t_end, 0.4, -0.3);

    let open = s.tb(32.0);
    ost.sweep(sr, Mode::Lp, 0.9, |t| {
        seg_exp(t, &[(0.0, 350.0), (open, 4500.0)])
    });
    let mut mix = s.bus();
    mix.add(&ost, 0, 2.1);
    mix.add(&strs, 0, 0.65);
    mix.add(&hi, 0, 0.55);
    mix.add(&perc, 0, 0.78);
    mix.add(&fx, 0, 0.9);
    let mut verb_in = strs.clone();
    verb_in.add(&hi, 0, 1.0);
    verb_in.add(&ost, 0, 0.4);
    verb_in.add(&perc, 0, 0.4);
    let mut big = hall(0.88, 1.3);
    big.hp = 200.0;
    send(&mut mix, &verb_in, sr, big, 0.35);
    glue(mix, 1.5)
}

/// 80s synthwave: gated-reverb snare, pulsing bass, square arpeggio,
/// sidechained pad and a saw lead. 100 bpm, Am – F – C – G.
pub fn retro_synthwave(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 100.0);
    let sr = s.sr;
    let pads = [
        [57.0, 60.0, 64.0, 69.0],
        [57.0, 60.0, 65.0, 69.0],
        [55.0, 60.0, 64.0, 67.0],
        [55.0, 59.0, 62.0, 67.0],
    ];
    let roots = [33.0, 29.0, 36.0, 31.0];
    let end = 20;
    let mut pad = s.bus();
    let mut arp = s.bus();
    let mut bass = s.bus();
    let mut drums = s.bus();
    let mut snares = s.bus();
    let mut lead = s.bus();
    let mut kicks = Vec::new();
    let mut snare_t = Vec::new();

    let lush = Pad {
        attack: 0.4,
        release: 0.6,
        cutoff: 2400.0,
        q: 0.8,
        detune: 0.15,
        voices: 4,
        lfo: 0.1,
        vib: 0.0,
        hp: 180.0,
    };
    for bar in 0..=end {
        let gate = if bar == end { 3.0 } else { 4.0 * s.spb };
        let p = inst::pad(sr, &mut s.rng, &pads[bar % 4], gate, &lush);
        put_st(&mut pad, sr, &p, s.tbar(bar, 0.0), 0.8);
    }
    let order = [0usize, 1, 2, 3, 2, 1, 2, 3, 0, 1, 2, 3, 2, 3, 2, 1];
    for bar in 0..end {
        let ch = pads[bar % 4];
        for (st, &ix) in order.iter().enumerate() {
            let note = ch[ix] + 12.0;
            let x = inst::square_pluck(
                sr,
                hz(note),
                if st % 4 == 0 { 0.8 } else { 0.6 },
                s.spb * 0.2,
            );
            put(&mut arp, sr, &x, s.ts(bar, st), 0.3, 0.0);
        }
    }
    for bar in 4..end {
        let r = roots[bar % 4];
        for k in 0..8 {
            let note = if k == 7 { r + 12.0 } else { r };
            let x = inst::synth_bass(
                sr,
                hz(note),
                if k % 2 == 0 { 0.9 } else { 0.7 },
                s.spb * 0.42,
                260.0,
                4.0,
                1.2,
            );
            put(&mut bass, sr, &x, s.tbar(bar, k as f32 * 0.5), 0.8, 0.0);
        }
    }
    let x = inst::synth_bass(sr, hz(33.0), 1.0, 2.5, 260.0, 4.0, 1.2);
    put(&mut bass, sr, &x, s.tbar(end, 0.0), 0.8, 0.0);

    let kick = drums::kick(sr, &mut s.rng, &Kick::DEEP);
    let snare = drums::snare(sr, &mut s.rng, 175.0, 0.9, 0.12);
    let clap = drums::clap(sr, &mut s.rng, 0.08);
    let hats: Vec<Vec<f32>> = (0..3).map(|_| drums::hat(sr, &mut s.rng, 0.03)).collect();
    let tom_a = drums::tom(sr, &mut s.rng, 160.0, 0.22);
    let tom_b = drums::tom(sr, &mut s.rng, 120.0, 0.25);
    let tom_c = drums::tom(sr, &mut s.rng, 85.0, 0.3);
    let crash = drums::cymbal(sr, &mut s.rng, 1.0, 0.6);
    for bar in 4..end {
        for (st, v) in steps("X.......X.......") {
            let t = s.ts(bar, st);
            kicks.push(t);
            put(&mut drums, sr, &kick, t, v, 0.0);
        }
        for (st, v) in steps("....X.......X...") {
            let t = s.ts(bar, st);
            snare_t.push(t);
            put(&mut snares, sr, &snare, t, v * 0.7, 0.0);
            put(&mut snares, sr, &clap, t, v * 0.3, 0.0);
        }
        for (st, v) in steps("x.x.x.x.x.x.x.xo") {
            let h = &hats[s.rng.below(3)];
            put(&mut drums, sr, h, s.ts(bar, st), v * 0.5, 0.3);
        }
        if bar == 11 || bar == 19 {
            for (k, st) in (8..16).enumerate() {
                let tom = [
                    &tom_a, &tom_a, &tom_b, &tom_b, &tom_c, &tom_c, &tom_c, &tom_c,
                ][k];
                put(
                    &mut drums,
                    sr,
                    tom,
                    s.ts(bar, st),
                    0.6,
                    0.4 - 0.1 * k as f32,
                );
            }
        }
    }
    for bar in [4usize, 12, end] {
        put(&mut drums, sr, &crash, s.tbar(bar, 0.0), 0.35, 0.3);
    }
    let t_end = s.tbar(end, 0.0);
    put(&mut drums, sr, &kick, t_end, 1.0, 0.0);
    put(&mut snares, sr, &snare, t_end, 0.7, 0.0);
    snare_t.push(t_end);

    let tune = [
        (0.0f32, 1.5f32, 81.0f32),
        (1.5, 0.5, 79.0),
        (2.0, 2.0, 76.0),
        (4.0, 1.5, 77.0),
        (5.5, 0.5, 76.0),
        (6.0, 1.0, 72.0),
        (7.0, 1.0, 74.0),
        (8.0, 1.5, 76.0),
        (9.5, 0.5, 74.0),
        (10.0, 2.0, 72.0),
        (12.0, 1.5, 74.0),
        (13.5, 0.5, 76.0),
        (14.0, 2.0, 79.0),
    ];
    for start in [12usize, 16] {
        for &(b, d, note) in &tune {
            let x = inst::saw_lead(sr, hz(note), 0.85, d * s.spb * 0.95);
            put(&mut lead, sr, &x, s.tbar(start, b), 0.4, 0.0);
        }
    }

    let intro = s.tb(16.0);
    arp.sweep(sr, Mode::Lp, 1.2, |t| {
        seg_exp(t, &[(0.0, 500.0), (intro, 5000.0)])
    });
    duck(&mut pad, sr, &kicks, 0.6, 0.15);
    duck(&mut bass, sr, &kicks, 0.5, 0.1);
    chorus(&mut pad, sr, 0.012, 0.003, 0.4, 0.5);
    pad.width(1.3);
    let mut gated = reverb(&snares, sr, &hall(0.92, 1.2));
    gate_tail(&mut gated, sr, &snare_t, 0.22);

    let mut mix = s.bus();
    mix.add(&pad, 0, 0.8);
    mix.add(&arp, 0, 0.7);
    mix.add(&bass, 0, 0.7);
    mix.add(&drums, 0, 0.8);
    mix.add(&snares, 0, 1.0);
    mix.add(&gated, 0, 0.25);
    mix.add(&lead, 0, 1.25);
    delay_send(&mut mix, &arp, sr, s.spb * 0.75, 0.4, 0.35);
    delay_send(&mut mix, &lead, sr, s.spb * 0.75, 0.4, 0.3);
    let mut verb_in = lead.clone();
    verb_in.add(&pad, 0, 0.6);
    verb_in.add(&arp, 0, 0.4);
    send(&mut mix, &verb_in, sr, hall(0.85, 1.1), 0.3);
    glue(mix, 1.5)
}

// ----------------------------------------------------------------- ambient

/// Slow evolving pads, a low drone and sparse echoing bells. 60 bpm,
/// Dmaj9 – Bm9 – Gmaj9(#11) – Aadd9.
pub fn ambient_dreams(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 60.0);
    let sr = s.sr;
    let n = s.n;
    let chords: [&[f32]; 5] = [
        &[50.0, 57.0, 61.0, 64.0, 66.0],
        &[47.0, 54.0, 57.0, 61.0, 62.0],
        &[43.0, 50.0, 54.0, 57.0, 61.0],
        &[45.0, 52.0, 59.0, 61.0, 64.0],
        &[50.0, 57.0, 61.0, 64.0, 66.0, 69.0],
    ];
    let starts = [2.5f32, 10.5, 18.5, 26.5, 34.5];
    let mut pads = s.bus();
    let mut bells = s.bus();
    let warm = Pad {
        attack: 2.5,
        release: 1.4,
        cutoff: 1300.0,
        q: 0.8,
        detune: 0.14,
        voices: 3,
        lfo: 0.07,
        vib: 0.0,
        hp: 120.0,
    };
    for (k, ch) in chords.iter().enumerate() {
        let gate = if k == 4 { 5.5 } else { 8.6 };
        let p = inst::pad(sr, &mut s.rng, ch, gate, &warm);
        put_st(&mut pads, sr, &p, starts[k], 0.9);
    }

    // drone on D and A, swelling in and out
    let (mut d1, mut d2) = (Osc::default(), Osc::default());
    let len = n as f32 / sr;
    let drone = gen(sr, n, |t| {
        let e = smoothstep(t / 5.0) * (1.0 - smoothstep((t - (len - 9.0)) / 7.0));
        (d1.sine(hz(38.0), sr) * 0.5 + d2.sine(hz(45.0), sr) * 0.18) * e
    });

    // sparse bells from the current chord's upper notes
    let mut t = 4.0;
    while t < 40.0 {
        let k = starts.iter().rposition(|&st| st <= t).unwrap_or(0);
        let ch = chords[k];
        let note = ch[ch.len() - 1 - s.rng.below(3)] + if s.rng.chance(0.5) { 12.0 } else { 24.0 };
        let vel = s.rng.range(0.3, 0.6);
        let x = if s.rng.chance(0.5) {
            inst::fm_bell(sr, hz(note), vel, 1.4)
        } else {
            inst::glock(sr, hz(note), vel, 1.2)
        };
        let pan = s.rng.range(-0.7, 0.7);
        put(&mut bells, sr, &x, t, 0.3, pan);
        t += s.rng.range(0.8, 2.2);
    }
    let x = inst::fm_bell(sr, hz(81.0), 0.5, 1.6);
    put(&mut bells, sr, &x, starts[4] + 0.5, 0.3, 0.0);

    // airy texture
    let mut air = s.bus();
    let mut pl = Pink::new(s.rng.fork());
    let mut pr = Pink::new(s.rng.fork());
    let mut wnd = Wander::new(s.rng.fork(), 0.1, sr);
    let mut fl = Svf::new(2500.0, 0.7, sr);
    let mut fr = fl;
    for i in 0..n {
        let ti = i as f32 / sr;
        let g = (0.6 + 0.4 * wnd.next()) * smoothstep(ti / 6.0) * 0.04;
        air.l[i] = fl.bp(pl.next()) * g;
        air.r[i] = fr.bp(pr.next()) * g;
    }

    chorus(&mut pads, sr, 0.015, 0.004, 0.25, 0.5);
    pads.width(1.35);
    let mut mix = s.bus();
    mix.add(&pads, 0, 1.0);
    mix.add_mono(&drone, 0, 0.25, 0.0);
    mix.add(&bells, 0, 1.4);
    mix.add(&air, 0, 4.0);
    delay_send(&mut mix, &bells, sr, 0.75, 0.45, 0.5);
    let mut verb_in = pads.clone();
    verb_in.add(&bells, 0, 1.5);
    let mut big = hall(0.95, 1.4);
    big.lp = 6000.0;
    send(&mut mix, &verb_in, sr, big, 0.6);
    glue(mix, 1.1)
}

/// Solo piano with pedalled left-hand arpeggios, a simple melody, soft
/// strings and a ritardando into the final chord. 72 bpm, C – G/B – Am – F.
pub fn calm_piano(c: &mut Ctx) -> Stereo {
    let mut s = Song::new(c, 72.0);
    s.rit = (44.0, 0.02);
    let sr = s.sr;
    let lh = [
        [36.0, 43.0, 48.0, 52.0],
        [35.0, 43.0, 47.0, 50.0],
        [33.0, 40.0, 45.0, 48.0],
        [29.0, 36.0, 41.0, 45.0],
    ];
    let strings = [
        [55.0, 60.0, 64.0],
        [55.0, 59.0, 62.0],
        [57.0, 60.0, 64.0],
        [57.0, 60.0, 65.0],
    ];
    let end = 13;
    let mut piano = s.bus();
    let mut strs = s.bus();

    // the intro bar is F, so the melody's first bar lands on C
    let chord = |bar: usize| (bar + 3) % 4;
    let pattern = [0usize, 1, 2, 3, 2, 1, 2, 3];
    for bar in 0..end {
        let ch = lh[chord(bar)];
        for (k, &ix) in pattern.iter().enumerate() {
            let b = bar as f32 * 4.0 + k as f32 * 0.5;
            let t = s.tb(b) + s.hum(0.006);
            let gate = s.tb(bar as f32 * 4.0 + 4.0) - s.tb(b) + 0.05;
            let vel = s.vel(if k == 0 { 0.5 } else { 0.38 }, 0.12);
            let x = inst::piano(sr, hz(ch[ix]), vel, gate, &mut s.rng);
            put(&mut piano, sr, &x, t, 0.5, -0.25 + 0.1 * ix as f32);
        }
    }
    let tune = [
        (0.0f32, 2.0f32, 76.0f32),
        (2.0, 1.0, 74.0),
        (3.0, 1.0, 72.0),
        (4.0, 2.0, 74.0),
        (6.0, 1.0, 71.0),
        (7.0, 1.0, 74.0),
        (8.0, 1.5, 72.0),
        (9.5, 0.5, 71.0),
        (10.0, 2.0, 69.0),
        (12.0, 1.0, 69.0),
        (13.0, 1.0, 72.0),
        (14.0, 2.0, 77.0),
        (16.0, 2.0, 79.0),
        (18.0, 1.0, 76.0),
        (19.0, 1.0, 72.0),
        (20.0, 2.0, 74.0),
        (22.0, 2.0, 79.0),
        (24.0, 1.5, 76.0),
        (25.5, 0.5, 74.0),
        (26.0, 2.0, 72.0),
        (28.0, 2.0, 69.0),
        (30.0, 2.0, 67.0),
    ];
    let play =
        |s: &mut Song, piano: &mut Stereo, start_beat: f32, upto: f32, shift: f32, vel: f32| {
            for &(b, d, note) in tune.iter().filter(|m| m.0 < upto) {
                let t = s.tb(start_beat + b) + s.hum(0.01);
                let gate = s.tb(start_beat + b + d) - s.tb(start_beat + b) + 0.1;
                let v = s.vel(vel, 0.1);
                let x = inst::piano(sr, hz(note + shift), v, gate, &mut s.rng);
                put(piano, sr, &x, t, 0.55, 0.2);
            }
        };
    play(&mut s, &mut piano, 4.0, 32.0, 0.0, 0.62);
    play(&mut s, &mut piano, 36.0, 16.0, 12.0, 0.5);

    // final chord: rolled C(add9) over a low C
    let t_end = s.tbar(end, 0.0);
    for (k, &note) in [24.0f32, 36.0, 43.0, 52.0, 55.0, 62.0, 64.0, 72.0]
        .iter()
        .enumerate()
    {
        let x = inst::piano(sr, hz(note), 0.5, 4.5, &mut s.rng);
        put(
            &mut piano,
            sr,
            &x,
            t_end + k as f32 * 0.045,
            0.5,
            -0.3 + 0.08 * k as f32,
        );
    }

    let soft = Pad {
        attack: 1.5,
        release: 0.8,
        cutoff: 1800.0,
        ..Pad::STRINGS
    };
    for bar in 5..=end {
        let notes = strings[chord(bar)];
        let gate = if bar == end {
            4.0
        } else {
            s.tbar(bar + 1, 0.0) - s.tbar(bar, 0.0)
        };
        let p = inst::pad(sr, &mut s.rng, &notes, gate, &soft);
        put_st(&mut strs, sr, &p, s.tbar(bar, 0.0), 0.5);
    }

    let mut mix = s.bus();
    mix.add(&piano, 0, 1.0);
    mix.add(&strs, 0, 0.85);
    let mut verb_in = piano.clone();
    verb_in.add(&strs, 0, 0.7);
    send(&mut mix, &verb_in, sr, hall(0.85, 1.1), 0.35);
    glue(mix, 1.1)
}
