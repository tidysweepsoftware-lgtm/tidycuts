//! Badges for products, places and reviews, and small on-screen graphics:
//! star ratings, discount seals, hanging price tags, ribbons, map pins,
//! check seals, and camera or loading widgets. All of them move.
//!
//! `ctx.variant` picks the wording or icon inside each drawing. The main
//! colour is the body, the second colour is the accent or lettering.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use crate::motion::Spring;
use std::f32::consts::{PI, TAU};
use tiny_skia::{PathBuilder, Pixmap};

const FONT: &str = "Poppins Black";

fn enter(t: f32) -> f32 {
    0.75 + 0.25 * Spring::new(3.2, 0.45).at(t / 0.7)
}

// ---------------------------------------------------------------------------
// Rating.

/// Five stars (variant 0), 4.5 stars (1) or five hearts (2) that fill one
/// after another, then twinkle.
pub fn rating(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.88);
    // Starts with every star already filled, so the first frame is not bare.
    let t = (p.t + 1.8).rem_euclid(3.6);
    let (c1, c2) = (p.c1, p.c2);
    let x0 = p.w * 0.5 - 1.0;
    for i in 0..5 {
        let cx = x0 + 0.25 + i as f32 * 0.5;
        let at = 0.2 + 0.24 * i as f32;
        let pop = Spring::new(3.4, 0.4).at((t - at) / 0.6);
        let fill = if t >= at { 1.0 } else { 0.0 };
        let k = if t >= at {
            0.8 + 0.4 * pop.min(1.3)
        } else {
            0.8
        };
        let shape = |k: f32| {
            if ctx.variant == 2 {
                heart(cx, 0.5, 0.36 * k, 0.32 * k)
            } else {
                star(cx, 0.5, 0.23 * k, 0.1 * k, 5, 0.0)
            }
        };
        let empty = shape(0.8);
        p.fill(&empty, fade(c2, 0.35));
        p.stroke(&empty, fade(c2, 0.6), 0.012);
        if fill > 0.0 {
            let full = shape(k);
            let half = ctx.variant == 1 && i == 4;
            if half {
                if let Some(mask) = p.clip(&rrect(cx - 0.4, 0.0, 0.4, 1.0, 0.0)) {
                    p.paint_masked(&full, &gloss(c1, 0.25, 0.75), &mask);
                }
            } else {
                p.sticker(
                    &[&full],
                    &gloss(c1, 0.25, 0.75),
                    Some((ink(c1), 0.01)),
                    None,
                );
            }
        }
    }
    // Twinkles once the row is complete.
    let done = (t - 1.5) / 1.6;
    if (0.0..1.0).contains(&done) {
        for k in 0..3 {
            let u = (done * 1.4 - k as f32 * 0.22).clamp(0.0, 1.0);
            let a = (u * PI).sin();
            let x = x0 + 0.25 + k as f32 * 1.0;
            p.fill(
                &sparkle(x, 0.28 + 0.14 * (k % 2) as f32, 0.1 * a + 0.01, 0.16),
                fade(WHITE, 0.95 * a),
            );
        }
    }
    p.shadow(0.45);
}

// ---------------------------------------------------------------------------
// Discount seal.

pub const PERCENTS: [&str; 8] = [
    "-10%", "-20%", "-30%", "-40%", "-50%", "-60%", "-70%", "-80%",
];

