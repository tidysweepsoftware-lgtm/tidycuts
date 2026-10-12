//! Countdown stickers: looping 3-2-1 timers in current styles (neon tube,
//! LED clock, dissolving dust, ring timer, glitch, glossy 3D).
//!
//! Every one is a pure function of `t` and loops every 3 seconds. Each second
//! shows one number: it springs in, then lives out the rest of the second.
//! Nothing here is ever blank, so the loop has no gaps.
//!
//! Each design is a family: `ctx.variant` (0, 1, 2) picks the font, layout or
//! options, and the catalog entry gives each variant its own name and colours.

use super::kit::*;
use super::lettering::text_path;
use super::DrawCtx;
use crate::motion::Spring;
use crate::{motion_blur, RgbaImage};
use std::f32::consts::{PI, TAU};
use tiny_skia::{BlendMode, FillRule, Path, PathBuilder, Pixmap, Rect, Transform};

/// Which number shows (3, 2, 1) and how far into its second we are (0..1).
fn beat(t: f32) -> (u32, f32) {
    let t = t.rem_euclid(3.0);
    (3 - (t.floor() as u32).min(2), t.fract())
}

/// Draws `frame` with motion blur: it is drawn at a few moments just before
/// now and averaged. The blur never reaches back past the start of the
/// current second, so a new number cuts in crisply, with no ghost of the old.
fn blurred(pm: &mut Pixmap, ctx: &DrawCtx, shutter: f32, frame: fn(&mut Pixmap, &DrawCtx)) {
    let (w, h) = (pm.width(), pm.height());
    let render = |t: f32| {
        let mut sub = Pixmap::new(w, h).expect("same size as the sticker");
        let at = DrawCtx {
            w: ctx.w,
            h: ctx.h,
            color: ctx.color,
            color2: ctx.color2,
            t,
            variant: ctx.variant,
        };
        frame(&mut sub, &at);
        RgbaImage {
            width: w,
            height: h,
            pixels: sub.take(),
            premultiplied: true,
        }
    };
    let out = motion_blur::accumulate(ctx.t, shutter, 5, ctx.t.floor(), render);
    pm.data_mut().copy_from_slice(&out.pixels);
}

/// The entrance: starts 28% too big and settles with a small bounce.
fn pop(u: f32) -> f32 {
    1.0 + 0.28 * spring(u)
}

/// A number "landing": 1 at the start, easing to 0 with a small bounce. A
/// beat lasts one second, so `u` is also the time in seconds.
fn spring(u: f32) -> f32 {
    1.0 - Spring::new(3.5, 0.42).at(u)
}

/// The digit `n` set in `font`, `h` tall, its ink centred on `c`. Every digit
/// uses the same scale, so a 1 stays slim next to an 8.
fn digit(font: &str, n: u32, h: f32, c: P) -> Path {
    let cap = text_path(font, "8", 0.0)
        .and_then(|p| p.compute_tight_bounds())
        .map_or(0.7, |b| b.height());
    let k = h / cap.max(1e-4);
    let Some(p) = text_path(font, &n.to_string(), 0.0) else {
        return circle(c.0, c.1, h * 0.3);
    };
    let Some(b) = p.compute_tight_bounds() else {
        return p;
    };
    xf(
        &p,
        Transform::from_translate(c.0, c.1)
            .pre_scale(k, k)
            .pre_translate(-(b.left() + b.right()) * 0.5, -(b.top() + b.bottom()) * 0.5),
    )
}

// ---------------------------------------------------------------------------
// Neon tube.

