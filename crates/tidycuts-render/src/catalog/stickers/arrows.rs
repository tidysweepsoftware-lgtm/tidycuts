//! Arrows: straight, curved, hand-drawn, circular, pointing hand,
//! chevrons and a doodled loop.

use super::kit::*;
use super::DrawCtx;
use tiny_skia::Pixmap;

/// A solid arrow body with the usual edge and rim.
fn solid_arrow(p: &mut Pen, body: &tiny_skia::Path) {
    let main = p.c1;
    let rim = p.c2;
    p.sticker(
        &[body],
        &gloss(main, 0.15, 0.85),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
}

pub fn straight(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let w = p.w;
    let body = arrow(&[(0.16, 0.5), (w - 0.14, 0.5)], 0.24, 0.3, 0.6, 0.62);
    solid_arrow(&mut p, &body);
    p.stroke(
        &seg((0.3, 0.435), (w - 0.86, 0.425)),
        fade(WHITE, 0.4),
        0.04,
    );
    p.shadow(0.4);
}

pub fn curved(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let c = arc_pts(0.64, 0.8, 0.4, 0.5, 180.0, 358.0, 64);
    let body = arrow(&c, 0.07, 0.2, 0.27, 0.44);
    solid_arrow(&mut p, &body);
    let hl = arc_pts(0.64, 0.8, 0.4, 0.5, 215.0, 275.0, 16);
    p.stroke(&line(&hl), fade(WHITE, 0.45), 0.035);
    p.shadow(0.4);
}

pub fn hand_drawn(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, outline) = (p.c1, p.c2);
    let c = sample(
        &[
            (0.14, 0.68),
            (0.5, 0.57),
            (0.95, 0.51),
            (1.35, 0.45),
            (1.62, 0.37),
        ],
        false,
        20,
    );
    let shaft = ribbon(&c, |s| {
        0.08 * (0.82 + 0.18 * (s * 11.0).sin())
            * (s * 12.0 + 0.15).min(1.0)
            * ((1.0 - s) * 25.0 + 0.7).min(1.0)
    });
    let tip = c[c.len() - 1];
    let d = norm((tip.0 - c[c.len() - 6].0, tip.1 - c[c.len() - 6].1));
    let wing = |deg: f32, len: f32| {
        let (s, co) = deg.to_radians().sin_cos();
        let v = (d.0 * co - d.1 * s, d.0 * s + d.1 * co);
        let start = (tip.0 - v.0 * len, tip.1 - v.1 * len);
        let mid = lerp(start, tip, 0.5);
        let bow = (
            mid.0 - v.1 * 0.012 * deg.signum(),
            mid.1 + v.0 * 0.012 * deg.signum(),
        );
        ribbon(&sample(&[start, bow, tip], false, 8), |s| 0.035 + 0.045 * s)
    };
    let (w1, w2) = (wing(34.0, 0.3), wing(-30.0, 0.34));
    let parts = [&shaft, &w1, &w2];
    for part in parts {
        p.stroke(part, outline, 0.06);
    }
    for part in parts {
        p.stroke(part, main, 0.012);
        p.fill(part, main);
    }
    p.shadow(0.35);
}

pub fn circular(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let c = arc_pts(0.5, 0.52, 0.27, 0.27, -20.0, 270.0, 80);
    let body = arrow(&c, 0.07, 0.14, 0.2, 0.3);
    solid_arrow(&mut p, &body);
    let hl = arc_pts(0.5, 0.52, 0.27, 0.27, 150.0, 215.0, 16);
    p.stroke(&line(&hl), fade(WHITE, 0.4), 0.03);
    p.halo(0.4);
}