/// A scalloped seal with a discount that wobbles and catches the light.
pub fn discount(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(3.0);
    let (c1, c2) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let k = enter(t);
    let wob = 3.0 * (t * TAU / 3.0).sin();
    let seal = rot(&scallop(c.0, c.1, 0.44, 0.44, 14, 0.035), wob * 2.0, c);
    let seal = scaled(&seal, k, c);
    p.sticker(
        &[&seal],
        &gloss(c1, 0.06, 0.94),
        Some((ink(c1), 0.014)),
        Some((WHITE, 0.018)),
    );
    p.stroke(
        &scaled(&circle(c.0, c.1, 0.34), k, c),
        fade(WHITE, 0.55),
        0.012,
    );
    let label = PERCENTS[(ctx.variant as usize).min(PERCENTS.len() - 1)];
    if let Some(g) = text(FONT, label, (c.0, c.1 + 0.02), 0.5, 0.2, 0.0, wob) {
        p.fill(&scaled(&g, k, c), c2);
    }
    if let Some(g) = text(FONT, "OFF", (c.0, c.1 + 0.2), 0.2, 0.06, 0.0, wob) {
        p.fill(&scaled(&g, k, c), fade(c2, 0.9));
    }
    let mask = p.clip(&seal);
    let u = smoothstep(1.0, 2.0, t);
    if u > 0.0 && u < 1.0 {
        let x = c.0 - 0.5 + u;
        let band = poly(&[
            (x - 0.05, 0.0),
            (x + 0.02, 0.0),
            (x - 0.1 + 0.3, 1.0),
            (x - 0.17 + 0.3, 1.0),
        ]);
        p.fill_in(&band, fade(WHITE, 0.5), &mask);
    }
    p.shadow(0.5);
}

// ---------------------------------------------------------------------------
// Hanging price tag.

pub const TAG_WORDS: [&str; 6] = ["SALE", "NEW", "DEAL", "SOLD", "HOT", "FREE"];

/// A tag on a string, swinging.
pub fn price_tag(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(3.0);
    let (c1, c2) = (p.c1, p.c2);
    let pivot = (p.w * 0.5, 0.06);
    let swing = 10.0 * (t * TAU / 3.0).sin() * (0.85 + 0.15 * (t * TAU / 1.5).cos());
    let c = (p.w * 0.5, 0.56);
    let body = poly(&[
        (c.0 - 0.2, c.1 - 0.27),
        (c.0 + 0.2, c.1 - 0.27),
        (c.0 + 0.3, c.1 - 0.15),
        (c.0 + 0.3, c.1 + 0.4),
        (c.0 - 0.3, c.1 + 0.4),
        (c.0 - 0.3, c.1 - 0.15),
    ]);
    let hole = circle(c.0, c.1 - 0.17, 0.045);
    let string = smooth(
        &[pivot, (c.0 + 0.02, c.1 - 0.35), (c.0, c.1 - 0.17)],
        false,
        &[],
    );
    let (body, hole, string) = (
        rot(&body, swing, pivot),
        rot(&hole, swing, pivot),
        rot(&string, swing, pivot),
    );
    p.stroke(&string, fade(WHITE, 0.9), 0.014);
    p.sticker(
        &[&body],
        &gloss(c1, 0.25, 0.97),
        Some((ink(c1), 0.014)),
        Some((WHITE, 0.016)),
    );
    p.erase(&hole);
    p.stroke(&hole, ink(c1), 0.01);
    let word = TAG_WORDS[(ctx.variant as usize).min(TAG_WORDS.len() - 1)];
    if let Some(g) = text(FONT, word, (c.0, c.1 + 0.16), 0.44, 0.16, 0.0, 0.0) {
        p.fill(&rot(&g, swing, pivot), c2);
    }
    p.shadow(0.5);
}

// ---------------------------------------------------------------------------
// Ribbon.

pub const RIBBON_WORDS: [&str; 6] = [
    "BEST SELLER",
    "NEW ARRIVAL",
    "LIMITED",
    "TOP PICK",
    "HOT DEAL",
    "STAFF PICK",
];