/// A glowing glass tube bent into the number, with a loose connection that
/// makes it stutter now and then.
pub fn neon(pm: &mut Pixmap, ctx: &DrawCtx) {
    // Drawn a little smaller, so the glow has room before the edge.
    let mut p = Pen::new(pm, ctx).inset(0.66);
    let (tube, glow) = (p.c1, p.c2);
    let (n, u) = beat(p.t);
    let c = (p.w * 0.5, 0.5);
    // 0: a bold outline tube. 1: a multi-line display face, which glows as
    // several parallel tubes. 2: a slim, tall tube.
    let font = ["Poppins Bold", "Monoton", "Bebas Neue"][(ctx.variant as usize).min(2)];
    let g = scaled(&digit(font, n, 0.5, c), pop(u), c);
    let frame = (p.t * 24.0).floor() as u32;
    let flick = if hash(frame.wrapping_mul(7).wrapping_add(n)) > 0.9 {
        0.45
    } else {
        1.0
    };
    // The unlit glass, a tight tinted halo, the lit tube, then a white-hot
    // core. Bloom turns the core into real light that spills outwards.
    // The multi-line face has thin parallel tubes, so its strokes are thinner.
    let k = if ctx.variant == 1 { 0.34 } else { 1.0 };
    p.stroke(&g, fade(tube, 0.14), 0.034 * k);
    p.stroke(&g, fade(glow, 0.22 * flick), 0.075 * k);
    p.stroke(&g, fade(tube, flick), 0.028 * k);
    p.stroke(&g, fade(WHITE, flick), 0.012 * k);
    p.bloom(1.5 * flick, 0.05, 0.8);
    p.bloom(0.9 * flick, 0.09, 0.8);
}

// ---------------------------------------------------------------------------
// LED clock.

/// Which of the seven segments (a b c d e f g = bits 0..6) each digit lights.
const SEGMENTS: [u8; 10] = [63, 6, 91, 79, 102, 109, 125, 7, 127, 111];

/// Segment `i` (0 = top, then clockwise, 6 = middle) of a digit cell.
fn segment(i: usize, x: f32, y: f32, dw: f32, dh: f32, th: f32) -> Path {
    let gap = th * 0.12;
    let lh = dw - th - 2.0 * gap;
    let lv = dh * 0.5 - th - 2.0 * gap;
    let (cx, cy, horizontal, len) = match i {
        0 => (x + dw * 0.5, y + th * 0.5, true, lh),
        1 => (x + dw - th * 0.5, y + dh * 0.25, false, lv),
        2 => (x + dw - th * 0.5, y + dh * 0.75, false, lv),
        3 => (x + dw * 0.5, y + dh - th * 0.5, true, lh),
        4 => (x + th * 0.5, y + dh * 0.75, false, lv),
        5 => (x + th * 0.5, y + dh * 0.25, false, lv),
        _ => (x + dw * 0.5, y + dh * 0.5, true, lh),
    };
    let (a, b) = (len * 0.5, th * 0.5);
    let pts = [
        (-a, 0.0),
        (-a + b, -b),
        (a - b, -b),
        (a, 0.0),
        (a - b, b),
        (-a + b, b),
    ];
    poly(&pts.map(|(l, s)| {
        if horizontal {
            (cx + l, cy + s)
        } else {
            (cx + s, cy + l)
        }
    }))
}

/// One slot of the display.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    /// A digit that stays 0.
    Zero,
    /// The digit that counts down.
    Count,
    Colon,
}

