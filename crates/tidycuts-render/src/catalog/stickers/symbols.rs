//! Symbols: check and cross marks, question and exclamation marks, a light
//! bulb, a lightning bolt, a music note, a crown, a trophy, a gift, a rocket,
//! a map pin, a play button, a camera, a microphone and a clapperboard.
//! Main color = the symbol, second = its trim.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use std::f32::consts::TAU;
use tiny_skia::{Path, Pixmap};

/// How much smaller than the full sticker box these are drawn, so the
/// outline and shadow stay inside it.
const INSET: f32 = 0.9;

/// A thick polyline sticker (check mark, cross strokes).
fn bold_line(p: &mut Pen, pts: &[P], width: f32, main: C, rim: C) {
    let path = line(pts);
    p.sticker_line(
        &path,
        width,
        &gloss(main, 0.2, 0.8),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
}

pub fn check(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    bold_line(
        &mut p,
        &[(x + 0.17, 0.53), (x + 0.4, 0.75), (x + 0.84, 0.25)],
        0.19,
        main,
        rim,
    );
    p.shadow(0.4);
}

pub fn cross(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    // One continuous shape so the two bars share one outline.
    let a = seg((x + 0.22, 0.22), (x + 0.78, 0.78));
    let b = seg((x + 0.78, 0.22), (x + 0.22, 0.78));
    let w = 0.18;
    p.stroke(&a, rim, w + 2.0 * (RIM + EDGE));
    p.stroke(&b, rim, w + 2.0 * (RIM + EDGE));
    p.stroke(&a, ink(main), w + 2.0 * EDGE);
    p.stroke(&b, ink(main), w + 2.0 * EDGE);
    p.stroke_with(&a, &gloss(main, 0.2, 0.8), &round(w));
    p.stroke_with(&b, &gloss(main, 0.2, 0.8), &round(w));
    p.shadow(0.4);
}

/// A big glyph with the sticker look.
fn glyph(p: &mut Pen, s: &str, w: f32, h: f32, deg: f32, main: C, rim: C) {
    let c = (p.w * 0.5, 0.5);
    if let Some(t) = text("Poppins Black", s, c, w, h, 0.0, deg) {
        p.sticker(
            &[&t],
            &gloss(main, c.1 - h * 0.5, c.1 + h * 0.5),
            Some((ink(main), EDGE * 1.2)),
            Some((rim, RIM)),
        );
    }
}

pub fn question(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    glyph(&mut p, "?", 0.5, 0.72, 6.0, main, rim);
    p.shadow(0.4);
}

pub fn exclamation(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    glyph(&mut p, "!", 0.26, 0.72, -6.0, main, rim);
    p.shadow(0.4);
}

pub fn bulb(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, base) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    // Rays around the glass.
    for i in 0..7 {
        let a = (-170.0 + 26.67 * i as f32).to_radians();
        let (x0, y0) = (cx + 0.36 * a.cos(), 0.4 + 0.31 * a.sin());
        let (x1, y1) = (cx + 0.45 * a.cos(), 0.4 + 0.39 * a.sin());
        p.stroke(&seg((x0, y0), (x1, y1)), fade(tint(main, 0.2), 0.95), 0.04);
    }
    // Glass: a round top running into a narrower neck.
    let glass = smooth(
        &[
            (cx - 0.15, 0.7),
            (cx - 0.17, 0.57),
            (cx - 0.27, 0.46),
            (cx - 0.27, 0.34),
            (cx - 0.17, 0.2),
            (cx, 0.15),
            (cx + 0.17, 0.2),
            (cx + 0.27, 0.34),
            (cx + 0.27, 0.46),
            (cx + 0.17, 0.57),
            (cx + 0.15, 0.7),
        ],
        true,
        &[0, 10],
    );
    p.sticker(
        &[&glass],
        &radial(
            (cx - 0.08, 0.3),
            0.5,
            &[
                (0.0, tint(main, 0.7)),
                (0.5, main),
                (1.0, shade(main, 0.78)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM * 0.8)),
    );
    // A filament.
    let fil = smooth(
        &[
            (cx - 0.07, 0.69),
            (cx - 0.07, 0.5),
            (cx - 0.035, 0.44),
            (cx, 0.5),
            (cx + 0.035, 0.44),
            (cx + 0.07, 0.5),
            (cx + 0.07, 0.69),
        ],
        false,
        &[],
    );
    p.stroke(&fil, fade(shade(main, 0.5), 0.7), 0.022);
    p.fill(
        &rot(
            &ellipse(cx - 0.12, 0.3, 0.035, 0.09),
            25.0,
            (cx - 0.12, 0.3),
        ),
        fade(WHITE, 0.6),
    );
    // Screw cap.
    let cap = rrect(cx - 0.14, 0.7, 0.28, 0.17, 0.04);
    p.sticker(
        &[&cap],
        &gloss(base, 0.7, 0.87),
        Some((ink(base), EDGE)),
        None,
    );
    for k in 0..2 {
        let y = 0.745 + 0.05 * k as f32;
        p.stroke(
            &seg((cx - 0.13, y), (cx + 0.13, y)),
            fade(ink(base), 0.6),
            0.012,
        );
    }
    let tip = poly(&[
        (cx - 0.06, 0.87),
        (cx + 0.06, 0.87),
        (cx + 0.035, 0.93),
        (cx - 0.035, 0.93),
    ]);
    p.sticker(
        &[&tip],
        &solid(shade(base, 0.7)),
        Some((ink(base), EDGE * 0.8)),
        None,
    );
    p.shadow(0.4);
}

pub fn bolt(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    let pts: Vec<P> = [
        (0.58, 0.1),
        (0.24, 0.54),
        (0.46, 0.54),
        (0.37, 0.9),
        (0.78, 0.42),
        (0.55, 0.42),
        (0.67, 0.1),
    ]
    .iter()
    .map(|(a, b)| (x + a, *b))
    .collect();
    let body = poly(&pts);
    p.stroke(&body, rim, 2.0 * (RIM + EDGE));
    p.stroke(&body, ink(main), 2.0 * EDGE);
    p.paint(
        &body,
        &linear(
            (x + 0.3, 0.07),
            (x + 0.6, 0.93),
            &[
                (0.0, tint(main, 0.55)),
                (0.5, main),
                (1.0, shade(main, 0.8)),
            ],
        ),
    );
    p.fill(
        &poly(&[
            (x + 0.58, 0.12),
            (x + 0.3, 0.5),
            (x + 0.4, 0.5),
            (x + 0.62, 0.15),
        ]),
        fade(WHITE, 0.45),
    );
    p.shadow(0.4);
}

pub fn music_note(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, rim) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    let heads = [(x + 0.3, 0.76), (x + 0.7, 0.68)];
    let tilt = |(cx, cy): P| rot(&ellipse(cx, cy, 0.125, 0.092), -22.0, (cx, cy));
    let stems = [
        seg(
            (heads[0].0 + 0.1, heads[0].1 - 0.02),
            (heads[0].0 + 0.1, 0.22),
        ),
        seg(
            (heads[1].0 + 0.1, heads[1].1 - 0.02),
            (heads[1].0 + 0.1, 0.14),
        ),
    ];
    let beam = poly(&[
        (heads[0].0 + 0.1 - 0.035, 0.18),
        (heads[1].0 + 0.1 + 0.035, 0.1),
        (heads[1].0 + 0.1 + 0.035, 0.24),
        (heads[0].0 + 0.1 - 0.035, 0.32),
    ]);
    let hs = [tilt(heads[0]), tilt(heads[1])];
    let body = gloss(main, 0.1, 0.9);
    for s in &stems {
        p.stroke(s, rim, 0.07 + 2.0 * (RIM + EDGE));
    }
    p.stroke(&hs[0], rim, 2.0 * (RIM + EDGE));
    p.stroke(&hs[1], rim, 2.0 * (RIM + EDGE));
    p.stroke(&beam, rim, 2.0 * (RIM + EDGE));
    for s in &stems {
        p.stroke(s, ink(main), 0.07 + 2.0 * EDGE);
    }
    p.stroke(&hs[0], ink(main), 2.0 * EDGE);
    p.stroke(&hs[1], ink(main), 2.0 * EDGE);
    p.stroke(&beam, ink(main), 2.0 * EDGE);
    for s in &stems {
        p.stroke_with(s, &body, &butt(0.07));
    }
    p.paint(&hs[0], &body);
    p.paint(&hs[1], &body);
    p.paint(&beam, &body);
    for (cx, cy) in heads {
        p.fill(
            &rot(
                &ellipse(cx - 0.04, cy - 0.03, 0.04, 0.02),
                -22.0,
                (cx - 0.04, cy - 0.03),
            ),
            fade(WHITE, 0.55),
        );
    }
    p.shadow(0.4);
}

