//! More social call-outs: link in bio, swipe up, watch now, new video, save,
//! repost, counters, thanks, notifications and a star rating. Plain buttons
//! with generic icons, no brand marks. Main color = the button, second =
//! its icon and lettering.

use super::kit::*;
use super::lettering::text;
use super::social::{bell_icon, draw_label, icon_label, pill};
use super::DrawCtx;
use std::f32::consts::TAU;
use tiny_skia::Pixmap;

/// Draws an icon centred on (cx, cy) in colour `c`; `back` is the button
/// colour, for details cut out of the icon.
type Icon = fn(&mut Pen, f32, f32, C, C);

/// A pill with an icon and a label, like the Like and Subscribe buttons.
fn button(pm: &mut Pixmap, ctx: &DrawCtx, label: &str, icon_w: f32, icon: Icon) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg, w) = (p.c1, p.c2, p.w);
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, t) = icon_label(0.3, w - 0.3, icon_w, 0.13, label, 0.25);
    let sh = fade(ink(main), 0.45);
    icon(&mut p, ix, 0.512, sh, sh);
    icon(&mut p, ix, 0.5, fg, main);
    draw_label(&mut p, &t, fg);
    p.shadow(0.4);
}

/// An arrow head with its tip at `at`, pointing `deg` degrees (0 = right,
/// clockwise).
fn head(p: &mut Pen, at: P, deg: f32, size: f32, c: C) {
    let a = deg.to_radians();
    let (d, n) = ((a.cos(), a.sin()), (-a.sin(), a.cos()));
    let base = (at.0 - d.0 * size, at.1 - d.1 * size);
    p.fill(
        &poly(&[
            at,
            (base.0 + n.0 * size * 0.7, base.1 + n.1 * size * 0.7),
            (base.0 - n.0 * size * 0.7, base.1 - n.1 * size * 0.7),
        ]),
        c,
    );
}

fn link(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    for dx in [-0.075f32, 0.075] {
        let r = rrect(cx + dx - 0.105, cy - 0.065, 0.21, 0.13, 0.065);
        p.stroke(&rot(&r, -45.0, (cx, cy)), c, 0.042);
    }
}

fn chevrons(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    for (k, a) in [(0.1f32, 0.55f32), (-0.06, 1.0)] {
        let pts = [
            (cx - 0.13, cy + 0.05 + k),
            (cx, cy - 0.08 + k),
            (cx + 0.13, cy + 0.05 + k),
        ];
        p.stroke(&line(&pts), fade(c, a), 0.055);
    }
}

fn play_icon(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    let t = smooth(
        &[
            (cx - 0.09, cy - 0.14),
            (cx + 0.15, cy),
            (cx - 0.09, cy + 0.14),
        ],
        true,
        &[],
    );
    p.fill(&t, c);
    p.stroke(&t, c, 0.03);
}

fn clip_icon(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    p.stroke(&rrect(cx - 0.17, cy - 0.12, 0.34, 0.24, 0.05), c, 0.04);
    p.fill(
        &poly(&[
            (cx - 0.04, cy - 0.06),
            (cx + 0.07, cy),
            (cx - 0.04, cy + 0.06),
        ]),
        c,
    );
}

fn bookmark(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    let b = poly(&[
        (cx - 0.1, cy - 0.15),
        (cx + 0.1, cy - 0.15),
        (cx + 0.1, cy + 0.15),
        (cx, cy + 0.05),
        (cx - 0.1, cy + 0.15),
    ]);
    p.fill(&b, c);
    p.stroke(&b, c, 0.02);
}

fn repost(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    for (a0, a1) in [(180.0f32, 330.0f32), (0.0, 150.0)] {
        p.stroke(&line(&arc_pts(cx, cy, 0.14, 0.11, a0, a1, 12)), c, 0.045);
        let a = a1.to_radians();
        let tip = (cx + 0.14 * a.cos(), cy + 0.11 * a.sin());
        head(p, (tip.0 + 0.0, tip.1 + 0.0), a1 + 90.0, 0.07, c);
    }
}

fn heart_icon(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    p.fill(&heart(cx, cy, 0.32, 0.29), c);
}

fn person(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    p.fill(&circle(cx, cy - 0.07, 0.075), c);
    p.fill(
        &smooth(
            &[
                (cx - 0.15, cy + 0.15),
                (cx - 0.12, cy + 0.05),
                (cx, cy + 0.02),
                (cx + 0.12, cy + 0.05),
                (cx + 0.15, cy + 0.15),
            ],
            true,
            &[0, 4],
        ),
        c,
    );
}

fn arrow_down(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    p.stroke(&seg((cx, cy - 0.14), (cx, cy + 0.05)), c, 0.055);
    head(p, (cx, cy + 0.15), 90.0, 0.1, c);
}