pub fn pointing_hand(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (skin, cuff_c) = (p.c1, p.c2);
    let edge = ink(skin);
    let cuff = rrect(0.07, 0.29, 0.2, 0.54, 0.06);
    let palm = rrect(0.22, 0.32, 0.56, 0.5, 0.16);
    let index = rrect(0.48, 0.32, 0.72, 0.165, 0.0825);
    let fingers = [
        rrect(0.56, 0.475, 0.31, 0.125, 0.0625),
        rrect(0.56, 0.585, 0.29, 0.12, 0.06),
        rrect(0.54, 0.69, 0.26, 0.12, 0.06),
    ];
    let body = gloss(skin, 0.3, 0.85);
    for part in [&cuff, &palm, &index] {
        p.stroke(part, edge, 2.0 * EDGE);
    }
    for f in &fingers {
        p.stroke(f, edge, 2.0 * EDGE);
    }
    p.paint(&cuff, &gloss(cuff_c, 0.29, 0.83));
    p.stroke(
        &seg((0.12, 0.36), (0.12, 0.76)),
        fade(ink(cuff_c), 0.35),
        0.018,
    );
    p.paint(&palm, &body);
    p.paint(&index, &body);
    for f in &fingers {
        p.paint(f, &body);
        p.stroke(f, edge, 0.016);
    }
    // Thumb resting on the curled fingers.
    let thumb = seg((0.3, 0.515), (0.62, 0.545));
    p.stroke(&thumb, edge, 0.13 + 2.0 * EDGE * 0.8);
    p.stroke_with(&thumb, &body, &round(0.13));
    // The thumb grows out of the palm: cover its base.
    p.paint(&rrect(0.23, 0.36, 0.15, 0.44, 0.1), &body);
    // Nail and knuckle shine.
    p.fill(
        &rrect(1.05, 0.348, 0.1, 0.06, 0.03),
        fade(tint(skin, 0.7), 0.8),
    );
    p.stroke(&seg((0.66, 0.36), (0.96, 0.36)), fade(WHITE, 0.35), 0.022);
    p.stroke(&seg((0.28, 0.4), (0.28, 0.7)), fade(WHITE, 0.25), 0.03);
    p.shadow(0.4);
}

pub fn chevrons(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    for i in 0..3 {
        let x = 0.36 + i as f32 * 0.44;
        let ph = (p.t * 1.4 - i as f32 / 3.0).rem_euclid(1.0);
        let k = 0.5 + 0.5 * (ph * std::f32::consts::TAU).cos();
        let s = 0.9 + 0.12 * k;
        let pts = [
            (x - 0.11 * s, 0.5 - 0.24 * s),
            (x + 0.11 * s, 0.5),
            (x - 0.11 * s, 0.5 + 0.24 * s),
        ];
        let c = mix(shade(main, 0.62), tint(main, 0.3), k);
        let path = line(&pts);
        p.sticker_line(
            &path,
            0.15,
            &gloss(c, 0.25, 0.75),
            Some((ink(main), EDGE)),
            Some((rim, RIM)),
        );
    }
    p.shadow(0.35);
}

pub fn doodle_loop(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let ctrl = [
        (0.12, 0.74),
        (0.38, 0.68),
        (0.62, 0.54),
        (0.75, 0.34),
        (0.66, 0.17),
        (0.49, 0.22),
        (0.5, 0.43),
        (0.68, 0.6),
        (0.96, 0.66),
        (1.26, 0.6),
        (1.5, 0.47),
    ];
    let c = sample(&ctrl, false, 16);
    let tip = c[c.len() - 1];
    let d = norm((tip.0 - c[c.len() - 4].0, tip.1 - c[c.len() - 4].1));
    let wing = |deg: f32| {
        let (s, co) = deg.to_radians().sin_cos();
        let v = (d.0 * co - d.1 * s, d.0 * s + d.1 * co);
        line(&[(tip.0 - v.0 * 0.22, tip.1 - v.1 * 0.22), tip])
    };
    let parts = [line(&c), wing(35.0), wing(-35.0)];
    let width = 0.066;
    for part in &parts {
        p.stroke(part, rim, width + 0.07);
    }
    for part in &parts {
        p.stroke(part, main, width);
    }
    // Where the line crosses itself, redraw the later pass on top with its
    // rim so it reads as going over.
    let mut best = (f32::MAX, 0usize);
    for j in 0..c.len() {
        for i in 0..j.saturating_sub(24) {
            let dd = dist(c[i], c[j]);
            if dd < best.0 {
                best = (dd, j);
            }
        }
    }
    let j = best.1;
    if j > 6 && j + 6 < c.len() {
        let piece = line(&c[j - 5..=j + 5]);
        p.stroke_with(&piece, &solid(rim), &butt(width + 0.07));
        p.stroke_with(&piece, &solid(main), &butt(width));
    }
    p.shadow(0.35);
}
