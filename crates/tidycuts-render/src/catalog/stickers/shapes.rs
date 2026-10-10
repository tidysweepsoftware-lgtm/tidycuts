//! Basic shapes: circle, ring, star, heart, burst, blob and polygon
//! outlines.

use super::kit::*;
use super::DrawCtx;
use tiny_skia::Pixmap;

pub fn circle_disc(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let (cx, cy, r) = (0.5, 0.48, 0.37);
    let body = radial(
        (cx - 0.12, cy - 0.15),
        0.58,
        &[
            (0.0, tint(main, 0.45)),
            (0.5, main),
            (1.0, shade(main, 0.72)),
        ],
    );
    p.sticker(
        &[&circle(cx, cy, r)],
        &body,
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    let hl = rot(
        &ellipse(cx - 0.13, cy - 0.17, 0.12, 0.065),
        -38.0,
        (cx - 0.13, cy - 0.17),
    );
    p.fill(&hl, fade(WHITE, 0.5));
    p.fill(&circle(cx + 0.02, cy - 0.27, 0.025), fade(WHITE, 0.45));
    p.shadow(0.4);
}

pub fn ring(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let (cx, cy, r, w) = (0.5, 0.48, 0.32, 0.11);
    let c = circle(cx, cy, r);
    p.stroke(&c, rim, w + 2.0 * (RIM + EDGE));
    p.stroke(&c, ink(main), w + 2.0 * EDGE);
    p.stroke_with(&c, &gloss(main, cy - r, cy + r), &round(w));
    p.stroke(
        &line(&arc_pts(cx, cy, r, r, 200.0, 255.0, 16)),
        fade(WHITE, 0.5),
        0.03,
    );
    p.stroke(
        &line(&arc_pts(cx, cy, r, r, 268.0, 280.0, 4)),
        fade(WHITE, 0.5),
        0.03,
    );
    p.shadow(0.4);
}

pub fn star_shape(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let (cx, cy, ro) = (0.5, 0.53, 0.41);
    let pts = star_pts(cx, cy, ro, ro * 0.47, 5, 0.0);
    let body = poly(&pts);
    p.sticker(
        &[&body],
        &gloss(main, 0.12, 0.88),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    // Bevel: one lighter and one darker facet per point.
    for i in 0..5 {
        let o = pts[2 * i];
        let prev = pts[(2 * i + 9) % 10];
        let next = pts[2 * i + 1];
        p.fill(&poly(&[(cx, cy), prev, o]), fade(tint(main, 0.6), 0.35));
        p.fill(&poly(&[(cx, cy), o, next]), fade(shade(main, 0.75), 0.25));
    }
    p.stroke(&body, fade(ink(main), 0.6), 0.008);
    p.shadow(0.4);
}

pub fn heart_shape(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let w = p.w;
    let body = heart(w * 0.5, 0.5, 0.84, 0.76);
    let fill = radial(
        (w * 0.5 - 0.15, 0.3),
        0.75,
        &[(0.0, tint(main, 0.4)), (0.5, main), (1.0, shade(main, 0.7))],
    );
    p.sticker(&[&body], &fill, Some((ink(main), EDGE)), Some((rim, RIM)));
    let hl = rot(
        &ellipse(w * 0.5 - 0.22, 0.3, 0.09, 0.05),
        -40.0,
        (w * 0.5 - 0.22, 0.3),
    );
    p.fill(&hl, fade(WHITE, 0.6));
    p.fill(&circle(w * 0.5 - 0.29, 0.4, 0.022), fade(WHITE, 0.5));
    p.shadow(0.4);
}

pub fn burst(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, inner) = (p.c1, p.c2);
    let (cx, cy) = (0.5, 0.5);
    let outer = star(cx, cy, 0.39, 0.31, 14, 0.0);
    p.sticker(
        &[&outer],
        &gloss(main, 0.1, 0.9),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let mid = star(cx, cy, 0.28, 0.22, 14, 360.0 / 28.0);
    p.stroke(&mid, fade(ink(main), 0.5), 0.016);
    p.paint(&mid, &gloss(inner, 0.2, 0.8));
    p.fill(&circle(cx, cy, 0.12), fade(tint(inner, 0.6), 0.6));
    p.halo(0.4);
}

pub fn blob(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.49);
    let pts: Vec<P> = (0..8)
        .map(|i| {
            let ang = i as f32 / 8.0 * std::f32::consts::TAU + 0.3;
            let k = 1.0 + [0.06, -0.05, 0.1, -0.08, 0.04, 0.09, -0.06, 0.02][i];
            (cx + 0.42 * k * ang.cos(), cy + 0.33 * k * ang.sin())
        })
        .collect();
    let body = smooth(&pts, true, &[]);
    p.stroke(&body, WHITE, 2.0 * RIM);
    let fill = linear(
        (cx - 0.4, 0.15),
        (cx + 0.4, 0.85),
        &[(0.0, tint(a, 0.15)), (0.55, mix(a, b, 0.5)), (1.0, b)],
    );
    p.paint(&body, &fill);
    // Soft inner light.
    let glow = radial(
        (cx - 0.15, cy - 0.12),
        0.32,
        &[(0.0, fade(WHITE, 0.45)), (1.0, fade(WHITE, 0.0))],
    );
    p.paint(&body, &glow);
    let hl = rot(
        &ellipse(cx - 0.2, cy - 0.17, 0.11, 0.045),
        -25.0,
        (cx - 0.2, cy - 0.17),
    );
    p.fill(&hl, fade(WHITE, 0.55));
    p.shadow(0.35);
}

fn outline_poly(p: &mut Pen, pts: &[P], w: f32) {
    let (main, rim) = (p.c1, p.c2);
    let path = poly(pts);
    p.stroke(&path, rim, w + 2.0 * (RIM + EDGE));
    p.stroke(&path, ink(main), w + 2.0 * EDGE);
    let ys = pts.iter().map(|q| q.1);
    let (y0, y1) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
    p.stroke_with(&path, &gloss(main, y0, y1), &round(w));
}

pub fn hexagon(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (cx, cy) = (p.w * 0.5, 0.49);
    let hex: Vec<P> = (0..6)
        .map(|i| {
            let a = (i as f32 * 60.0).to_radians();
            (cx + 0.345 * a.cos(), cy + 0.345 * a.sin())
        })
        .collect();
    outline_poly(&mut p, &hex, 0.095);
    p.shadow(0.4);
}

pub fn triangle(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let cx = p.w * 0.5;
    let tri = [(cx, 0.15), (cx + 0.39, 0.79), (cx - 0.39, 0.79)];
    outline_poly(&mut p, &tri, 0.095);
    p.shadow(0.4);
}
