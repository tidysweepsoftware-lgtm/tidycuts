//! Animated sparkles: twinkle, glitter, confetti, stars, floating hearts
//! and fireworks. All motion is a pure function of `t` and loops.

use super::kit::*;
use super::DrawCtx;
use std::f32::consts::TAU;
use tiny_skia::Pixmap;

/// A glowing four-point sparkle.
fn glint(p: &mut Pen, x: f32, y: f32, r: f32, core: C, glow: C) {
    if r <= 1e-3 {
        return;
    }
    p.paint(
        &circle(x, y, r * 0.9),
        &radial(
            (x, y),
            r * 0.9,
            &[(0.0, fade(glow, 0.55)), (1.0, fade(glow, 0.0))],
        ),
    );
    let s = sparkle(x, y, r, 0.14);
    p.stroke(&s, fade(glow, 0.8), r * 0.06);
    p.paint(
        &s,
        &radial(
            (x, y),
            r,
            &[(0.0, WHITE), (0.35, tint(core, 0.5)), (1.0, core)],
        ),
    );
}

/// 0..1 twinkle for a phase: mostly bright, briefly dipping.
fn twinkle(t: f32, speed: f32, phase: f32) -> f32 {
    0.5 + 0.5 * ((t * speed + phase) * TAU).sin()
}

pub fn twinkle_star(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (core, glow) = (p.c1, p.c2);
    let t = p.t;
    let k = twinkle(t, 0.8, 0.0);
    p.paint(
        &circle(0.47, 0.5, 0.36),
        &radial(
            (0.47, 0.5),
            0.36,
            &[(0.0, fade(glow, 0.45 + 0.2 * k)), (1.0, fade(glow, 0.0))],
        ),
    );
    let big = sparkle(0.47, 0.5, 0.3 * (0.86 + 0.14 * k), 0.13);
    let big = rot(&big, 6.0 * (t * TAU * 0.4).sin(), (0.47, 0.5));
    p.stroke(&big, fade(glow, 0.9), 0.018);
    p.paint(
        &big,
        &radial(
            (0.47, 0.5),
            0.3,
            &[(0.0, WHITE), (0.3, tint(core, 0.5)), (1.0, core)],
        ),
    );
    let a = twinkle(t, 1.1, 0.35);
    glint(&mut p, 0.78, 0.22, 0.13 * (0.3 + 0.7 * a), core, glow);
    let b = twinkle(t, 1.3, 0.7);
    glint(&mut p, 0.24, 0.76, 0.1 * (0.3 + 0.7 * b), core, glow);
    let c = twinkle(t, 1.7, 0.15);
    glint(&mut p, 0.8, 0.74, 0.06 * (0.2 + 0.8 * c), core, glow);
    p.shadow(0.2);
}

pub fn glitter(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let (w, t) = (p.w, p.t);
    for i in 0..20u32 {
        let size = 0.05 + 0.1 * hash(i * 7 + 1).powf(1.4);
        let x = 0.1 + size + (w - 0.2 - 2.0 * size) * hash(i * 7 + 2);
        let y = 0.1 + size + (0.78 - 2.0 * size) * hash(i * 7 + 3);
        let k = twinkle(t, 0.8 + 1.2 * hash(i * 7 + 4), hash(i * 7 + 5));
        let k = k * k;
        let c = match i % 3 {
            0 => a,
            1 => b,
            _ => tint(a, 0.6),
        };
        if i % 5 == 4 {
            p.fill(
                &circle(x, y, size * 0.3 * (0.4 + 0.6 * k)),
                fade(tint(c, 0.3), 0.5 + 0.5 * k),
            );
        } else {
            glint(&mut p, x, y, size * (0.3 + 0.7 * k), c, c);
        }
    }
    p.shadow(0.2);
}

