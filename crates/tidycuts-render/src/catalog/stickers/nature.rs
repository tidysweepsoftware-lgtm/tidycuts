//! Nature and weather: a sun, a cloud, a rainbow, a crescent moon, a
//! snowflake, a flower, a leaf and a raindrop.

use super::kit::*;
use super::DrawCtx;
use std::f32::consts::TAU;
use tiny_skia::{Path, Pixmap};

/// How much smaller than the full sticker box these are drawn, so the
/// outline and shadow stay inside it.
const INSET: f32 = 0.9;

pub fn sun(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rays) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let spikes = star(cx, cy, 0.46, 0.3, 12, 0.0);
    p.sticker(
        &[&spikes],
        &radial(
            (cx, cy),
            0.46,
            &[(0.0, tint(rays, 0.4)), (1.0, shade(rays, 0.9))],
        ),
        Some((ink(rays), EDGE)),
        Some((WHITE, RIM * 0.7)),
    );
    let disc = circle(cx, cy, 0.27);
    p.sticker(
        &[&disc],
        &radial(
            (cx - 0.08, cy - 0.1),
            0.4,
            &[(0.0, tint(main, 0.6)), (0.5, main), (1.0, shade(main, 0.8))],
        ),
        Some((ink(rays), EDGE)),
        None,
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.1, cy - 0.12, 0.08, 0.04),
            -38.0,
            (cx - 0.1, cy - 0.12),
        ),
        fade(WHITE, 0.55),
    );
    p.halo(0.35);
}

/// The body of a cloud: bumps over a flat base, centred on `cx`.
fn cloud_parts(cx: f32, cy: f32, k: f32) -> Vec<Path> {
    vec![
        circle(cx - 0.22 * k, cy + 0.03 * k, 0.15 * k),
        circle(cx - 0.02 * k, cy - 0.07 * k, 0.21 * k),
        circle(cx + 0.2 * k, cy + 0.01 * k, 0.16 * k),
        rrect(cx - 0.34 * k, cy + 0.02 * k, 0.68 * k, 0.17 * k, 0.085 * k),
    ]
}

pub fn cloud(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, under) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.48);
    let parts = cloud_parts(cx, cy, 1.25);
    let refs: Vec<&Path> = parts.iter().collect();
    p.sticker(
        &refs,
        &linear(
            (cx, cy - 0.3),
            (cx, cy + 0.3),
            &[(0.0, WHITE), (0.45, main), (1.0, under)],
        ),
        Some((ink(under), EDGE)),
        Some((WHITE, RIM * 0.6)),
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.1, cy - 0.2, 0.09, 0.035),
            -30.0,
            (cx - 0.1, cy - 0.2),
        ),
        fade(WHITE, 0.8),
    );
    p.shadow(0.35);
}

pub fn rainbow(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (cloud, under) = (p.c1, p.c2);
    let (cx, base) = (p.w * 0.5, 0.72);
    let bands = [
        rgb(0.93, 0.25, 0.27),
        rgb(1.0, 0.6, 0.15),
        rgb(1.0, 0.86, 0.2),
        rgb(0.3, 0.78, 0.38),
        rgb(0.25, 0.6, 0.98),
        rgb(0.55, 0.38, 0.9),
    ];
    let bw = 0.058;
    let r_out = 0.52;
    let total = bw * bands.len() as f32;
    let mid = r_out - total * 0.5;
    let arc = |r: f32| line(&arc_pts(cx, base, r, r, 180.0, 360.0, 48));
    let whole = arc(mid);
    p.stroke(&whole, WHITE, total + 2.0 * (RIM + EDGE));
    p.stroke(&whole, fade(ink(bands[4]), 0.9), total + 2.0 * EDGE);
    for (i, c) in bands.iter().enumerate() {
        let r = r_out - bw * (i as f32 + 0.5);
        p.stroke_with(&arc(r), &solid(*c), &butt(bw + 0.002));
    }
    let hl = line(&arc_pts(
        cx,
        base,
        r_out - 0.012,
        r_out - 0.012,
        200.0,
        250.0,
        12,
    ));
    p.stroke(&hl, fade(WHITE, 0.4), 0.014);
    // A cloud at each end.
    for s in [-1.0f32, 1.0] {
        let parts = cloud_parts(cx + s * (r_out - total * 0.5), base + 0.03, 0.8);
        let refs: Vec<&Path> = parts.iter().collect();
        p.sticker(
            &refs,
            &linear(
                (cx, base - 0.2),
                (cx, base + 0.2),
                &[(0.0, WHITE), (0.5, cloud), (1.0, under)],
            ),
            Some((ink(under), EDGE)),
            Some((WHITE, RIM * 0.6)),
        );
    }
    p.shadow(0.35);
}

