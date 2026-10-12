//! Marks that draw themselves with a marker: arrows, loops, underlines, a
//! circle round something, a check, a cross, brackets, a spiral, a star and
//! a heart. Each is drawn on in under a second, held with a travelling
//! glint, then wiped away from its start, and the loop begins again.
//!
//! `ctx.variant` picks the mark. The main colour is the ink, the second
//! colour is the outline round it.

use super::kit::*;
use super::DrawCtx;
use std::f32::consts::{PI, TAU};
use tiny_skia::Pixmap;

const PERIOD: f32 = 3.4;
/// Seconds of the loop that have already passed when t = 0, so the first
/// frame shows the finished mark.
const LEAD: f32 = 0.9;
const DRAW: f32 = 0.8;

/// One mark: its strokes (each a list of points) and whether the last one
/// ends in an arrow head.
pub struct Shape {
    pub strokes: Vec<Vec<P>>,
    pub head: bool,
}

/// How wide each mark is (width / height). The catalog lists the same
/// numbers.
#[cfg(test)]
pub fn aspect(variant: u32) -> f32 {
    match variant {
        0..=3 => 1.6,
        4 | 12 | 13 => 2.6,
        5 => 1.7,
        8 => 1.3,
        11 => 1.1,
        14 => 1.2,
        _ => 1.0,
    }
}

fn map(w: f32, pts: &[P]) -> Vec<P> {
    pts.iter()
        .map(|&(u, v)| (0.16 + u * (w - 0.32), 0.16 + v * 0.68))
        .collect()
}