pub fn confetti(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let palette = [
        a,
        b,
        mix(a, b, 0.5),
        tint(a, 0.45),
        tint(b, 0.4),
        rgb(1.0, 0.84, 0.2),
    ];
    let (cx, cy) = (p.w * 0.5, 0.5);
    let period = 1.8;
    for i in 0..34u32 {
        let u = (p.t / period + hash(i * 5 + 1)).rem_euclid(1.0);
        let ang = hash(i * 5 + 2) * TAU;
        let dist = 0.22 + 0.2 * hash(i * 5 + 3);
        let e = ease_out(u * 1.3);
        let x = cx + ang.cos() * dist * e * 1.25;
        let y = cy + ang.sin() * dist * e * 0.85 + 0.12 * u * u;
        let alpha = smoothstep(0.0, 0.06, u) * (1.0 - smoothstep(0.7, 1.0, u));
        if alpha <= 0.01 {
            continue;
        }
        let c = fade(palette[(i % 6) as usize], alpha);
        let spin = (hash(i * 5 + 4) - 0.5) * 900.0 * u + hash(i * 5 + 4) * 360.0;
        let flip = (0.25 + 0.75 * ((u * 6.0 + hash(i)) * TAU).cos().abs()).max(0.25);
        match i % 3 {
            0 => {
                let r = rrect(x - 0.045, y - 0.024 * flip, 0.09, 0.048 * flip, 0.008);
                p.fill(&rot(&r, spin, (x, y)), c);
            }
            1 => p.fill(&circle(x, y, 0.028), c),
            _ => {
                let s = sample(
                    &[
                        (x - 0.055, y),
                        (x - 0.018, y - 0.028 * flip),
                        (x + 0.018, y + 0.028 * flip),
                        (x + 0.055, y),
                    ],
                    false,
                    6,
                );
                p.stroke(&rot(&line(&s), spin, (x, y)), c, 0.022);
            }
        }
    }
    p.shadow(0.2);
}

pub fn stars(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, dots) = (p.c1, p.c2);
    let t = p.t;
    let spots = [
        (0.38, 0.44, 0.24, 0.0),
        (0.98, 0.3, 0.15, 0.3),
        (1.04, 0.72, 0.12, 0.6),
        (0.7, 0.78, 0.08, 0.15),
        (0.72, 0.18, 0.07, 0.8),
        (0.14, 0.78, 0.07, 0.45),
    ];
    for (i, &(x, y, r, ph)) in spots.iter().enumerate() {
        let k = twinkle(t, 0.7 + 0.25 * i as f32, ph);
        let s = r * (0.7 + 0.3 * k);
        let st = star(
            x,
            y,
            s,
            s * 0.48,
            5,
            12.0 * (t * TAU * 0.3 + ph * 4.0).sin(),
        );
        p.sticker(
            &[&st],
            &gloss(mix(main, tint(main, 0.4), k), y - s, y + s),
            Some((ink(main), EDGE * (0.5 + r))),
            None,
        );
        p.fill(
            &circle(x - s * 0.18, y - s * 0.2, s * 0.13),
            fade(WHITE, 0.5 + 0.4 * k),
        );
    }
    for i in 0..9u32 {
        let x = 0.12 + (p.w - 0.24) * hash(i * 3 + 500);
        let y = 0.12 + 0.76 * hash(i * 3 + 501);
        let k = twinkle(t, 1.5 + hash(i * 3 + 502), hash(i * 3 + 503));
        glint(&mut p, x, y, 0.045 * (0.2 + 0.8 * k * k), dots, dots);
    }
    p.shadow(0.3);
}