/// A banner with notched ends and a waving edge.
pub fn ribbon_banner(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.86);
    let t = p.t.rem_euclid(3.0);
    let (c1, c2) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let k = enter(t);
    let (hw, hh) = (p.w * 0.5 - 0.22, 0.2);
    let wave = |x: f32| 0.018 * ((x - c.0) * 9.0 - t * TAU / 3.0 * 2.0).sin();
    let n = 24;
    let mut top = Vec::new();
    let mut bot = Vec::new();
    for i in 0..=n {
        let x = c.0 - hw + 2.0 * hw * i as f32 / n as f32;
        top.push((x, c.1 - hh + wave(x)));
        bot.push((x, c.1 + hh + wave(x)));
    }
    let mut pts = top.clone();
    pts.extend(bot.iter().rev().copied());
    let body = poly(&pts);
    // The folded tails behind, notched.
    let tail = |side: f32| {
        let x = c.0 + side * hw;
        poly(&[
            (x, c.1 - hh * 0.6 + wave(x) + 0.1),
            (x + side * 0.2, c.1 - hh * 0.6 + 0.1),
            (x + side * 0.12, c.1 + 0.1),
            (x + side * 0.2, c.1 + hh * 0.6 + 0.1),
            (x, c.1 + hh * 0.6 + wave(x) + 0.1),
        ])
    };
    let (tl, tr) = (scaled(&tail(-1.0), k, c), scaled(&tail(1.0), k, c));
    p.fill(&tl, shade(c1, 0.62));
    p.fill(&tr, shade(c1, 0.62));
    let body = scaled(&body, k, c);
    p.sticker(
        &[&body],
        &gloss(c1, c.1 - hh, c.1 + hh),
        Some((ink(c1), 0.012)),
        Some((WHITE, 0.014)),
    );
    let word = RIBBON_WORDS[(ctx.variant as usize).min(RIBBON_WORDS.len() - 1)];
    if let Some(g) = text(
        FONT,
        word,
        (c.0, c.1 + 0.005),
        2.0 * hw - 0.3,
        0.16,
        0.0,
        0.0,
    ) {
        p.fill(&scaled(&g, k, c), c2);
    }
    let mask = p.clip(&body);
    let u = smoothstep(0.8, 1.9, t);
    if u > 0.0 && u < 1.0 {
        let x = c.0 - hw + 2.0 * hw * u;
        let band = poly(&[
            (x - 0.05, 0.0),
            (x + 0.03, 0.0),
            (x - 0.07 + 0.25, 1.0),
            (x - 0.15 + 0.25, 1.0),
        ]);
        p.fill_in(&band, fade(WHITE, 0.5), &mask);
    }
    p.shadow(0.5);
}

// ---------------------------------------------------------------------------
// Map pin.

/// A map pin that drops in, bounces and sends out a ripple. Variant 0 has a
/// dot, 1 a star, 2 a heart.
pub fn pin(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(3.0);
    let (c1, c2) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    let land = Spring::new(2.8, 0.35).at((t / 0.8).min(1.0));
    let drop = (1.0 - land) * -0.1;
    // The ground ripple.
    for i in 0..2 {
        let u = ((t - 0.35) / 1.8 - i as f32 * 0.3).clamp(0.0, 1.0);
        if u > 0.0 && u < 1.0 {
            p.stroke(
                &ellipse(cx, 0.9, 0.04 + 0.3 * u, 0.012 + 0.09 * u),
                fade(c1, 0.7 * (1.0 - u)),
                0.012,
            );
        }
    }
    let r = 0.24;
    let cy = 0.4 + drop;
    let mut pb = PathBuilder::new();
    pb.move_to(cx, 0.9 + drop);
    pb.cubic_to(cx - 0.1, 0.75 + drop, cx - r, cy + 0.12, cx - r, cy);
    pb.cubic_to(cx - r, cy - r * 1.33, cx + r, cy - r * 1.33, cx + r, cy);
    pb.cubic_to(cx + r, cy + 0.12, cx + 0.1, 0.75 + drop, cx, 0.9 + drop);
    pb.close();
    let body = pb.finish().unwrap_or_else(|| circle(cx, cy, r));
    p.sticker(
        &[&body],
        &gloss(c1, cy - r, 0.9),
        Some((ink(c1), 0.014)),
        Some((WHITE, 0.016)),
    );
    let ic = (cx, cy - 0.02);
    match ctx.variant {
        1 => p.fill(&star(ic.0, ic.1, 0.12, 0.05, 5, 0.0), c2),
        2 => p.fill(&heart(ic.0, ic.1, 0.2, 0.18), c2),
        _ => {
            p.fill(&circle(ic.0, ic.1, 0.095), c2);
            p.fill(&circle(ic.0, ic.1, 0.04), shade(c1, 0.85));
        }
    }
    p.shadow(0.5);
}

