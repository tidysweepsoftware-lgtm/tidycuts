//! Generic social-media call-outs: like, subscribe, bell, share, comment,
//! follow, live and a clicking cursor. No brand marks.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use std::f32::consts::TAU;
use tiny_skia::{Path, PathBuilder, Pixmap};

/// A pill button with a gloss band; returns nothing, draws the body.
pub(super) fn pill(p: &mut Pen, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let main = p.c1;
    let body = rrect(x, y, w, h, r);
    p.sticker(
        &[&body],
        &gloss(main, y, y + h),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.fill(
        &rrect(x + r * 0.6, y + h * 0.09, w - r * 1.2, h * 0.2, h * 0.1),
        fade(WHITE, 0.22),
    );
}

/// Lays out an icon `icon_w` wide and the label `s` as one group centred
/// between `x0` and `x1`; returns the icon centre x and the label path.
pub(super) fn icon_label(
    x0: f32,
    x1: f32,
    icon_w: f32,
    gap: f32,
    s: &str,
    max_h: f32,
) -> (f32, Option<Path>) {
    let room = (x1 - x0 - icon_w - gap).max(0.1);
    let Some(t) = text("Poppins Black", s, (0.0, 0.5), room, max_h, 0.0, 0.0) else {
        return ((x0 + x1) * 0.5, None);
    };
    let b = t.compute_tight_bounds().unwrap_or(t.bounds());
    let start = (x0 + x1) * 0.5 - (icon_w + gap + b.width()) * 0.5;
    (
        start + icon_w * 0.5,
        Some(moved(&t, start + icon_w + gap - b.left(), 0.0)),
    )
}

pub(super) fn draw_label(p: &mut Pen, t: &Option<Path>, color: C) {
    if let Some(t) = t {
        p.fill(&moved(t, 0.0, 0.012), fade(ink(p.c1), 0.45));
        p.fill(t, color);
    }
}

/// Bell body, knob and clapper, `s` = height scale.
pub(super) fn bell(cx: f32, cy: f32, s: f32) -> (Path, Path, Path) {
    let p = |x: f32, y: f32| (cx + x * s, cy + y * s);
    let mut pb = PathBuilder::new();
    let m = p(0.0, -0.42);
    pb.move_to(m.0, m.1);
    {
        let mut c = |a: P, b: P, e: P| pb.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
        c(p(0.24, -0.42), p(0.31, -0.25), p(0.31, -0.05));
        c(p(0.31, 0.1), p(0.36, 0.19), p(0.47, 0.25));
    }
    let a = p(0.47, 0.32);
    pb.line_to(a.0, a.1);
    let b = p(-0.47, 0.32);
    pb.line_to(b.0, b.1);
    let c0 = p(-0.47, 0.25);
    pb.line_to(c0.0, c0.1);
    {
        let mut c = |a: P, b: P, e: P| pb.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
        c(p(-0.36, 0.19), p(-0.31, 0.1), p(-0.31, -0.05));
        c(p(-0.31, -0.25), p(-0.24, -0.42), p(0.0, -0.42));
    }
    pb.close();
    let body = pb.finish().unwrap_or_else(|| circle(cx, cy, 0.3 * s));
    let k = p(0.0, -0.46);
    let cl = p(0.0, 0.38);
    (
        body,
        circle(k.0, k.1, 0.065 * s),
        circle(cl.0, cl.1, 0.1 * s),
    )
}

/// A small flat bell glyph for buttons.
pub(super) fn bell_icon(p: &mut Pen, cx: f32, cy: f32, s: f32, c: C) {
    let (body, knob, clapper) = bell(cx, cy, s);
    for part in [&clapper, &knob, &body] {
        p.fill(part, c);
    }
}

pub fn like(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let w = p.w;
    let fg = p.c2;
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, t) = icon_label(0.3, w - 0.3, 0.34, 0.14, "LIKE", 0.27);
    let h = heart(ix, 0.51, 0.34, 0.31);
    p.fill(&moved(&h, 0.0, 0.012), fade(ink(p.c1), 0.45));
    p.fill(&h, fg);
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}

