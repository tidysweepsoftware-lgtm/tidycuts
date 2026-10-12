//! Word stickers: short punchy words in four current treatments (neon tube,
//! glossy 3D, outlined pop, solid label). Each bounces in and then moves a
//! little (a flicker, a wobble, a sweep of light) so it never sits dead.
//!
//! `ctx.variant` is `word * 4 + treatment`. The main colour is the face of
//! the letters (or the label), the second colour is their edge or glow.

use super::kit::*;
use super::lettering::text;
use super::DrawCtx;
use crate::motion::Spring;
use std::f32::consts::TAU;
use tiny_skia::Pixmap;

pub const WORDS: [&str; 16] = [
    "NEW", "SALE", "HOT", "LIVE", "WOW", "OMG", "LOL", "REC", "FREE", "BEST", "TOP", "WIN", "GOAL",
    "YES", "COOL", "SOON",
];

const LOOP: f32 = 3.0;

fn glint(p: &mut Pen, mask: &Option<tiny_skia::Mask>, t: f32, c: P, w: f32) {
    // A diagonal band of light sweeping across the letters once a loop.
    let u = smoothstep(0.9, 1.9, t.rem_euclid(LOOP));
    if u <= 0.0 || u >= 1.0 {
        return;
    }
    let x = c.0 - w * 0.7 + u * w * 1.4;
    let band = poly(&[
        (x - 0.07, 0.0),
        (x + 0.03, 0.0),
        (x - 0.07 + 0.3, 1.0),
        (x - 0.17 + 0.3, 1.0),
    ]);
    p.fill_in(&band, fade(WHITE, 0.55), mask);
}

pub fn draw(pm: &mut Pixmap, ctx: &DrawCtx) {
    let word = WORDS[(ctx.variant / 4) as usize % WORDS.len()];
    let kind = ctx.variant % 4;
    // The neon glow spills a long way, so it gets more room.
    let mut p = Pen::new(pm, ctx).inset(if kind == 0 { 0.7 } else { 0.84 });
    let pw = p.w;
    let t = p.t.rem_euclid(LOOP);
    let c = (p.w * 0.5, 0.5);
    let (c1, c2) = (p.c1, p.c2);
    let pop = 0.7 + 0.3 * Spring::new(3.2, 0.42).at(t / 0.7);
    let sway = 2.0 * (t * TAU / LOOP).sin();
    let (max_w, max_h) = (1.6, 0.52);
    match kind {
        0 => {
            // Neon tube: an outline in the glow colour with a lit core.
            let Some(g) = text("Poppins Black", word, c, max_w, max_h, 0.0, 0.0) else {
                return;
            };
            let g = scaled(&g, pop, c);
            let f = (t * 22.0).floor() as u32;
            let flick = if hash(f.wrapping_mul(11)) > 0.93 {
                0.55
            } else {
                1.0
            };
            p.stroke(&g, fade(c1, 0.14), 0.03);
            p.stroke(&g, fade(c2, 0.25 * flick), 0.075);
            p.stroke(&g, fade(c1, flick), 0.026);
            p.stroke(&g, fade(WHITE, flick), 0.01);
            p.bloom(1.4 * flick, 0.05, 0.8);
            p.bloom(0.8 * flick, 0.09, 0.8);
        }
        1 => {
            // Glossy 3D: a stack of darker copies for depth, then the face.
            let Some(g) = text("Titan One", word, c, max_w, max_h, 0.0, 2.0 * sway) else {
                return;
            };
            let g = scaled(&g, pop, c);
            let depth = 0.05;
            for i in (1..=10).rev() {
                let d = depth * i as f32 / 10.0;
                p.fill(&moved(&g, 0.0, d), mix(c2, shade(c2, 0.6), i as f32 / 10.0));
            }
            p.stroke(&g, ink(c1), 0.03);
            p.paint(&g, &gloss(c1, 0.25, 0.75));
            let mask = p.clip(&g);
            glint(&mut p, &mask, t, c, pw);
            // A soft highlight along the top of the letters.
            let top = rrect(c.0 - 0.7, 0.28, 1.4, 0.07, 0.035);
            p.fill_in(&top, fade(WHITE, 0.35), &mask);
        }
        2 => {
            // Outlined pop: a thick white outline round the colour, tilted.
            let Some(g) = text("Lilita One", word, c, max_w, max_h, 0.0, -6.0 + sway) else {
                return;
            };
            let g = scaled(&g, pop, c);
            p.stroke(&moved(&g, 0.0, 0.025), fade(c2, 0.9), 0.1);
            p.stroke(&g, WHITE, 0.085);
            p.paint(&g, &gloss(c1, 0.25, 0.75));
            let mask = p.clip(&g);
            glint(&mut p, &mask, t, c, pw);
        }
        _ => {
            // A solid label with the word cut out of it.
            let body = rrect(c.0 - 0.95, 0.22, 1.9, 0.56, 0.12);
            let body = scaled(&body, pop, c);
            p.paint(&body, &gloss(c1, 0.22, 0.78));
            let mask = p.clip(&body);
            let Some(g) = text("Poppins Black", word, c, 1.45, 0.28, 0.0, 0.0) else {
                return;
            };
            p.fill(&scaled(&g, pop, c), c2);
            glint(&mut p, &mask, t, c, pw);
            p.stroke(
                &scaled(&rrect(c.0 - 0.9, 0.27, 1.8, 0.46, 0.09), pop, c),
                fade(WHITE, 0.25),
                0.01,
            );
        }
    }
    if kind != 0 {
        p.shadow(0.5);
    }
}
