//! Badges with lettering: NEW, SALE, WOW, OMG, LOL, plus a ribbon banner,
//! an award seal and a price tag. Main color = badge, second = lettering.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use tiny_skia::{Path, Pixmap};

/// Lettering with a soft dark copy underneath.
fn letters(p: &mut Pen, t: &Path, fill: C, under: C, drop: f32) {
    p.fill(&moved(t, drop * 0.4, drop), under);
    p.fill(t, fill);
}

pub fn new_badge(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.49);
    let body = star(cx, cy, 0.39, 0.34, 16, 0.0);
    p.sticker(
        &[&body],
        &gloss(main, 0.1, 0.9),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.stroke_with(
        &circle(cx, cy, 0.27),
        &solid(fade(fg, 0.8)),
        &dashed(0.016, 0.03, 0.03),
    );
    if let Some(t) = text("Poppins Black", "NEW", (cx, cy), 0.45, 0.19, 0.0, -12.0) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.02);
    }
    p.halo(0.4);
}

pub fn sale(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let (bw, bh) = (1.4, 0.56);
    let body = rot(
        &rrect(c.0 - bw * 0.5, c.1 - bh * 0.5, bw, bh, 0.09),
        -6.0,
        c,
    );
    p.sticker(
        &[&body],
        &gloss(main, 0.15, 0.85),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let inner = rot(
        &rrect(
            c.0 - bw * 0.5 + 0.06,
            c.1 - bh * 0.5 + 0.06,
            bw - 0.12,
            bh - 0.12,
            0.05,
        ),
        -6.0,
        c,
    );
    p.stroke_with(&inner, &solid(fade(fg, 0.85)), &dashed(0.016, 0.045, 0.03));
    if let Some(t) = text("Poppins Black", "SALE", c, 1.02, 0.28, 0.0, -6.0) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.022);
    }
    p.shadow(0.4);
}

/// Jagged comic burst around (cx, cy).
fn comic_burst(cx: f32, cy: f32, rx: f32, ry: f32, n: usize, seed: u32) -> Path {
    let mut pts = Vec::new();
    for i in 0..2 * n {
        let a = (i as f32 / (2 * n) as f32 * 360.0 - 90.0 + 6.0 * (hash(seed + i as u32) - 0.5))
            .to_radians();
        let k = if i % 2 == 0 {
            0.92 + 0.16 * hash(seed + 100 + i as u32)
        } else {
            0.68 + 0.08 * hash(seed + 200 + i as u32)
        };
        pts.push((cx + rx * k * a.cos(), cy + ry * k * a.sin()));
    }
    poly(&pts)
}

pub fn wow(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.48);
    let outer = comic_burst(cx, cy, 0.53, 0.37, 13, 7);
    p.stroke(&outer, WHITE, 2.0 * (RIM + EDGE));
    p.stroke_with(&outer, &solid(ink(main)), &miter(2.0 * EDGE));
    p.paint(&outer, &gloss(main, 0.12, 0.85));
    let inner = comic_burst(cx, cy, 0.4, 0.28, 13, 7);
    p.paint(
        &inner,
        &radial(
            (cx, cy),
            0.4,
            &[(0.0, tint(main, 0.75)), (1.0, tint(main, 0.3))],
        ),
    );
    if let Some(t) = text("Bangers", "WOW!", (cx, cy), 0.66, 0.27, 0.0, -8.0) {
        let edge = ink(fg);
        p.fill(&moved(&t, 0.012, 0.03), edge);
        p.stroke(&moved(&t, 0.012, 0.03), edge, 0.05);
        p.stroke(&t, edge, 0.08);
        p.stroke(&t, WHITE, 0.045);
        p.paint(&t, &gloss(fg, cy - 0.17, cy + 0.17));
    }
    p.shadow(0.4);
}