fn trend(p: &mut Pen, cx: f32, cy: f32, c: C, _back: C) {
    let pts = [
        (cx - 0.15, cy + 0.1),
        (cx - 0.04, cy - 0.01),
        (cx + 0.03, cy + 0.05),
        (cx + 0.11, cy - 0.06),
    ];
    p.stroke(&line(&pts), c, 0.05);
    head(p, (cx + 0.16, cy - 0.12), -50.0, 0.1, c);
}

fn gift_icon(p: &mut Pen, cx: f32, cy: f32, c: C, back: C) {
    p.fill(&rrect(cx - 0.14, cy - 0.02, 0.28, 0.17, 0.02), c);
    p.fill(&rrect(cx - 0.16, cy - 0.1, 0.32, 0.1, 0.02), c);
    p.fill(&rrect(cx - 0.02, cy - 0.1, 0.04, 0.25, 0.0), back);
    for s in [-1.0f32, 1.0] {
        p.stroke(
            &rot(
                &ellipse(cx + s * 0.06, cy - 0.15, 0.06, 0.035),
                s * 25.0,
                (cx + s * 0.03, cy - 0.12),
            ),
            c,
            0.026,
        );
    }
}

pub fn link_in_bio(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "LINK IN BIO", 0.34, link);
}

pub fn swipe_up(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "SWIPE UP", 0.3, chevrons);
}

pub fn watch_now(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "WATCH NOW", 0.28, play_icon);
}

pub fn new_video(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "NEW VIDEO", 0.36, clip_icon);
}

pub fn save(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "SAVE", 0.24, bookmark);
}

pub fn repost_button(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "REPOST", 0.34, repost);
}

pub fn like_count(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "12.5K", 0.34, heart_icon);
}

pub fn subscribers(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "100K", 0.32, person);
}

pub fn comment_below(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "COMMENT BELOW", 0.26, arrow_down);
}

pub fn thanks(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "THANKS FOR WATCHING", 0.34, heart_icon);
}

pub fn giveaway(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "GIVEAWAY", 0.34, gift_icon);
}

pub fn trending(pm: &mut Pixmap, ctx: &DrawCtx) {
    button(pm, ctx, "TRENDING", 0.36, trend);
}

/// A dark pill with bouncing level bars, like a player.
pub fn now_playing(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg, w, t) = (p.c1, p.c2, p.w, p.t);
    pill(&mut p, 0.13, 0.2, w - 0.26, 0.6, 0.3);
    let (ix, label) = icon_label(0.3, w - 0.3, 0.3, 0.13, "NOW PLAYING", 0.22);
    for i in 0..4 {
        let k = 0.5 + 0.5 * (t * TAU * (0.9 + 0.35 * i as f32) + i as f32 * 1.7).sin();
        let h = 0.07 + 0.2 * k;
        let x = ix - 0.15 + 0.1 * i as f32;
        let bar = rrect(x - 0.03, 0.62 - h, 0.06, h, 0.03);
        p.fill(&moved(&bar, 0.0, 0.012), fade(ink(main), 0.45));
        p.fill(&bar, fg);
    }
    draw_label(&mut p, &label, fg);
    p.shadow(0.4);
}

/// A switch turned on with a bell on its knob.
pub fn notify_on(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, fg, w) = (p.c1, p.c2, p.w);
    let cx = w * 0.5;
    let track = rrect(cx - 0.62, 0.22, 1.24, 0.56, 0.28);
    p.sticker(
        &[&track],
        &gloss(main, 0.22, 0.78),
        Some((ink(main), EDGE)),
        Some((WHITE, RIM)),
    );
    if let Some(t) = text("Poppins Black", "ON", (cx - 0.27, 0.5), 0.3, 0.2, 0.0, 0.0) {
        draw_label(&mut p, &Some(t), fg);
    }
    let knob = circle(cx + 0.34, 0.5, 0.2);
    p.sticker(
        &[&knob],
        &gloss(fg, 0.3, 0.7),
        Some((ink(main), EDGE * 0.8)),
        None,
    );
    bell_icon(&mut p, cx + 0.34, 0.505, 0.3, main);
    p.shadow(0.4);
}

/// Five gold stars in a row.
pub fn rating(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (main, rim, w) = (p.c1, p.c2, p.w);
    let step = ((w - 0.3) / 5.0).min(0.31);
    let x0 = w * 0.5 - step * 2.0;
    let stars: Vec<_> = (0..5)
        .map(|i| star(x0 + step * i as f32, 0.5, step * 0.5, step * 0.23, 5, 0.0))
        .collect();
    let refs: Vec<&_> = stars.iter().collect();
    p.sticker(
        &refs,
        &gloss(main, 0.28, 0.72),
        Some((ink(main), EDGE)),
        Some((rim, RIM)),
    );
    for (i, _) in stars.iter().enumerate() {
        let x = x0 + step * i as f32;
        p.fill(
            &rot(
                &ellipse(x - step * 0.14, 0.45, step * 0.07, step * 0.14),
                30.0,
                (x, 0.5),
            ),
            fade(WHITE, 0.4),
        );
    }
    p.shadow(0.35);
}