// ---------------------------------------------------------------------------
// Check seal.

/// A scalloped seal with a check, star or heart inside that draws itself,
/// with light sweeping across.
pub fn seal(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(3.2);
    let (c1, c2) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let k = enter(t);
    let body = scaled(&scallop(c.0, c.1, 0.44, 0.44, 12, 0.04), k, c);
    p.sticker(
        &[&body],
        &gloss(c1, 0.06, 0.94),
        Some((ink(c1), 0.014)),
        Some((WHITE, 0.018)),
    );
    p.stroke(
        &scaled(&circle(c.0, c.1, 0.33), k, c),
        fade(WHITE, 0.5),
        0.012,
    );
    let draw = ease_out(((t - 0.5) / 0.6).clamp(0.0, 1.0));
    match ctx.variant {
        1 => {
            let s = 0.6 + 0.4 * draw;
            p.fill(
                &scaled(&star(c.0, c.1 + 0.01, 0.2, 0.085, 5, 0.0), s * k, c),
                c2,
            );
        }
        2 => {
            let s = 0.6 + 0.4 * draw;
            p.fill(&scaled(&heart(c.0, c.1, 0.3, 0.27), s * k, c), c2);
        }
        _ => {
            let a = (c.0 - 0.15, c.1 + 0.01);
            let b = (c.0 - 0.03, c.1 + 0.13);
            let d = (c.0 + 0.18, c.1 - 0.12);
            let total = dist(a, b) + dist(b, d);
            let len = draw * total;
            let mut pts = vec![a];
            if len <= dist(a, b) {
                pts.push(lerp(a, b, len / dist(a, b)));
            } else {
                pts.push(b);
                pts.push(lerp(b, d, (len - dist(a, b)) / dist(b, d)));
            }
            p.stroke(&scaled(&line(&pts), k, c), c2, 0.07);
        }
    }
    let mask = p.clip(&body);
    let u = smoothstep(1.2, 2.3, t);
    if u > 0.0 && u < 1.0 {
        let x = c.0 - 0.5 + u;
        let band = poly(&[
            (x - 0.05, 0.0),
            (x + 0.03, 0.0),
            (x - 0.07 + 0.3, 1.0),
            (x - 0.15 + 0.3, 1.0),
        ]);
        p.fill_in(&band, fade(WHITE, 0.5), &mask);
    }
    p.shadow(0.5);
}

// ---------------------------------------------------------------------------
// Camera and loading widgets.