pub fn omg(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, back) = (p.c1, p.c2);
    let c = (p.w * 0.5 - 0.05, 0.5);
    if let Some(t) = text("Poppins Black", "OMG", c, 1.14, 0.44, 0.06, -6.0) {
        let block = moved(&t, 0.03, 0.05);
        p.stroke(&block, ink(back), 0.17);
        p.fill(&block, ink(back));
        p.stroke(&block, back, 0.13);
        p.stroke(&t, WHITE, 0.15);
        p.stroke(&t, ink(main), 0.05);
        p.paint(&t, &gloss(main, c.1 - 0.23, c.1 + 0.23));
        let m = p.clip(&t);
        p.fill_in(&rect_band(c.1 - 0.23, 0.07), fade(WHITE, 0.3), &m);
        // Shock marks off the top right corner of the word.
        let b = t.compute_tight_bounds().unwrap_or(t.bounds());
        let (x, y) = (b.right() - 0.02, b.top() + 0.03);
        for deg in [-80.0f32, -45.0, -10.0] {
            let a = deg.to_radians();
            let s = seg(
                (x + 0.07 * a.cos(), y + 0.07 * a.sin()),
                (x + 0.16 * a.cos(), y + 0.16 * a.sin()),
            );
            p.stroke(&s, WHITE, 0.07);
            p.stroke(&s, ink(back), 0.05);
            p.stroke(&s, main, 0.028);
        }
    }
    p.shadow(0.4);
}

fn rect_band(y: f32, h: f32) -> Path {
    rrect(-1.0, y, 10.0, h, 0.0)
}

pub fn lol(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.48);
    let body = rot(&scallop(cx, cy, 0.52, 0.32, 14, 0.06), 5.0, (cx, cy));
    p.sticker(
        &[&body],
        &gloss(main, 0.1, 0.85),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.stroke_with(
        &rot(&ellipse(cx, cy, 0.43, 0.25), 5.0, (cx, cy)),
        &solid(fade(WHITE, 0.6)),
        &dashed(0.014, 0.001, 0.035),
    );
    if let Some(t) = text("Poppins Black", "LOL", (cx, cy), 0.66, 0.27, 0.0, 5.0) {
        p.stroke(&moved(&t, 0.01, 0.025), ink(main), 0.05);
        p.fill(&moved(&t, 0.01, 0.025), ink(main));
        p.stroke(&t, ink(main), 0.045);
        p.paint(&t, &gloss(fg, cy - 0.14, cy + 0.14));
    }
    p.shadow(0.4);
}

pub fn ribbon_banner(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, stitch) = (p.c1, p.c2);
    let w = p.w;
    let cx = w * 0.5;
    let half = 0.95;
    let thick = 0.34;
    let top = |x: f32| 0.2 + 0.07 * ((x - cx) / half).powi(2);
    let n = 40;
    let xs: Vec<f32> = (0..=n)
        .map(|i| cx - half + 2.0 * half * i as f32 / n as f32)
        .collect();
    let mut band_pts: Vec<P> = xs.iter().map(|&x| (x, top(x))).collect();
    band_pts.extend(xs.iter().rev().map(|&x| (x, top(x) + thick)));
    let band = poly(&band_pts);
    let (lx, rx) = (cx - half, cx + half);
    let tail_top = top(lx) + 0.12;
    let tail = |s: f32| {
        let inner = cx + s * (half - 0.16);
        let outer = cx + s * (half + 0.33);
        poly(&[
            (inner, tail_top),
            (outer, tail_top),
            (outer - s * 0.12, tail_top + thick * 0.5),
            (outer, tail_top + thick),
            (inner, tail_top + thick),
        ])
    };
    let fold = |s: f32| {
        let x = if s < 0.0 { lx } else { rx };
        let bb = top(x) + thick;
        poly(&[(x, bb), (x - s * 0.16, bb), (x - s * 0.16, bb + 0.12)])
    };
    let (tl, tr, fl, fr) = (tail(-1.0), tail(1.0), fold(-1.0), fold(1.0));
    let all = [&tl, &tr, &fl, &fr, &band];
    let e = ink(main);
    for part in all {
        p.stroke(part, WHITE, 2.0 * (RIM + EDGE));
    }
    for part in all {
        p.stroke(part, e, 2.0 * EDGE);
    }
    p.paint(&tl, &gloss(shade(main, 0.82), tail_top, tail_top + thick));
    p.paint(&tr, &gloss(shade(main, 0.82), tail_top, tail_top + thick));
    p.fill(&fl, shade(main, 0.5));
    p.fill(&fr, shade(main, 0.5));
    p.stroke(&band, e, 2.0 * EDGE);
    p.paint(&band, &gloss(main, 0.2, 0.62));
    for off in [0.045, thick - 0.045] {
        let pts: Vec<P> = xs.iter().map(|&x| (x, top(x) + off)).collect();
        let l = line(&pts[1..n]);
        p.stroke_with(&l, &solid(fade(stitch, 0.9)), &dashed(0.014, 0.04, 0.03));
    }
    p.shadow(0.4);
}

