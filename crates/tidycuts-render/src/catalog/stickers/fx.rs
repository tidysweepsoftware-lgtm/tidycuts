//! Moving effects as stickers: bursts and impacts, confetti and balloons,
//! fire, smoke, lightning and energy. All loop seamlessly and are never
//! empty, so they can sit on top of a clip as they are.
//!
//! `ctx.variant` picks the look inside each drawing; the catalog gives every
//! variant its own name and colours. The main colour is the body of the
//! effect, the second colour is its accent or glow.

use super::kit::*;
use super::DrawCtx;
use crate::motion::Spring;
use std::f32::consts::{PI, TAU};
use tiny_skia::{PathBuilder, Pixmap};

/// A seamless random number for slot `i`, stream `s`.
fn rnd(i: u32, s: u32) -> f32 {
    hash(i.wrapping_mul(131).wrapping_add(s.wrapping_mul(7919)))
}

/// 1 well inside the picture, fading to 0 towards its edges, so moving
/// pieces never end in a hard cut at the border.
fn edge_k(w: f32, x: f32, y: f32) -> f32 {
    edge_soft(w, x, y, 0.08, 0.22)
}

/// The same with the fade starting  from the edge and finished  in.
fn edge_soft(w: f32, x: f32, y: f32, a: f32, b: f32) -> f32 {
    let d = x.min(w - x).min(y).min(1.0 - y);
    smoothstep(a, b, d)
}

// ---------------------------------------------------------------------------
// Bursts.

