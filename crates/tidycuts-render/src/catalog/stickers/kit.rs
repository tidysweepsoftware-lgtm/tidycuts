//! Drawing kit shared by the sticker drawings.
//!
//! Drawings work in "units": a sticker is 1 unit tall and [`Pen::w`] units
//! wide (its aspect ratio), y pointing down. The pen scales everything to
//! the pixmap, so the same code draws crisp stickers at any size and stroke
//! widths scale with them.

use super::DrawCtx;
use std::f32::consts::PI;
use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, Path,
    PathBuilder, Pixmap, Point, RadialGradient, Rect, Shader, SpreadMode, Stroke, StrokeDash,
    Transform,
};

/// A straight (not premultiplied) RGBA color, 0..1.
pub type C = [f32; 4];
/// A point in units.
pub type P = (f32, f32);

pub const WHITE: C = [1.0, 1.0, 1.0, 1.0];

/// Width of the dark edge most stickers get, in units.
pub const EDGE: f32 = 0.022;
/// Width of the light rim outside the edge, in units.
pub const RIM: f32 = 0.04;

pub const fn rgb(r: f32, g: f32, b: f32) -> C {
    [r, g, b, 1.0]
}

pub fn mix(a: C, b: C, k: f32) -> C {
    let mut o = a;
    for i in 0..4 {
        o[i] = a[i] + (b[i] - a[i]) * k;
    }
    o
}

/// Darker (k < 1) or brighter (k > 1) version of `c`.
pub fn shade(c: C, k: f32) -> C {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

/// `c` moved towards white by `k`.
pub fn tint(c: C, k: f32) -> C {
    mix(c, [1.0, 1.0, 1.0, c[3]], k)
}

pub fn fade(c: C, a: f32) -> C {
    [c[0], c[1], c[2], c[3] * a]
}

/// A dark outline color that goes with `c`.
pub fn ink(c: C) -> C {
    mix(shade(c, 0.42), [0.08, 0.06, 0.12, c[3]], 0.4)
}

fn color(c: C) -> Color {
    let f = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    Color::from_rgba(f(c[0]), f(c[1]), f(c[2]), f(c[3])).unwrap_or(Color::BLACK)
}

pub fn solid(c: C) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
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

fn stops(s: &[(f32, C)]) -> Vec<GradientStop> {
    s.iter()
        .map(|(p, c)| GradientStop::new(*p, color(*c)))
        .collect()
}

/// A linear gradient from `a` to `b` (units).
pub fn linear(a: P, b: P, s: &[(f32, C)]) -> Paint<'static> {
    let sh = LinearGradient::new(
        Point::from_xy(a.0, a.1),
        Point::from_xy(b.0, b.1),
        stops(s),
        SpreadMode::Pad,
        Transform::identity(),
    );
    shader_paint(sh, s[0].1)
}

/// A radial gradient around `c` (units).
pub fn radial(c: P, r: f32, s: &[(f32, C)]) -> Paint<'static> {
    let sh = RadialGradient::new(
        Point::from_xy(c.0, c.1),
        Point::from_xy(c.0, c.1),
        r.max(1e-4),
        stops(s),
        SpreadMode::Pad,
        Transform::identity(),
    );
    shader_paint(sh, s[0].1)
}

/// The usual sticker body: lighter at the top, a little darker at the
/// bottom, between `y0` and `y1`.
pub fn gloss(c: C, y0: f32, y1: f32) -> Paint<'static> {
    linear(
        (0.0, y0),
        (0.0, y1),
        &[(0.0, tint(c, 0.38)), (0.45, c), (1.0, shade(c, 0.8))],
    )
}

pub fn round(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

pub fn butt(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

pub fn miter(width: f32) -> Stroke {
    Stroke {
        width,
        line_join: LineJoin::Miter,
        miter_limit: 5.0,
        ..Default::default()
    }
}

pub fn dashed(width: f32, on: f32, off: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        dash: StrokeDash::new(vec![on, off], 0.0),
        ..Default::default()
    }
}

/// Draws into a sticker pixmap in units.
pub struct Pen<'a> {
    pm: &'a mut Pixmap,
    ts: Transform,
    /// Width in units (the aspect ratio); the height is 1.
    pub w: f32,
    /// Main and secondary colors chosen by the user.
    pub c1: C,
    pub c2: C,
    /// Seconds, for animated stickers.
    pub t: f32,
}

