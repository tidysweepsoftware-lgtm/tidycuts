//! Emoji-style faces and symbols.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use tiny_skia::{Path, Pixmap};

const CX: f32 = 0.5;
const CY: f32 = 0.48;
const R: f32 = 0.41;

/// Draws the round face and returns the color for eyes and mouths.
fn face(p: &mut Pen) -> C {
    let main = p.c1;
    let body = radial(
        (CX - 0.06, CY - 0.2),
        R * 1.45,
        &[
            (0.0, tint(main, 0.42)),
            (0.45, main),
            (1.0, warm_shade(main)),
        ],
    );
    p.sticker(&[&circle(CX, CY, R)], &body, Some((ink(main), EDGE)), None);
    p.fill(
        &ellipse(CX, CY - 0.66 * R, 0.5 * R, 0.15 * R),
        fade(WHITE, 0.24),
    );
    features(main)
}

/// A darker, warmer version of `c` for the rim of a face.
fn warm_shade(c: C) -> C {
    mix(shade(c, 0.8), [c[0], c[1] * 0.68, c[2] * 0.45, c[3]], 0.45)
}

fn features(main: C) -> C {
    mix(shade(main, 0.3), rgb(0.3, 0.16, 0.06), 0.55)
}

fn eye(p: &mut Pen, x: f32, y: f32, rx: f32, ry: f32, f: C) {
    p.fill(&ellipse(x, y, rx, ry), f);
    p.fill(
        &circle(x + rx * 0.3, y - ry * 0.45, rx * 0.36),
        fade(WHITE, 0.9),
    );
}

/// Open "D" mouth: flat top at `y`, half ellipse below.
fn open_mouth(p: &mut Pen, y: f32, rx: f32, ry: f32, f: C, tongue: Option<C>) {
    let mut pts = arc_pts(CX, y, rx, ry, 0.0, 180.0, 32);
    pts.push((CX - rx * 0.6, y - 0.012));
    pts.push((CX + rx * 0.6, y - 0.012));
    let mouth = smooth(&pts, true, &[0, 32]);
    p.stroke(&mouth, f, 0.02);
    p.fill(&mouth, f);
    let m = p.clip(&mouth);
    p.fill_in(
        &rrect(CX - rx, y - 0.05, 2.0 * rx, 0.05 + ry * 0.24, 0.02),
        WHITE,
        &m,
    );
    if let Some(t) = tongue {
        p.fill_in(&ellipse(CX, y + ry * 0.98, rx * 0.55, ry * 0.42), t, &m);
    }
}

fn tear(p: &mut Pen, x: f32, y: f32, s: f32, deg: f32) {
    let c2 = p.c2;
    let d = drop(x, y, s, deg);
    p.sticker(
        &[&d],
        &gloss(c2, y - s * 1.6, y + s),
        Some((ink(c2), EDGE * 0.6)),
        None,
    );
    p.fill(
        &ellipse(x - s * 0.35, y - s * 0.05, s * 0.2, s * 0.32),
        fade(WHITE, 0.7),
    );
}

pub fn smile(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let blush = p.c2;
    for s in [-1.0, 1.0] {
        eye(
            &mut p,
            CX + s * 0.32 * R,
            CY - 0.2 * R,
            0.1 * R,
            0.17 * R,
            f,
        );
        p.fill(
            &ellipse(CX + s * 0.6 * R, CY + 0.2 * R, 0.16 * R, 0.09 * R),
            fade(blush, 0.6),
        );
    }
    let pts = arc_pts(CX, CY + 0.02 * R, 0.52 * R, 0.48 * R, 22.0, 158.0, 24);
    p.stroke(&line(&pts), f, 0.085 * R);
    p.shadow(0.4);
}

pub fn laugh(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    for s in [-1.0, 1.0] {
        let pts = arc_pts(
            CX + s * 0.33 * R,
            CY - 0.12 * R,
            0.15 * R,
            0.13 * R,
            195.0,
            345.0,
            12,
        );
        p.stroke(&line(&pts), f, 0.085 * R);
        // Brows lifted with joy.
        let b = arc_pts(
            CX + s * 0.36 * R,
            CY - 0.38 * R,
            0.15 * R,
            0.07 * R,
            205.0,
            335.0,
            8,
        );
        p.stroke(&line(&b), f, 0.05 * R);
    }
    open_mouth(
        &mut p,
        CY + 0.1 * R,
        0.58 * R,
        0.56 * R,
        f,
        Some(rgb(1.0, 0.42, 0.48)),
    );
    for s in [-1.0f32, 1.0] {
        tear(
            &mut p,
            CX + s * 0.73 * R,
            CY + 0.06 * R,
            0.13 * R,
            -s * 40.0,
        );
    }
    p.shadow(0.4);
}

