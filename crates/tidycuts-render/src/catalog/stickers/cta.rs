//! Call-out stickers for creators: subscribe, follow, like, share, comment,
//! notification bell, link and swipe prompts. Each is a short looping scene
//! (the button pops in, a cursor clicks it, it changes state), drawn flat
//! with a soft gloss and a drop shadow.
//!
//! `ctx.variant` picks the wording or icon of a drawing; the catalog gives
//! every variant its own name and colours. The main colour is the button,
//! the second colour is the "done" state.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use crate::motion::Spring;
use std::f32::consts::PI;
use tiny_skia::{Path, Pixmap};

/// Every scene loops in this many seconds.
const PERIOD: f32 = 3.2;
const FONT: &str = "Poppins Black";

/// A smooth 0 → 1 → 0 bump between `a` and `b` seconds.
fn bump(t: f32, a: f32, b: f32) -> f32 {
    if t <= a || t >= b {
        0.0
    } else {
        ((t - a) / (b - a) * PI).sin()
    }
}

/// The entrance: grows from 80% with a little overshoot.
fn enter(t: f32) -> f32 {
    0.8 + 0.2 * Spring::new(3.2, 0.5).at(t / 0.7)
}

/// Fades the whole picture (used to close a loop gently).
fn fade_all(pm: &mut Pixmap, k: f32) {
    let k = k.clamp(0.0, 1.0);
    if k >= 1.0 {
        return;
    }
    for v in pm.data_mut() {
        *v = (*v as f32 * k + 0.5) as u8;
    }
}

/// The loop's last moments fade out and its first ones fade in.
fn loop_fade(t: f32) -> f32 {
    smoothstep(PERIOD, PERIOD - 0.3, t)
}

/// A glossy pill centred on `c`, `w` x `h`, pressed in by `press` (0..1).
fn pill(p: &mut Pen, c: P, w: f32, h: f32, col: C, k: f32) {
    let body = scaled(&rrect(c.0 - w * 0.5, c.1 - h * 0.5, w, h, h * 0.5), k, c);
    p.paint(&body, &gloss(col, c.1 - h * 0.5 * k, c.1 + h * 0.5 * k));
    let hl = scaled(
        &rrect(c.0 - w * 0.46, c.1 - h * 0.46, w * 0.92, h * 0.34, h * 0.17),
        k,
        c,
    );
    p.fill(&hl, fade(WHITE, 0.16));
}

/// A bell `s` tall, hanging from (cx, top).
fn bell(cx: f32, top: f32, s: f32) -> Vec<Path> {
    let dome = circle(cx, top + 0.34 * s, 0.26 * s);
    let body = poly(&[
        (cx - 0.26 * s, top + 0.34 * s),
        (cx - 0.44 * s, top + 0.76 * s),
        (cx + 0.44 * s, top + 0.76 * s),
        (cx + 0.26 * s, top + 0.34 * s),
    ]);
    let rim = rrect(cx - 0.5 * s, top + 0.7 * s, s, 0.12 * s, 0.06 * s);
    let knob = circle(cx, top + 0.06 * s, 0.07 * s);
    let clapper = circle(cx, top + 0.95 * s, 0.1 * s);
    vec![dome, body, rim, knob, clapper]
}

/// The ringing angle (degrees) `age` seconds after the click.
fn ring_angle(age: f32) -> f32 {
    if age < 0.0 {
        0.0
    } else {
        18.0 * (-age * 2.2).exp() * (age * 17.0).sin()
    }
}

/// A mouse pointer with its tip at `tip`, `s` tall.
fn pointer(tip: P, s: f32) -> Path {
    let q = |x: f32, y: f32| (tip.0 + x * s, tip.1 + y * s);
    poly(&[
        q(0.0, 0.0),
        q(0.0, 0.78),
        q(0.19, 0.62),
        q(0.31, 0.92),
        q(0.43, 0.87),
        q(0.31, 0.58),
        q(0.55, 0.58),
    ])
}