pub fn seal(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, tails_c) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.4);
    let tail = |a: P, b: P| {
        let d = norm((b.0 - a.0, b.1 - a.1));
        let n = (-d.1 * 0.075, d.0 * 0.075);
        poly(&[
            (a.0 + n.0, a.1 + n.1),
            (b.0 + n.0, b.1 + n.1),
            (b.0 - d.0 * 0.07, b.1 - d.1 * 0.07),
            (b.0 - n.0, b.1 - n.1),
            (a.0 - n.0, a.1 - n.1),
        ])
    };
    let t1 = tail((cx - 0.06, cy + 0.12), (cx - 0.2, 0.84));
    let t2 = tail((cx + 0.06, cy + 0.12), (cx + 0.2, 0.84));
    p.sticker(
        &[&t1, &t2],
        &gloss(tails_c, 0.5, 0.85),
        Some((ink(tails_c), EDGE)),
        Some((WHITE, RIM)),
    );
    for t in [&t1, &t2] {
        let m = p.clip(t);
        p.fill_in(&rrect(cx - 0.4, 0.5, 0.8, 0.03, 0.0), fade(WHITE, 0.25), &m);
    }
    let rosette = scallop(cx, cy, 0.28, 0.28, 18, 0.04);
    p.sticker(
        &[&rosette],
        &gloss(main, 0.05, 0.7),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let inner = circle(cx, cy, 0.2);
    p.paint(&inner, &gloss(shade(main, 0.9), cy - 0.2, cy + 0.2));
    p.stroke(&inner, fade(tint(main, 0.5), 0.9), 0.016);
    p.stroke_with(
        &circle(cx, cy, 0.168),
        &solid(fade(ink(main), 0.5)),
        &dashed(0.01, 0.001, 0.028),
    );
    let st = star(cx, cy + 0.01, 0.11, 0.048, 5, 0.0);
    p.sticker(
        &[&st],
        &solid(WHITE),
        Some((fade(ink(main), 0.8), EDGE * 0.7)),
        None,
    );
    p.shadow(0.4);
}

pub fn price_tag(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let c = (p.w * 0.5 + 0.06, 0.52);
    let deg = -18.0;
    let tag = rot(
        &poly(&[
            (c.0 - 0.24, c.1 - 0.21),
            (c.0 + 0.4, c.1 - 0.21),
            (c.0 + 0.4, c.1 + 0.21),
            (c.0 - 0.24, c.1 + 0.21),
            (c.0 - 0.46, c.1),
        ]),
        deg,
        c,
    );
    let hole_c = {
        let a = deg.to_radians();
        let (dx, dy) = (-0.29, 0.0);
        (
            c.0 + dx * a.cos() - dy * a.sin(),
            c.1 + dx * a.sin() + dy * a.cos(),
        )
    };
    let string = smooth(
        &[
            hole_c,
            (hole_c.0 - 0.1, hole_c.1 - 0.14),
            (hole_c.0 - 0.04, hole_c.1 - 0.3),
            (hole_c.0 + 0.1, hole_c.1 - 0.34),
        ],
        false,
        &[],
    );
    p.stroke(&string, WHITE, 0.05);
    p.stroke(&string, ink(main), 0.02);
    p.stroke_with(&tag, &solid(WHITE), &round(2.0 * (RIM + EDGE)));
    p.stroke_with(&tag, &solid(ink(main)), &round(2.0 * EDGE));
    p.paint(&tag, &gloss(main, 0.2, 0.85));
    let inner = scaled(&tag, 0.86, (c.0 + 0.02, c.1));
    p.stroke_with(
        &inner,
        &solid(fade(WHITE, 0.55)),
        &dashed(0.012, 0.035, 0.025),
    );
    p.erase(&circle(hole_c.0, hole_c.1, 0.045));
    p.stroke(&circle(hole_c.0, hole_c.1, 0.045), ink(main), 0.02);
    p.stroke(
        &seg(hole_c, (hole_c.0 - 0.012, hole_c.1 - 0.045)),
        ink(main),
        0.02,
    );
    if let Some(t) = text("Poppins Black", "%", (c.0 + 0.1, c.1), 0.34, 0.27, 0.0, deg) {
        letters(&mut p, &t, fg, fade(ink(main), 0.55), 0.02);
    }
    p.shadow(0.4);
}