pub fn heart_eyes(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let red = p.c2;
    for s in [-1.0f32, 1.0] {
        let (x, y) = (CX + s * 0.36 * R, CY - 0.17 * R);
        let h = rot(&heart(x, y, 0.42 * R, 0.38 * R), s * 12.0, (x, y));
        p.sticker(
            &[&h],
            &gloss(red, y - 0.2 * R, y + 0.2 * R),
            Some((ink(red), EDGE * 0.7)),
            None,
        );
        p.fill(
            &circle(x - 0.09 * R, y - 0.07 * R, 0.045 * R),
            fade(WHITE, 0.8),
        );
    }
    open_mouth(
        &mut p,
        CY + 0.24 * R,
        0.44 * R,
        0.4 * R,
        f,
        Some(rgb(1.0, 0.42, 0.48)),
    );
    p.shadow(0.4);
}

pub fn wow(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    for s in [-1.0, 1.0] {
        let b = arc_pts(
            CX + s * 0.34 * R,
            CY - 0.42 * R,
            0.17 * R,
            0.1 * R,
            210.0,
            330.0,
            10,
        );
        p.stroke(&line(&b), f, 0.06 * R);
        p.fill(
            &ellipse(CX + s * 0.32 * R, CY - 0.1 * R, 0.13 * R, 0.17 * R),
            WHITE,
        );
        p.stroke(
            &ellipse(CX + s * 0.32 * R, CY - 0.1 * R, 0.13 * R, 0.17 * R),
            f,
            0.03 * R,
        );
        eye(
            &mut p,
            CX + s * 0.32 * R,
            CY - 0.08 * R,
            0.075 * R,
            0.1 * R,
            f,
        );
    }
    let m = ellipse(CX, CY + 0.44 * R, 0.2 * R, 0.26 * R);
    p.fill(&m, f);
    let mk = p.clip(&m);
    let tongue = p.c2;
    p.fill_in(&ellipse(CX, CY + 0.66 * R, 0.14 * R, 0.1 * R), tongue, &mk);
    p.shadow(0.4);
}

pub fn sad(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    for s in [-1.0f32, 1.0] {
        p.stroke(
            &seg(
                (CX + s * 0.5 * R, CY - 0.36 * R),
                (CX + s * 0.18 * R, CY - 0.5 * R),
            ),
            f,
            0.065 * R,
        );
        eye(
            &mut p,
            CX + s * 0.32 * R,
            CY - 0.1 * R,
            0.1 * R,
            0.14 * R,
            f,
        );
    }
    let pts = arc_pts(CX, CY + 0.82 * R, 0.42 * R, 0.32 * R, 215.0, 325.0, 16);
    p.stroke(&line(&pts), f, 0.085 * R);
    tear(&mut p, CX + 0.36 * R, CY + 0.28 * R, 0.13 * R, 0.0);
    p.shadow(0.4);
}