/// The cursor's path during the loop: it comes in, clicks and leaves.
/// Returns its tip and opacity.
fn cursor_at(t: f32, from: P, target: P) -> (P, f32) {
    let k = ease_out(smoothstep(0.7, 1.15, t));
    let leave = smoothstep(1.5, 1.9, t);
    let tip = lerp(lerp(from, target, k), (from.0 + 0.3, from.1 + 0.3), leave);
    (tip, smoothstep(0.7, 0.9, t) * (1.0 - leave))
}

fn draw_cursor(p: &mut Pen, t: f32, from: P, target: P, size: f32) {
    let (tip, a) = cursor_at(t, from, target);
    if a <= 0.01 {
        return;
    }
    // The click ripple.
    let r = (t - 1.2) / 0.45;
    if (0.0..1.0).contains(&r) {
        let rc = circle(target.0, target.1, 0.04 + 0.3 * ease_out(r));
        p.stroke(&rc, fade(WHITE, 0.8 * (1.0 - r)), 0.03);
    }
    let ptr = pointer(tip, size);
    p.sticker(
        &[&ptr],
        &solid(fade(WHITE, a)),
        Some((fade([0.1, 0.1, 0.14, 1.0], a), 0.012)),
        None,
    );
}

fn label(s: &str, c: P, max_w: f32, max_h: f32) -> Option<Path> {
    text(FONT, s, c, max_w, max_h, 0.0, 0.0)
}

// ---------------------------------------------------------------------------
// Subscribe / follow.

