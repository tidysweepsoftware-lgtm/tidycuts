//! Text for badges, turned into vector paths from the embedded fonts so it
//! stays crisp at any sticker size.

use super::kit::{xf, P};
use ab_glyph::{Font, FontRef, OutlineCurve, Point};
use tiny_skia::{Path, PathBuilder, Transform};

/// The outline of `text` set in `font` (a name from `crate::fonts`): 1 unit
/// = 1 em, baseline at y = 0, y pointing down. `tracking` is extra space
/// between letters in ems.
pub fn text_path(font: &str, text: &str, tracking: f32) -> Option<Path> {
    let font = FontRef::try_from_slice(crate::fonts::get(font).data).ok()?;
    let s = 1.0 / font.units_per_em().unwrap_or(1000.0);
    let mut pb = PathBuilder::new();
    let mut x = 0.0f32;
    let mut prev = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(p) = prev {
            x += font.kern_unscaled(p, id) * s;
        }
        if let Some(outline) = font.outline(id) {
            let pt = |p: Point| (x + p.x * s, -p.y * s);
            let mut start: Option<Point> = None;
            let mut last: Option<Point> = None;
            for c in &outline.curves {
                let (a, end) = match c {
                    OutlineCurve::Line(a, b) => (*a, *b),
                    OutlineCurve::Quad(a, _, b) => (*a, *b),
                    OutlineCurve::Cubic(a, _, _, b) => (*a, *b),
                };
                if start.is_none() || last != Some(a) {
                    if start.is_some() {
                        pb.close();
                    }
                    let m = pt(a);
                    pb.move_to(m.0, m.1);
                    start = Some(a);
                }
                match c {
                    OutlineCurve::Line(_, b) => {
                        let e = pt(*b);
                        pb.line_to(e.0, e.1);
                    }
                    OutlineCurve::Quad(_, c1, b) => {
                        let (c1, e) = (pt(*c1), pt(*b));
                        pb.quad_to(c1.0, c1.1, e.0, e.1);
                    }
                    OutlineCurve::Cubic(_, c1, c2, b) => {
                        let (c1, c2, e) = (pt(*c1), pt(*c2), pt(*b));
                        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
                    }
                }
                last = Some(end);
                if start == Some(end) {
                    pb.close();
                    start = None;
                }
            }
            if start.is_some() {
                pb.close();
            }
        }
        x += font.h_advance_unscaled(id) * s + tracking;
        prev = Some(id);
    }
    pb.finish()
}

/// Text fitted (uniformly scaled) into a `max_w` x `max_h` box centred on
/// `c`, its ink centred there, slanted by `skew` (positive leans right) and
/// turned by `deg` degrees. Returns the path in units.
pub fn text(
    font: &str,
    s: &str,
    c: P,
    max_w: f32,
    max_h: f32,
    skew: f32,
    deg: f32,
) -> Option<Path> {
    let p = text_path(font, s, 0.0)?;
    let p = xf(&p, Transform::from_skew(-skew, 0.0));
    let b = p.compute_tight_bounds()?;
    let k = (max_w / b.width().max(1e-6)).min(max_h / b.height().max(1e-6));
    let ts = Transform::from_rotate_at(deg, c.0, c.1)
        .pre_translate(c.0, c.1)
        .pre_scale(k, k)
        .pre_translate(-(b.left() + b.right()) * 0.5, -(b.top() + b.bottom()) * 0.5);
    p.transform(ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_has_ink_and_holes() {
        let p = text_path("Poppins Black", "NEW", 0.0).expect("path");
        let b = p.compute_tight_bounds().unwrap();
        assert!(b.width() > 1.0 && b.height() > 0.5, "{b:?}");
        assert!(b.bottom() <= 0.01, "caps sit on the baseline: {b:?}");
        let p = text("Bangers", "WOW!", (1.0, 0.5), 0.8, 0.3, 0.1, -8.0).expect("fitted");
        let b = p.compute_tight_bounds().unwrap();
        assert!(b.width() <= 0.9 && b.height() <= 0.5, "{b:?}");
    }
}