/// Looping bursts and impacts. 0 starburst, 1 shock rings, 2 spark burst,
/// 3 sparkle pops, 4 impact lines, 5 confetti pop, 6 heart burst.
pub fn burst(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let pw = p.w;
    let c = (p.w * 0.5, 0.5);
    let (c1, c2) = (p.c1, p.c2);
    match ctx.variant {
        0 => {
            // A spiky star that pops in, then breathes.
            let t = p.t.rem_euclid(2.4);
            let grow = 0.72 + 0.28 * Spring::new(3.4, 0.4).at(t / 0.8);
            let breathe = 1.0 + 0.04 * (t * TAU / 1.2).sin();
            let k = grow * breathe;
            let turn = p.t * 12.0;
            let outer = star(c.0, c.1, 0.46 * k, 0.33 * k, 16, turn);
            let inner = star(c.0, c.1, 0.34 * k, 0.24 * k, 16, -turn * 0.5);
            p.sticker(
                &[&outer],
                &gloss(c1, 0.04, 0.96),
                Some((ink(c1), 0.014)),
                Some((WHITE, 0.016)),
            );
            p.fill(&inner, fade(c2, 0.95));
            p.fill(&circle(c.0, c.1, 0.14 * k), fade(WHITE, 0.9));
        }
        1 => {
            // Rings spreading from a bright core.
            let t = p.t.rem_euclid(1.8);
            for i in 0..3 {
                let u = (t / 1.8 + i as f32 / 3.0).rem_euclid(1.0);
                let r = 0.06 + 0.4 * ease_out(u);
                let a = (1.0 - u).powf(1.4);
                p.stroke(&circle(c.0, c.1, r), fade(c1, a), 0.05 * (1.0 - u * 0.6));
                p.stroke(&circle(c.0, c.1, r), fade(c2, a * 0.8), 0.016);
            }
            p.fill(&circle(c.0, c.1, 0.075), fade(c1, 1.0));
            p.fill(&circle(c.0, c.1, 0.04), WHITE);
            p.bloom(0.7, 0.04, 0.85);
        }
        2 => {
            // Sparks flying out of the middle.
            let t = p.t.rem_euclid(1.5);
            let u = (t / 1.5 + 0.35).rem_euclid(1.0);
            for i in 0..18u32 {
                let a = TAU * (i as f32 + rnd(i, 1) * 0.6) / 18.0;
                let (ca, sa) = (a.cos(), a.sin());
                let reach = 0.2 + 0.2 * rnd(i, 2);
                let r0 = reach * ease_out(u) * 0.8;
                let r1 = reach * ease_out((u + 0.18).min(1.0));
                let col = if i % 3 == 0 { c2 } else { c1 };
                p.stroke(
                    &seg(
                        (c.0 + ca * r0, c.1 + sa * r0),
                        (c.0 + ca * r1, c.1 + sa * r1),
                    ),
                    fade(col, (1.0 - u).powf(0.8)),
                    0.03 * (1.0 - u * 0.5),
                );
            }
            let core = (1.0 - u).powf(2.0);
            p.fill(
                &circle(c.0, c.1, 0.05 + 0.1 * core),
                fade(WHITE, 0.4 + 0.6 * core),
            );
            p.bloom(0.6, 0.04, 0.85);
        }
        3 => {
            // Four-point sparkles popping in turn.
            let t = p.t.rem_euclid(2.4);
            for i in 0..5u32 {
                let u = (t / 2.4 + i as f32 / 5.0).rem_euclid(1.0);
                let a = (u * PI).sin().powf(1.5);
                let pos = (
                    c.0 + (rnd(i, 3) - 0.5) * (p.w - 0.5).max(0.4),
                    0.5 + (rnd(i, 4) - 0.5) * 0.6,
                );
                let r = (0.07 + 0.1 * rnd(i, 5)) * a + 0.02;
                let col = if i % 2 == 0 { c1 } else { c2 };
                p.fill(&sparkle(pos.0, pos.1, r * 1.9, 0.14), fade(col, 0.95));
                p.fill(&sparkle(pos.0, pos.1, r * 1.0, 0.3), fade(WHITE, 0.95 * a));
            }
            p.bloom(0.7, 0.05, 0.8);
        }
        4 => {
            // Manga impact lines pulsing round an empty middle.
            let t = p.t.rem_euclid(0.9);
            let pulse = 0.88 + 0.12 * (t / 0.9 * TAU).cos();
            let n = 34u32;
            for i in 0..n {
                let a = TAU * i as f32 / n as f32 + 0.1 * rnd(i, 6);
                let len = (0.18 + 0.12 * rnd(i, 7)) * pulse;
                let r0 = 0.2 + 0.1 * rnd(i, 8);
                let w = 0.018 + 0.024 * rnd(i, 9);
                let (ca, sa) = (a.cos(), a.sin());
                let (pa, pb) = (r0, r0 + len);
                let nrm = (-sa, ca);
                let tri = poly(&[
                    (c.0 + ca * pb, c.1 + sa * pb),
                    (c.0 + ca * pa + nrm.0 * w, c.1 + sa * pa + nrm.1 * w),
                    (c.0 + ca * pa - nrm.0 * w, c.1 + sa * pa - nrm.1 * w),
                ]);
                p.fill(&tri, if i % 4 == 0 { c2 } else { c1 });
            }
        }
        5 => {
            // A pop of confetti that flies out and falls.
            let t = p.t.rem_euclid(2.0);
            let u = (t / 2.0 + 0.3).rem_euclid(1.0);
            for i in 0..26u32 {
                let a = -PI / 2.0 + (rnd(i, 10) - 0.5) * 3.0;
                let v = 0.5 + 0.5 * rnd(i, 11);
                let (ca, sa) = (a.cos(), a.sin());
                let tt = u * 1.6;
                let reach = v * 0.42 * ease_out(u);
                let pos = (c.0 + ca * reach, 0.62 + sa * reach + 0.14 * u * u);
                let col = confetti_colour(c1, c2, i);
                let fl = ((tt * 9.0 + rnd(i, 12) * TAU).cos()).abs().max(0.25);
                let (w, h) = (0.05 + 0.03 * rnd(i, 13), 0.03 + 0.02 * rnd(i, 14));
                let piece = rrect(pos.0 - w * 0.5, pos.1 - h * 0.5 * fl, w, h * fl, 0.006);
                let piece = rot(
                    &piece,
                    (tt * 300.0 * (rnd(i, 15) - 0.5)).to_degrees() * 0.1,
                    pos,
                );
                p.fill(
                    &piece,
                    fade(
                        col,
                        (1.0 - smoothstep(0.88, 1.0, u)) * edge_k(pw, pos.0, pos.1),
                    ),
                );
            }
        }
        _ => {
            // Hearts floating up and fading.
            let t = p.t.rem_euclid(2.4);
            for i in 0..7u32 {
                let u = (t / 2.4 + i as f32 / 7.0).rem_euclid(1.0);
                let x = c.0 + (rnd(i, 16) - 0.5) * 0.6 + 0.05 * (u * TAU * 2.0 + i as f32).sin();
                let y = 0.9 - 0.8 * u;
                let s = (0.1 + 0.08 * rnd(i, 17)) * (0.6 + 0.4 * smoothstep(0.0, 0.2, u));
                let col = if i % 2 == 0 { c1 } else { c2 };
                p.fill(
                    &heart(x, y, s, s * 0.9),
                    fade(col, (1.0 - smoothstep(0.65, 1.0, u)) * edge_k(pw, x, y)),
                );
            }
        }
    }
    p.shadow(0.35);
}

