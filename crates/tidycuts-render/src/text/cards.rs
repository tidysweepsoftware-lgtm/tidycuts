//! Card titles: a lower third or label that builds itself in stages, the
//! way current motion-graphics templates do. A thin line draws, a card wipes
//! open behind a bright leading edge with a few sparks, its gradient keeps
//! flowing while it opens, and only then does the text arrive. At rest there
//! is nothing left but the card and the text.
//!
//! Everything is a function of `reveal` (0..1), so any moment of the build
//! can be drawn on its own. The text itself is drawn afterwards by the normal
//! title code, with the soft letter animation (see [`text_progress`]).

use super::raster;
use crate::motion::{ease_out_expo, smoothstep};
use tidycuts_core::TitleSpec;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap, Point,
    RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};

type C = [f32; 4];

/// The card styles.
pub fn is_card(style: &str) -> bool {
    matches!(style, "card_gradient" | "card_bar" | "card_pill")
}

/// How far through its own animation the text is, for a card at `reveal`:
/// it starts once the card is mostly open.
pub fn text_progress(reveal: f32) -> f32 {
    smoothstep(0.38, 0.98, reveal)
}

/// The card's rectangle around a text block of `w` x `h` pixels, in block
/// coordinates.
pub fn rect(style: &str, em: f32, w: f32, h: f32) -> Rect {
    let (left, right) = match style {
        "card_pill" => (1.0 * em, 0.7 * em),
        "card_bar" => (0.75 * em, 0.7 * em),
        _ => (0.6 * em, 0.6 * em),
    };
    Rect::from_xywh(-left, -0.34 * em, w + left + right, h + 0.68 * em)
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())
}

/// Extra room the card's shadow and sparks need around its rectangle.
pub fn margin(em: f32) -> f32 {
    0.5 * em
}

fn solid(c: C) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
}

fn color(c: C) -> Color {
    Color::from_rgba(
        c[0].clamp(0.0, 1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
        c[3].clamp(0.0, 1.0),
    )
    .unwrap_or(Color::TRANSPARENT)
}

fn shader_paint(shader: Option<Shader<'static>>, fallback: C) -> Paint<'static> {
    match shader {
        Some(shader) => Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        },
        None => solid(fallback),
    }
}

fn linear(a: (f32, f32), b: (f32, f32), stops: &[(f32, C)]) -> Paint<'static> {
    let sh = LinearGradient::new(
        Point::from_xy(a.0, a.1),
        Point::from_xy(b.0, b.1),
        stops
            .iter()
            .map(|(p, c)| GradientStop::new(*p, color(*c)))
            .collect(),
        SpreadMode::Pad,
        Transform::identity(),
    );
    shader_paint(sh, stops[0].1)
}

fn radial(c: (f32, f32), r: f32, stops: &[(f32, C)]) -> Paint<'static> {
    let sh = RadialGradient::new(
        Point::from_xy(c.0, c.1),
        Point::from_xy(c.0, c.1),
        r.max(1e-3),
        stops
            .iter()
            .map(|(p, c)| GradientStop::new(*p, color(*c)))
            .collect(),
        SpreadMode::Pad,
        Transform::identity(),
    );
    shader_paint(sh, stops[0].1)
}

fn tint(c: C, k: f32) -> C {
    [
        c[0] + (1.0 - c[0]) * k,
        c[1] + (1.0 - c[1]) * k,
        c[2] + (1.0 - c[2]) * k,
        c[3],
    ]
}

fn alpha(c: C, a: f32) -> C {
    [c[0], c[1], c[2], c[3] * a]
}

/// `c` with its hue turned by `deg` degrees (a rotation in YIQ colour space).
fn hue_shift(c: C, deg: f32) -> C {
    let (s, co) = deg.to_radians().sin_cos();
    let (r, g, b) = (c[0], c[1], c[2]);
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let i = 0.596 * r - 0.274 * g - 0.322 * b;
    let q = 0.211 * r - 0.523 * g + 0.312 * b;
    let (i2, q2) = (i * co - q * s, i * s + q * co);
    [
        (y + 0.956 * i2 + 0.621 * q2).clamp(0.0, 1.0),
        (y - 0.272 * i2 - 0.647 * q2).clamp(0.0, 1.0),
        (y - 1.106 * i2 + 1.703 * q2).clamp(0.0, 1.0),
        c[3],
    ]
}

