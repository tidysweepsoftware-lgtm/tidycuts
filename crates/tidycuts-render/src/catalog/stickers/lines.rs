//! Lines: underline swoosh, scribbled circle, brush stroke, divider,
//! squiggle and a highlighter mark.

use super::kit::*;
use super::DrawCtx;
use tiny_skia::Pixmap;

/// Width profile of a brush stroke: a round start over `s0`, then `body`.
fn round_start(s: f32, s0: f32) -> f32 {
    if s >= s0 {
        1.0
    } else {
        let k = 1.0 - s / s0;
        (1.0 - k * k).max(0.0).sqrt()
    }
}

pub fn swoosh(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, second) = (p.c1, p.c2);
    let a = sample(
        &[
            (0.14, 0.58),
            (0.9, 0.5),
            (1.8, 0.44),
            (2.5, 0.42),
            (2.88, 0.34),
        ],
        false,
        24,
    );
    let top = ribbon(&a, |s| {
        0.21 * (1.0 - s).powf(0.5) * round_start(s, 0.04) + 0.01
    });
    let b = sample(
        &[(0.46, 0.74), (1.3, 0.66), (2.1, 0.6), (2.6, 0.57)],
        false,
        24,
    );
    let low = ribbon(&b, |s| {
        0.1 * (1.0 - s).powf(0.55) * round_start(s, 0.025) + 0.008
    });
    p.paint(
        &low,
        &linear(
            (0.5, 0.0),
            (2.6, 0.0),
            &[(0.0, second), (1.0, tint(second, 0.3))],
        ),
    );
    p.paint(
        &top,
        &linear(
            (0.14, 0.0),
            (2.9, 0.0),
            &[
                (0.0, shade(main, 0.9)),
                (0.5, main),
                (1.0, tint(main, 0.35)),
            ],
        ),
    );
    let shine = sample(&[(0.3, 0.53), (1.0, 0.465), (1.7, 0.43)], false, 12);
    p.paint(
        &ribbon(&shine, |s| 0.03 * (1.0 - s) * round_start(s, 0.1)),
        &solid(fade(WHITE, 0.35)),
    );
    p.shadow(0.3);
}

pub fn scribble_circle(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let main = p.c1;
    let (cx, cy) = (p.w * 0.5, 0.5);
    let n = 160;
    let turns = 1.16;
    let pts: Vec<P> = (0..=n)
        .map(|k| {
            let f = k as f32 / n as f32;
            let th = (-165.0 + f * turns * 360.0).to_radians();
            let rf = 1.0 + 0.04 * (3.0 * th + 1.0).sin() - 0.08 * f;
            (cx + 0.55 * rf * th.cos(), cy + 0.33 * rf * th.sin())
        })
        .collect();
    let body = ribbon(&pts, |s| {
        0.075
            * (0.78 + 0.22 * (s * 9.0).sin())
            * round_start(s, 0.02)
            * round_start(1.0 - s, 0.06).max(0.15)
    });
    let body = rot(&body, -6.0, (cx, cy));
    p.stroke(&body, main, 0.006);
    p.paint(&body, &gloss(main, 0.1, 0.9));
    p.shadow(0.3);
}