/// A seven-segment display counting down, in a housing. Unlit segments stay
/// faintly visible, like a real display. Variants: 0 is a metal clock reading
/// 00:03, 1 is a tall two-digit display in metal, 2 is a dark plastic
/// 0:03 timer.
pub fn digital(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let lit = p.c1;
    let (n, u) = beat(p.t);
    let w = p.w;
    let slots: &[Slot] = match ctx.variant {
        1 => &[Slot::Zero, Slot::Count],
        2 => &[Slot::Zero, Slot::Colon, Slot::Zero, Slot::Count],
        _ => &[Slot::Zero, Slot::Zero, Slot::Colon, Slot::Zero, Slot::Count],
    };
    let body = rrect(0.1, 0.14, w - 0.2, 0.72, 0.09);
    let housing = if ctx.variant == 2 {
        linear(
            (0.0, 0.14),
            (0.0, 0.86),
            &[
                (0.0, rgb(0.34, 0.35, 0.4)),
                (0.5, rgb(0.17, 0.17, 0.21)),
                (1.0, rgb(0.09, 0.09, 0.12)),
            ],
        )
    } else {
        linear(
            (0.0, 0.14),
            (0.0, 0.86),
            &[
                (0.0, rgb(0.88, 0.9, 0.94)),
                (0.5, rgb(0.56, 0.58, 0.64)),
                (1.0, rgb(0.3, 0.31, 0.37)),
            ],
        )
    };
    p.sticker(
        &[&body],
        &housing,
        Some((rgb(0.06, 0.06, 0.09), EDGE)),
        None,
    );
    let screen = rrect(0.165, 0.205, w - 0.33, 0.59, 0.05);
    p.paint(
        &screen,
        &linear(
            (0.0, 0.2),
            (0.0, 0.8),
            &[(0.0, rgb(0.06, 0.06, 0.08)), (1.0, rgb(0.02, 0.02, 0.03))],
        ),
    );

    let (dw, dh, th, gap, colon) = (0.34, 0.4, 0.062, 0.1, 0.1);
    let digits = slots.iter().filter(|s| **s != Slot::Colon).count() as f32;
    let colons = slots.len() as f32 - digits;
    let total = digits * dw + colons * colon + (slots.len() as f32 - 1.0) * gap;
    let y = 0.5 - dh * 0.5;
    let mut x = (w - total) * 0.5;
    // The seconds digit flashes brighter as it changes.
    let flash = (1.0 - u / 0.15).max(0.0);
    for slot in slots {
        let value = if *slot == Slot::Count { n } else { 0 };
        if *slot == Slot::Colon {
            let blink = if u < 0.5 { 1.0 } else { 0.15 };
            for cy in [y + dh * 0.32, y + dh * 0.68] {
                let dot = circle(x + colon * 0.5, cy, 0.027);
                p.stroke(&dot, fade(lit, 0.18 * blink), 0.05);
                p.fill(&dot, fade(tint(lit, 0.2), blink));
            }
            x += colon + gap;
            continue;
        }
        let mask = SEGMENTS[value as usize];
        let hot = if *slot == Slot::Count { flash } else { 0.0 };
        for i in 0..7 {
            let seg = segment(i, x, y, dw, dh, th);
            if mask & (1 << i) == 0 {
                p.fill(&seg, fade(lit, 0.08));
            } else {
                p.stroke(&seg, fade(lit, 0.16 + 0.2 * hot), 0.045);
                p.fill(&seg, tint(lit, 0.12 + 0.5 * hot));
            }
        }
        x += dw + gap;
    }
    // The lit segments glow into the dark glass around them.
    p.bloom(1.1, 0.035, 0.93);
    // Glass reflection across the top of the screen.
    let clip = p.clip(&screen);
    p.fill_in(
        &poly(&[(0.17, 0.2), (w * 0.62, 0.2), (w * 0.5, 0.36), (0.17, 0.36)]),
        fade(WHITE, 0.05),
        &clip,
    );
    p.shadow(0.3);
}

// ---------------------------------------------------------------------------
// Dissolving dust.

/// A bold number that crumbles into drifting dust from left to right, with
/// embers rising off it, just before the next number takes its place.
pub fn dust(pm: &mut Pixmap, ctx: &DrawCtx) {
    blurred(pm, ctx, 0.04, dust_frame);
}