pub fn cool(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let lens_c = p.c2;
    let frame = mix(lens_c, rgb(0.05, 0.05, 0.08), 0.5);
    let y = CY - 0.3 * R;
    let lens = |s: f32| -> Path {
        let x = CX + s * 0.42 * R;
        let pts = [
            (x - 0.33 * R, y),
            (x + 0.33 * R, y),
            (x + 0.3 * R, y + 0.24 * R),
            (x + 0.06 * R, y + 0.4 * R),
            (x - 0.2 * R, y + 0.36 * R),
            (x - 0.32 * R, y + 0.2 * R),
        ];
        smooth(&pts, true, &[0, 1])
    };
    let (l, r) = (lens(-1.0), lens(1.0));
    p.stroke(
        &seg((CX - 0.82 * R, y - 0.08 * R), (CX + 0.82 * R, y - 0.08 * R)),
        frame,
        0.07 * R,
    );
    for (lp, s) in [(&l, -1.0f32), (&r, 1.0)] {
        p.stroke(lp, frame, 0.07 * R);
        let x = CX + s * 0.42 * R;
        p.paint(
            lp,
            &linear(
                (x - 0.3 * R, y),
                (x + 0.3 * R, y + 0.4 * R),
                &[(0.0, tint(lens_c, 0.3)), (1.0, lens_c)],
            ),
        );
        let m = p.clip(lp);
        p.fill_in(
            &rot(
                &rrect(x - 0.2 * R, y - 0.1 * R, 0.08 * R, 0.6 * R, 0.02),
                35.0,
                (x, y),
            ),
            fade(WHITE, 0.55),
            &m,
        );
        p.fill_in(
            &rot(
                &rrect(x - 0.05 * R, y - 0.1 * R, 0.035 * R, 0.6 * R, 0.01),
                35.0,
                (x, y),
            ),
            fade(WHITE, 0.4),
            &m,
        );
    }
    p.stroke(
        &seg((CX - 0.12 * R, y + 0.02 * R), (CX + 0.12 * R, y + 0.02 * R)),
        frame,
        0.08 * R,
    );
    let smirk = smooth(
        &[
            (CX - 0.32 * R, CY + 0.42 * R),
            (CX + 0.05 * R, CY + 0.52 * R),
            (CX + 0.4 * R, CY + 0.3 * R),
        ],
        false,
        &[],
    );
    p.stroke(&smirk, f, 0.085 * R);
    p.shadow(0.4);
}

pub fn thumbs_up(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (skin, cuff_c) = (p.c1, p.c2);
    let edge = ink(skin);
    let body = gloss(skin, 0.1, 0.9);
    let cuff = rrect(0.07, 0.46, 0.17, 0.42, 0.05);
    let palm = rrect(0.2, 0.43, 0.42, 0.45, 0.12);
    let thumb = seg((0.35, 0.47), (0.4, 0.21));
    let tw = 0.2;
    let fingers: Vec<Path> = [
        (0.44, 0.43, 0.125),
        (0.43, 0.545, 0.12),
        (0.4, 0.655, 0.12),
        (0.35, 0.765, 0.115),
    ]
    .iter()
    .map(|&(len, y, h)| rrect(0.42, y, len, h, h * 0.5))
    .collect();
    for part in [&cuff, &palm] {
        p.stroke(part, edge, 2.0 * EDGE);
    }
    p.stroke(&thumb, edge, tw + 2.0 * EDGE);
    for f in &fingers {
        p.stroke(f, edge, 2.0 * EDGE);
    }
    p.paint(&cuff, &gloss(cuff_c, 0.46, 0.88));
    p.stroke(
        &seg((0.12, 0.51), (0.12, 0.83)),
        fade(ink(cuff_c), 0.35),
        0.018,
    );
    p.stroke_with(&thumb, &body, &round(tw));
    p.paint(&palm, &body);
    for f in &fingers {
        p.paint(f, &body);
        p.stroke(f, edge, 0.016);
    }
    // Thumbnail, crease and shine.
    let nail = rot(&rrect(0.39, 0.14, 0.085, 0.11, 0.04), 10.0, (0.43, 0.19));
    p.fill(&nail, fade(tint(skin, 0.65), 0.9));
    p.stroke(&nail, fade(edge, 0.45), 0.01);
    p.stroke(
        &line(&arc_pts(0.33, 0.5, 0.075, 0.045, 200.0, 330.0, 8)),
        fade(edge, 0.7),
        0.014,
    );
    p.stroke(&seg((0.31, 0.26), (0.29, 0.4)), fade(WHITE, 0.4), 0.03);
    p.stroke(&seg((0.52, 0.465), (0.78, 0.465)), fade(WHITE, 0.35), 0.022);
    p.shadow(0.4);
}

/// A flame standing on (cx, base) `w` wide and `h` tall; `t` sways the tips.
fn flame(cx: f32, base: f32, w: f32, h: f32, t: f32, seed: f32) -> Path {
    let sway = |k: f32, a: f32| a * (t * (5.0 + k) + seed + k * 1.7).sin();
    let pts = [
        (0.0, 0.0),
        (0.42, -0.07),
        (0.52, -0.36),
        (0.36 + sway(1.0, 0.02), -0.6),
        (0.38 + sway(2.0, 0.05), -0.84 + sway(3.0, 0.03)),
        (0.16, -0.66),
        (0.04 + sway(4.0, 0.07), -1.0 + sway(5.0, 0.03)),
        (-0.22, -0.66),
        (-0.4 + sway(6.0, 0.05), -0.76 + sway(7.0, 0.03)),
        (-0.48, -0.46),
        (-0.42, -0.1),
    ];
    let pts: Vec<P> = pts
        .iter()
        .map(|&(x, y)| (cx + x * w, base + y * h))
        .collect();
    smooth(&pts, true, &[4, 6, 8])
}