pub fn moon(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, star_c) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    // A crescent: the big circle minus a smaller one pushed to the right.
    let (cy, big, small, d) = (0.52, 0.36f32, 0.3f32, 0.18f32);
    let x = (big * big - small * small + d * d) / (2.0 * d);
    let y = (big * big - x * x).sqrt();
    let a = y.atan2(x).to_degrees();
    let b = (-y).atan2(x - d).to_degrees() + 360.0;
    let (ox, oy) = (cx - 0.06, cy);
    let mut pts = arc_pts(ox, oy, big, big, a, 360.0 - a, 48);
    pts.extend(arc_pts(ox + d, oy, small, small, b, 360.0 - b, 48));
    let crescent = rot(&smooth(&pts, true, &[0, 48, 49, 97]), -18.0, (ox, oy));
    p.sticker(
        &[&crescent],
        &linear(
            (ox - big, oy - big),
            (ox + 0.1, oy + big),
            &[
                (0.0, tint(main, 0.5)),
                (0.55, main),
                (1.0, shade(main, 0.78)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM * 0.7)),
    );
    for (px, py, r) in [
        (-0.27, 0.0, 0.04),
        (-0.2, 0.22, 0.03),
        (-0.31, -0.18, 0.025),
    ] {
        p.fill(&circle(ox + px, oy + py, r), fade(shade(main, 0.75), 0.45));
    }
    // A star and a speck beside it.
    let s = sparkle(cx + 0.27, 0.3, 0.11, 0.16);
    p.stroke(&s, fade(ink(star_c), 0.8), 0.014);
    p.paint(
        &s,
        &radial(
            (cx + 0.27, 0.3),
            0.11,
            &[(0.0, WHITE), (0.5, tint(star_c, 0.4)), (1.0, star_c)],
        ),
    );
    p.fill(&circle(cx + 0.3, 0.62, 0.025), fade(star_c, 0.95));
    p.shadow(0.35);
}

pub fn snowflake(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let mut strokes: Vec<Path> = Vec::new();
    for k in 0..6 {
        let a = (k as f32 * 60.0 - 90.0).to_radians();
        let at = |r: f32| (cx + r * a.cos(), cy + r * a.sin());
        strokes.push(seg(at(0.0), at(0.4)));
        for (r, len) in [(0.2, 0.12), (0.31, 0.1)] {
            for s in [-1.0f32, 1.0] {
                let b = a + s * 0.85;
                let from = at(r);
                strokes.push(seg(from, (from.0 + len * b.cos(), from.1 + len * b.sin())));
            }
        }
    }
    let w = 0.055;
    for s in &strokes {
        p.stroke(s, rim, w + 2.0 * (RIM * 0.7 + EDGE));
    }
    for s in &strokes {
        p.stroke(s, ink(main), w + 2.0 * EDGE);
    }
    for s in &strokes {
        p.stroke_with(
            s,
            &linear(
                (cx, cy - 0.4),
                (cx, cy + 0.4),
                &[(0.0, tint(main, 0.6)), (1.0, main)],
            ),
            &round(w),
        );
    }
    p.fill(&circle(cx, cy, 0.05), WHITE);
    p.halo(0.35);
}