/// Small on-screen graphics. 0 viewfinder with a blinking REC dot, 1
/// loading ring, 2 equalizer, 3 sound wave, 4 progress bar, 5 play button.
pub fn hud(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.9);
    let t = p.t;
    let (c1, c2) = (p.c1, p.c2);
    let (w, c) = (p.w, (p.w * 0.5, 0.5));
    match ctx.variant {
        0 => {
            let (x0, x1, y0, y1, l) = (0.1, w - 0.1, 0.1, 0.9, 0.16);
            let breathe = 0.012 * (t * TAU / 2.0).sin();
            for (sx, sy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                let (dx, dy) = (
                    if sx < c.0 { 1.0 } else { -1.0 },
                    if sy < c.1 { 1.0 } else { -1.0 },
                );
                let (sx, sy) = (sx + dx * breathe, sy + dy * breathe);
                p.stroke(
                    &line(&[(sx + dx * l, sy), (sx, sy), (sx, sy + dy * l)]),
                    c1,
                    0.03,
                );
            }
            let on = (t * 1.0).rem_euclid(1.0) < 0.62;
            p.fill(
                &circle(x0 + 0.22, y0 + 0.2, 0.05),
                fade(c2, if on { 1.0 } else { 0.25 }),
            );
            if let Some(g) = text(FONT, "REC", (x0 + 0.52, y0 + 0.2), 0.2, 0.08, 0.0, 0.0) {
                p.fill(&g, c1);
            }
            p.stroke(
                &line(&[(c.0 - 0.06, c.1), (c.0 + 0.06, c.1)]),
                fade(c1, 0.8),
                0.014,
            );
            p.stroke(
                &line(&[(c.0, c.1 - 0.06), (c.0, c.1 + 0.06)]),
                fade(c1, 0.8),
                0.014,
            );
        }
        1 => {
            let spin = t * 360.0 / 1.2;
            for i in 0..12 {
                let a = (spin - i as f32 * 30.0).to_radians();
                let fadek = 1.0 - i as f32 / 12.0;
                let (ca, sa) = (a.cos(), a.sin());
                p.stroke(
                    &seg(
                        (c.0 + ca * 0.22, 0.5 + sa * 0.22),
                        (c.0 + ca * 0.36, 0.5 + sa * 0.36),
                    ),
                    fade(if i == 0 { c2 } else { c1 }, 0.15 + 0.85 * fadek),
                    0.05,
                );
            }
        }
        2 => {
            let n = 9;
            for i in 0..n {
                let x = 0.2 + (i as f32 + 0.5) / n as f32 * (w - 0.4);
                let h = 0.12
                    + 0.3
                        * (0.5 + 0.5 * (t * 6.0 + i as f32 * 1.7).sin())
                        * (0.6 + 0.4 * (t * 3.1 + i as f32).cos().abs());
                let bar = rrect(x - 0.03, 0.5 - h, 0.06, 2.0 * h, 0.03);
                p.paint(
                    &bar,
                    &gloss(mix(c1, c2, i as f32 / n as f32), 0.5 - h, 0.5 + h),
                );
            }
        }
        3 => {
            let pts: Vec<P> = (0..=60)
                .map(|i| {
                    let u = i as f32 / 60.0;
                    let env = (u * PI).sin();
                    (
                        0.14 + u * (w - 0.28),
                        0.5 + 0.3
                            * env
                            * (u * 18.0 + t * 7.0).sin()
                            * (0.7 + 0.3 * (t * 2.0).sin()),
                    )
                })
                .collect();
            let wave = line(&pts);
            p.stroke(&wave, fade(c2, 0.35), 0.07);
            p.stroke(&wave, c1, 0.032);
            p.bloom(0.5, 0.04, 0.8);
        }
        4 => {
            let u = ((t + 1.4) / 3.0).rem_euclid(1.0);
            let fill = ease_out((u / 0.85).min(1.0));
            let (bx, bw, bh) = (0.14, w - 0.28, 0.14);
            p.fill(&rrect(bx, 0.5 - bh * 0.5, bw, bh, bh * 0.5), fade(c2, 0.35));
            let fw = (bw * fill).max(bh);
            let bar = rrect(bx, 0.5 - bh * 0.5, fw, bh, bh * 0.5);
            p.paint(&bar, &gloss(c1, 0.43, 0.57));
            if let Some(g) = text(
                FONT,
                &format!("{}%", (fill * 100.0).round() as u32),
                (c.0, 0.78),
                0.34,
                0.1,
                0.0,
                0.0,
            ) {
                p.fill(&g, c1);
            }
        }
        _ => {
            let pulse = 1.0 + 0.05 * (t * TAU / 1.4).sin();
            for i in 0..2 {
                let u = (t / 1.8 + i as f32 * 0.5).rem_euclid(1.0);
                p.stroke(
                    &circle(c.0, 0.5, 0.26 + 0.2 * u),
                    fade(c1, 0.5 * (1.0 - u)),
                    0.02,
                );
            }
            p.paint(&circle(c.0, 0.5, 0.26 * pulse), &gloss(c1, 0.24, 0.76));
            p.fill(
                &poly(&[
                    (c.0 - 0.07, 0.5 - 0.12),
                    (c.0 - 0.07, 0.5 + 0.12),
                    (c.0 + 0.13, 0.5),
                ]),
                c2,
            );
        }
    }
    p.shadow(0.4);
}