/// Confetti colours: the two chosen ones plus a spread of festive hues.
fn confetti_colour(c1: C, c2: C, i: u32) -> C {
    match i % 5 {
        0 => c1,
        1 => c2,
        2 => rgb(1.0, 0.82, 0.2),
        3 => rgb(0.3, 0.8, 1.0),
        _ => rgb(1.0, 0.4, 0.65),
    }
}

// ---------------------------------------------------------------------------
// Confetti and balloons.

/// Looping celebration. 0 confetti rain, 1 cannon from the bottom, 2 falling
/// streamers, 3 balloons rising, 4 gold sparkle rain, 5 snow-like specks.
pub fn confetti(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.92);
    let (c1, c2) = (p.c1, p.c2);
    let w = p.w;
    let t = p.t;
    match ctx.variant {
        0 | 4 | 5 => {
            let n = if ctx.variant == 5 { 60u32 } else { 36 };
            for i in 0..n {
                let cycles = 1.0 + (i % 2) as f32;
                let u = (rnd(i, 20) + t * cycles / 4.0).rem_euclid(1.0);
                let x =
                    0.1 + (w - 0.2) * rnd(i, 21) + 0.04 * (u * TAU * cycles * 2.0 + i as f32).sin();
                let y = -0.05 + 1.1 * u;
                let size = 0.045 + 0.04 * rnd(i, 22);
                if ctx.variant == 0 {
                    let flip = ((t * cycles * TAU / 4.0 * 3.0 + rnd(i, 23) * TAU).cos())
                        .abs()
                        .max(0.2);
                    let piece = rrect(
                        x - size * 0.5,
                        y - size * 0.3 * flip,
                        size,
                        size * 0.6 * flip,
                        0.006,
                    );
                    let piece = rot(&piece, 360.0 * u * cycles + 40.0 * rnd(i, 24), (x, y));
                    p.fill(&piece, fade(confetti_colour(c1, c2, i), edge_k(w, x, y)));
                } else if ctx.variant == 4 {
                    let tw = (t * 3.0 + rnd(i, 25) * TAU).sin() * 0.5 + 0.5;
                    let r = size * 0.55 * (0.5 + tw);
                    p.fill(
                        &sparkle(x, y, r * 1.6, 0.16),
                        fade(mix(c1, c2, rnd(i, 26)), (0.4 + 0.6 * tw) * edge_k(w, x, y)),
                    );
                } else {
                    p.fill(
                        &circle(x, y, size * 0.7),
                        fade(c1, (0.55 + 0.4 * rnd(i, 27)) * edge_k(w, x, y)),
                    );
                }
            }
            if ctx.variant == 4 {
                p.bloom(0.5, 0.04, 0.8);
            }
        }
        1 => {
            let period = 2.6;
            let u = (t / period + 0.28).rem_euclid(1.0);
            let tt = u * 1.5;
            for i in 0..34u32 {
                let a = -PI / 2.0 + (rnd(i, 30) - 0.5) * 1.5;
                let v = 0.9 + 0.7 * rnd(i, 31);
                let pos = (
                    w * 0.5 + a.cos() * v * tt * 0.9,
                    0.9 + a.sin() * v * tt * 0.95 + 0.5 * 1.7 * tt * tt,
                );
                let size = 0.045 + 0.035 * rnd(i, 32);
                let flip = ((tt * 11.0 + rnd(i, 33) * TAU).cos()).abs().max(0.2);
                let piece = rrect(
                    pos.0 - size * 0.5,
                    pos.1 - size * 0.3 * flip,
                    size,
                    size * 0.6 * flip,
                    0.006,
                );
                let piece = rot(&piece, 600.0 * tt * (rnd(i, 34) - 0.5), pos);
                p.fill(
                    &piece,
                    fade(
                        confetti_colour(c1, c2, i),
                        (1.0 - smoothstep(0.78, 1.0, u)) * edge_k(w, pos.0, pos.1),
                    ),
                );
            }
            // The cannon mouth.
            let mouth = poly(&[
                (w * 0.5 - 0.12, 0.96),
                (w * 0.5 - 0.06, 0.88),
                (w * 0.5 + 0.06, 0.88),
                (w * 0.5 + 0.12, 0.96),
            ]);
            p.fill(&mouth, shade(c1, 0.7));
        }
        2 => {
            for i in 0..9u32 {
                let u = (rnd(i, 40) + t / 3.2).rem_euclid(1.0);
                let x0 = 0.12 + (w - 0.24) * rnd(i, 41);
                let top = -0.2 + 1.3 * u;
                let pts: Vec<P> = (0..14)
                    .map(|k| {
                        let s = k as f32 / 13.0;
                        (
                            x0 + 0.06 * (s * 7.0 + t * 3.0 + i as f32).sin(),
                            top - 0.28 * s,
                        )
                    })
                    .collect();
                let rib = ribbon(&pts, |s| 0.03 * (1.0 - 0.5 * s));
                p.fill(
                    &rib,
                    fade(
                        confetti_colour(c1, c2, i),
                        0.95 * edge_k(w, x0, top).min(edge_k(w, x0, top - 0.28)),
                    ),
                );
            }
        }
        _ => {
            for i in 0..6u32 {
                let u = (rnd(i, 50) + t / 4.0).rem_euclid(1.0);
                let x = 0.3 + (w - 0.6) * rnd(i, 51) + 0.03 * (u * TAU * 2.0 + i as f32).sin();
                let y = 0.85 - 0.75 * u;
                let col = confetti_colour(c1, c2, i);
                let r = 0.1 + 0.03 * rnd(i, 52);
                let ek = edge_soft(w, x - r, y, 0.02, 0.1)
                    .min(edge_soft(w, x + r, y, 0.02, 0.1))
                    .min(edge_soft(w, x, y - r * 1.2, 0.02, 0.1))
                    .min(edge_soft(w, x, y + r * 1.3, 0.02, 0.1));
                let ek_string = edge_soft(w, x, y + 0.56, 0.02, 0.1);
                // The string, then the balloon with its highlight.
                let string = smooth(
                    &[
                        (x, y + r * 1.2),
                        (x + 0.04, y + 0.3),
                        (x - 0.03, y + 0.42),
                        (x + 0.02, y + 0.56),
                    ],
                    false,
                    &[],
                );
                p.stroke(&string, fade(WHITE, 0.8 * ek.min(ek_string)), 0.008);
                let body = ellipse(x, y, r, r * 1.18);
                p.paint(
                    &body,
                    &radial(
                        (x - r * 0.3, y - r * 0.4),
                        r * 1.6,
                        &[
                            (0.0, fade(tint(col, 0.5), ek)),
                            (0.55, fade(col, ek)),
                            (1.0, fade(shade(col, 0.65), ek)),
                        ],
                    ),
                );
                p.fill(
                    &poly(&[
                        (x - 0.014, y + r * 1.15),
                        (x + 0.014, y + r * 1.15),
                        (x, y + r * 1.3),
                    ]),
                    fade(shade(col, 0.7), ek),
                );
                p.fill(
                    &ellipse(x - r * 0.35, y - r * 0.45, r * 0.18, r * 0.3),
                    fade(WHITE, 0.55 * ek),
                );
            }
        }
    }
    p.shadow(0.3);
}