fn hash(i: u32) -> f32 {
    crate::noise::hash3(i as i32, 0, 0, 0x51)
}

/// Where a stage that runs from `a` to `b` is at `reveal`, 0..1.
fn stage(reveal: f32, a: f32, b: f32) -> f32 {
    ((reveal - a) / (b - a)).clamp(0.0, 1.0)
}

fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    raster::rrect(x, y, w, h, r)
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    Rect::from_xywh(x, y, w.max(0.01), h.max(0.01)).map(PathBuilder::from_rect)
}

/// Draws the card for `spec` at `reveal` behind a text block of `w` x `h`
/// pixels. `origin` places the block on the canvas.
pub fn draw(
    canvas: &mut Pixmap,
    origin: Transform,
    spec: &TitleSpec,
    em: f32,
    w: f32,
    h: f32,
    reveal: f32,
) {
    let r = reveal.clamp(0.0, 1.0);
    let rc = rect(&spec.style, em, w, h);
    let (x0, y0, rw, rh) = (rc.left(), rc.top(), rc.width(), rc.height());
    let accent = spec.accent;
    let wipe = ease_out_expo(stage(r, 0.1, 0.58));
    let radius = match spec.style.as_str() {
        "card_pill" => rh * 0.5,
        "card_bar" => 0.06 * em,
        _ => (0.22 * em).min(rh * 0.5),
    };
    let Some(card) = rrect(x0, y0, rw, rh, radius) else {
        return;
    };

    // The opening: from the left edge, or out from the middle for the pill.
    let (edge_l, edge_r) = if spec.style == "card_pill" {
        let mid = x0 + rw * 0.5;
        (mid - wipe * rw * 0.5, mid + wipe * rw * 0.5)
    } else {
        (x0, x0 + wipe * rw)
    };
    // Everything is clipped to what has opened so far; once it is nearly
    // open the clip loosens a little so the soft shadow is not cut off.
    let ext = 0.3 * em * smoothstep(0.85, 1.0, wipe);
    let (open_l, open_r) = (edge_l - ext, edge_r + ext);
    let mut mask = Mask::new(canvas.width(), canvas.height());
    if let (Some(m), Some(rp)) = (
        mask.as_mut(),
        rect_path(open_l, y0 - em, open_r - open_l, rh + 2.0 * em),
    ) {
        m.fill_path(&rp, FillRule::Winding, true, origin);
    }
    let mask = mask.as_ref();
    let fill = |canvas: &mut Pixmap, path: &Path, paint: &Paint| {
        canvas.fill_path(path, paint, FillRule::Winding, origin, mask);
    };

    // A soft shadow under the card: a few widening, fainter layers.
    for k in 0..6 {
        let e = k as f32 * 0.035 * em;
        if let Some(p) = rrect(
            x0 - e,
            y0 + 0.1 * em - e,
            rw + 2.0 * e,
            rh + 2.0 * e,
            radius + e,
        ) {
            fill(canvas, &p, &solid([0.0, 0.0, 0.0, 0.07]));
        }
    }

    match spec.style.as_str() {
        "card_bar" => {
            fill(canvas, &card, &solid([0.05, 0.06, 0.09, 0.78]));
            if let Some(top) = rect_path(x0, y0, rw, 0.025 * em) {
                fill(canvas, &top, &solid([1.0, 1.0, 1.0, 0.16]));
            }
            // The accent slab at the left grows from the middle first.
            let grow = ease_out_expo(stage(r, 0.0, 0.3));
            let sh = rh * grow;
            if let Some(slab) = rect_path(x0, y0 + (rh - sh) * 0.5, 0.15 * em, sh) {
                canvas.fill_path(&slab, &solid(accent), FillRule::Winding, origin, None);
            }
        }
        "card_pill" => {
            let a = hue_shift(accent, -28.0);
            let b = hue_shift(accent, 42.0);
            fill(
                canvas,
                &card,
                &linear(
                    (x0, y0),
                    (x0 + rw, y0),
                    &[(0.0, a), (0.55, accent), (1.0, b)],
                ),
            );
            fill(
                canvas,
                &card,
                &linear(
                    (0.0, y0),
                    (0.0, y0 + rh * 0.6),
                    &[(0.0, [1.0, 1.0, 1.0, 0.22]), (1.0, [1.0, 1.0, 1.0, 0.0])],
                ),
            );
            // A small white dot that pops in at the left end.
            let pop = ease_out_expo(stage(r, 0.5, 0.72));
            if let Some(dot) =
                PathBuilder::from_circle(x0 + 0.5 * em, y0 + rh * 0.5, 0.11 * em * pop)
            {
                canvas.fill_path(
                    &dot,
                    &solid([1.0, 1.0, 1.0, 0.95]),
                    FillRule::Winding,
                    origin,
                    None,
                );
            }
        }
        _ => {
            // A flowing mesh gradient: a base sweep and three soft colour
            // blobs that drift across as the card opens.
            let flow = r;
            fill(
                canvas,
                &card,
                &linear(
                    (x0, y0),
                    (x0 + rw, y0 + rh),
                    &[
                        (0.0, hue_shift(accent, -30.0)),
                        (0.55, accent),
                        (1.0, hue_shift(accent, 50.0)),
                    ],
                ),
            );
            let blobs = [
                (
                    0.12 + 0.3 * flow,
                    0.15,
                    hue_shift(tint(accent, 0.3), 70.0),
                    0.6,
                ),
                (
                    0.88 - 0.25 * flow,
                    0.95,
                    hue_shift(tint(accent, 0.2), -70.0),
                    0.55,
                ),
                (0.5 + 0.1 * flow, 0.0, [1.0, 1.0, 1.0, 1.0], 0.2),
            ];
            for (fx, fy, c, a) in blobs {
                fill(
                    canvas,
                    &card,
                    &radial(
                        (x0 + rw * fx, y0 + rh * fy),
                        rw.min(rh * 3.0) * 0.55,
                        &[(0.0, alpha(c, a)), (1.0, alpha(c, 0.0))],
                    ),
                );
            }
            fill(
                canvas,
                &card,
                &linear(
                    (0.0, y0),
                    (0.0, y0 + rh * 0.55),
                    &[(0.0, [1.0, 1.0, 1.0, 0.2]), (1.0, [1.0, 1.0, 1.0, 0.0])],
                ),
            );
        }
    }
    // A hairline rim, so the card has an edge against any picture.
    let rim = Stroke {
        width: 0.012 * em,
        ..Default::default()
    };
    canvas.stroke_path(&card, &solid([1.0, 1.0, 1.0, 0.18]), &rim, origin, mask);

    // The line that draws just before the card opens.
    let line_in = ease_out_expo(stage(r, 0.0, 0.3));
    let line_out = 1.0 - stage(r, 0.42, 0.62);
    if line_in > 0.0 && line_out > 0.0 {
        let (lx, ly) = if spec.style == "card_pill" {
            (x0 + rw * 0.5 - rw * 0.5 * line_in, y0 + rh * 0.5)
        } else {
            (x0, y0 + rh)
        };
        if let Some(l) = rect_path(lx, ly - 0.015 * em, rw * line_in, 0.03 * em) {
            canvas.fill_path(
                &l,
                &solid(alpha(tint(accent, 0.6), line_out)),
                FillRule::Winding,
                origin,
                None,
            );
        }
    }

    // The bright leading edge, the blurred trail behind it and a few sparks,
    // only while the card is still opening.
    if wipe < 0.995 {
        // Fades in from nothing as the card starts to open, and out as it
        // finishes.
        let still = (1.0 - wipe) * smoothstep(0.0, 0.05, wipe);
        let edges: &[(f32, f32)] = if spec.style == "card_pill" {
            &[(edge_l, -1.0), (edge_r, 1.0)]
        } else {
            &[(edge_r, 1.0)]
        };
        for &(ex, dir) in edges {
            let trail = 2.2 * em * still.sqrt();
            let (ta, tb) = if dir > 0.0 {
                (ex - trail, ex)
            } else {
                (ex + trail, ex)
            };
            if let Some(p) = rect_path(ta.min(tb), y0, (ta - tb).abs(), rh) {
                let g = linear(
                    (ta, 0.0),
                    (tb, 0.0),
                    &[
                        (0.0, alpha(tint(accent, 0.7), 0.0)),
                        (1.0, alpha(tint(accent, 0.7), 0.4 * still.sqrt())),
                    ],
                );
                canvas.fill_path(&p, &g, FillRule::Winding, origin, mask);
            }
            let bar_w = 0.07 * em;
            if let Some(p) = rect_path(ex - bar_w * 0.5, y0, bar_w, rh) {
                canvas.fill_path(
                    &p,
                    &solid([1.0, 1.0, 1.0, 0.75 * still.sqrt()]),
                    FillRule::Winding,
                    origin,
                    mask,
                );
            }
            for i in 0..12u32 {
                let (h1, h2, h3) = (hash(i * 3), hash(i * 3 + 1), hash(i * 3 + 2));
                let px = ex - dir * (0.1 + h1 * 1.8) * em * (0.4 + still);
                let py = y0 - 0.12 * em + h2 * (rh + 0.24 * em);
                let size = (0.012 + 0.028 * h3) * em;
                if let Some(dot) = PathBuilder::from_circle(px, py, size) {
                    canvas.fill_path(
                        &dot,
                        &solid([1.0, 1.0, 1.0, still.sqrt() * (0.25 + 0.6 * h3)]),
                        FillRule::Winding,
                        origin,
                        mask,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card_spec(style: &str) -> TitleSpec {
        TitleSpec {
            style: style.into(),
            accent: [0.2, 0.55, 0.95, 1.0],
            size: 80.0,
            ..TitleSpec::default()
        }
    }

    fn painted(style: &str, reveal: f32) -> usize {
        let mut canvas = Pixmap::new(900, 300).unwrap();
        let origin = Transform::from_translate(150.0, 110.0);
        draw(
            &mut canvas,
            origin,
            &card_spec(style),
            80.0,
            500.0,
            90.0,
            reveal,
        );
        canvas.data().chunks(4).filter(|p| p[3] > 8).count()
    }

    #[test]
    fn the_card_builds_up_and_never_disappears_at_rest() {
        for style in ["card_gradient", "card_bar", "card_pill"] {
            assert!(painted(style, 0.0) < 400, "{style} starts almost empty");
            let (a, b, c) = (
                painted(style, 0.3),
                painted(style, 0.6),
                painted(style, 1.0),
            );
            assert!(a < b + 20_000 && c > 20_000, "{style}: {a} {b} {c}");
            // Settled: the same picture at 1.0 as just before it.
            assert!(
                painted(style, 0.9).abs_diff(c) < 3000,
                "{style} is still moving at the end"
            );
        }
    }

    #[test]
    fn sparks_and_the_edge_are_gone_when_settled() {
        // Nothing is drawn beyond the card's own rectangle once it is open.
        let mut canvas = Pixmap::new(900, 300).unwrap();
        let origin = Transform::from_translate(150.0, 110.0);
        draw(
            &mut canvas,
            origin,
            &card_spec("card_gradient"),
            80.0,
            500.0,
            90.0,
            1.0,
        );
        let rc = rect("card_gradient", 80.0, 500.0, 90.0);
        let far = (rc.right() + 150.0 + 80.0 * margin(1.0) + 4.0) as u32;
        for y in 0..300u32 {
            for x in far..900u32 {
                assert_eq!(canvas.pixel(x, y).unwrap().alpha(), 0, "ink at {x},{y}");
            }
        }
    }

    #[test]
    fn hue_shift_turns_colours_and_keeps_grey() {
        let blue = [0.1, 0.4, 1.0, 1.0];
        let shifted = hue_shift(blue, 90.0);
        assert!((shifted[0] - blue[0]).abs() > 0.1 || (shifted[1] - blue[1]).abs() > 0.1);
        let grey = hue_shift([0.5, 0.5, 0.5, 1.0], 120.0);
        assert!((grey[0] - grey[1]).abs() < 0.01 && (grey[1] - grey[2]).abs() < 0.01);
    }
}