fn dust_frame(pm: &mut Pixmap, ctx: &DrawCtx) {
    const GRID: u32 = 72;
    const HALF: f32 = 0.36;
    const START: f32 = 0.45;
    let mut p = Pen::new(pm, ctx);
    let (face, ember) = (p.c1, p.c2);
    let (n, u) = beat(p.t);
    let c = (p.w * 0.5, 0.5);
    let font = ["Poppins Black", "Anton", "Bebas Neue"][(ctx.variant as usize).min(2)];
    let g = scaled(&digit(font, n, 0.52, c), pop(u), c);
    p.paint(
        &circle(c.0, c.1, 0.46),
        &radial(c, 0.46, &[(0.0, fade(ember, 0.3)), (1.0, fade(ember, 0.0))]),
    );
    let body = linear(
        (0.0, 0.22),
        (0.0, 0.78),
        &[(0.0, tint(face, 0.55)), (1.0, shade(face, 0.8))],
    );
    p.paint(&g, &body);

    let prog = ((u - START) / (1.0 - START)).clamp(0.0, 1.0);
    if prog <= 0.0 {
        return;
    }
    // Which little squares of the number are ink, found on a coarse grid.
    let Some(mut scratch) = Pixmap::new(GRID, GRID) else {
        return;
    };
    let (x0, y0) = (c.0 - HALF, c.1 - HALF);
    let k = GRID as f32 / (2.0 * HALF);
    scratch.fill_path(
        &g,
        &solid(WHITE),
        FillRule::Winding,
        Transform::from_scale(k, k).pre_translate(-x0, -y0),
        None,
    );
    let cell = 2.0 * HALF / GRID as f32;
    let mut gone = PathBuilder::new();
    let mut dust = Vec::new();
    for j in 0..GRID {
        for i in 0..GRID {
            let ink = scratch.data()[((j * GRID + i) * 4 + 3) as usize];
            if ink == 0 {
                continue;
            }
            let id = j * GRID + i;
            let across = i as f32 / GRID as f32;
            let delay = 0.4 * (0.65 * across + 0.35 * hash(id * 3 + 1));
            let life = (prog - delay).clamp(0.0, 1.0);
            if life <= 0.0 {
                continue;
            }
            let (x, y) = (x0 + (i as f32 + 0.5) * cell, y0 + (j as f32 + 0.5) * cell);
            // Even the faint edge cells are wiped, or a ghost outline stays.
            if let Some(r) = Rect::from_xywh(x - cell * 0.6, y - cell * 0.6, cell * 1.2, cell * 1.2)
            {
                gone.push_rect(r);
            }
            if ink >= 128 {
                dust.push((id, x, y, life));
            }
        }
    }
    if let Some(path) = gone.finish() {
        p.erase(&path);
    }
    for (id, x, y, life) in dust {
        let e = ease_out(life);
        let (h1, h2) = (hash(id * 3 + 2), hash(id * 3 + 3));
        let px = x + (0.04 + 0.14 * h1) * e;
        let py = y - (0.03 + 0.12 * h2) * e + 0.02 * e * e;
        let size = cell * (1.0 - 0.35 * life) * (0.9 + 0.5 * h1);
        // Stays solid for most of its flight, then melts away.
        let a = 1.0 - smoothstep(0.6, 1.0, life);
        // Hot at first, cooling to the ember color as it drifts away.
        let col = mix(tint(face, 0.4), ember, (life * (0.4 + 0.9 * h2)).min(1.0));
        let mut light = solid(fade(col, a * 0.5));
        light.blend_mode = BlendMode::Plus;
        p.paint(&circle(px, py, size * 0.75), &light);
    }
    p.bloom(0.7, 0.035, 0.8);
}

// ---------------------------------------------------------------------------
// Ring timer.

