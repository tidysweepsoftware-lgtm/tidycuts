//! What each title style adds: how it lays out lines (heading + subtitle,
//! label above), the text fill and effects (glow, extrusion, glitch…) and
//! the vector decorations drawn behind or in front of the text, all
//! animated by `reveal` (0 = not shown yet, 1 = fully shown).

use super::layout::{Layout, LineConf};
use super::raster::{circle, partial_polyline, polygon, rect, rrect};
use tidycuts_core::{TextAlign, TitleSpec};
use tiny_skia::{LineCap, LineJoin, Path};

pub type Rgba = [f32; 4];

/// One shape of a style's decoration, in text-block coordinates.
pub struct Op {
    pub path: Path,
    pub color: Rgba,
    /// Stroked with this width and cap instead of filled.
    pub stroke: Option<(f32, LineCap)>,
    /// Drawn over the text rather than under it.
    pub front: bool,
}

/// How the letters themselves are painted.
#[derive(Clone, Copy)]
pub enum Fill {
    Solid,
    /// Vertical gradient from `color` (top) to this color (bottom) per line.
    Gradient(Rgba),
    /// Top half `color`, bottom half `accent`, moved apart by `offset` px.
    Split {
        offset: f32,
    },
    /// Only the outline of the letters, this wide.
    Hollow(f32),
}

pub struct Glow {
    pub radius: f32,
    pub color: Rgba,
    /// Multiplies the blurred alpha so the soft edge still reads.
    pub boost: f32,
}

/// The text pushed back along the diagonal (1, 1).
pub struct Extrude {
    /// Length in output pixels along each axis.
    pub len: f32,
    pub near: Rgba,
    pub far: Rgba,
    /// Transparent at the far end.
    pub fade: bool,
}

pub struct Glitch {
    /// Offset of the cyan and red copies.
    pub split: f32,
    /// Sideways shift of the sliced bands.
    pub slice: f32,
}

pub struct Effects {
    pub fill: Fill,
    pub glows: Vec<Glow>,
    pub extrude: Option<Extrude>,
    pub glitch: Option<Glitch>,
    /// Extra room around the text for animated states that move beyond
    /// the fully revealed one, in output pixels.
    pub margin: f32,
}

fn seg(r: f32, a: f32, b: f32) -> f32 {
    ((r - a) / (b - a)).clamp(0.0, 1.0)
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}