pub fn subscribe(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let w = p.w;
    let fg = p.c2;
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, t) = icon_label(0.3, w - 0.3, 0.3, 0.12, "SUBSCRIBE", 0.25);
    let sh = fade(ink(p.c1), 0.45);
    bell_icon(&mut p, ix, 0.502, 0.36, sh);
    bell_icon(&mut p, ix, 0.49, 0.36, fg);
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}

pub fn bell_alert(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, dot) = (p.c1, p.c2);
    let (cx, cy, s) = (0.5, 0.48, 0.74);
    let (body, knob, clapper) = bell(cx, cy, s);
    p.sticker(
        &[&clapper, &knob, &body],
        &gloss(main, 0.1, 0.8),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.paint(&clapper, &gloss(shade(main, 0.8), 0.7, 0.8));
    p.paint(&body, &gloss(main, 0.1, 0.8));
    p.stroke(
        &seg((cx - 0.13, cy - 0.16), (cx - 0.15, cy + 0.06)),
        fade(WHITE, 0.45),
        0.04,
    );
    p.stroke(
        &seg(
            (cx - 0.31 * s, cy + 0.24 * s),
            (cx + 0.31 * s, cy + 0.24 * s),
        ),
        fade(ink(main), 0.35),
        0.014,
    );
    // Ringing marks.
    for s in [-1.0f32, 1.0] {
        let (a0, a1) = if s < 0.0 {
            (155.0, 195.0)
        } else {
            (-15.0, 25.0)
        };
        p.stroke(
            &line(&arc_pts(cx, cy + 0.04, 0.35, 0.35, a0, a1, 8)),
            ink(main),
            0.03,
        );
        p.stroke(
            &line(&arc_pts(cx, cy + 0.04, 0.41, 0.41, a0 + 6.0, a1 - 6.0, 8)),
            ink(main),
            0.03,
        );
    }
    let d = circle(0.73, 0.24, 0.1);
    p.sticker(
        &[&d],
        &gloss(dot, 0.14, 0.34),
        Some((ink(dot), EDGE * 0.8)),
        Some((WHITE, RIM * 0.8)),
    );
    p.fill(&ellipse(0.7, 0.2, 0.035, 0.022), fade(WHITE, 0.6));
    p.shadow(0.4);
}