impl<'a> Pen<'a> {
    pub fn new(pm: &'a mut Pixmap, ctx: &DrawCtx) -> Self {
        Pen {
            ts: Transform::from_scale(ctx.h, ctx.h),
            w: ctx.w / ctx.h,
            c1: ctx.color,
            c2: ctx.color2,
            t: ctx.t,
            pm,
        }
    }

    /// Draws everything `k` times smaller around the middle of the sticker,
    /// leaving room for a wide outline and the shadow.
    pub fn inset(mut self, k: f32) -> Self {
        let (cx, cy) = (self.w * 0.5, 0.5);
        self.ts = self
            .ts
            .pre_translate(cx, cy)
            .pre_scale(k, k)
            .pre_translate(-cx, -cy);
        self
    }

    pub fn fill(&mut self, p: &Path, c: C) {
        self.paint(p, &solid(c));
    }

    pub fn paint(&mut self, p: &Path, paint: &Paint) {
        self.pm
            .fill_path(p, paint, FillRule::Winding, self.ts, None);
    }

    pub fn paint_masked(&mut self, p: &Path, paint: &Paint, mask: &Mask) {
        self.pm
            .fill_path(p, paint, FillRule::Winding, self.ts, Some(mask));
    }

    /// Round-capped, round-joined stroke.
    pub fn stroke(&mut self, p: &Path, c: C, width: f32) {
        self.stroke_with(p, &solid(c), &round(width));
    }

    pub fn stroke_with(&mut self, p: &Path, paint: &Paint, s: &Stroke) {
        self.pm.stroke_path(p, paint, s, self.ts, None);
    }

    /// A mask covering `p`, to draw details inside a shape.
    pub fn clip(&self, p: &Path) -> Option<Mask> {
        let mut m = Mask::new(self.pm.width(), self.pm.height())?;
        m.fill_path(p, FillRule::Winding, true, self.ts);
        Some(m)
    }

    /// Fills `p` clipped to `mask` (does nothing without a mask).
    pub fn fill_in(&mut self, p: &Path, c: C, mask: &Option<Mask>) {
        if let Some(m) = mask {
            self.paint_masked(p, &solid(c), m);
        }
    }

    /// Makes `p` transparent again.
    pub fn erase(&mut self, p: &Path) {
        let mut paint = solid(WHITE);
        paint.blend_mode = BlendMode::DestinationOut;
        self.paint(p, &paint);
    }

    pub fn erase_stroke(&mut self, p: &Path, s: &Stroke) {
        let mut paint = solid(WHITE);
        paint.blend_mode = BlendMode::DestinationOut;
        self.stroke_with(p, &paint, s);
    }

    /// The die-cut sticker look for the union of `parts`: an optional light
    /// rim, a dark edge, then the body on top.
    pub fn sticker(
        &mut self,
        parts: &[&Path],
        body: &Paint,
        edge: Option<(C, f32)>,
        rim: Option<(C, f32)>,
    ) {
        let e = edge.map_or(0.0, |e| e.1);
        if let Some((rc, rw)) = rim {
            for p in parts {
                self.stroke(p, rc, 2.0 * (rw + e));
            }
        }
        if let Some((ec, ew)) = edge {
            for p in parts {
                self.stroke(p, ec, 2.0 * ew);
            }
        }
        for p in parts {
            self.paint(p, body);
        }
    }

    /// The same for a stroked line `width` wide.
    pub fn sticker_line(
        &mut self,
        p: &Path,
        width: f32,
        body: &Paint,
        edge: Option<(C, f32)>,
        rim: Option<(C, f32)>,
    ) {
        let e = edge.map_or(0.0, |e| e.1);
        if let Some((rc, rw)) = rim {
            self.stroke(p, rc, width + 2.0 * (rw + e));
        }
        if let Some((ec, ew)) = edge {
            self.stroke(p, ec, width + 2.0 * ew);
        }
        self.stroke_with(p, body, &round(width));
    }

