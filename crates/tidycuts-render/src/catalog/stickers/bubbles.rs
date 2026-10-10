//! Speech bubbles: speech, thought, shout, rounded chat and typing.
//! The main color fills the bubble, the second color outlines it.

use super::kit::*;
use super::DrawCtx;
use tiny_skia::{Path, PathBuilder, Pixmap};

const LINE: f32 = 0.034;

fn bubble_fill(c: C, y0: f32, y1: f32) -> tiny_skia::Paint<'static> {
    linear(
        (0.0, y0),
        (0.0, y1),
        &[(0.0, tint(c, 0.3)), (1.0, shade(c, 0.9))],
    )
}

pub fn speech(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (fill, line_c) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.52, 0.42);
    let body = ellipse(cx, cy, 0.55, 0.32);
    let mut pb = PathBuilder::new();
    pb.move_to(cx - 0.26, cy + 0.22);
    pb.quad_to(cx - 0.34, cy + 0.38, cx - 0.47, cy + 0.46);
    pb.quad_to(cx - 0.2, cy + 0.42, cx - 0.04, cy + 0.27);
    pb.close();
    let tail = pb.finish().unwrap_or_else(|| circle(cx, cy, 0.01));
    p.sticker(
        &[&body, &tail],
        &bubble_fill(fill, 0.1, 0.88),
        Some((line_c, LINE)),
        None,
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.3, cy - 0.17, 0.09, 0.035),
            -30.0,
            (cx - 0.3, cy - 0.17),
        ),
        fade(WHITE, 0.7),
    );
    p.shadow(0.35);
}

pub fn thought(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (fill, line_c) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.53, 0.42);
    let mut parts: Vec<Path> = vec![ellipse(cx, cy, 0.42, 0.2)];
    let n = 10;
    for i in 0..n {
        let a = (i as f32 / n as f32 * 360.0 + 8.0).to_radians();
        let r = 0.125 + 0.03 * hash(i as u32 + 3);
        parts.push(circle(cx + 0.42 * a.cos(), cy + 0.2 * a.sin(), r));
    }
    parts.push(circle(cx - 0.3, 0.76, 0.055));
    parts.push(circle(cx - 0.43, 0.845, 0.034));
    let refs: Vec<&Path> = parts.iter().collect();
    p.sticker(
        &refs,
        &bubble_fill(fill, 0.1, 0.85),
        Some((line_c, LINE)),
        None,
    );
    p.fill(
        &rot(
            &ellipse(cx - 0.26, cy - 0.17, 0.08, 0.03),
            -25.0,
            (cx - 0.26, cy - 0.17),
        ),
        fade(WHITE, 0.7),
    );
    p.shadow(0.35);
}

pub fn shout(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (fill, line_c) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.53, 0.44);
    let n = 26;
    let mut pts = Vec::new();
    for i in 0..n {
        let a = (i as f32 / n as f32 * 360.0 - 90.0).to_radians();
        let k = if i % 2 == 0 {
            1.0 + 0.1 * (hash(i as u32) - 0.5)
        } else {
            0.76 + 0.05 * hash(i as u32 + 40)
        };
        let mut q = (cx + 0.53 * k * a.cos(), cy + 0.33 * k * a.sin());
        // One long spike towards the speaker.
        if i == 16 {
            q = (cx - 0.44, 0.83);
        }
        pts.push(q);
    }
    let body = poly(&pts);
    p.stroke_with(&body, &solid(line_c), &miter(2.0 * LINE));
    p.paint(&body, &bubble_fill(fill, 0.1, 0.85));
    p.fill(
        &rot(
            &ellipse(cx - 0.24, cy - 0.15, 0.1, 0.035),
            -25.0,
            (cx - 0.24, cy - 0.15),
        ),
        fade(WHITE, 0.55),
    );
    p.shadow(0.35);
}

pub fn rounded(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (fill, line_c) = (p.c1, p.c2);
    let w = p.w;
    let body = rrect(0.11, 0.1, w - 0.22, 0.62, 0.2);
    let tail = poly(&[(0.3, 0.66), (0.52, 0.66), (0.26, 0.85)]);
    p.sticker(
        &[&body, &tail],
        &gloss(fill, 0.1, 0.85),
        Some((ink(fill), EDGE * 0.8)),
        Some((line_c, LINE)),
    );
    p.fill(&rrect(0.22, 0.15, w - 0.44, 0.1, 0.05), fade(WHITE, 0.22));
    p.shadow(0.35);
}

pub fn typing(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (fill, dot) = (p.c1, p.c2);
    let w = p.w;
    let edge = mix(fill, dot, 0.45);
    let body = rrect(0.14, 0.16, w - 0.28, 0.56, 0.28);
    let t1 = circle(0.22, 0.72, 0.075);
    let t2 = circle(0.12, 0.83, 0.04);
    p.sticker(
        &[&body, &t1, &t2],
        &bubble_fill(fill, 0.16, 0.85),
        Some((edge, LINE * 0.8)),
        None,
    );
    for i in 0..3 {
        let x = w * 0.5 + (i as f32 - 1.0) * 0.3;
        let u = (p.t * 1.3 - i as f32 * 0.18).rem_euclid(1.0);
        let b = if u < 0.4 {
            (u / 0.4 * std::f32::consts::PI).sin()
        } else {
            0.0
        };
        let c = mix(tint(dot, 0.45), dot, b);
        p.fill(&circle(x, 0.44 - 0.075 * b, 0.085), c);
    }
    p.shadow(0.35);
}