pub fn brush(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let main = p.c1;
    let c = sample(
        &[(0.18, 0.54), (1.0, 0.5), (2.0, 0.49), (3.04, 0.43)],
        false,
        60,
    );
    let env = |s: f32| {
        let end = 1.0 - 0.45 * smoothstep(0.7, 1.0, s);
        0.22 * round_start(s, 0.035).max(0.35) * end
    };
    let rough = |s: f32, k: u32| {
        let i = (s * 240.0) as u32;
        let f = s * 240.0 - i as f32;
        let n = hash(i * 2 + k) * (1.0 - f) + hash((i + 1) * 2 + k) * f;
        1.0 + 0.035 * (n - 0.5) + 0.02 * (s * 37.0 + k as f32).sin()
    };
    let body = band(&c, |s| env(s) * rough(s, 0), |s| env(s) * rough(s, 1));
    p.paint(
        &body,
        &linear(
            (0.0, 0.3),
            (0.0, 0.7),
            &[(0.0, tint(main, 0.2)), (1.0, shade(main, 0.88))],
        ),
    );
    // Bristle texture: lighter streaks, then dry gaps towards the end.
    let along = |off: f32, s0: f32, s1: f32| -> Vec<P> {
        let n = c.len();
        let i0 = ((n - 1) as f32 * s0) as usize;
        let i1 = ((n - 1) as f32 * s1) as usize;
        (i0..=i1.min(n - 1))
            .map(|i| {
                let s = i as f32 / (n - 1) as f32;
                (c[i].0, c[i].1 + off * env(s))
            })
            .collect()
    };
    for k in 0..9 {
        let off = -0.85 + 1.7 * hash(k + 11);
        let s0 = 0.03 + 0.4 * hash(k + 31);
        let pts = along(off, s0, (s0 + 0.25 + 0.3 * hash(k + 51)).min(0.97));
        p.stroke(
            &line(&pts),
            fade(tint(main, 0.5), 0.3),
            0.008 + 0.01 * hash(k + 71),
        );
    }
    for k in 0..14 {
        let off = -0.92 + 1.84 * hash(k + 101);
        let s0 = 0.6 + 0.3 * hash(k + 131);
        let pts = along(off, s0, 1.0);
        p.erase_stroke(&line(&pts), &butt(0.005 + 0.012 * hash(k + 151)));
    }
    p.shadow(0.3);
}

pub fn divider(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, inner) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let edge = Some((ink(main), EDGE * 0.8));
    for s in [-1.0f32, 1.0] {
        let pts: Vec<P> = (0..=40)
            .map(|i| (cx + s * (0.42 + 1.32 * i as f32 / 40.0), cy))
            .collect();
        let l = ribbon(&pts, |t| 0.075 * (1.0 - t).powf(0.8) + 0.004);
        p.sticker(&[&l], &gloss(main, 0.45, 0.55), edge, None);
        let dot = circle(cx + s * 0.34, cy, 0.045);
        p.sticker(&[&dot], &gloss(main, 0.45, 0.55), edge, None);
        let d2 = poly(&[
            (cx + s * 1.82, cy - 0.06),
            (cx + s * 1.88, cy),
            (cx + s * 1.82, cy + 0.06),
            (cx + s * 1.76, cy),
        ]);
        p.sticker(&[&d2], &solid(main), edge, None);
    }
    let big = poly(&[
        (cx, cy - 0.3),
        (cx + 0.24, cy),
        (cx, cy + 0.3),
        (cx - 0.24, cy),
    ]);
    p.sticker(
        &[&big],
        &gloss(main, 0.2, 0.8),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM * 0.7)),
    );
    let small = poly(&[
        (cx, cy - 0.15),
        (cx + 0.12, cy),
        (cx, cy + 0.15),
        (cx - 0.12, cy),
    ]);
    p.fill(&small, inner);
    p.fill(
        &poly(&[(cx, cy - 0.3), (cx + 0.24, cy), (cx, cy)]),
        fade(WHITE, 0.2),
    );
    p.shadow(0.3);
}