pub fn crown(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, gem) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    let body = smooth(
        &[
            (x + 0.1, 0.78),
            (x + 0.06, 0.28),
            (x + 0.3, 0.5),
            (x + 0.5, 0.18),
            (x + 0.7, 0.5),
            (x + 0.94, 0.28),
            (x + 0.9, 0.78),
        ],
        true,
        &[0, 1, 3, 5, 6],
    );
    p.sticker(
        &[&body],
        &linear(
            (x, 0.2),
            (x, 0.8),
            &[
                (0.0, tint(main, 0.5)),
                (0.55, main),
                (1.0, shade(main, 0.75)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    // The band at the bottom.
    let band = rrect(x + 0.1, 0.66, 0.8, 0.14, 0.04);
    p.sticker(
        &[&band],
        &gloss(shade(main, 0.9), 0.66, 0.8),
        Some((ink(main), EDGE * 0.8)),
        None,
    );
    // Jewels on the points and on the band.
    for (gx, gy, r) in [(0.06, 0.28, 0.045), (0.5, 0.18, 0.055), (0.94, 0.28, 0.045)] {
        p.sticker(
            &[&circle(x + gx, gy, r)],
            &radial(
                (x + gx - 0.01, gy - 0.01),
                r * 1.4,
                &[(0.0, WHITE), (1.0, tint(gem, 0.1))],
            ),
            Some((ink(gem), EDGE * 0.7)),
            None,
        );
    }
    for gx in [0.28, 0.5, 0.72] {
        let d = poly(&[
            (x + gx, 0.675),
            (x + gx + 0.045, 0.73),
            (x + gx, 0.785),
            (x + gx - 0.045, 0.73),
        ]);
        p.sticker(
            &[&d],
            &gloss(gem, 0.675, 0.785),
            Some((ink(gem), EDGE * 0.6)),
            None,
        );
    }
    p.fill(
        &rot(
            &ellipse(x + 0.34, 0.45, 0.025, 0.1),
            -35.0,
            (x + 0.34, 0.45),
        ),
        fade(WHITE, 0.5),
    );
    p.shadow(0.4);
}

pub fn trophy(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, base) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    // Handles.
    for s in [-1.0f32, 1.0] {
        let h = line(&arc_pts(
            x + 0.5 + s * 0.3,
            0.3,
            0.15,
            0.12,
            if s < 0.0 { 90.0 } else { 270.0 },
            if s < 0.0 { 270.0 } else { 450.0 },
            14,
        ));
        p.sticker_line(
            &h,
            0.05,
            &solid(shade(main, 0.9)),
            Some((ink(main), EDGE * 0.9)),
            None,
        );
    }
    // Cup.
    let cup = smooth(
        &[
            (x + 0.22, 0.12),
            (x + 0.78, 0.12),
            (x + 0.76, 0.34),
            (x + 0.66, 0.5),
            (x + 0.5, 0.57),
            (x + 0.34, 0.5),
            (x + 0.24, 0.34),
        ],
        true,
        &[0, 1],
    );
    p.sticker(
        &[&cup],
        &linear(
            (x + 0.2, 0.12),
            (x + 0.8, 0.5),
            &[
                (0.0, tint(main, 0.5)),
                (0.5, main),
                (1.0, shade(main, 0.75)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.fill(
        &rot(&ellipse(x + 0.34, 0.27, 0.03, 0.12), 10.0, (x + 0.34, 0.27)),
        fade(WHITE, 0.55),
    );
    p.paint(
        &star(x + 0.5, 0.31, 0.075, 0.035, 5, 0.0),
        &solid(fade(WHITE, 0.8)),
    );
    // Stem and base.
    let stem = poly(&[
        (x + 0.45, 0.56),
        (x + 0.55, 0.56),
        (x + 0.57, 0.7),
        (x + 0.43, 0.7),
    ]);
    p.sticker(
        &[&stem],
        &gloss(main, 0.56, 0.7),
        Some((ink(main), EDGE)),
        None,
    );
    let b1 = rrect(x + 0.3, 0.69, 0.4, 0.07, 0.025);
    p.sticker(
        &[&b1],
        &gloss(main, 0.69, 0.76),
        Some((ink(main), EDGE)),
        None,
    );
    let b2 = rrect(x + 0.24, 0.76, 0.52, 0.13, 0.03);
    p.sticker(
        &[&b2],
        &gloss(base, 0.76, 0.89),
        Some((ink(base), EDGE)),
        None,
    );
    p.fill(&rrect(x + 0.4, 0.8, 0.2, 0.04, 0.02), fade(WHITE, 0.6));
    p.shadow(0.4);
}

pub fn gift(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, ribbon) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    // Bow loops behind the lid.
    for s in [-1.0f32, 1.0] {
        let cx = x + 0.5 + s * 0.15;
        let loop_ = rot(
            &ellipse(cx, 0.21, 0.15, 0.09),
            s * 28.0,
            (x + 0.5 + s * 0.07, 0.27),
        );
        p.sticker(
            &[&loop_],
            &gloss(ribbon, 0.12, 0.3),
            Some((ink(ribbon), EDGE * 0.9)),
            None,
        );
    }
    let boxp = rrect(x + 0.14, 0.43, 0.72, 0.46, 0.04);
    p.sticker(
        &[&boxp],
        &gloss(main, 0.43, 0.89),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let lid = rrect(x + 0.09, 0.3, 0.82, 0.16, 0.035);
    p.sticker(
        &[&lid],
        &gloss(tint(main, 0.1), 0.3, 0.46),
        Some((ink(main), EDGE)),
        None,
    );
    // Ribbon down the middle.
    let strap = rrect(x + 0.42, 0.3, 0.16, 0.59, 0.0);
    p.fill(&strap, ribbon);
    p.stroke(&strap, fade(ink(ribbon), 0.6), 0.01);
    p.fill(&rrect(x + 0.44, 0.3, 0.035, 0.59, 0.0), fade(WHITE, 0.4));
    // Bow knot.
    p.sticker(
        &[&ellipse(x + 0.5, 0.29, 0.065, 0.055)],
        &gloss(shade(ribbon, 0.95), 0.235, 0.345),
        Some((ink(ribbon), EDGE * 0.8)),
        None,
    );
    p.fill(&rrect(x + 0.2, 0.5, 0.05, 0.28, 0.02), fade(WHITE, 0.25));
    p.shadow(0.4);
}

pub fn rocket(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, trim) = (p.c1, p.c2);
    let t = p.t;
    let (cx, cy) = (p.w * 0.5, 0.5);
    let turn = 40.0f32;
    let rt = |path: &Path| rot(path, turn, (cx, cy));
    // Flame: flickers with time.
    let flick = 0.82 + 0.18 * (t * TAU * 3.1).sin() * (t * TAU * 1.7).cos();
    let outer = drop(cx, 0.87 + 0.04 * flick, 0.1, 180.0);
    p.paint(&rt(&outer), &solid(fade(trim, 0.95)));
    p.paint(
        &rt(&drop(cx, 0.86 + 0.02 * flick, 0.055, 180.0)),
        &solid(tint(trim, 0.8)),
    );
    // Fins.
    for s in [-1.0f32, 1.0] {
        let fin = poly(&[
            (cx + s * 0.13, 0.52),
            (cx + s * 0.3, 0.72),
            (cx + s * 0.3, 0.84),
            (cx + s * 0.12, 0.76),
        ]);
        p.sticker(
            &[&rt(&fin)],
            &gloss(trim, 0.52, 0.84),
            Some((ink(trim), EDGE)),
            None,
        );
    }
    // Body.
    let body = smooth(
        &[
            (cx, 0.04),
            (cx + 0.1, 0.17),
            (cx + 0.16, 0.38),
            (cx + 0.15, 0.78),
            (cx - 0.15, 0.78),
            (cx - 0.16, 0.38),
            (cx - 0.1, 0.17),
        ],
        true,
        &[3, 4],
    );
    p.sticker(
        &[&rt(&body)],
        &linear(
            (cx - 0.16, 0.4),
            (cx + 0.16, 0.4),
            &[
                (0.0, shade(main, 0.9)),
                (0.35, tint(main, 0.6)),
                (1.0, shade(main, 0.8)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    // Nose and band, then the window.
    let nose = smooth(
        &[
            (cx, 0.04),
            (cx + 0.1, 0.17),
            (cx + 0.12, 0.26),
            (cx - 0.12, 0.26),
            (cx - 0.1, 0.17),
        ],
        true,
        &[2, 3],
    );
    p.paint(&rt(&nose), &gloss(trim, 0.04, 0.26));
    p.stroke(
        &rt(&seg((cx - 0.15, 0.7), (cx + 0.15, 0.7))),
        fade(trim, 0.95),
        0.05,
    );
    let win = circle(cx, 0.45, 0.075);
    p.sticker(
        &[&rt(&win)],
        &radial(
            (cx - 0.02, 0.43),
            0.1,
            &[
                (0.0, tint(rgb(0.4, 0.8, 1.0), 0.6)),
                (1.0, rgb(0.15, 0.45, 0.85)),
            ],
        ),
        Some((ink(trim), EDGE * 1.1)),
        Some((tint(trim, 0.3), 0.012)),
    );
    p.fill(&rt(&circle(cx - 0.03, 0.42, 0.018)), fade(WHITE, 0.8));
    p.halo(0.35);
}

pub fn pin(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, dot) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    p.fill(
        &ellipse(cx, 0.9, 0.17, 0.045),
        fade(rgb(0.0, 0.0, 0.0), 0.25),
    );
    let body = drop(cx, 0.4, 0.28, 180.0);
    p.sticker(
        &[&body],
        &radial(
            (cx - 0.1, 0.25),
            0.7,
            &[
                (0.0, tint(main, 0.5)),
                (0.5, main),
                (1.0, shade(main, 0.72)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.sticker(
        &[&circle(cx, 0.4, 0.11)],
        &gloss(dot, 0.29, 0.51),
        Some((ink(main), EDGE * 0.8)),
        None,
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.12, 0.28, 0.035, 0.09),
            35.0,
            (cx - 0.12, 0.28),
        ),
        fade(WHITE, 0.5),
    );
    p.shadow(0.35);
}

pub fn play(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, fg) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    p.sticker(
        &[&circle(cx, 0.5, 0.4)],
        &radial(
            (cx - 0.1, 0.3),
            0.8,
            &[
                (0.0, tint(main, 0.45)),
                (0.5, main),
                (1.0, shade(main, 0.72)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let tri = smooth(
        &[(cx - 0.1, 0.32), (cx + 0.2, 0.5), (cx - 0.1, 0.68)],
        true,
        &[],
    );
    p.fill(&moved(&tri, 0.0, 0.015), fade(ink(main), 0.5));
    p.fill(&tri, fg);
    p.fill(
        &rot(
            &ellipse(cx - 0.18, 0.25, 0.1, 0.05),
            -35.0,
            (cx - 0.18, 0.25),
        ),
        fade(WHITE, 0.4),
    );
    p.shadow(0.4);
}

pub fn camera(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, trim) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    let bump = rrect(x + 0.3, 0.2, 0.28, 0.14, 0.04);
    p.sticker(
        &[&bump],
        &gloss(shade(main, 0.9), 0.2, 0.34),
        Some((ink(main), EDGE)),
        None,
    );
    let body = rrect(x + 0.06, 0.3, 0.88, 0.52, 0.08);
    p.sticker(
        &[&body],
        &gloss(main, 0.3, 0.82),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    // A strip of leather-look trim along the top and the flash.
    p.fill(&rrect(x + 0.1, 0.35, 0.8, 0.07, 0.03), fade(trim, 0.35));
    p.sticker(
        &[&rrect(x + 0.74, 0.22, 0.12, 0.08, 0.02)],
        &gloss(trim, 0.22, 0.3),
        Some((ink(trim), EDGE * 0.8)),
        None,
    );
    // Lens.
    let (lx, ly) = (x + 0.5, 0.57);
    p.sticker(
        &[&circle(lx, ly, 0.2)],
        &linear(
            (lx, ly - 0.2),
            (lx, ly + 0.2),
            &[(0.0, tint(trim, 0.5)), (1.0, shade(trim, 0.7))],
        ),
        Some((ink(trim), EDGE)),
        None,
    );
    p.sticker(
        &[&circle(lx, ly, 0.145)],
        &solid(rgb(0.06, 0.07, 0.12)),
        Some((fade(WHITE, 0.4), 0.008)),
        None,
    );
    p.paint(
        &circle(lx, ly, 0.11),
        &radial(
            (lx - 0.03, ly - 0.03),
            0.14,
            &[
                (0.0, rgb(0.45, 0.65, 0.95)),
                (0.6, rgb(0.14, 0.24, 0.55)),
                (1.0, rgb(0.05, 0.07, 0.2)),
            ],
        ),
    );
    p.fill(&circle(lx - 0.045, ly - 0.05, 0.026), fade(WHITE, 0.85));
    p.fill(&circle(lx + 0.04, ly + 0.045, 0.014), fade(WHITE, 0.5));
    p.fill(&circle(x + 0.17, 0.4, 0.02), fade(tint(trim, 0.2), 0.9));
    p.shadow(0.4);
}

pub fn microphone(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, trim) = (p.c1, p.c2);
    let cx = p.w * 0.5;
    // Cradle.
    let cradle = line(&arc_pts(cx, 0.44, 0.23, 0.26, 10.0, 170.0, 18));
    p.sticker_line(
        &cradle,
        0.045,
        &solid(trim),
        Some((ink(trim), EDGE * 0.9)),
        None,
    );
    let stand = seg((cx, 0.7), (cx, 0.84));
    p.sticker_line(
        &stand,
        0.045,
        &solid(trim),
        Some((ink(trim), EDGE * 0.9)),
        None,
    );
    let foot = rrect(cx - 0.17, 0.83, 0.34, 0.07, 0.035);
    p.sticker(
        &[&foot],
        &gloss(trim, 0.83, 0.9),
        Some((ink(trim), EDGE * 0.9)),
        None,
    );
    // Capsule with a grille.
    let cap = rrect(cx - 0.15, 0.08, 0.3, 0.55, 0.15);
    p.sticker(
        &[&cap],
        &linear(
            (cx - 0.15, 0.3),
            (cx + 0.15, 0.3),
            &[
                (0.0, shade(main, 0.85)),
                (0.4, tint(main, 0.35)),
                (1.0, shade(main, 0.75)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let m = p.clip(&cap);
    for k in 0..6 {
        let y = 0.17 + 0.075 * k as f32;
        p.fill_in(
            &rrect(cx - 0.16, y, 0.32, 0.016, 0.008),
            fade(ink(main), 0.45),
            &m,
        );
    }
    for k in 0..3 {
        let x = cx - 0.08 + 0.08 * k as f32;
        p.fill_in(
            &rrect(x - 0.008, 0.08, 0.016, 0.55, 0.008),
            fade(ink(main), 0.35),
            &m,
        );
    }
    p.fill_in(
        &rrect(cx - 0.12, 0.12, 0.035, 0.4, 0.015),
        fade(WHITE, 0.45),
        &m,
    );
    p.shadow(0.4);
}

pub fn clapper(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(INSET);
    let (main, stripe) = (p.c1, p.c2);
    let x = p.w * 0.5 - 0.5;
    // The board.
    let board = rrect(x + 0.1, 0.45, 0.8, 0.44, 0.04);
    p.sticker(
        &[&board],
        &gloss(main, 0.4, 0.86),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    for k in 0..3 {
        let y = 0.6 + 0.09 * k as f32;
        p.stroke(
            &seg((x + 0.18, y), (x + 0.82, y)),
            fade(stripe, 0.55),
            0.012,
        );
    }
    p.fill(&rrect(x + 0.18, 0.67, 0.18, 0.04, 0.02), fade(stripe, 0.8));
    // The arm: striped, tilted open.
    let arm = rrect(x + 0.1, 0.3, 0.8, 0.14, 0.03);
    let pivot = (x + 0.1, 0.44);
    let open = |path: &Path| rot(path, -14.0, pivot);
    let arm_o = open(&arm);
    p.sticker(
        &[&arm_o],
        &solid(main),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    if let Some(m) = p.clip(&arm_o) {
        for k in 0..5 {
            let sx = x + 0.12 + 0.17 * k as f32;
            let s = poly(&[
                (sx, 0.2),
                (sx + 0.085, 0.2),
                (sx + 0.045, 0.4),
                (sx - 0.04, 0.4),
            ]);
            p.paint_masked(&open(&s), &solid(stripe), &m);
        }
    }
    p.stroke(&arm_o, fade(ink(main), 0.8), 0.014);
    p.fill(
        &circle(pivot.0 + 0.02, pivot.1 - 0.02, 0.022),
        fade(tint(main, 0.6), 0.9),
    );
    p.shadow(0.4);
}