    /// Lets the bright parts of everything drawn so far spill light around
    /// them: they are blurred `radius` units wide and added back on top.
    /// Only pixels brighter than `threshold` (0..1) glow. Call it after the
    /// drawing and before [`Pen::shadow`], if there is one.
    pub fn bloom(&mut self, strength: f32, radius: f32, threshold: f32) {
        bloom_pass(self.pm, strength, radius, threshold);
    }

    /// Finishes the sticker with a soft drop shadow under everything drawn.
    pub fn shadow(self, strength: f32) {
        soft_shadow(self.pm, strength, true);
    }

    /// A soft dark halo with no offset, for stickers that spin (an offset
    /// shadow would turn with them).
    pub fn halo(self, strength: f32) {
        soft_shadow(self.pm, strength, false);
    }
}

// ---------------------------------------------------------------------------
// Paths.

fn finish(pb: PathBuilder) -> Path {
    pb.finish().unwrap_or_else(|| {
        // Nothing to draw: a tiny triangle far outside the sticker.
        let mut pb = PathBuilder::new();
        pb.move_to(-10.0, -10.0);
        pb.line_to(-9.0, -10.0);
        pb.line_to(-10.0, -9.0);
        pb.close();
        pb.finish().expect("fallback path")
    })
}

/// `p` transformed (unchanged if the transform is invalid).
pub fn xf(p: &Path, ts: Transform) -> Path {
    p.clone().transform(ts).unwrap_or_else(|| p.clone())
}

/// `p` rotated by `deg` degrees (clockwise) around `c`.
pub fn rot(p: &Path, deg: f32, c: P) -> Path {
    xf(p, Transform::from_rotate_at(deg, c.0, c.1))
}

pub fn moved(p: &Path, dx: f32, dy: f32) -> Path {
    xf(p, Transform::from_translate(dx, dy))
}

/// `p` scaled by `k` around `c`.
pub fn scaled(p: &Path, k: f32, c: P) -> Path {
    xf(
        p,
        Transform::from_translate(c.0, c.1)
            .pre_scale(k, k)
            .pre_translate(-c.0, -c.1),
    )
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Path {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r.max(1e-4));
    finish(pb)
}

pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    let mut pb = PathBuilder::new();
    if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, 2.0 * rx.max(1e-4), 2.0 * ry.max(1e-4)) {
        pb.push_oval(r);
    }
    finish(pb)
}

/// Rounded rectangle.
pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let k = 0.4477 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - k, y, x + w, y + k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + k, y + h, x, y + h - k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + k, x + k, y, x + r, y);
    pb.close();
    finish(pb)
}

/// Closed polygon.
pub fn poly(pts: &[P]) -> Path {
    let mut pb = PathBuilder::new();
    if let Some(&(x, y)) = pts.first() {
        pb.move_to(x, y);
        for &(x, y) in &pts[1..] {
            pb.line_to(x, y);
        }
        pb.close();
    }
    finish(pb)
}

/// Open polyline.
pub fn line(pts: &[P]) -> Path {
    let mut pb = PathBuilder::new();
    if let Some(&(x, y)) = pts.first() {
        pb.move_to(x, y);
        for &(x, y) in &pts[1..] {
            pb.line_to(x, y);
        }
    }
    finish(pb)
}

pub fn seg(a: P, b: P) -> Path {
    line(&[a, b])
}