/// A pill that says one thing, gets clicked and says another, with a bell
/// that rings once it is on. Variant 0 subscribes, 1 joins.
pub fn subscribe(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let (off, on) = if ctx.variant == 1 {
        ("JOIN", "JOINED")
    } else {
        ("SUBSCRIBE", "SUBSCRIBED")
    };
    let done = t > 1.25;
    let col = if done { p.c2 } else { p.c1 };
    let k = enter(t) * (1.0 - 0.07 * bump(t, 1.1, 1.4));
    let mid = (p.w * 0.5, 0.5);
    let pc = (mid.0 - 0.32, 0.5);
    pill(&mut p, scaled_pt(pc, mid, k), 1.75 * k, 0.5 * k, col, 1.0);
    if let Some(l) = label(if done { on } else { off }, pc, 1.35, 0.17) {
        p.fill(&scaled(&l, k, mid), WHITE);
    }
    // The bell in its own round badge.
    let bc = (mid.0 + 0.98, 0.5);
    let badge = scaled(&circle(bc.0, bc.1, 0.26), k, mid);
    p.paint(&badge, &gloss(if done { p.c1 } else { WHITE }, 0.24, 0.76));
    let bell_col = if done { WHITE } else { p.c1 };
    let pivot = (bc.0, bc.1 - 0.14);
    let ang = ring_angle(t - 1.3);
    for part in bell(bc.0, bc.1 - 0.14, 0.3) {
        let part = rot(&part, ang, pivot);
        p.fill(&scaled(&part, k, mid), bell_col);
    }
    draw_cursor(&mut p, t, (pc.0 + 1.0, 0.98), (pc.0 + 0.1, 0.6), 0.28);
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

/// A plus pill that turns into a check. Variant 0 follows, 1 connects.
pub fn follow(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let (off, on) = if ctx.variant == 1 {
        ("CONNECT", "CONNECTED")
    } else {
        ("FOLLOW", "FOLLOWING")
    };
    let done = t > 1.25;
    let col = if done { p.c2 } else { p.c1 };
    let k = enter(t) * (1.0 - 0.07 * bump(t, 1.1, 1.4));
    let mid = (p.w * 0.5, 0.5);
    let pc = (mid.0, 0.5);
    pill(&mut p, pc, 1.9 * k, 0.5 * k, col, 1.0);
    // The icon: a plus, then a check.
    let ic = (mid.0 - 0.68, 0.5);
    let swap = smoothstep(1.2, 1.4, t);
    if swap < 0.5 {
        let a = 0.1 * (1.0 - swap * 2.0).max(0.1);
        let plus = [
            seg((ic.0 - a, ic.1), (ic.0 + a, ic.1)),
            seg((ic.0, ic.1 - a), (ic.0, ic.1 + a)),
        ];
        for s in &plus {
            p.stroke(&scaled(s, k, mid), WHITE, 0.045);
        }
    } else {
        let a = 0.1 * ((swap - 0.5) * 2.0);
        let chk = line(&[
            (ic.0 - a, ic.1 + a * 0.05),
            (ic.0 - a * 0.25, ic.1 + a * 0.75),
            (ic.0 + a, ic.1 - a * 0.7),
        ]);
        p.stroke(&scaled(&chk, k, mid), WHITE, 0.05);
    }
    if let Some(l) = label(if done { on } else { off }, (mid.0 + 0.2, 0.5), 1.1, 0.17) {
        p.fill(&scaled(&l, k, mid), WHITE);
    }
    draw_cursor(&mut p, t, (mid.0 + 1.1, 1.0), (mid.0 + 0.2, 0.62), 0.28);
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

/// A point scaled about `c` (as `scaled` does for paths).
fn scaled_pt(pt: P, c: P, k: f32) -> P {
    (c.0 + (pt.0 - c.0) * k, c.1 + (pt.1 - c.1) * k)
}

// ---------------------------------------------------------------------------
// Like.

/// A heart (variant 0) or star (variant 1) that fills and pops when tapped,
/// with a ring of sparks.
pub fn like(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let word = if ctx.variant == 1 {
        "FAVOURITE"
    } else {
        "LIKE"
    };
    let k = enter(t);
    let (pw, ph) = (if ctx.variant == 1 { 2.3 } else { 1.6 }, 0.5);
    let body = scaled(&rrect(mid.0 - pw * 0.5, 0.25, pw, ph, ph * 0.5), k, mid);
    p.paint(&body, &gloss(WHITE, 0.25, 0.75));
    p.stroke(&body, fade([0.1, 0.1, 0.14, 1.0], 0.1), 0.012);
    let done = t > 1.25;
    let ic = (mid.0 - pw * 0.5 + 0.3, 0.5);
    let beat = Spring::new(3.0, 0.38).at((t - 1.25).max(0.0) / 0.9);
    let pop = if done {
        1.0 + 0.45 * (1.0 - beat)
    } else {
        1.0 - 0.06 * bump(t, 1.1, 1.35)
    };
    let icon_col = if done {
        p.c1
    } else {
        mix([0.62, 0.64, 0.7, 1.0], p.c1, 0.3)
    };
    let icon = if ctx.variant == 1 {
        star(ic.0, ic.1, 0.15, 0.065, 5, 0.0)
    } else {
        heart(ic.0, ic.1, 0.3, 0.27)
    };
    let icon = scaled(&scaled(&icon, pop, ic), k, mid);
    p.fill(&icon, icon_col);
    // Sparks around the icon right after the tap.
    let s = (t - 1.25) / 0.6;
    if (0.0..1.0).contains(&s) {
        for i in 0..8 {
            let a = i as f32 * PI / 4.0 + 0.2;
            let r0 = 0.2 + 0.2 * ease_out(s);
            let r1 = r0 + 0.07 * (1.0 - s);
            let (ca, sa) = (a.cos(), a.sin());
            p.stroke(
                &seg(
                    (ic.0 + ca * r0, ic.1 + sa * r0),
                    (ic.0 + ca * r1, ic.1 + sa * r1),
                ),
                fade(p.c1, 1.0 - s),
                0.025,
            );
        }
    }
    if let Some(l) = label(word, (mid.0 + 0.22, 0.5), pw - 0.8, 0.17) {
        p.fill(
            &scaled(&l, k, mid),
            if done {
                shade(p.c1, 0.9)
            } else {
                [0.2, 0.22, 0.28, 1.0]
            },
        );
    }
    draw_cursor(&mut p, t, (ic.0 + 0.9, 1.0), (ic.0 + 0.04, 0.56), 0.28);
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

// ---------------------------------------------------------------------------
// Share.

/// A pill with an arrow that flies out of a tray. Variant 0 says SHARE,
/// 1 says SEND.
pub fn share(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let word = if ctx.variant == 1 { "SEND" } else { "SHARE" };
    let k = enter(t) * (1.0 - 0.07 * bump(t, 1.1, 1.4));
    let c1 = p.c1;
    pill(&mut p, mid, 1.7 * k, 0.5 * k, c1, 1.0);
    let ic = (mid.0 - 0.52, 0.5);
    // The tray.
    let tray = line(&[
        (ic.0 - 0.12, ic.1 + 0.02),
        (ic.0 - 0.12, ic.1 + 0.12),
        (ic.0 + 0.12, ic.1 + 0.12),
        (ic.0 + 0.12, ic.1 + 0.02),
    ]);
    p.stroke(&scaled(&tray, k, mid), WHITE, 0.04);
    // The arrow leaves upwards after the tap and comes back.
    let fly = smoothstep(1.2, 1.7, t) * (1.0 - smoothstep(1.9, 2.4, t));
    let dy = -0.14 * fly;
    let a = line(&[(ic.0, ic.1 + 0.06 + dy), (ic.0, ic.1 - 0.14 + dy)]);
    let head = line(&[
        (ic.0 - 0.08, ic.1 - 0.06 + dy),
        (ic.0, ic.1 - 0.15 + dy),
        (ic.0 + 0.08, ic.1 - 0.06 + dy),
    ]);
    p.stroke(&scaled(&a, k, mid), fade(WHITE, 1.0 - fly * 0.4), 0.04);
    p.stroke(&scaled(&head, k, mid), fade(WHITE, 1.0 - fly * 0.4), 0.04);
    if let Some(l) = label(word, (mid.0 + 0.2, 0.5), 0.95, 0.17) {
        p.fill(&scaled(&l, k, mid), WHITE);
    }
    draw_cursor(&mut p, t, (mid.0 + 1.0, 1.0), (mid.0 + 0.1, 0.62), 0.28);
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

// ---------------------------------------------------------------------------
// Comment.

/// A speech bubble whose three dots bounce in turn, then a line of text
/// appears. Variant 0 says COMMENT, 1 says REPLY.
pub fn comment(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let word = if ctx.variant == 1 { "REPLY" } else { "COMMENT" };
    let k = enter(t);
    let (bw, bh) = (1.9, 0.5);
    let tail = poly(&[
        (mid.0 - bw * 0.5 + 0.2, 0.25 + bh - 0.02),
        (mid.0 - bw * 0.5 + 0.14, 0.25 + bh + 0.14),
        (mid.0 - bw * 0.5 + 0.38, 0.25 + bh - 0.02),
    ]);
    let body = rrect(mid.0 - bw * 0.5, 0.25, bw, bh, 0.2);
    let (body, tail) = (scaled(&body, k, mid), scaled(&tail, k, mid));
    p.paint(&body, &gloss(p.c1, 0.25, 0.75));
    p.fill(&tail, shade(p.c1, 0.82));
    let typing = t < 1.6;
    if typing {
        let c0 = (mid.0 - 0.28, 0.5);
        for i in 0..3 {
            let phase = ((t * 2.2 - i as f32 * 0.22).rem_euclid(1.0) * PI)
                .sin()
                .max(0.0);
            let d = (c0.0 + 0.28 * i as f32, 0.5 - 0.07 * phase);
            p.fill(&scaled(&circle(d.0, d.1, 0.05), k, mid), WHITE);
        }
    } else if let Some(l) = label(word, mid, 1.4, 0.17) {
        let u = smoothstep(1.6, 1.9, t);
        p.fill(&scaled(&l, k * (0.9 + 0.1 * u), mid), fade(WHITE, u));
    }
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

// ---------------------------------------------------------------------------
// Bell.

/// A big notification bell that rings, with a red dot that pops up.
pub fn bell_ring(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.78);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let k = enter(t);
    let pivot = (mid.0, 0.2);
    let ang = if t < 0.9 {
        0.0
    } else {
        ring_angle(t - 0.9) * 1.3
    };
    let parts = bell(mid.0, 0.2, 0.6);
    // A darker body underneath, then the lit one.
    for part in &parts {
        let part = scaled(&rot(part, ang, pivot), k, mid);
        p.sticker(
            &[&part],
            &gloss(p.c1, 0.2, 0.85),
            Some((ink(p.c1), 0.012)),
            None,
        );
    }
    // Ringing waves.
    for side in [-1.0f32, 1.0] {
        for i in 0..2 {
            let on = bump(t, 0.9 + i as f32 * 0.15, 1.7 + i as f32 * 0.15);
            if on > 0.0 {
                let r = 0.38 + 0.08 * i as f32;
                let arc = line(&arc_pts(
                    mid.0,
                    0.5,
                    r,
                    r,
                    if side < 0.0 { 150.0 } else { -30.0 },
                    if side < 0.0 { 210.0 } else { 30.0 },
                    8,
                ));
                p.stroke(&scaled(&arc, k, mid), fade(p.c2, on), 0.03);
            }
        }
    }
    // The badge dot.
    let dot = Spring::new(3.0, 0.45).at((t - 1.0) / 0.6);
    if t > 1.0 {
        let c = (mid.0 + 0.32, 0.3);
        let d = scaled(&circle(c.0, c.1, 0.12), k * dot, c);
        p.fill(&d, WHITE);
        p.fill(&scaled(&circle(c.0, c.1, 0.09), k * dot, c), p.c2);
    }
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

// ---------------------------------------------------------------------------
// Link and swipe.

/// A pill pointing at something with an arrow that bobs. Variant 0 points
/// up ("LINK IN BIO"), 1 points down ("LINK BELOW").
pub fn link(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let up = ctx.variant != 1;
    let word = if up { "LINK IN BIO" } else { "LINK BELOW" };
    let k = enter(t);
    let c1 = p.c1;
    pill(&mut p, mid, 2.0 * k, 0.5 * k, c1, 1.0);
    let ic = (mid.0 - 0.72, 0.5);
    let bob = 0.04 * (t * 5.5).sin();
    let dir = if up { -1.0 } else { 1.0 };
    let shaft = seg(
        (ic.0, ic.1 - dir * 0.1 + bob),
        (ic.0, ic.1 + dir * 0.12 + bob),
    );
    let head = line(&[
        (ic.0 - 0.08, ic.1 - dir * 0.02 + bob),
        (ic.0, ic.1 - dir * 0.12 + bob),
        (ic.0 + 0.08, ic.1 - dir * 0.02 + bob),
    ]);
    p.stroke(&scaled(&shaft, k, mid), WHITE, 0.045);
    p.stroke(&scaled(&head, k, mid), WHITE, 0.045);
    if let Some(l) = label(word, (mid.0 + 0.2, 0.5), 1.3, 0.17) {
        p.fill(&scaled(&l, k, mid), WHITE);
    }
    p.shadow(0.5);
    fade_all(pm, loop_fade(t));
}

/// Stacked chevrons that sweep upward with a label under them.
pub fn swipe(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let t = p.t.rem_euclid(PERIOD);
    let mid = (p.w * 0.5, 0.5);
    let k = enter(t);
    let word = if ctx.variant == 1 {
        "SEE MORE"
    } else {
        "SWIPE UP"
    };
    for i in 0..3 {
        // Each chevron lights in turn from the bottom.
        let phase = ((t * 1.6 - i as f32 * 0.18).rem_euclid(1.0)).clamp(0.0, 1.0);
        let a = 0.25 + 0.75 * (phase * PI).sin().max(0.0);
        let y = 0.34 + 0.13 * i as f32;
        let ch = line(&[
            (mid.0 - 0.2, y + 0.07),
            (mid.0, y - 0.07),
            (mid.0 + 0.2, y + 0.07),
        ]);
        p.stroke(&scaled(&ch, k, mid), fade(p.c1, a), 0.06);
    }
    if let Some(l) = label(word, (mid.0, 0.84), p.w * 0.8, 0.1) {
        p.fill(&scaled(&l, k, mid), p.c1);
    }
    p.shadow(0.45);
    fade_all(pm, loop_fade(t));
}