/// A ring that drains over each second, a glowing head riding its end, and a
/// pulse that ripples out when the number changes.
pub fn ring(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.8);
    let (arc, accent) = (p.c1, p.c2);
    let (n, u) = beat(p.t);
    let c = (p.w * 0.5, 0.5);
    let r = 0.37;
    // 0: a fine ring with tick marks. 1: a thick bold ring. 2: two rings, the
    // inner one filling up as the outer one drains.
    let (ticks, thick, num_h) = match ctx.variant {
        1 => (0, 0.11, 0.34),
        2 => (0, 0.06, 0.28),
        _ => (60, 0.07, 0.3),
    };
    p.stroke(&circle(c.0, c.1, r), fade(WHITE, 0.1), thick);
    // Tick marks inside the ring: the ones that have passed go dark.
    for i in 0..ticks {
        let f = i as f32 / ticks as f32;
        let a = f * TAU - PI * 0.5;
        let (ri, ro) = (r - 0.115, r - (if i % 5 == 0 { 0.075 } else { 0.092 }));
        let t = seg(
            (c.0 + a.cos() * ri, c.1 + a.sin() * ri),
            (c.0 + a.cos() * ro, c.1 + a.sin() * ro),
        );
        let alive = f < 1.0 - u;
        p.stroke(
            &t,
            fade(
                if alive { arc } else { WHITE },
                if alive { 0.9 } else { 0.18 },
            ),
            0.008,
        );
    }
    let sweep = 360.0 * (1.0 - u);
    if sweep > 1.0 {
        let pts = arc_pts(c.0, c.1, r, r, -90.0, -90.0 + sweep, 96);
        let path = line(&pts);
        p.stroke(&path, fade(accent, 0.08), 0.15);
        p.stroke(&path, fade(accent, 0.14), 0.105);
        p.stroke(&path, arc, thick);
        p.stroke(&path, fade(tint(arc, 0.65), 0.9), thick * 0.3);
        if let Some(&h) = pts.last() {
            p.paint(
                &circle(h.0, h.1, 0.1),
                &radial(
                    h,
                    0.1,
                    &[
                        (0.0, fade(WHITE, 0.95)),
                        (0.35, fade(tint(accent, 0.4), 0.55)),
                        (1.0, fade(accent, 0.0)),
                    ],
                ),
            );
        }
    }
    if ctx.variant == 2 {
        // The inner ring grows while the outer one drains.
        let ri = r - 0.12;
        p.stroke(&circle(c.0, c.1, ri), fade(WHITE, 0.08), 0.035);
        if u > 0.003 {
            let pts = arc_pts(c.0, c.1, ri, ri, -90.0, -90.0 + 360.0 * u, 72);
            let inner = line(&pts);
            p.stroke(&inner, fade(accent, 0.25), 0.07);
            p.stroke(&inner, accent, 0.035);
        }
    }
    // The ripple when a new number lands.
    let ripple = (1.0 - u / 0.3).max(0.0);
    if ripple > 0.0 {
        let grow = 1.0 - ripple;
        p.stroke(
            &circle(c.0, c.1, r + 0.02 + 0.05 * grow),
            fade(accent, 0.55 * ripple),
            0.014,
        );
    }
    let g = scaled(&digit("Poppins Bold", n, num_h, c), pop(u), c);
    p.paint(
        &g,
        &linear(
            (0.0, 0.35),
            (0.0, 0.65),
            &[(0.0, WHITE), (1.0, tint(arc, 0.55))],
        ),
    );
    p.bloom(1.0, 0.045, 0.8);
}

// ---------------------------------------------------------------------------
// Glitch.