pub fn hearts(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let w = p.w;
    for i in 0..7u32 {
        let u = (p.t * 0.42 + i as f32 / 7.0 + 0.1 * hash(i + 9)).rem_euclid(1.0);
        let size = 0.17 + 0.1 * hash(i + 20);
        let x0 = 0.24 + (w - 0.48) * ((i as f32 * 0.43 + 0.2 * hash(i + 30)).rem_euclid(1.0));
        let x = x0 + 0.05 * ((u * 1.5 + hash(i + 40)) * TAU).sin();
        let y = 0.76 - 0.52 * u;
        let alpha = smoothstep(0.0, 0.15, u) * (1.0 - smoothstep(0.7, 1.0, u));
        if alpha <= 0.01 {
            continue;
        }
        let s = size * (0.6 + 0.4 * smoothstep(0.0, 0.25, u));
        let c = if i % 2 == 0 { a } else { b };
        let h = rot(
            &heart(x, y, s, s * 0.9),
            15.0 * ((u * 2.0 + hash(i)) * TAU).sin(),
            (x, y),
        );
        p.paint(&h, &gloss(fade(c, alpha), y - s * 0.5, y + s * 0.5));
        p.fill(
            &circle(x - s * 0.22, y - s * 0.15, s * 0.1),
            fade(WHITE, 0.6 * alpha),
        );
    }
    p.shadow(0.3);
}

pub fn fireworks(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let bursts = [
        (0.42, 0.42, 0.32, 0.0, a, 16),
        (0.66, 0.62, 0.25, 0.45, b, 14),
        (0.72, 0.27, 0.19, 0.75, mix(a, b, 0.5), 12),
    ];
    for (bx, by, br, off, c, n) in bursts {
        let u = (p.t / 1.6 + off).rem_euclid(1.0);
        let e = ease_out(u * 1.15);
        let alpha = 1.0 - smoothstep(0.55, 1.0, u);
        if alpha <= 0.01 || e <= 0.01 {
            continue;
        }
        for k in 0..n {
            let ang = (k as f32 / n as f32
                + 0.5 * hash(k as u32 + (off * 100.0) as u32) / n as f32)
                * TAU;
            let (r0, r1) = (br * e * (0.35 + 0.3 * u), br * e);
            let (ca, sa) = (ang.cos(), ang.sin());
            let drop = 0.05 * u * u;
            let s = seg(
                (bx + ca * r0, by + sa * r0 + drop * 0.5),
                (bx + ca * r1, by + sa * r1 + drop),
            );
            p.stroke(&s, fade(c, alpha * 0.9), 0.03 * (1.0 - 0.5 * u));
            p.fill(
                &circle(bx + ca * r1, by + sa * r1 + drop, 0.024 * (1.0 - 0.4 * u)),
                fade(tint(c, 0.5), alpha),
            );
        }
        if u < 0.3 {
            let f = 1.0 - u / 0.3;
            glint(&mut p, bx, by, 0.12 * f, tint(c, 0.4), c);
        }
    }
    p.shadow(0.25);
}

pub fn shooting_star(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (core, glow) = (p.c1, p.c2);
    let w = p.w;
    let t = p.t;
    let k = (t / 2.4 + 0.3).fract();
    // The head runs from the top right to the bottom left, easing in.
    let head = (w * 0.62 - (w * 0.5) * k, 0.34 + 0.5 * k);
    let dir = norm((0.72 * w, -0.6));
    let fade_all = (k * 8.0).min(1.0) * (1.0 - smoothstep(0.82, 1.0, k));
    // The tail: a tapering trail of glowing dots behind the head.
    for i in 0..26u32 {
        let d = i as f32 / 25.0;
        let (x, y) = (head.0 + dir.0 * d * 0.55, head.1 + dir.1 * d * 0.55);
        let r = 0.05 * (1.0 - d) + 0.006;
        let a = (1.0 - d).powf(1.3) * fade_all;
        p.fill(&circle(x, y, r * 1.8), fade(glow, 0.25 * a));
        p.fill(&circle(x, y, r), fade(tint(core, 0.4), 0.85 * a));
    }
    let star_path = star(head.0, head.1, 0.12, 0.052, 5, 0.0);
    p.paint(
        &circle(head.0, head.1, 0.2),
        &radial(
            head,
            0.2,
            &[(0.0, fade(glow, 0.6 * fade_all)), (1.0, fade(glow, 0.0))],
        ),
    );
    p.sticker(
        &[&star_path],
        &radial(
            head,
            0.12,
            &[(0.0, fade(WHITE, fade_all)), (1.0, fade(core, fade_all))],
        ),
        Some((fade(ink(core), fade_all), EDGE * 0.7)),
        None,
    );
    // A few twinkles left in the sky.
    for (i, (x, y, r)) in [
        (0.2f32, 0.22f32, 0.07f32),
        (0.5, 0.12, 0.05),
        (0.78, 0.8, 0.06),
    ]
    .iter()
    .enumerate()
    {
        let a = twinkle(t, 1.0 + 0.3 * i as f32, 0.3 * i as f32);
        glint(&mut p, x * w, *y, *r * (0.3 + 0.7 * a), core, glow);
    }
    p.shadow(0.25);
}