pub fn fire(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, core) = (p.c1, p.c2);
    let (cx, base, t) = (p.w * 0.5, 0.88, p.t);
    let outer = flame(cx, base, 0.6, 0.78, t, 0.0);
    let fill = linear(
        (0.0, base - 0.78),
        (0.0, base),
        &[
            (0.0, mix(main, rgb(0.85, 0.1, 0.1), 0.35)),
            (0.6, main),
            (1.0, mix(main, core, 0.35)),
        ],
    );
    p.sticker(&[&outer], &fill, Some((ink(main), EDGE)), None);
    let mid = flame(cx, base - 0.02, 0.4, 0.5, t, 2.0);
    p.paint(
        &mid,
        &linear(
            (0.0, base - 0.5),
            (0.0, base),
            &[(0.0, core), (1.0, tint(core, 0.35))],
        ),
    );
    let inner = flame(cx, base - 0.03, 0.2, 0.24, t, 4.0);
    p.fill(&inner, tint(core, 0.7));
    p.shadow(0.35);
}

pub fn hundred(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim) = (p.c1, p.c2);
    let w = p.w;
    let txt = text(
        "Poppins Black",
        "100",
        (w * 0.5, 0.38),
        w * 0.84,
        0.46,
        0.2,
        -4.0,
    );
    let u1 = ribbon(
        &sample(&[(0.2, 0.72), (0.7, 0.705), (1.12, 0.68)], false, 10),
        |s| 0.085 - 0.04 * s,
    );
    let u2 = ribbon(
        &sample(&[(0.3, 0.82), (0.7, 0.81), (1.04, 0.79)], false, 10),
        |s| 0.065 - 0.03 * s,
    );
    let mut parts: Vec<&Path> = vec![&u1, &u2];
    if let Some(t) = &txt {
        parts.push(t);
    }
    p.sticker(
        &parts,
        &gloss(main, 0.15, 0.85),
        Some((ink(main), EDGE * 0.7)),
        Some((rim, RIM)),
    );
    p.shadow(0.4);
}

pub fn party(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, accent) = (p.c1, p.c2);
    let tip = (0.15, 0.86);
    let m = (0.48, 0.52);
    let d = norm((m.0 - tip.0, m.1 - tip.1));
    let n = (-d.1, d.0);
    let hw = 0.17;
    let cone = poly(&[
        tip,
        (m.0 + n.0 * hw, m.1 + n.1 * hw),
        (m.0 - n.0 * hw, m.1 - n.1 * hw),
    ]);
    let ang = d.1.atan2(d.0).to_degrees();
    let mouth = rot(&ellipse(m.0, m.1, 0.065, hw), ang, m);
    // Streamers and confetti come out first so the cone sits in front.
    let blue = rgb(0.25, 0.7, 1.0);
    let green = rgb(0.35, 0.85, 0.45);
    let s1 = sample(
        &[m, (0.6, 0.34), (0.7, 0.4), (0.74, 0.24), (0.86, 0.17)],
        false,
        10,
    );
    let s2 = sample(&[m, (0.68, 0.5), (0.74, 0.42), (0.88, 0.46)], false, 10);
    let s3 = sample(&[m, (0.46, 0.32), (0.38, 0.28), (0.42, 0.14)], false, 10);
    for (s, c) in [(&s1, accent), (&s2, blue), (&s3, green)] {
        p.stroke(&line(s), ink(c), 0.034 + 2.0 * EDGE * 0.6);
        p.stroke(&line(s), c, 0.034);
    }
    let bits: [(f32, f32, u8, C, f32); 8] = [
        (0.6, 0.14, 0, blue, 20.0),
        (0.86, 0.32, 1, accent, 0.0),
        (0.8, 0.62, 0, green, -30.0),
        (0.3, 0.2, 2, accent, 10.0),
        (0.9, 0.62, 1, rgb(1.0, 0.85, 0.2), 0.0),
        (0.66, 0.26, 1, green, 0.0),
        (0.52, 0.2, 0, rgb(1.0, 0.85, 0.2), 50.0),
        (0.72, 0.72, 2, blue, -15.0),
    ];
    for (x, y, kind, c, deg) in bits {
        let shape = match kind {
            0 => rot(
                &rrect(x - 0.035, y - 0.018, 0.07, 0.036, 0.008),
                deg,
                (x, y),
            ),
            1 => circle(x, y, 0.026),
            _ => rot(
                &poly(&[
                    (x, y - 0.035),
                    (x + 0.032, y + 0.024),
                    (x - 0.032, y + 0.024),
                ]),
                deg,
                (x, y),
            ),
        };
        p.sticker(&[&shape], &solid(c), Some((ink(c), EDGE * 0.5)), None);
    }
    p.sticker(
        &[&cone, &mouth],
        &gloss(main, 0.4, 0.9),
        Some((ink(main), EDGE)),
        None,
    );
    let mk = p.clip(&cone);
    for k in [0.3f32, 0.55, 0.8] {
        let c = (tip.0 + (m.0 - tip.0) * k, tip.1 + (m.1 - tip.1) * k);
        let band = rot(&rrect(c.0 - 0.025, c.1 - 0.3, 0.05, 0.6, 0.0), ang, c);
        p.fill_in(&band, accent, &mk);
    }
    p.fill(&mouth, shade(main, 0.45));
    p.stroke(&mouth, ink(main), 0.014);
    p.shadow(0.4);
}