pub fn squiggle(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let w = p.w;
    let (x0, x1) = (0.22, w - 0.22);
    let pts: Vec<P> = (0..=160)
        .map(|i| {
            let f = i as f32 / 160.0;
            (
                x0 + (x1 - x0) * f,
                0.5 + 0.17 * (f * 3.0 * std::f32::consts::TAU).sin(),
            )
        })
        .collect();
    let path = line(&pts);
    p.sticker_line(
        &path,
        0.09,
        &gloss(main, 0.3, 0.7),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    p.shadow(0.3);
}

pub fn highlight(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let main = p.c1;
    let w = p.w;
    // A chisel-tip marker pass: slanted ends, slightly wavy edges.
    let n = 80;
    let (x0, x1, y0, y1) = (0.2, w - 0.2, 0.28, 0.72);
    let slant = 0.09;
    let mut pts = Vec::new();
    for i in 0..=n {
        let f = i as f32 / n as f32;
        pts.push((
            x0 + slant + (x1 - x0) * f,
            y0 + 0.01 * (f * 17.0).sin() + 0.006 * (f * 41.0).sin(),
        ));
    }
    for i in (0..=n).rev() {
        let f = i as f32 / n as f32;
        pts.push((
            x0 + (x1 - x0 - slant * 0.4) * f,
            y1 + 0.01 * (f * 13.0 + 1.0).sin() + 0.006 * (f * 37.0).sin(),
        ));
    }
    let band = poly(&pts);
    p.fill(&band, fade(main, 0.6));
    let m = p.clip(&band);
    // Ink pools along the edges and at the start, streaks in the middle.
    for (y, a) in [(y0 + 0.012, 0.22), (y1 - 0.012, 0.22)] {
        p.fill_in(
            &rrect(0.0, y - 0.02, w, 0.04, 0.0),
            fade(shade(main, 0.9), a),
            &m,
        );
    }
    p.fill_in(
        &poly(&[
            (x0, y1 + 0.05),
            (x0 + slant + 0.06, y0 - 0.05),
            (x0 + slant + 0.1, y0 - 0.05),
            (x0 + 0.04, y1 + 0.05),
        ]),
        fade(main, 0.3),
        &m,
    );
    for k in 0..5u32 {
        let y = y0 + 0.08 + (y1 - y0 - 0.16) * hash(k + 3);
        let s0 = x0 + 0.3 + (w - 1.2) * hash(k + 13);
        p.fill_in(
            &rrect(s0, y, 0.4 + 0.5 * hash(k + 23), 0.012, 0.006),
            fade(WHITE, 0.12),
            &m,
        );
    }
}

pub fn zigzag(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let w = p.w;
    let n = 8;
    let pts: Vec<P> = (0..=n)
        .map(|i| {
            let x = 0.2 + (w - 0.4) * i as f32 / n as f32;
            (x, if i % 2 == 0 { 0.66 } else { 0.34 })
        })
        .collect();
    let path = line(&pts);
    p.stroke_with(&path, &solid(ink(main)), &{
        let mut s = round(0.1 + 2.0 * EDGE);
        s.line_join = tiny_skia::LineJoin::Round;
        s
    });
    p.sticker_line(
        &path,
        0.1,
        &gloss(main, 0.3, 0.7),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    p.shadow(0.35);
}

pub fn spiral(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.94);
    let (main, rim) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let turns = 2.6f32;
    let pts: Vec<P> = (0..=120)
        .map(|i| {
            let k = i as f32 / 120.0;
            let a = k * turns * std::f32::consts::TAU - 1.2;
            let r = 0.05 + 0.3 * k;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    let path = line(&pts);
    p.sticker_line(
        &path,
        0.075,
        &gloss(main, 0.15, 0.85),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    p.shadow(0.35);
}

pub fn dotted(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, second) = (p.c1, p.c2);
    let w = p.w;
    let n = 9;
    for i in 0..n {
        let k = i as f32 / (n - 1) as f32;
        let x = 0.2 + (w - 0.4) * k;
        let y = 0.5 + 0.14 * (k * std::f32::consts::TAU * 1.25).sin();
        let r = 0.045 + 0.05 * (0.5 - (k - 0.5).abs() * 1.0);
        let c = if i % 2 == 0 { main } else { second };
        p.sticker(
            &[&circle(x, y, r)],
            &radial(
                (x - r * 0.3, y - r * 0.4),
                r * 1.6,
                &[(0.0, tint(c, 0.5)), (1.0, c)],
            ),
            Some((ink(c), EDGE * 0.8)),
            Some((WHITE, RIM * 0.5)),
        );
    }
    p.shadow(0.35);
}