pub fn share(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (0.5, 0.48);
    p.sticker(
        &[&circle(cx, cy, 0.38)],
        &gloss(main, 0.1, 0.86),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.fill(&ellipse(cx, cy - 0.25, 0.22, 0.07), fade(WHITE, 0.2));
    let nodes = [
        (cx + 0.11, cy - 0.15),
        (cx - 0.13, cy + 0.0),
        (cx + 0.11, cy + 0.15),
    ];
    let shadow = fade(ink(main), 0.45);
    for (c, dy) in [(shadow, 0.012), (fg, 0.0)] {
        p.stroke(
            &seg((nodes[0].0, nodes[0].1 + dy), (nodes[1].0, nodes[1].1 + dy)),
            c,
            0.045,
        );
        p.stroke(
            &seg((nodes[2].0, nodes[2].1 + dy), (nodes[1].0, nodes[1].1 + dy)),
            c,
            0.045,
        );
        for n in nodes {
            p.fill(&circle(n.0, n.1 + dy, 0.072), c);
        }
    }
    p.shadow(0.4);
}

pub fn comment(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, badge) = (p.c1, p.c2);
    let body = rrect(0.11, 0.18, 0.87, 0.54, 0.16);
    let tail = poly(&[(0.25, 0.66), (0.47, 0.66), (0.2, 0.84)]);
    p.sticker(
        &[&body, &tail],
        &gloss(main, 0.18, 0.84),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    for (x1, y) in [(0.86, 0.36), (0.66, 0.52)] {
        p.stroke(
            &seg((0.28, y + 0.01), (x1, y + 0.01)),
            fade(ink(main), 0.4),
            0.07,
        );
        p.stroke(&seg((0.28, y), (x1, y)), WHITE, 0.07);
    }
    let d = circle(0.97, 0.22, 0.1);
    p.sticker(
        &[&d],
        &gloss(badge, 0.1, 0.32),
        Some((ink(badge), EDGE * 0.8)),
        Some((WHITE, RIM * 0.8)),
    );
    if let Some(t) = text("Poppins Black", "1", (0.97, 0.22), 0.1, 0.1, 0.0, 0.0) {
        p.fill(&t, WHITE);
    }
    p.shadow(0.4);
}

pub fn follow(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let w = p.w;
    let fg = p.c2;
    let sh = fade(ink(p.c1), 0.45);
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, t) = icon_label(0.3, w - 0.3, 0.46, 0.1, "FOLLOW", 0.26);
    let x = ix - 0.07;
    for (c, dy) in [(sh, 0.012), (fg, 0.0)] {
        p.fill(&circle(x, 0.4 + dy, 0.08), c);
        let mut sh_pts = arc_pts(x, 0.67 + dy, 0.16, 0.14, 180.0, 360.0, 16);
        sh_pts.push((x + 0.16, 0.67 + dy));
        p.fill(&smooth(&sh_pts, true, &[0, 16]), c);
        p.stroke(&seg((x + 0.23, 0.36 + dy), (x + 0.23, 0.5 + dy)), c, 0.045);
        p.stroke(&seg((x + 0.16, 0.43 + dy), (x + 0.3, 0.43 + dy)), c, 0.045);
    }
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}

pub fn live(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let fg = p.c2;
    let w = p.w;
    pill(&mut p, 0.13, 0.24, w - 0.26, 0.52, 0.12);
    let (ix, t) = icon_label(0.3, w - 0.3, 0.17, 0.13, "LIVE", 0.25);
    let u = (p.t * 0.9).rem_euclid(1.0);
    let ring = circle(ix, 0.5, 0.085 + 0.1 * u);
    p.stroke(&ring, fade(fg, 0.85 * (1.0 - u)), 0.024);
    let blink = 0.75 + 0.25 * (p.t * TAU * 0.9).cos();
    p.fill(&circle(ix, 0.5, 0.08), fade(fg, blink));
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}

pub fn cursor_click(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, ray) = (p.c1, p.c2);
    let tip = (0.42, 0.32);
    let u = (p.t * 0.8).rem_euclid(1.0);
    // Ripples and click rays around the tip.
    for k in [0.0f32, 0.5] {
        let v = (u + k).rem_euclid(1.0);
        p.stroke(
            &circle(tip.0, tip.1, 0.04 + 0.22 * v),
            fade(ray, 0.9 * (1.0 - v)),
            0.03 * (1.0 - v * 0.5),
        );
    }
    let pop = 1.0 - smoothstep(0.0, 0.35, u);
    for (i, deg) in [180.0f32, 215.0, 250.0, 285.0].iter().enumerate() {
        let a = deg.to_radians();
        let (r0, r1) = (0.1 + 0.02 * pop, 0.17 + 0.05 * pop + 0.015 * (i % 2) as f32);
        let s = seg(
            (tip.0 + r0 * a.cos(), tip.1 + r0 * a.sin()),
            (tip.0 + r1 * a.cos(), tip.1 + r1 * a.sin()),
        );
        p.stroke(&s, ink(ray), 0.045 + EDGE);
        p.stroke(&s, ray, 0.045);
    }
    let press = 1.0 - 0.07 * pop;
    let shape = [
        (0.0, 0.0),
        (0.0, 0.84),
        (0.2, 0.66),
        (0.34, 0.97),
        (0.47, 0.91),
        (0.33, 0.6),
        (0.6, 0.6),
    ];
    let s = 0.56 * press;
    let pts: Vec<P> = shape
        .iter()
        .map(|&(x, y)| (tip.0 + x * s, tip.1 + y * s))
        .collect();
    let arrow_p = poly(&pts);
    let edge = ink(mix(main, rgb(0.1, 0.1, 0.12), 0.7));
    p.sticker(
        &[&arrow_p],
        &gloss(main, tip.1, tip.1 + 0.5),
        Some((edge, EDGE * 1.4)),
        Some((WHITE, RIM * 0.6)),
    );
    p.shadow(0.45);
}