pub fn shape(variant: u32, w: f32) -> Shape {
    let smooth_map = |pts: &[P]| sample(&map(w, pts), false, 10);
    let c = (w * 0.5, 0.5);
    match variant {
        0 => Shape {
            strokes: vec![smooth_map(&[
                (0.0, 0.95),
                (0.22, 0.35),
                (0.5, 0.05),
                (0.78, 0.35),
                (1.0, 0.95),
            ])],
            head: true,
        },
        1 => Shape {
            strokes: vec![smooth_map(&[
                (0.0, 0.9),
                (0.3, 0.35),
                (0.68, 0.1),
                (1.0, 0.35),
            ])],
            head: true,
        },
        2 => Shape {
            strokes: vec![smooth_map(&[
                (0.0, 0.9),
                (0.25, 0.7),
                (0.52, 0.55),
                (0.64, 0.25),
                (0.5, 0.05),
                (0.36, 0.25),
                (0.5, 0.55),
                (0.78, 0.62),
                (1.0, 0.4),
            ])],
            head: true,
        },
        3 => Shape {
            strokes: vec![map(
                w,
                &[
                    (0.0, 0.85),
                    (0.2, 0.15),
                    (0.4, 0.85),
                    (0.6, 0.15),
                    (0.8, 0.85),
                    (1.0, 0.4),
                ],
            )],
            head: true,
        },
        4 => {
            let n = 9;
            let pts: Vec<P> = (0..=n)
                .map(|i| (i as f32 / n as f32, if i % 2 == 0 { 0.3 } else { 0.7 }))
                .collect();
            Shape {
                strokes: vec![map(w, &pts)],
                head: false,
            }
        }
        5 => {
            let pts: Vec<P> = (0..=48)
                .map(|i| {
                    let a = -2.2 + i as f32 / 48.0 * (TAU + 0.5);
                    let r = 1.0 + 0.07 * (a + 2.2) / TAU;
                    (
                        c.0 + 0.7 * (w * 0.5 - 0.18) * r * a.cos(),
                        c.1 + 0.34 * r * a.sin(),
                    )
                })
                .collect();
            Shape {
                strokes: vec![pts],
                head: false,
            }
        }
        6 => Shape {
            strokes: vec![sample(
                &map(w, &[(0.0, 0.55), (0.3, 0.9), (1.0, 0.0)]),
                false,
                8,
            )],
            head: false,
        },
        7 => Shape {
            strokes: vec![
                map(w, &[(0.05, 0.05), (0.95, 0.95)]),
                map(w, &[(0.95, 0.05), (0.05, 0.95)]),
            ],
            head: false,
        },
        8 => {
            let x0 = 0.2;
            let x1 = w - 0.2;
            Shape {
                strokes: vec![
                    vec![(x0 + 0.14, 0.14), (x0, 0.14), (x0, 0.86), (x0 + 0.14, 0.86)],
                    vec![(x1 - 0.14, 0.14), (x1, 0.14), (x1, 0.86), (x1 - 0.14, 0.86)],
                ],
                head: false,
            }
        }
        9 => {
            let pts: Vec<P> = (0..=64)
                .map(|i| {
                    let f = i as f32 / 64.0;
                    let a = f * 4.2 * PI;
                    let r = 0.04 + 0.32 * f;
                    (c.0 + r * a.cos(), c.1 + r * a.sin())
                })
                .collect();
            Shape {
                strokes: vec![pts],
                head: false,
            }
        }
        10 => {
            let v = star_pts(c.0, c.1 + 0.03, 0.34, 0.1, 5, 0.0);
            let outer: Vec<P> = (0..5).map(|k| v[k * 2]).collect();
            let order = [0usize, 2, 4, 1, 3, 0];
            Shape {
                strokes: vec![order.iter().map(|&i| outer[i]).collect()],
                head: false,
            }
        }
        11 => {
            let pts: Vec<P> = (0..=48)
                .map(|i| {
                    let a = i as f32 / 48.0 * TAU;
                    let x = 16.0 * a.sin().powi(3);
                    let y = 13.0 * a.cos()
                        - 5.0 * (2.0 * a).cos()
                        - 2.0 * (3.0 * a).cos()
                        - (4.0 * a).cos();
                    (c.0 + x * 0.0215, c.1 - y * 0.0215 - 0.02)
                })
                .collect();
            Shape {
                strokes: vec![pts],
                head: false,
            }
        }
        12 => Shape {
            strokes: vec![
                sample(&map(w, &[(0.0, 0.3), (0.5, 0.2), (1.0, 0.3)]), false, 8),
                sample(
                    &map(w, &[(0.05, 0.75), (0.5, 0.65), (0.95, 0.75)]),
                    false,
                    8,
                ),
            ],
            head: false,
        },
        13 => {
            let pts: Vec<P> = (0..=60)
                .map(|i| {
                    let u = i as f32 / 60.0;
                    (u, 0.5 + 0.3 * (u * 5.0 * TAU).sin())
                })
                .collect();
            Shape {
                strokes: vec![map(w, &pts)],
                head: false,
            }
        }
        _ => {
            // Three short strokes fanning out: "look here".
            let mut strokes = Vec::new();
            for a in [-50.0f32, 0.0, 50.0] {
                let r = (a - 90.0).to_radians();
                let (ca, sa) = (r.cos(), r.sin());
                strokes.push(vec![
                    (c.0 + ca * 0.18, c.1 + 0.28 + sa * 0.18),
                    (c.0 + ca * 0.42, c.1 + 0.28 + sa * 0.42),
                ]);
            }
            Shape {
                strokes,
                head: false,
            }
        }
    }
}

/// Total length of all strokes.
fn total_len(s: &Shape) -> f32 {
    s.strokes
        .iter()
        .map(|st| st.windows(2).map(|w| dist(w[0], w[1])).sum::<f32>())
        .sum()
}