pub fn eyes(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (white, pupil) = (p.c1, p.c2);
    let tau = std::f32::consts::TAU;
    let look = ((p.t / 2.4 * tau).sin() * 2.2).clamp(-1.0, 1.0);
    let up = 0.3 * (p.t / 1.2 * tau).sin().max(0.0);
    let cx = p.w * 0.5;
    for s in [-1.0f32, 1.0] {
        let (x, y) = (cx + s * 0.22, 0.5);
        let e = ellipse(x, y, 0.19, 0.3);
        let fill = radial(
            (x - 0.03, y - 0.08),
            0.34,
            &[
                (0.0, tint(white, 0.6)),
                (0.7, white),
                (1.0, shade(white, 0.8)),
            ],
        );
        p.sticker(&[&e], &fill, Some((ink(pupil), EDGE * 1.1)), None);
        let (px, py) = (x + look * 0.075, y + 0.04 - up * 0.1);
        let iris = circle(px, py, 0.105);
        p.paint(
            &iris,
            &radial((px, py), 0.105, &[(0.0, tint(pupil, 0.15)), (1.0, pupil)]),
        );
        p.fill(&circle(px + 0.035, py - 0.04, 0.032), fade(WHITE, 0.9));
        p.fill(&circle(px - 0.03, py + 0.04, 0.014), fade(WHITE, 0.6));
    }
    p.shadow(0.4);
}

pub fn wink(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let blush = p.c2;
    eye(&mut p, CX - 0.32 * R, CY - 0.2 * R, 0.1 * R, 0.17 * R, f);
    // The winking eye: a curve like a closed lid with a lash flick.
    let lid = arc_pts(
        CX + 0.32 * R,
        CY - 0.17 * R,
        0.17 * R,
        0.12 * R,
        195.0,
        345.0,
        12,
    );
    p.stroke(&line(&lid), f, 0.075 * R);
    for s in [-1.0, 1.0] {
        p.fill(
            &ellipse(CX + s * 0.62 * R, CY + 0.22 * R, 0.16 * R, 0.09 * R),
            fade(blush, 0.6),
        );
    }
    let smile = arc_pts(
        CX + 0.04 * R,
        CY + 0.04 * R,
        0.5 * R,
        0.46 * R,
        22.0,
        150.0,
        24,
    );
    p.stroke(&line(&smile), f, 0.085 * R);
    // A tongue poking out of the corner.
    let tongue = rrect(CX + 0.22 * R, CY + 0.46 * R, 0.24 * R, 0.3 * R, 0.12 * R);
    p.sticker(
        &[&tongue],
        &gloss(rgb(1.0, 0.45, 0.52), CY + 0.46 * R, CY + 0.76 * R),
        Some((fade(f, 0.9), EDGE * 0.6)),
        None,
    );
    p.stroke(
        &seg(
            (CX + 0.34 * R, CY + 0.5 * R),
            (CX + 0.34 * R, CY + 0.64 * R),
        ),
        fade(f, 0.45),
        0.014,
    );
    p.shadow(0.4);
}