pub fn snowfall(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (flake, edge) = (p.c1, p.c2);
    let (w, t) = (p.w, p.t);
    for i in 0..22u32 {
        let size = 0.025 + 0.04 * hash(i * 5 + 1);
        let speed = 0.18 + 0.22 * hash(i * 5 + 2);
        let k = (t * speed / 1.0 + hash(i * 5 + 3)).fract();
        let y = 0.2 + 0.6 * k;
        let x =
            0.26 + (w - 0.52) * hash(i * 5 + 4) + 0.03 * (t * 1.3 + hash(i * 5 + 5) * TAU).sin();
        let a = (k * 8.0).min(1.0) * ((1.0 - k) * 8.0).min(1.0);
        p.fill(&circle(x, y, size * 1.9), fade(edge, 0.22 * a));
        p.fill(&circle(x, y, size), fade(flake, 0.95 * a));
        if size > 0.045 {
            for j in 0..3 {
                let ang = j as f32 * 60.0;
                p.stroke(
                    &rot(
                        &seg((x - size * 1.5, y), (x + size * 1.5, y)),
                        ang + 20.0 * t,
                        (x, y),
                    ),
                    fade(flake, 0.9 * a),
                    size * 0.28,
                );
            }
        }
    }
    p.shadow(0.2);
}

pub fn bubbles(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (a, b) = (p.c1, p.c2);
    let (w, t) = (p.w, p.t);
    for i in 0..9u32 {
        let r = 0.04 + 0.07 * hash(i * 3 + 1);
        let speed = 0.12 + 0.16 * hash(i * 3 + 2);
        let k = (t * speed + hash(i * 3 + 3)).fract();
        let y = 0.84 - 0.68 * k;
        let x = 0.12
            + r
            + (w - 0.24 - 2.0 * r) * hash(i * 3 + 4)
            + 0.03 * (t * 1.1 + hash(i * 3 + 5) * TAU).sin();
        let alpha = (k * 6.0).min(1.0) * ((1.0 - k) * 6.0).min(1.0);
        let tintc = mix(a, b, hash(i * 3 + 6));
        p.paint(
            &circle(x, y, r),
            &radial(
                (x, y),
                r,
                &[
                    (0.0, fade(tintc, 0.04 * alpha)),
                    (0.75, fade(tintc, 0.18 * alpha)),
                    (1.0, fade(tint(tintc, 0.6), 0.5 * alpha)),
                ],
            ),
        );
        p.stroke(
            &circle(x, y, r),
            fade(tint(tintc, 0.5), 0.85 * alpha),
            r * 0.09,
        );
        p.fill(
            &rot(
                &ellipse(x - r * 0.38, y - r * 0.4, r * 0.28, r * 0.13),
                -40.0,
                (x - r * 0.38, y - r * 0.4),
            ),
            fade(WHITE, 0.9 * alpha),
        );
    }
    p.shadow(0.2);
}