/// A number sliced into bands that slip sideways, with red and cyan
/// fringes and scanlines. It glitches hard as it lands, then now and then.
pub fn glitch(pm: &mut Pixmap, ctx: &DrawCtx) {
    const BANDS: u32 = 14;
    let mut p = Pen::new(pm, ctx);
    let (face, fringe) = (p.c1, p.c2);
    let (n, u) = beat(p.t);
    let c = (p.w * 0.5, 0.5);
    let font = ["Anton", "Bebas Neue", "Black Ops One"][(ctx.variant as usize).min(2)];
    let g = scaled(&digit(font, n, 0.56, c), pop(u), c);
    let frame = (p.t * 30.0).floor() as u32;
    let burst = (1.0 - u / 0.25).max(0.0);
    let spike = if hash(frame.wrapping_mul(5).wrapping_add(n)) > 0.9 {
        0.8
    } else {
        0.0
    };
    let amount = burst.max(spike);
    let split = 0.012 + 0.03 * amount;
    let (top, bottom) = (0.1, 0.9);
    let band_h = (bottom - top) / BANDS as f32;
    for b in 0..BANDS {
        let jump = if hash(frame.wrapping_mul(31).wrapping_add(b * 7 + n)) > 0.55 {
            (hash(frame.wrapping_mul(17).wrapping_add(b * 3 + 1)) - 0.5) * 0.22 * amount
        } else {
            0.0
        };
        let strip = rrect(0.0, top + b as f32 * band_h, p.w, band_h * 1.02, 0.0);
        let Some(mask) = p.clip(&strip) else {
            continue;
        };
        for (dx, color, add) in [
            (-split, fringe, true),
            (split, rgb(1.0, 0.15, 0.35), true),
            (0.0, face, false),
        ] {
            let mut paint = solid(fade(color, if add { 0.9 } else { 1.0 }));
            if add {
                paint.blend_mode = BlendMode::Plus;
            }
            p.paint_masked(&moved(&g, jump + dx, 0.0), &paint, &mask);
        }
    }
    let mut lines = solid(fade(WHITE, 0.35));
    lines.blend_mode = BlendMode::DestinationOut;
    for k in 0..40 {
        let y = 0.1 + k as f32 * 0.02;
        p.stroke_with(&seg((0.0, y), (p.w, y)), &lines, &butt(0.006));
    }
}

// ---------------------------------------------------------------------------
// Glossy 3D.

/// A fat glossy number with depth and a white sticker rim. It drops in with
/// a squash, and a shine sweeps across it every second.
pub fn chunky(pm: &mut Pixmap, ctx: &DrawCtx) {
    blurred(pm, ctx, 0.03, chunky_frame);
}

fn chunky_frame(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx);
    let (face, side) = (p.c1, p.c2);
    let (n, u) = beat(p.t);
    let c = (p.w * 0.5, 0.48);
    let e = spring(u);
    let (px, py) = (c.0, c.1 + 0.25);
    let squash = Transform::from_translate(px, py)
        .pre_scale(1.0 - 0.08 * e, 1.0 + 0.12 * e)
        .pre_translate(-px, -py);
    let font = ["Poppins Black", "Bangers", "Righteous"][(ctx.variant as usize).min(2)];
    let shape = moved(&xf(&digit(font, n, 0.46, c), squash), 0.0, -0.05 * e);

    // Depth: the number stacked down and to the right, darker as it goes.
    let layers = 12;
    let deepest = moved(&shape, 0.0035 * layers as f32, 0.0055 * layers as f32);
    p.stroke(&deepest, ink(side), 0.03);
    for i in (1..=layers).rev() {
        let k = i as f32 / layers as f32;
        let part = moved(&shape, 0.0035 * i as f32, 0.0055 * i as f32);
        p.fill(&part, mix(side, shade(side, 0.5), k));
    }
    let body = linear(
        (0.0, c.1 - 0.28),
        (0.0, c.1 + 0.28),
        &[
            (0.0, tint(face, 0.7)),
            (0.45, face),
            (1.0, shade(face, 0.6)),
        ],
    );
    p.sticker(
        &[&shape],
        &body,
        Some((ink(face), 0.012)),
        Some((WHITE, 0.012)),
    );
    // Shine: a soft sheen on the upper half and a diagonal streak that
    // sweeps from left to right over the second.
    let clip = p.clip(&shape);
    p.fill_in(&ellipse(c.0, c.1 - 0.2, 0.2, 0.06), fade(WHITE, 0.4), &clip);
    let sx = c.0 - 0.5 + u * 1.0;
    p.fill_in(
        &poly(&[
            (sx - 0.06, c.1 - 0.35),
            (sx + 0.02, c.1 - 0.35),
            (sx - 0.1, c.1 + 0.35),
            (sx - 0.18, c.1 + 0.35),
        ]),
        fade(WHITE, 0.45),
        &clip,
    );
    p.shadow(0.35);
}