/// A rounded tile with one big symbol on it (hashtag, mention).
fn symbol_tile(pm: &mut Pixmap, ctx: &DrawCtx, label: &str, turn: f32) {
    let mut p = Pen::new(pm, ctx).inset(0.92);
    let (main, fg) = (p.c1, p.c2);
    let c = (p.w * 0.5, 0.5);
    let tile = rot(&rrect(c.0 - 0.36, c.1 - 0.36, 0.72, 0.72, 0.2), turn, c);
    p.sticker(
        &[&tile],
        &gloss(main, 0.14, 0.86),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    p.fill(
        &rot(&rrect(c.0 - 0.28, c.1 - 0.31, 0.56, 0.12, 0.06), turn, c),
        fade(WHITE, 0.2),
    );
    if let Some(t) = text(
        "Poppins Black",
        label,
        (c.0, c.1 + 0.02),
        0.44,
        0.44,
        0.0,
        turn,
    ) {
        p.fill(&moved(&t, 0.0, 0.018), fade(ink(main), 0.55));
        p.fill(&t, fg);
    }
    p.shadow(0.4);
}

pub fn hashtag(pm: &mut Pixmap, ctx: &DrawCtx) {
    symbol_tile(pm, ctx, "#", -6.0);
}

pub fn mention(pm: &mut Pixmap, ctx: &DrawCtx) {
    symbol_tile(pm, ctx, "@", 5.0);
}

pub fn verified(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.92);
    let (main, fg) = (p.c1, p.c2);
    let (cx, cy) = (p.w * 0.5, 0.5);
    let body = scallop(cx, cy, 0.34, 0.34, 10, 0.09);
    p.sticker(
        &[&body],
        &radial(
            (cx - 0.1, cy - 0.13),
            0.6,
            &[
                (0.0, tint(main, 0.45)),
                (0.55, main),
                (1.0, shade(main, 0.72)),
            ],
        ),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    let tick = line(&[
        (cx - 0.15, cy + 0.01),
        (cx - 0.04, cy + 0.12),
        (cx + 0.17, cy - 0.12),
    ]);
    p.stroke(&moved(&tick, 0.0, 0.014), fade(ink(main), 0.5), 0.085);
    p.stroke(&tick, fg, 0.085);
    p.fill(
        &rot(
            &ellipse(cx - 0.14, cy - 0.2, 0.09, 0.04),
            -38.0,
            (cx - 0.14, cy - 0.2),
        ),
        fade(WHITE, 0.4),
    );
    p.shadow(0.4);
}

pub fn views(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let w = p.w;
    let fg = p.c2;
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, t) = icon_label(0.3, w - 0.3, 0.38, 0.14, "1.2M", 0.27);
    // An eye: an almond outline with a round pupil.
    let (ex, ey) = (ix, 0.5);
    let almond = smooth(
        &[
            (ex - 0.19, ey),
            (ex, ey - 0.13),
            (ex + 0.19, ey),
            (ex, ey + 0.13),
        ],
        true,
        &[0, 2],
    );
    p.fill(&moved(&almond, 0.0, 0.012), fade(ink(p.c1), 0.45));
    p.fill(&almond, fg);
    p.fill(&circle(ex, ey, 0.075), p.c1);
    p.fill(&circle(ex - 0.02, ey - 0.02, 0.022), fade(WHITE, 0.9));
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}