pub fn angry(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let steam = p.c2;
    for s in [-1.0f32, 1.0] {
        // Heavy brows slanting down to the middle.
        p.stroke(
            &seg(
                (CX + s * 0.62 * R, CY - 0.5 * R),
                (CX + s * 0.14 * R, CY - 0.26 * R),
            ),
            f,
            0.11 * R,
        );
        eye(
            &mut p,
            CX + s * 0.34 * R,
            CY - 0.06 * R,
            0.1 * R,
            0.13 * R,
            f,
        );
        p.fill(
            &ellipse(CX + s * 0.64 * R, CY + 0.28 * R, 0.14 * R, 0.08 * R),
            fade(rgb(1.0, 0.2, 0.2), 0.5),
        );
    }
    // A tight, downturned mouth with clenched teeth.
    let mouth = smooth(
        &[
            (CX - 0.4 * R, CY + 0.62 * R),
            (CX, CY + 0.46 * R),
            (CX + 0.4 * R, CY + 0.62 * R),
        ],
        false,
        &[],
    );
    p.stroke(&mouth, f, 0.09 * R);
    for k in [-0.2f32, 0.0, 0.2] {
        p.stroke(
            &seg((CX + k * R, CY + 0.5 * R), (CX + k * R, CY + 0.57 * R)),
            fade(WHITE, 0.9),
            0.03 * R,
        );
    }
    // Puffs of steam over the head.
    for (dx, dy, r) in [
        (-0.62f32, -0.78f32, 0.1f32),
        (-0.78, -0.62, 0.07),
        (0.64, -0.78, 0.1),
        (0.8, -0.62, 0.07),
    ] {
        let c = (CX + dx * R, CY + dy * R);
        p.sticker(
            &[&circle(c.0, c.1, r * R * 1.6)],
            &gloss(steam, c.1 - r * R, c.1 + r * R),
            Some((fade(ink(steam), 0.7), EDGE * 0.6)),
            None,
        );
    }
    p.shadow(0.4);
}

pub fn sob(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let f = face(&mut p);
    let water = p.c2;
    // Streams running down from both eyes.
    for s in [-1.0f32, 1.0] {
        let c = sample(
            &[
                (CX + s * 0.34 * R, CY - 0.08 * R),
                (CX + s * 0.46 * R, CY + 0.3 * R),
                (CX + s * 0.58 * R, CY + 0.7 * R),
                (CX + s * 0.6 * R, CY + 0.95 * R),
            ],
            false,
            8,
        );
        let stream = ribbon(&c, |t| 0.1 * R * (1.0 - 0.3 * t));
        p.sticker(
            &[&stream],
            &gloss(water, CY - 0.1 * R, CY + 0.95 * R),
            Some((fade(ink(water), 0.8), EDGE * 0.6)),
            None,
        );
    }
    for s in [-1.0f32, 1.0] {
        // Eyes squeezed shut with raised inner brows.
        let lid = arc_pts(
            CX + s * 0.34 * R,
            CY - 0.18 * R,
            0.17 * R,
            0.13 * R,
            20.0,
            160.0,
            12,
        );
        p.stroke(&line(&lid), f, 0.075 * R);
        p.stroke(
            &seg(
                (CX + s * 0.58 * R, CY - 0.45 * R),
                (CX + s * 0.16 * R, CY - 0.55 * R),
            ),
            f,
            0.05 * R,
        );
    }
    open_mouth(
        &mut p,
        CY + 0.34 * R,
        0.4 * R,
        0.4 * R,
        f,
        Some(rgb(1.0, 0.42, 0.48)),
    );
    p.shadow(0.4);
}