pub fn flower(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, mid) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let petals: Vec<Path> = (0..8)
        .map(|k| {
            let a = k as f32 * 45.0;
            rot(&ellipse(cx, cy - 0.23, 0.095, 0.17), a, (cx, cy))
        })
        .collect();
    let refs: Vec<&Path> = petals.iter().collect();
    p.sticker(
        &refs,
        &radial(
            (cx, cy),
            0.42,
            &[
                (0.0, shade(main, 0.85)),
                (0.6, main),
                (1.0, tint(main, 0.35)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM * 0.7)),
    );
    for k in 0..8 {
        let a = (k as f32 * 45.0 - 90.0).to_radians();
        p.stroke(
            &seg(
                (cx + 0.18 * a.cos(), cy + 0.18 * a.sin()),
                (cx + 0.34 * a.cos(), cy + 0.34 * a.sin()),
            ),
            fade(shade(main, 0.75), 0.5),
            0.012,
        );
    }
    p.sticker(
        &[&circle(cx, cy, 0.13)],
        &radial(
            (cx - 0.04, cy - 0.04),
            0.2,
            &[(0.0, tint(mid, 0.5)), (0.6, mid), (1.0, shade(mid, 0.8))],
        ),
        Some((ink(mid), EDGE * 0.9)),
        None,
    );
    for i in 0..7u32 {
        let a = i as f32 / 7.0 * TAU + 0.4;
        let r = 0.05 + 0.03 * hash(i);
        p.fill(
            &circle(cx + r * a.cos(), cy + r * a.sin(), 0.011),
            fade(shade(mid, 0.55), 0.7),
        );
    }
    p.shadow(0.35);
}

pub fn leaf(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, vein) = (p.c1, p.c2);
    let cx = p.w * 0.5 - 0.5;
    let blade = smooth(
        &[
            (cx + 0.2, 0.8),
            (cx + 0.14, 0.52),
            (cx + 0.3, 0.26),
            (cx + 0.62, 0.12),
            (cx + 0.88, 0.1),
            (cx + 0.86, 0.38),
            (cx + 0.7, 0.66),
            (cx + 0.42, 0.78),
        ],
        true,
        &[0, 4],
    );
    p.sticker_line(
        &seg((cx + 0.22, 0.78), (cx + 0.1, 0.92)),
        0.05,
        &solid(shade(main, 0.7)),
        Some((ink(main), EDGE)),
        None,
    );
    p.sticker(
        &[&blade],
        &linear(
            (cx + 0.2, 0.8),
            (cx + 0.88, 0.1),
            &[
                (0.0, shade(main, 0.75)),
                (0.5, main),
                (1.0, tint(main, 0.35)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM * 0.7)),
    );
    let m = p.clip(&blade);
    let spine = smooth(
        &[
            (cx + 0.2, 0.8),
            (cx + 0.42, 0.52),
            (cx + 0.66, 0.3),
            (cx + 0.88, 0.1),
        ],
        false,
        &[],
    );
    p.stroke(&spine, fade(vein, 0.9), 0.03);
    for (t, len) in [(0.3, 0.2), (0.5, 0.22), (0.7, 0.16)] {
        let (x, y) = (cx + 0.2 + 0.68 * t, 0.8 - 0.7 * t);
        p.stroke(
            &seg((x, y), (x + len * 0.7, y - len * 0.1)),
            fade(vein, 0.6),
            0.014,
        );
        p.stroke(
            &seg((x, y), (x - len * 0.2, y + len * 0.65)),
            fade(vein, 0.6),
            0.014,
        );
    }
    p.fill_in(
        &rot(
            &ellipse(cx + 0.36, 0.34, 0.12, 0.03),
            -50.0,
            (cx + 0.36, 0.34),
        ),
        fade(WHITE, 0.3),
        &m,
    );
    p.shadow(0.35);
}

pub fn raindrop(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, spark) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    let body = drop(cx, 0.56, 0.3, 0.0);
    p.sticker(
        &[&body],
        &radial(
            (cx - 0.1, 0.5),
            0.55,
            &[
                (0.0, tint(main, 0.55)),
                (0.5, main),
                (1.0, shade(main, 0.7)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.12, 0.55, 0.04, 0.11),
            12.0,
            (cx - 0.12, 0.55),
        ),
        fade(spark, 0.7),
    );
    p.fill(&circle(cx - 0.07, 0.38, 0.018), fade(spark, 0.8));
    p.shadow(0.35);
}