// ---------------------------------------------------------------------------
// Fire, smoke, lightning, energy.

/// A flame tongue: base centre `b`, height `h`, half width `w`, leaning `lean`.
fn tongue(b: P, h: f32, w: f32, lean: f32) -> tiny_skia::Path {
    let tip = (b.0 + lean, b.1 - h);
    let mut pb = PathBuilder::new();
    pb.move_to(b.0 - w, b.1);
    pb.cubic_to(
        b.0 - w * 1.25,
        b.1 - h * 0.45,
        b.0 - w * 0.25 + lean * 0.4,
        b.1 - h * 0.7,
        tip.0,
        tip.1,
    );
    pb.cubic_to(
        b.0 + w * 0.3 + lean * 0.4,
        b.1 - h * 0.65,
        b.0 + w * 1.25,
        b.1 - h * 0.4,
        b.0 + w,
        b.1,
    );
    pb.quad_to(b.0, b.1 + w * 0.7, b.0 - w, b.1);
    pb.close();
    pb.finish().unwrap_or_else(|| circle(b.0, b.1, w))
}

/// Fire and energy. 0 flame, 1 tall torch flame, 2 wide bonfire, 3 rising
/// embers, 4 smoke, 5 lightning bolt, 6 electric arc, 7 energy orb.
pub fn fire(pm: &mut Pixmap, ctx: &DrawCtx) {
    // Lightning and energy glow a long way, so they get more room.
    let room = if ctx.variant >= 5 { 0.72 } else { 0.86 };
    let mut p = Pen::new(pm, ctx).inset(room);
    let (c1, c2) = (p.c1, p.c2);
    let t = p.t;
    let mid = p.w * 0.5;
    match ctx.variant {
        0..=2 => {
            let (hs, ws, n) = match ctx.variant {
                1 => (1.15, 0.7, 3),
                2 => (0.85, 1.15, 5),
                _ => (1.0, 1.0, 4),
            };
            let layers = [(c1, 1.0), (mix(c1, c2, 0.55), 0.72), (WHITE, 0.42)];
            for (li, (col, sc)) in layers.iter().enumerate() {
                for i in 0..n {
                    let fi = i as f32;
                    let spread = (fi - (n as f32 - 1.0) * 0.5) * 0.17 * ws;
                    let big = 1.0 - (fi - (n as f32 - 1.0) * 0.5).abs() / n as f32;
                    let sway = 0.05 * (t * (5.0 + fi) + fi * 1.7 + li as f32 * 0.5).sin();
                    let lick = 1.0 + 0.1 * (t * (7.0 + fi * 1.3) + fi).sin();
                    let h = 0.58 * hs * big * sc * lick;
                    let w = 0.15 * ws * (0.7 + 0.5 * big) * sc;
                    let path = tongue((mid + spread * *sc, 0.8), h, w, sway + spread * 0.2);
                    p.fill(&path, fade(*col, if li == 0 { 0.96 } else { 0.92 }));
                }
            }
            p.bloom(0.9, 0.05, 0.7);
        }
        3 => {
            for i in 0..22u32 {
                let u = (rnd(i, 60) + t * (1.0 + (i % 2) as f32) / 3.0).rem_euclid(1.0);
                let x = mid + (rnd(i, 61) - 0.5) * 0.7 + 0.06 * (u * TAU * 2.0 + i as f32).sin();
                let y = 0.95 - 0.95 * u;
                let r = 0.025 + 0.03 * rnd(i, 62);
                let col = mix(c2, c1, u);
                p.fill(
                    &circle(x, y, r),
                    fade(col, (1.0 - smoothstep(0.55, 1.0, u)) * edge_k(p.w, x, y)),
                );
            }
            p.bloom(0.9, 0.04, 0.6);
        }
        4 => {
            for i in 0..7u32 {
                let u = (i as f32 / 7.0 + t / 3.0).rem_euclid(1.0);
                let x = mid + 0.12 * (u * TAU + i as f32 * 1.9).sin() + (rnd(i, 70) - 0.5) * 0.12;
                let y = 0.85 - 0.5 * u;
                let r = 0.05 + 0.12 * u;
                let a = 0.8 * (1.0 - u).powf(1.2) * smoothstep(0.0, 0.1, u) * edge_k(p.w, x, y - r);
                p.fill(&circle(x, y, r), fade(c1, a));
                p.fill(
                    &circle(x - r * 0.25, y - r * 0.25, r * 0.6),
                    fade(tint(c1, 0.3), a * 0.6),
                );
            }
        }
        5 => {
            // A bolt that flickers but never goes out.
            let f = (t * 18.0).floor() as u32;
            let flick = if hash(f.wrapping_mul(13)) > 0.78 {
                0.5
            } else {
                1.0
            };
            let flick = flick * (0.93 + 0.07 * (t * 9.0).sin());
            let o = (p.w - 1.0) * 0.5;
            let bolt = poly(&[
                (o + 0.62, 0.1),
                (o + 0.28, 0.54),
                (o + 0.48, 0.54),
                (o + 0.34, 0.9),
                (o + 0.76, 0.42),
                (o + 0.54, 0.42),
            ]);
            p.fill(&bolt, fade(c1, 0.35 * flick));
            p.stroke(&bolt, fade(c2, 0.6 * flick), 0.05);
            p.fill(&bolt, fade(mix(c1, WHITE, 0.55), flick));
            p.bloom(1.4 * flick, 0.05, 0.6);
            p.bloom(0.8 * flick, 0.06, 0.6);
        }
        6 => {
            // A jagged arc crackling between two points.
            let f = (t * 20.0).floor() as u32;
            let w = p.w;
            let mut pts: Vec<P> = Vec::new();
            for k in 0..=14 {
                let s = k as f32 / 14.0;
                let amp = (s * (1.0 - s)).sqrt() * 0.4;
                let j = (rnd(k as u32, f.wrapping_mul(5)) - 0.5) * amp;
                pts.push((0.16 + s * (w - 0.32), 0.5 + j));
            }
            let arc = line(&pts);
            p.stroke(&arc, fade(c2, 0.35), 0.09);
            p.stroke(&arc, fade(c1, 0.9), 0.04);
            p.stroke(&arc, WHITE, 0.016);
            // A short branch.
            let k = 6 + (f % 4) as usize;
            let b0 = pts[k];
            let dir = if f.is_multiple_of(2) { -1.0 } else { 1.0 };
            let br = line(&[
                b0,
                (b0.0 + 0.07, b0.1 + dir * 0.1),
                (b0.0 + 0.04, b0.1 + dir * 0.2),
            ]);
            p.stroke(&br, fade(c1, 0.8), 0.022);
            p.fill(&circle(0.16, 0.5, 0.045), WHITE);
            p.fill(&circle(w - 0.16, 0.5, 0.045), WHITE);
            p.bloom(1.2, 0.05, 0.6);
        }
        _ => {
            // A glowing orb with rings turning round it.
            let c = (mid, 0.5);
            let pulse = 1.0 + 0.05 * (t * TAU / 1.5).sin();
            let r = 0.22 * pulse;
            p.paint(
                &circle(c.0, c.1, r),
                &radial(
                    c,
                    r,
                    &[(0.0, WHITE), (0.45, tint(c1, 0.4)), (1.0, shade(c1, 0.7))],
                ),
            );
            for i in 0..3 {
                let a = t * (1.0 + 0.4 * i as f32) * 1.2 + i as f32 * 1.1;
                let ring = ellipse(c.0, c.1, 0.38, 0.38 * (0.3 + 0.7 * ((a).cos()).abs()));
                let ring = rot(&ring, 60.0 * i as f32 + 20.0 * a.sin(), c);
                p.stroke(&ring, fade(c2, 0.9), 0.014);
                let ang = a * 2.0;
                let dot = (
                    c.0 + 0.38 * ang.cos(),
                    c.1 + 0.38 * (0.3 + 0.7 * a.cos().abs()) * ang.sin(),
                );
                let dot = rot(
                    &circle(dot.0, dot.1, 0.022),
                    60.0 * i as f32 + 20.0 * a.sin(),
                    c,
                );
                p.fill(&dot, WHITE);
            }
            p.bloom(1.1, 0.06, 0.65);
        }
    }
    if ctx.variant == 4 {
        p.shadow(0.25);
    }
}