/// The part of the strokes between arc lengths `a` and `b` (over all
/// strokes laid end to end), as polylines.
fn slice(s: &Shape, a: f32, b: f32) -> Vec<Vec<P>> {
    let mut out = Vec::new();
    let mut at = 0.0f32;
    for st in &s.strokes {
        let mut cur: Vec<P> = Vec::new();
        for w in st.windows(2) {
            let l = dist(w[0], w[1]);
            let (s0, s1) = (at, at + l);
            at = s1;
            if s1 <= a || s0 >= b || l <= 1e-9 {
                continue;
            }
            let k0 = ((a - s0) / l).clamp(0.0, 1.0);
            let k1 = ((b - s0) / l).clamp(0.0, 1.0);
            let (p0, p1) = (lerp(w[0], w[1], k0), lerp(w[0], w[1], k1));
            if cur.is_empty() {
                cur.push(p0);
            }
            cur.push(p1);
        }
        if cur.len() >= 2 {
            out.push(cur);
        }
    }
    out
}

/// The point at arc length `d` and the direction of travel there.
fn at_length(s: &Shape, d: f32) -> Option<(P, P)> {
    let mut at = 0.0f32;
    for st in &s.strokes {
        for w in st.windows(2) {
            let l = dist(w[0], w[1]);
            if d <= at + l && l > 1e-9 {
                let k = ((d - at) / l).clamp(0.0, 1.0);
                return Some((
                    lerp(w[0], w[1], k),
                    norm((w[1].0 - w[0].0, w[1].1 - w[0].1)),
                ));
            }
            at += l;
        }
    }
    None
}

/// Draws the mark chosen by `ctx.variant`.
pub fn draw(pm: &mut Pixmap, ctx: &DrawCtx) {
    let mut p = Pen::new(pm, ctx).inset(0.86);
    let u = (p.t + LEAD).rem_euclid(PERIOD);
    let shape = shape(ctx.variant, p.w);
    let total = total_len(&shape).max(1e-6);
    let grow = ease_out((u / DRAW).clamp(0.0, 1.0));
    let wipe = smoothstep(2.8, 3.3, u);
    let (a, b) = (wipe * total, grow * total);
    let parts = slice(&shape, a, b);
    let width = 0.052;
    let (ink_c, edge_c) = (p.c1, p.c2);
    let head_len = 0.17;
    let head_k = smoothstep(0.88, 1.0, grow) * (1.0 - wipe);
    let mut head_paths = Vec::new();
    if shape.head && head_k > 0.0 {
        if let Some((tip, dir)) = at_length(&shape, total - 1e-4) {
            for side in [-1.0f32, 1.0] {
                let ang = 150.0f32.to_radians() * side;
                let (c, s) = (ang.cos(), ang.sin());
                let d = (dir.0 * c - dir.1 * s, dir.0 * s + dir.1 * c);
                head_paths.push(line(&[
                    (
                        tip.0 + d.0 * head_len * head_k,
                        tip.1 + d.1 * head_len * head_k,
                    ),
                    tip,
                ]));
            }
        }
    }
    // The outline first, then the ink on top.
    for part in &parts {
        p.stroke(&line(part), edge_c, width + 0.036);
    }
    for h in &head_paths {
        p.stroke(h, edge_c, width + 0.036);
    }
    for part in &parts {
        p.stroke(&line(part), ink_c, width);
    }
    for h in &head_paths {
        p.stroke(h, ink_c, width);
    }
    // A glint sliding along the finished part.
    if grow > 0.99 && wipe < 0.01 {
        let g = (p.t * 0.45).rem_euclid(1.0) * total;
        if let Some((pt, _)) = at_length(&shape, g) {
            p.fill(&circle(pt.0, pt.1, 0.02), fade(WHITE, 0.85));
        }
    }
    p.shadow(0.4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mark_fits_its_picture_and_has_length() {
        for v in 0..15 {
            let w = aspect(v);
            let s = shape(v, w);
            assert!(total_len(&s) > 0.5, "mark {v} is too short");
            for st in &s.strokes {
                for &(x, y) in st {
                    assert!(
                        x > 0.05 && x < w - 0.05 && y > 0.05 && y < 0.95,
                        "mark {v} leaves its picture at ({x}, {y})"
                    );
                }
            }
        }
    }
}