fn darken(c: Rgba, k: f32) -> Rgba {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// Black or white, whichever reads better on `c`.
pub fn contrast(c: Rgba) -> Rgba {
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    if l > 0.6 {
        [0.07, 0.07, 0.09, 1.0]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

/// A cheap repeatable pseudo-random number in -1..1.
fn hash(i: i32) -> f32 {
    let x = (i as f32 * 12.9898 + 78.233).sin() * 43758.547;
    x.fract() * 2.0 - 1.0
}

/// Uses `background_color` for panels, falling back to a dark tone when the
/// user made it invisible.
fn panel_color(spec: &TitleSpec) -> Rgba {
    if spec.background_color[3] < 0.05 {
        [0.06, 0.07, 0.09, 0.85]
    } else {
        spec.background_color
    }
}

/// Layout of line `i` of `n` for a style.
pub fn line_conf(style: &str, i: usize, n: usize) -> LineConf {
    let mut c = LineConf::default();
    match style {
        "lower_bar" | "callout" => {
            if i > 0 {
                c.factor = 0.56;
                c.extra_spacing = 0.04;
            } else if n > 1 {
                c.gap_after = 0.04;
            }
        }
        "lower_slant" => {
            if i > 0 {
                c.factor = 0.5;
                c.extra_spacing = 0.05;
            } else if n > 1 {
                c.gap_after = 0.3;
            }
        }
        "tag" if n > 1 && i == 0 => {
            c.factor = 0.45;
            c.extra_spacing = 0.12;
            c.gap_after = 0.24;
        }
        "subtitle" if n > 1 => c.gap_after = 0.12,
        "notification" => {
            if i > 0 {
                c.factor = 0.7;
            } else if n > 1 {
                c.gap_after = 0.1;
            }
        }
        "stack" => {
            if i > 0 {
                c.factor = 0.32;
                c.extra_spacing = 0.28;
            } else if n > 1 {
                c.gap_after = 0.1;
            }
        }
        "refined" => {
            c.extra_spacing = 0.4;
            if i > 0 {
                c.factor = 0.55;
            }
        }
        "editorial" => {
            if i > 0 {
                c.factor = 0.42;
                c.extra_spacing = 0.12;
            } else if n > 1 {
                c.gap_after = 0.12;
            }
        }
        "quote" if n > 1 => {
            if i == 0 {
                c.factor = 2.4;
                c.gap_after = -0.32;
            } else if i + 1 == n && n > 2 {
                c.factor = 0.4;
                c.extra_spacing = 0.12;
                c.gap_after = 0.0;
            }
        }
        _ => {}
    }
    c
}

/// Text color of line `i` (the tag label reads on the accent pill).
pub fn line_color(spec: &TitleSpec, lay: &Layout, i: usize) -> Rgba {
    if spec.style == "tag" && lay.lines.len() > 1 && i == 0 {
        return contrast(spec.accent);
    }
    let n = lay.lines.len();
    match spec.style.as_str() {
        "notification" | "editorial" if i > 0 => with_alpha(spec.color, 0.72),
        "stack" if i > 0 => spec.accent,
        "quote" if n > 1 && i == 0 => spec.accent,
        "quote" if n > 2 && i + 1 == n => with_alpha(spec.color, 0.75),
        _ => spec.color,
    }
}

/// Neon tubes stutter before they light up.
fn flicker(r: f32) -> f32 {
    match r {
        r if r >= 0.55 => 1.0,
        r if r < 0.12 => 0.0,
        r if r < 0.22 => 0.85,
        r if r < 0.3 => 0.1,
        r if r < 0.42 => 1.0,
        r if r < 0.48 => 0.25,
        _ => 1.0,
    }
}

pub fn effects(spec: &TitleSpec, em: f32, reveal: f32) -> Effects {
    let e = ease(reveal);
    let mut fx = Effects {
        fill: Fill::Solid,
        glows: Vec::new(),
        extrude: None,
        glitch: None,
        margin: 0.0,
    };
    match spec.style.as_str() {
        "neon" => {
            let f = flicker(reveal);
            fx.glows.push(Glow {
                radius: 0.3 * em,
                color: with_alpha(spec.accent, 0.9 * f),
                boost: 2.6,
            });
            fx.glows.push(Glow {
                radius: 0.06 * em,
                color: with_alpha(spec.accent, f),
                boost: 2.2,
            });
        }
        "long_shadow" => {
            fx.extrude = Some(Extrude {
                len: 0.6 * em * e,
                near: spec.accent,
                far: darken(spec.accent, 0.7),
                fade: true,
            });
        }
        "retro_3d" => {
            fx.extrude = Some(Extrude {
                len: 0.15 * em * e,
                near: spec.accent,
                far: darken(spec.accent, 0.45),
                fade: false,
            });
        }
        "glitch" => {
            let j = if reveal >= 1.0 {
                1.0
            } else {
                1.0 + 2.5 * (1.0 - reveal) * hash((reveal * 14.0) as i32)
            };
            fx.glitch = Some(Glitch {
                split: 0.045 * em * j,
                slice: 0.07 * em * j,
            });
            fx.margin = 0.22 * em;
        }
        "gradient" => fx.fill = Fill::Gradient(lerp(spec.color, spec.accent, e)),
        "gold" => {
            fx.fill = Fill::Gradient(spec.accent);
            fx.extrude = Some(Extrude {
                len: 0.05 * em * e,
                near: darken(spec.accent, 0.62),
                far: darken(spec.accent, 0.3),
                fade: false,
            });
            fx.glows.push(Glow {
                radius: 0.12 * em,
                color: with_alpha(spec.accent, 0.35 * e),
                boost: 1.6,
            });
        }
        "split" => {
            fx.fill = Fill::Split {
                offset: (1.0 - e) * 0.15 * em,
            };
            fx.margin = 0.16 * em;
        }
        "stroke_only" => fx.fill = Fill::Hollow(0.04 * em),
        _ => {}
    }
    fx
}

struct Deco {
    ops: Vec<Op>,
}

impl Deco {
    fn fill(&mut self, path: Option<Path>, color: Rgba, front: bool) {
        if let Some(path) = path {
            if color[3] > 0.0 {
                self.ops.push(Op {
                    path,
                    color,
                    stroke: None,
                    front,
                });
            }
        }
    }

    fn stroke(&mut self, path: Option<Path>, color: Rgba, width: f32, cap: LineCap) {
        if let Some(path) = path {
            if color[3] > 0.0 && width > 0.0 {
                self.ops.push(Op {
                    path,
                    color,
                    stroke: Some((width, cap)),
                    front: true,
                });
            }
        }
    }
}

pub const JOIN: LineJoin = LineJoin::Miter;

/// The decoration shapes of `spec.style` at `reveal`.
pub fn decorations(spec: &TitleSpec, lay: &Layout, em: f32, reveal: f32) -> Vec<Op> {
    let mut d = Deco { ops: Vec::new() };
    let (w, h) = (lay.width, lay.height);
    let r = reveal;
    let accent = spec.accent;
    let n = lay.lines.len();
    match spec.style.as_str() {
        "underline" => {
            let t = 0.085 * em;
            let y = h + 0.2 * em;
            let len = w * ease(r);
            let x = match spec.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (w - len) * 0.5,
                TextAlign::Right => w - len,
            };
            d.fill(rrect(x, y, len, t, t * 0.3), accent, true);
        }
        "highlight" => {
            let span = if n == 1 { 1.0 } else { 0.6 };
            for (i, l) in lay.lines.iter().enumerate() {
                if l.width <= 0.0 {
                    continue;
                }
                let start = if n == 1 {
                    0.0
                } else {
                    i as f32 * (1.0 - span) / (n - 1) as f32
                };
                let p = ease(seg(r, start, start + span));
                let e = l.em;
                let x0 = l.x - 0.14 * e;
                let full = l.width + 0.28 * e;
                let x1 = x0 + full * p;
                let y0 = l.baseline - 0.62 * l.cap();
                let y1 = l.baseline + 0.16 * e;
                let s = (0.07 * e).min((x1 - x0) * 0.5);
                if x1 - x0 > 0.5 {
                    d.fill(
                        polygon(&[
                            (x0 + s, y0),
                            (x1, y0 - 0.02 * e),
                            (x1 - s, y1),
                            (x0, y1 + 0.02 * e),
                        ]),
                        accent,
                        false,
                    );
                }
            }
        }
        "box" => {
            let (px, py) = (0.42 * em, 0.24 * em);
            let bw = (w + 2.0 * px) * ease(r);
            let bh = (h + 2.0 * py) * (0.35 + 0.65 * ease(seg(r, 0.0, 0.6)));
            d.fill(
                rrect(w * 0.5 - bw * 0.5, h * 0.5 - bh * 0.5, bw, bh, 0.05 * em),
                accent,
                false,
            );
        }
        "outline_box" => {
            let t = 0.065 * em;
            let (x0, y0, x1, y1) = (-0.45 * em, -0.3 * em, w + 0.45 * em, h + 0.3 * em);
            if r >= 0.999 {
                d.stroke(
                    polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]),
                    accent,
                    t,
                    LineCap::Square,
                );
            } else {
                let p = ease(r);
                d.stroke(
                    partial_polyline(&[(x0, y0), (x1, y0), (x1, y1)], p),
                    accent,
                    t,
                    LineCap::Square,
                );
                d.stroke(
                    partial_polyline(&[(x1, y1), (x0, y1), (x0, y0)], p),
                    accent,
                    t,
                    LineCap::Square,
                );
            }
        }
        "pill" => {
            let (pw, ph) = (w + 1.1 * em, h + 0.5 * em);
            let s = ease(seg(r, 0.0, 0.3));
            let hh = ph * s;
            let ww = hh + (pw - ph).max(0.0) * ease(seg(r, 0.2, 1.0));
            d.fill(
                rrect(w * 0.5 - ww * 0.5, h * 0.5 - hh * 0.5, ww, hh, hh * 0.5),
                accent,
                false,
            );
        }
        "lower_bar" => {
            let (py, gap, bar, pr) = (0.26 * em, 0.34 * em, 0.11 * em, 0.55 * em);
            let (y0, y1) = (-py, h + py);
            let bh = (y1 - y0) * ease(seg(r, 0.0, 0.45));
            let cy = (y0 + y1) * 0.5;
            let pw = (w + pr + gap) * ease(seg(r, 0.3, 1.0));
            d.fill(rect(-gap, y0, pw, y1 - y0), panel_color(spec), false);
            d.fill(rect(-gap - bar, cy - bh * 0.5, bar, bh), accent, false);
        }
        "lower_slant" => {
            let l0 = &lay.lines[0];
            let e0 = l0.em;
            let (y0, y1) = (l0.top - 0.22 * e0, l0.bottom + 0.22 * e0);
            let s0 = (y1 - y0) * 0.3;
            let bl = l0.x - 0.32 * e0 - s0;
            let br = l0.x + l0.width + 0.32 * e0;
            let p0 = ease(seg(r, 0.0, 0.6));
            let br = bl + (br - bl) * p0;
            if p0 > 0.0 {
                d.fill(
                    polygon(&[(bl + s0, y0), (br + s0, y0), (br, y1), (bl, y1)]),
                    accent,
                    false,
                );
            }
            if n > 1 {
                let rest = &lay.lines[1..];
                let e1 = rest[0].em;
                let lx = rest.iter().map(|l| l.x).fold(f32::INFINITY, f32::min);
                let rx = rest.iter().map(|l| l.x + l.width).fold(0.0, f32::max);
                let bottom = rest.iter().map(|l| l.bottom).fold(0.0, f32::max);
                let (z0, z1) = (y1, bottom + 0.3 * e1);
                let s1 = (z1 - z0) * 0.3;
                let tl = lx - 0.35 * e1;
                let bl1 = tl - s1;
                let br1 = rx + 0.4 * e1;
                let p1 = ease(seg(r, 0.3, 1.0));
                let br1 = bl1 + (br1 - bl1) * p1;
                if p1 > 0.0 {
                    d.fill(
                        polygon(&[(bl1 + s1, z0), (br1 + s1, z0), (br1, z1), (bl1, z1)]),
                        panel_color(spec),
                        false,
                    );
                }
            }
        }
        "callout" => {
            let mirror = spec.align == TextAlign::Right;
            let fx = |x: f32| if mirror { w - x } else { x };
            let y = h + 0.26 * em;
            let s = (fx(-0.2 * em), y);
            let e = (fx(w + 0.15 * em), y);
            let dot = (fx(-0.95 * em), y + 0.75 * em);
            let t = 0.07 * em;
            let pd = ease(seg(r, 0.0, 0.25));
            d.fill(circle(dot.0, dot.1, 0.14 * em * pd), accent, true);
            if let Some(ring) = circle(dot.0, dot.1, 0.28 * em * pd) {
                d.stroke(
                    Some(ring),
                    with_alpha(accent, 0.6),
                    0.04 * em,
                    LineCap::Butt,
                );
            }
            d.stroke(
                partial_polyline(&[dot, s, e], ease(seg(r, 0.15, 1.0))),
                accent,
                t,
                LineCap::Butt,
            );
        }
        "brackets" => {
            let t = 0.08 * em;
            let arm = (0.38 * em).min(w * 0.3 + 0.12 * em);
            let (x0, x1, y0, y1) = (-0.42 * em, w + 0.42 * em, -0.28 * em, h + 0.28 * em);
            let cy = (y0 + y1) * 0.5;
            let p = ease(r);
            for (x, dir) in [(x0, 1.0), (x1, -1.0)] {
                d.stroke(
                    partial_polyline(&[(x, cy), (x, y0), (x + arm * dir, y0)], p),
                    accent,
                    t,
                    LineCap::Square,
                );
                d.stroke(
                    partial_polyline(&[(x, cy), (x, y1), (x + arm * dir, y1)], p),
                    accent,
                    t,
                    LineCap::Square,
                );
            }
        }
        "double_line" => {
            let t = 0.06 * em;
            let ext = 0.3 * em;
            let (x0, len) = (-ext, w + 2.0 * ext);
            let p = ease(r);
            d.fill(rect(x0, -0.36 * em - t, len * p, t), accent, true);
            d.fill(
                rect(x0 + len * (1.0 - p), h + 0.3 * em, len * p, t),
                accent,
                true,
            );
        }
        "subtitle" => {
            let c = with_alpha(panel_color(spec), ease(r));
            let mut pb = tiny_skia::PathBuilder::new();
            for l in &lay.lines {
                if l.width <= 0.0 {
                    continue;
                }
                let e = l.em;
                if let Some(p) = rrect(
                    l.x - 0.3 * e,
                    l.top - 0.2 * e,
                    l.width + 0.6 * e,
                    l.bottom - l.top + 0.42 * e,
                    0.1 * e,
                ) {
                    pb.push_path(&p);
                }
            }
            d.fill(pb.finish(), c, false);
        }
        "glass" => {
            let (px, py) = (0.55 * em, 0.32 * em);
            let a = ease(r);
            let bw = (w + 2.0 * px) * a;
            let bh = (h + 2.0 * py) * (0.4 + 0.6 * ease(seg(r, 0.0, 0.6)));
            let (x, y, rad) = (w * 0.5 - bw * 0.5, h * 0.5 - bh * 0.5, 0.22 * em);
            d.fill(rrect(x, y, bw, bh, rad), [1.0, 1.0, 1.0, 0.13 * a], false);
            d.fill(
                rrect(x, y, bw, bh * 0.5, rad),
                [1.0, 1.0, 1.0, 0.06 * a],
                false,
            );
            d.stroke(
                rrect(x, y, bw, bh, rad),
                [1.0, 1.0, 1.0, 0.5 * a],
                0.014 * em,
                LineCap::Butt,
            );
        }
        "notification" => {
            let a = ease(r);
            let e = lay.lines[0].em;
            let (icon, pad) = (1.05 * e, 0.42 * e);
            let x0 = -(icon + pad * 1.6);
            let (y0, y1) = (-0.42 * e, h + 0.42 * e);
            let card_w = (w + pad - x0) * a;
            d.fill(
                rrect(x0, y0, card_w, y1 - y0, 0.34 * e),
                with_alpha(panel_color(spec), a),
                false,
            );
            let (cx, cy) = (x0 + pad + icon * 0.5, (y0 + y1) * 0.5);
            d.fill(circle(cx, cy, icon * 0.5 * a), accent, false);
            // A speech bubble on the badge.
            d.fill(
                rrect(
                    cx - icon * 0.26,
                    cy - icon * 0.2,
                    icon * 0.52 * a,
                    icon * 0.34,
                    icon * 0.1,
                ),
                [1.0, 1.0, 1.0, a],
                false,
            );
            d.fill(
                polygon(&[
                    (cx - icon * 0.14, cy + icon * 0.13),
                    (cx - icon * 0.02, cy + icon * 0.13),
                    (cx - icon * 0.16, cy + icon * 0.27),
                ]),
                [1.0, 1.0, 1.0, a],
                false,
            );
        }
        "stack" => {
            let l = &lay.lines[0];
            let bw = 0.9 * l.em * ease(r);
            let x = match spec.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (w - bw) * 0.5,
                TextAlign::Right => w - bw,
            };
            d.fill(rect(x, l.top - 0.14 * l.em, bw, 0.07 * l.em), accent, false);
        }
        "refined" => {
            let len = w * 0.3 * ease(r);
            let t = (0.012 * em).max(1.0);
            d.fill(rect((w - len) * 0.5, h + 0.42 * em, len, t), accent, true);
        }
        "editorial" => {
            let ext = 0.4 * em;
            let len = (w + 2.0 * ext) * ease(r);
            let x = (w - len) * 0.5;
            d.fill(rect(x, -0.46 * em, len, 0.05 * em), accent, true);
            d.fill(rect(x, -0.33 * em, len, 0.014 * em), accent, true);
            d.fill(rect(x, h + 0.3 * em, len, 0.014 * em), accent, true);
        }
        "tag" => {
            let l = &lay.lines[0];
            if n > 1 {
                let e = l.em;
                let pw = l.width + 0.9 * e;
                d.fill(
                    rrect(
                        l.x - 0.45 * e,
                        l.top - 0.4 * e,
                        pw * ease(seg(r, 0.0, 0.6)),
                        l.bottom - l.top + 0.8 * e,
                        0.22 * e,
                    ),
                    accent,
                    false,
                );
            } else {
                let bw = 0.9 * em * ease(r);
                let x = match spec.align {
                    TextAlign::Left => 0.0,
                    TextAlign::Center => (w - bw) * 0.5,
                    TextAlign::Right => w - bw,
                };
                d.fill(rect(x, -0.34 * em, bw, 0.09 * em), accent, false);
            }
        }
        _ => {}
    }
    d.ops
}