/// Star with `n` points, the first one pointing up (turned by `deg`).
pub fn star_pts(cx: f32, cy: f32, ro: f32, ri: f32, n: usize, deg: f32) -> Vec<P> {
    (0..2 * n)
        .map(|i| {
            let r = if i % 2 == 0 { ro } else { ri };
            let a = deg.to_radians() - PI / 2.0 + i as f32 * PI / n as f32;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

pub fn star(cx: f32, cy: f32, ro: f32, ri: f32, n: usize, deg: f32) -> Path {
    poly(&star_pts(cx, cy, ro, ri, n, deg))
}

/// A heart filling the `w` x `h` box centred on (cx, cy).
pub fn heart(cx: f32, cy: f32, w: f32, h: f32) -> Path {
    let (x0, y0) = (cx - w * 0.5, cy - h * 0.5);
    let p = |u: f32, v: f32| (x0 + u * w, y0 + v * h);
    let mut pb = PathBuilder::new();
    let m = p(0.5, 1.0);
    pb.move_to(m.0, m.1);
    let mut c = |a: P, b: P, e: P| pb.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
    c(p(0.2, 0.78), p(0.0, 0.58), p(0.0, 0.31));
    c(p(0.0, 0.11), p(0.14, 0.0), p(0.28, 0.0));
    c(p(0.39, 0.0), p(0.47, 0.06), p(0.5, 0.17));
    c(p(0.53, 0.06), p(0.61, 0.0), p(0.72, 0.0));
    c(p(0.86, 0.0), p(1.0, 0.11), p(1.0, 0.31));
    c(p(1.0, 0.58), p(0.8, 0.78), p(0.5, 1.0));
    pb.close();
    finish(pb)
}

/// A drop with its point up, `s` = radius of the round part, turned by `deg`.
pub fn drop(cx: f32, cy: f32, s: f32, deg: f32) -> Path {
    let p = |x: f32, y: f32| (cx + x * s, cy + y * s);
    let mut pb = PathBuilder::new();
    let m = p(0.0, -1.6);
    pb.move_to(m.0, m.1);
    let mut c = |a: P, b: P, e: P| pb.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
    c(p(0.45, -1.0), p(1.0, -0.5), p(1.0, 0.05));
    c(p(1.0, 0.6), p(0.55, 1.0), p(0.0, 1.0));
    c(p(-0.55, 1.0), p(-1.0, 0.6), p(-1.0, 0.05));
    c(p(-1.0, -0.5), p(-0.45, -1.0), p(0.0, -1.6));
    pb.close();
    rot(&finish(pb), deg, (cx, cy))
}

/// A four-pointed sparkle with curved sides.
pub fn sparkle(cx: f32, cy: f32, r: f32, waist: f32) -> Path {
    let k = r * waist;
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy - r);
    pb.quad_to(cx + k, cy - k, cx + r, cy);
    pb.quad_to(cx + k, cy + k, cx, cy + r);
    pb.quad_to(cx - k, cy + k, cx - r, cy);
    pb.quad_to(cx - k, cy - k, cx, cy - r);
    pb.close();
    finish(pb)
}

/// A circle or ellipse with `n` round bumps around it.
pub fn scallop(cx: f32, cy: f32, rx: f32, ry: f32, n: usize, bulge: f32) -> Path {
    let pt = |a: f32, k: f32| (cx + rx * k * a.cos(), cy + ry * k * a.sin());
    let step = 2.0 * PI / n as f32;
    let mut pb = PathBuilder::new();
    let s = pt(-PI / 2.0, 1.0);
    pb.move_to(s.0, s.1);
    for i in 0..n {
        let a0 = -PI / 2.0 + i as f32 * step;
        let c = pt(a0 + step * 0.5, 1.0 + 2.0 * bulge);
        let e = pt(a0 + step, 1.0);
        pb.quad_to(c.0, c.1, e.0, e.1);
    }
    pb.close();
    finish(pb)
}

/// Points on an ellipse from `a0` to `a1` degrees (0 = right, clockwise).
pub fn arc_pts(cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, n: usize) -> Vec<P> {
    let n = n.max(1);
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

fn cr_controls(p0: P, p1: P, p2: P, p3: P, sharp1: bool, sharp2: bool) -> (P, P) {
    let c1 = if sharp1 {
        (p1.0 + (p2.0 - p1.0) * 0.2, p1.1 + (p2.1 - p1.1) * 0.2)
    } else {
        (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0)
    };
    let c2 = if sharp2 {
        (p2.0 + (p1.0 - p2.0) * 0.2, p2.1 + (p1.1 - p2.1) * 0.2)
    } else {
        (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0)
    };
    (c1, c2)
}

/// A smooth curve through `pts` (Catmull-Rom); points listed in `sharp`
/// become corners.
pub fn smooth(pts: &[P], closed: bool, sharp: &[usize]) -> Path {
    let n = pts.len();
    let mut pb = PathBuilder::new();
    if n < 2 {
        return finish(pb);
    }
    let get = |i: isize| -> P {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let idx = |i: isize| -> usize { i.rem_euclid(n as isize) as usize };
    pb.move_to(pts[0].0, pts[0].1);
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs as isize {
        let (c1, c2) = cr_controls(
            get(i - 1),
            get(i),
            get(i + 1),
            get(i + 2),
            sharp.contains(&idx(i)),
            sharp.contains(&idx(i + 1)),
        );
        let e = get(i + 1);
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
    }
    if closed {
        pb.close();
    }
    finish(pb)
}

/// Samples the smooth curve through `pts`, `per` points per segment.
pub fn sample(pts: &[P], closed: bool, per: usize) -> Vec<P> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    let get = |i: isize| -> P {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let mut out = vec![pts[0]];
    for i in 0..segs as isize {
        let (p1, p2) = (get(i), get(i + 1));
        let (c1, c2) = cr_controls(get(i - 1), p1, p2, get(i + 2), false, false);
        for k in 1..=per {
            let t = k as f32 / per as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push((
                a * p1.0 + b * c1.0 + c * c2.0 + d * p2.0,
                a * p1.1 + b * c1.1 + c * c2.1 + d * p2.1,
            ));
        }
    }
    out
}

pub fn dist(a: P, b: P) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

pub fn norm(v: P) -> P {
    let l = (v.0 * v.0 + v.1 * v.1).sqrt().max(1e-9);
    (v.0 / l, v.1 / l)
}

pub fn lerp(a: P, b: P, k: f32) -> P {
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

/// Cumulative arc length of a polyline, normalised to 0..1.
fn arc_fractions(c: &[P]) -> Vec<f32> {
    let mut cum = vec![0.0f32; c.len()];
    for i in 1..c.len() {
        cum[i] = cum[i - 1] + dist(c[i - 1], c[i]);
    }
    let total = cum.last().copied().unwrap_or(0.0).max(1e-9);
    cum.iter().map(|v| v / total).collect()
}

fn normal_at(c: &[P], i: usize) -> P {
    let a = c[i.saturating_sub(1)];
    let b = c[(i + 1).min(c.len() - 1)];
    let d = norm((b.0 - a.0, b.1 - a.1));
    (-d.1, d.0)
}

/// A filled band along the polyline `c` whose half-widths on each side
/// are given as functions of the position along it (0..1).
pub fn band(c: &[P], left: impl Fn(f32) -> f32, right: impl Fn(f32) -> f32) -> Path {
    if c.len() < 2 {
        return finish(PathBuilder::new());
    }
    let s = arc_fractions(c);
    let mut l = Vec::with_capacity(c.len() * 2);
    let mut r = Vec::with_capacity(c.len());
    for i in 0..c.len() {
        let n = normal_at(c, i);
        let (wl, wr) = (left(s[i]), right(s[i]));
        l.push((c[i].0 + n.0 * wl, c[i].1 + n.1 * wl));
        r.push((c[i].0 - n.0 * wr, c[i].1 - n.1 * wr));
    }
    r.reverse();
    l.extend(r);
    poly(&l)
}

/// A band `width(s)` wide along `c` (tapered brush strokes).
pub fn ribbon(c: &[P], width: impl Fn(f32) -> f32) -> Path {
    band(c, |s| width(s) * 0.5, |s| width(s) * 0.5)
}

/// A solid arrow along the polyline `c`: the shaft goes from `w0` to `w1`
/// wide and ends in a barbed head `head_len` long and `head_w` wide whose
/// point is the last point of `c`.
pub fn arrow(c: &[P], w0: f32, w1: f32, head_len: f32, head_w: f32) -> Path {
    if c.len() < 2 {
        return finish(PathBuilder::new());
    }
    let mut cum = vec![0.0f32; c.len()];
    for i in 1..c.len() {
        cum[i] = cum[i - 1] + dist(c[i - 1], c[i]);
    }
    let total = cum[c.len() - 1];
    let base_s = (total - head_len).max(total * 0.05);
    let mut shaft: Vec<(P, f32)> = Vec::new();
    for i in 0..c.len() {
        if cum[i] < base_s {
            shaft.push((c[i], cum[i]));
        } else {
            let (a, b) = (c[i - 1], c[i]);
            let k = (base_s - cum[i - 1]) / (cum[i] - cum[i - 1]).max(1e-9);
            shaft.push((lerp(a, b, k), base_s));
            break;
        }
    }
    let tip = c[c.len() - 1];
    let base = shaft.last().map(|s| s.0).unwrap_or(c[0]);
    let dir = norm((tip.0 - base.0, tip.1 - base.1));
    let hn = (-dir.1, dir.0);
    let pts: Vec<P> = shaft.iter().map(|s| s.0).collect();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (i, (p, s)) in shaft.iter().enumerate() {
        let n = if i + 1 == shaft.len() {
            hn
        } else {
            normal_at(&pts, i)
        };
        let w = 0.5 * (w0 + (w1 - w0) * (s / base_s.max(1e-9)));
        left.push((p.0 + n.0 * w, p.1 + n.1 * w));
        right.push((p.0 - n.0 * w, p.1 - n.1 * w));
    }
    let barb = head_len * 0.16;
    let wing = |side: f32| {
        (
            base.0 + hn.0 * head_w * 0.5 * side - dir.0 * barb,
            base.1 + hn.1 * head_w * 0.5 * side - dir.1 * barb,
        )
    };
    left.push(wing(1.0));
    left.push(tip);
    left.push(wing(-1.0));
    right.reverse();
    left.extend(right);
    poly(&left)
}

// ---------------------------------------------------------------------------
// Numbers.

/// A deterministic pseudo-random number in 0..1.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B1).wrapping_add(0x7F4A_7C15);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE3D);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

// ---------------------------------------------------------------------------
// Shadow.

/// Adds a soft dark shadow under everything in `pm`: the alpha is blurred
/// on a coarse grid (cheap at any size), offset down a little when `drop`,
/// and composited under the drawing.
fn soft_shadow(pm: &mut Pixmap, strength: f32, drop: bool) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let u = h as f32;
    let blur = u * 0.016;
    let (ox, oy) = if drop {
        (u * 0.006, u * 0.02)
    } else {
        (0.0, 0.0)
    };
    let f = ((blur / 3.0) as usize).max(1);
    let (gw, gh) = (w.div_ceil(f) + 2, h.div_ceil(f) + 2);
    let mut g = vec![0f32; gw * gh];
    {
        let data = pm.data();
        for y in 0..h {
            let row = (y / f + 1) * gw;
            for x in 0..w {
                g[row + x / f + 1] += data[(y * w + x) * 4 + 3] as f32;
            }
        }
    }
    let norm = 1.0 / (255.0 * (f * f) as f32);
    g.iter_mut().for_each(|v| *v *= norm);
    let r = ((blur / f as f32).round() as usize).max(1);
    let mut tmp = Vec::new();
    for _ in 0..2 {
        box_blur(&mut g, gh, gw, 1, gw, r, &mut tmp);
        box_blur(&mut g, gw, gh, gw, 1, r, &mut tmp);
    }
    let data = pm.data_mut();
    let fs = f as f32;
    for y in 0..h {
        let sy = ((y as f32 + 0.5 - oy) / fs + 0.5).clamp(0.0, (gh - 1) as f32);
        let (y0, ty) = (sy.floor() as usize, sy.fract());
        let y1 = (y0 + 1).min(gh - 1);
        for x in 0..w {
            let sx = ((x as f32 + 0.5 - ox) / fs + 0.5).clamp(0.0, (gw - 1) as f32);
            let (x0, tx) = (sx.floor() as usize, sx.fract());
            let x1 = (x0 + 1).min(gw - 1);
            let top = g[y0 * gw + x0] * (1.0 - tx) + g[y0 * gw + x1] * tx;
            let bot = g[y1 * gw + x0] * (1.0 - tx) + g[y1 * gw + x1] * tx;
            let s = (top * (1.0 - ty) + bot * ty) * strength;
            if s <= 1e-4 {
                continue;
            }
            let i = (y * w + x) * 4 + 3;
            let a = data[i] as f32 / 255.0;
            // Black shadow under premultiplied color: only alpha changes.
            data[i] = ((a + s * (1.0 - a)) * 255.0 + 0.5).min(255.0) as u8;
        }
    }
}

/// Bloom: the bright pixels of `pm` are blurred on a coarse grid and added
/// back as light, so glowing things glow into the space around them. The
/// result stays a valid premultiplied image (color never exceeds alpha).
fn bloom_pass(pm: &mut Pixmap, strength: f32, radius: f32, threshold: f32) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let u = h as f32;
    let blur = (u * radius).max(2.0);
    let f = ((blur / 3.0) as usize).max(1);
    let (gw, gh) = (w.div_ceil(f) + 2, h.div_ceil(f) + 2);
    let mut grids = [
        vec![0f32; gw * gh],
        vec![0f32; gw * gh],
        vec![0f32; gw * gh],
    ];
    {
        let data = pm.data();
        for y in 0..h {
            let row = (y / f + 1) * gw;
            for x in 0..w {
                let i = (y * w + x) * 4;
                let a = data[i + 3] as f32;
                if a < 1.0 {
                    continue;
                }
                let rgb = [data[i] as f32, data[i + 1] as f32, data[i + 2] as f32];
                // Only what is bright enough to glow feeds the bloom.
                let peak = rgb[0].max(rgb[1]).max(rgb[2]) / a;
                let gain = smoothstep(threshold.min(0.98), 1.0, peak);
                for k in 0..3 {
                    grids[k][row + x / f + 1] += rgb[k] * gain;
                }
            }
        }
    }
    let norm = 1.0 / (255.0 * (f * f) as f32);
    let r = ((blur / f as f32).round() as usize).max(1);
    let mut tmp = Vec::new();
    for g in grids.iter_mut() {
        g.iter_mut().for_each(|v| *v *= norm);
        for _ in 0..3 {
            box_blur(g, gh, gw, 1, gw, r, &mut tmp);
            box_blur(g, gw, gh, gw, 1, r, &mut tmp);
        }
    }
    let data = pm.data_mut();
    let fs = f as f32;
    for y in 0..h {
        let sy = ((y as f32 + 0.5) / fs + 0.5).clamp(0.0, (gh - 1) as f32);
        let (y0, ty) = (sy.floor() as usize, sy.fract());
        let y1 = (y0 + 1).min(gh - 1);
        for x in 0..w {
            let sx = ((x as f32 + 0.5) / fs + 0.5).clamp(0.0, (gw - 1) as f32);
            let (x0, tx) = (sx.floor() as usize, sx.fract());
            let x1 = (x0 + 1).min(gw - 1);
            let mut add = [0f32; 3];
            for k in 0..3 {
                let g = &grids[k];
                let top = g[y0 * gw + x0] * (1.0 - tx) + g[y0 * gw + x1] * tx;
                let bot = g[y1 * gw + x0] * (1.0 - tx) + g[y1 * gw + x1] * tx;
                add[k] = (top * (1.0 - ty) + bot * ty) * strength;
            }
            let peak = add[0].max(add[1]).max(add[2]);
            if peak <= 1e-4 {
                continue;
            }
            let i = (y * w + x) * 4;
            let a = (data[i + 3] as f32 / 255.0 + peak).min(1.0);
            for k in 0..3 {
                let v = (data[i + k] as f32 / 255.0 + add[k]).min(a);
                data[i + k] = (v * 255.0 + 0.5) as u8;
            }
            data[i + 3] = (a * 255.0 + 0.5) as u8;
        }
    }
}

/// Box blur of radius `r` along one axis: `lines` lines of `len` values,
/// `step` apart within a line and `stride` apart between lines.
fn box_blur(
    g: &mut [f32],
    lines: usize,
    len: usize,
    step: usize,
    stride: usize,
    r: usize,
    tmp: &mut Vec<f32>,
) {
    let k = 1.0 / (2 * r + 1) as f32;
    for l in 0..lines {
        let base = l * stride;
        tmp.clear();
        tmp.extend((0..len).map(|i| g[base + i * step]));
        let mut acc: f32 = tmp[..(r + 1).min(len)].iter().sum();
        for i in 0..len {
            g[base + i * step] = acc * k;
            if i + r + 1 < len {
                acc += tmp[i + r + 1];
            }
            if i >= r {
                acc -= tmp[i - r];
            }
        }
    }
}