pub fn thinking(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.88);
    let f = face(&mut p);
    let dots = p.c2;
    // One brow raised, the other level; eyes looking up and to the side.
    p.stroke(
        &line(&arc_pts(
            CX - 0.34 * R,
            CY - 0.44 * R,
            0.17 * R,
            0.1 * R,
            205.0,
            335.0,
            8,
        )),
        f,
        0.055 * R,
    );
    p.stroke(
        &seg((CX + 0.18 * R, CY - 0.3 * R), (CX + 0.5 * R, CY - 0.34 * R)),
        f,
        0.055 * R,
    );
    for s in [-1.0f32, 1.0] {
        let (ex, ey) = (CX + s * 0.34 * R, CY - 0.1 * R);
        p.fill(&ellipse(ex, ey, 0.14 * R, 0.17 * R), WHITE);
        p.stroke(
            &ellipse(ex, ey, 0.14 * R, 0.17 * R),
            fade(f, 0.8),
            0.025 * R,
        );
        p.fill(&circle(ex - 0.04 * R, ey - 0.06 * R, 0.07 * R), f);
        p.fill(&circle(ex - 0.06 * R, ey - 0.09 * R, 0.022 * R), WHITE);
    }
    // A small, lopsided mouth.
    p.stroke(
        &smooth(
            &[
                (CX - 0.2 * R, CY + 0.5 * R),
                (CX + 0.05 * R, CY + 0.44 * R),
                (CX + 0.3 * R, CY + 0.52 * R),
            ],
            false,
            &[],
        ),
        f,
        0.075 * R,
    );
    // A chin hand: a rounded fist under the mouth.
    let hand = rrect(CX - 0.25 * R, CY + 0.62 * R, 0.5 * R, 0.34 * R, 0.14 * R);
    let skin = mix(p.c1, WHITE, 0.12);
    p.sticker(
        &[&hand],
        &gloss(skin, CY + 0.62 * R, CY + 0.96 * R),
        Some((ink(skin), EDGE)),
        None,
    );
    for k in [-0.13f32, 0.0, 0.13] {
        p.stroke(
            &seg((CX + k * R, CY + 0.7 * R), (CX + k * R, CY + 0.82 * R)),
            fade(ink(skin), 0.5),
            0.012,
        );
    }
    // Thought dots rising up to the right.
    for (i, (dx, dy, r)) in [
        (0.72f32, -0.7f32, 0.05f32),
        (0.86, -0.9, 0.07),
        (1.0, -1.1, 0.1),
    ]
    .iter()
    .enumerate()
    {
        let _ = i;
        let c = (CX + dx * R, CY + dy * R);
        p.sticker(
            &[&circle(c.0, c.1, r * R * 1.4)],
            &gloss(dots, c.1 - r * R, c.1 + r * R),
            Some((ink(dots), EDGE * 0.6)),
            None,
        );
    }
    p.shadow(0.4);
}

pub fn sleepy(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.88);
    let f = face(&mut p);
    let zc = p.c2;
    let t = p.t;
    // Closed, drooping eyes and a small round mouth.
    for s in [-1.0f32, 1.0] {
        let lid = arc_pts(
            CX + s * 0.34 * R,
            CY - 0.14 * R,
            0.17 * R,
            0.12 * R,
            20.0,
            160.0,
            12,
        );
        p.stroke(&line(&lid), f, 0.075 * R);
    }
    p.fill(&ellipse(CX + 0.04 * R, CY + 0.46 * R, 0.1 * R, 0.13 * R), f);
    p.fill(
        &ellipse(CX + 0.04 * R, CY + 0.52 * R, 0.07 * R, 0.07 * R),
        fade(rgb(1.0, 0.45, 0.52), 0.95),
    );
    for s in [-1.0f32, 1.0] {
        p.fill(
            &ellipse(CX + s * 0.62 * R, CY + 0.2 * R, 0.16 * R, 0.09 * R),
            fade(p.c1, 0.0),
        );
    }
    // Three Z's float up and fade, each a third of a loop behind the last.
    for i in 0..3u32 {
        let k = (t / 3.0 + i as f32 / 3.0).fract();
        let size = 0.1 + 0.1 * k;
        let (x, y) = (CX + (0.68 + 0.3 * k) * R, CY - (0.45 + 0.45 * k) * R);
        let alpha = (k * 6.0).min(1.0) * (1.0 - k).powf(0.6);
        if let Some(z) = text("Poppins Black", "Z", (x, y), size, size, 0.0, -10.0) {
            let a = alpha.clamp(0.0, 1.0);
            p.fill(&moved(&z, 0.0, 0.01), fade(ink(zc), 0.6 * a));
            p.stroke(&z, fade(ink(zc), a), 0.018);
            p.fill(&z, fade(zc, a));
        }
    }
    p.shadow(0.4);
}