pub fn hot(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.94);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let body = star(cx, cy, 0.42, 0.31, 18, 0.0);
    p.sticker(
        &[&body],
        &radial(
            (cx, cy + 0.25),
            0.6,
            &[
                (0.0, tint(main, 0.35)),
                (0.5, main),
                (1.0, shade(main, 0.7)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    if let Some(t) = text(
        "Poppins Black",
        "HOT",
        (cx, cy + 0.01),
        0.5,
        0.21,
        0.0,
        -8.0,
    ) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.02);
    }
    p.halo(0.4);
}

pub fn free(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let (bw, bh) = (1.4, 0.56);
    // A ticket: a rounded bar with a notch bitten out of each side.
    let body = rot(&rrect(c.0 - bw * 0.5, c.1 - bh * 0.5, bw, bh, 0.1), 5.0, c);
    p.sticker(
        &[&body],
        &gloss(main, 0.15, 0.85),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    for s in [-1.0f32, 1.0] {
        let notch = rot(&circle(c.0 + s * bw * 0.5, c.1, 0.07), 5.0, c);
        p.fill(&notch, fade(ink(main), 0.55));
    }
    let inner = rot(
        &rrect(
            c.0 - bw * 0.5 + 0.1,
            c.1 - bh * 0.5 + 0.06,
            bw - 0.2,
            bh - 0.12,
            0.05,
        ),
        5.0,
        c,
    );
    p.stroke_with(&inner, &solid(fade(fg, 0.85)), &dashed(0.016, 0.045, 0.03));
    if let Some(t) = text("Poppins Black", "FREE", c, 0.86, 0.26, 0.0, 5.0) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.022);
    }
    p.shadow(0.4);
}

pub fn best(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.94);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let hex: Vec<P> = (0..6)
        .map(|k| {
            let a = (k as f32 * 60.0 - 90.0).to_radians();
            (cx + 0.4 * a.cos(), cy + 0.4 * a.sin())
        })
        .collect();
    let body = smooth(&hex, true, &[]);
    p.sticker(
        &[&body],
        &gloss(main, 0.1, 0.9),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let ring: Vec<P> = hex
        .iter()
        .map(|(x, y)| (cx + (x - cx) * 0.82, cy + (y - cy) * 0.82))
        .collect();
    p.stroke_with(
        &smooth(&ring, true, &[]),
        &solid(fade(fg, 0.85)),
        &dashed(0.014, 0.04, 0.03),
    );
    p.fill(&star(cx, cy - 0.2, 0.065, 0.03, 5, 0.0), fg);
    if let Some(t) = text(
        "Poppins Black",
        "BEST",
        (cx, cy + 0.07),
        0.46,
        0.17,
        0.0,
        0.0,
    ) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.016);
    }
    p.halo(0.4);
}

pub fn discount(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.94);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let body = scallop(cx, cy, 0.36, 0.36, 12, 0.07);
    p.sticker(
        &[&body],
        &radial(
            (cx - 0.1, cy - 0.14),
            0.65,
            &[
                (0.0, tint(main, 0.4)),
                (0.55, main),
                (1.0, shade(main, 0.72)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    if let Some(t) = text("Poppins Black", "50%", (cx, cy - 0.04), 0.5, 0.2, 0.0, -8.0) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.02);
    }
    if let Some(t) = text(
        "Poppins Black",
        "OFF",
        (cx + 0.01, cy + 0.15),
        0.28,
        0.1,
        0.0,
        -8.0,
    ) {
        letters(&mut p, &t, fg, fade(ink(main), 0.6), 0.014);
    }
    p.halo(0.4);
}
