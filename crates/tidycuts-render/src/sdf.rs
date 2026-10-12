//! Distance-field styling: turn any shape (a title, a sticker) into a map of
//! "how far is each pixel from the edge", then build the look from that:
//! crisp fills, outlines of exact width, soft glows, shadows and stacked 3D
//! depth. Because every effect is a function of distance, edges stay sharp
//! and glows fall off smoothly at any size.
//!
//! Distances are in pixels: negative inside the shape, positive outside.

use crate::RgbaImage;

type C = [f32; 4];

const FAR: f32 = 1e20;

// ---------------------------------------------------------------------------
// The distance field.

pub struct Field {
    pub width: u32,
    pub height: u32,
    /// Signed distance to the edge, row by row.
    pub d: Vec<f32>,
}

impl Field {
    /// From an alpha mask (0..255, 128 and above counts as inside).
    pub fn from_alpha(alpha: &[u8], width: u32, height: u32) -> Field {
        let (w, h) = (width as usize, height as usize);
        assert_eq!(alpha.len(), w * h, "one alpha value per pixel");
        // Squared distance to the nearest inside pixel (for outside pixels),
        // and to the nearest outside pixel (for inside ones).
        let to_inside = edt(w, h, |i| alpha[i] >= 128);
        let to_outside = edt(w, h, |i| alpha[i] < 128);
        let d = (0..w * h)
            .map(|i| {
                if alpha[i] >= 128 {
                    // The edge runs between pixel centres, half a pixel away.
                    -(to_outside[i].sqrt() - 0.5)
                } else {
                    to_inside[i].sqrt() - 0.5
                }
            })
            .collect();
        Field { width, height, d }
    }

    /// The distance at a pixel; outside the image it keeps growing with the
    /// distance from the border, so glows fade out instead of being cut off.
    pub fn at(&self, x: i32, y: i32) -> f32 {
        let (w, h) = (self.width as i32, self.height as i32);
        let (cx, cy) = (x.clamp(0, w - 1), y.clamp(0, h - 1));
        let base = self.d[(cy * w + cx) as usize];
        let (dx, dy) = ((x - cx) as f32, (y - cy) as f32);
        if dx == 0.0 && dy == 0.0 {
            base
        } else {
            base.max(0.0) + (dx * dx + dy * dy).sqrt()
        }
    }

    /// Bilinear sample at a fractional position.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (fx, fy) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx, y - fy);
        let (ix, iy) = (fx as i32, fy as i32);
        let a = self.at(ix, iy) * (1.0 - tx) + self.at(ix + 1, iy) * tx;
        let b = self.at(ix, iy + 1) * (1.0 - tx) + self.at(ix + 1, iy + 1) * tx;
        a * (1.0 - ty) + b * ty
    }
}

/// Squared distance from every pixel to the nearest pixel where `site` is
/// true (Felzenszwalb and Huttenlocher's exact linear-time transform).
fn edt(w: usize, h: usize, site: impl Fn(usize) -> bool) -> Vec<f32> {
    let mut g: Vec<f32> = (0..w * h)
        .map(|i| if site(i) { 0.0 } else { FAR })
        .collect();
    let n = w.max(h);
    let (mut f, mut out, mut v, mut z) = (
        vec![0f32; n],
        vec![0f32; n],
        vec![0usize; n],
        vec![0f32; n + 1],
    );
    for x in 0..w {
        for y in 0..h {
            f[y] = g[y * w + x];
        }
        edt_1d(&f[..h], &mut out[..h], &mut v, &mut z);
        for y in 0..h {
            g[y * w + x] = out[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&g[y * w..(y + 1) * w]);
        edt_1d(&f[..w], &mut out[..w], &mut v, &mut z);
        g[y * w..(y + 1) * w].copy_from_slice(&out[..w]);
    }
    g
}

fn edt_1d(f: &[f32], out: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let sq = |q: usize| (q * q) as f32;
    let mut k = 0usize;
    v[0] = 0;
    z[0] = -FAR;
    z[1] = FAR;
    for q in 1..n {
        let cross =
            |p: usize| ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * q as f32 - 2.0 * p as f32);
        let mut s = cross(v[k]);
        while k > 0 && s <= z[k] {
            k -= 1;
            s = cross(v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = FAR;
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        *o = (q as f32 - v[k] as f32).powi(2) + f[v[k]];
    }
}

// ---------------------------------------------------------------------------
// The look.

/// A vertical two-colour fill across the shape's height.
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    pub top: C,
    pub bottom: C,
}

#[derive(Debug, Clone, Copy)]
pub struct Outline {
    pub color: C,
    /// Width in pixels, outside the shape.
    pub width: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Glow {
    pub color: C,
    /// How far it reaches in pixels.
    pub radius: f32,
    /// 0..1+, how bright it is at the edge.
    pub strength: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Shadow {
    pub color: C,
    pub offset: (f32, f32),
    /// Softness in pixels.
    pub blur: f32,
}

/// The shape repeated back and away from the viewer, for a 3D block.
#[derive(Debug, Clone, Copy)]
pub struct Extrude {
    /// The colour at the far end (the near end is darker than `Fill::bottom`).
    pub color: C,
    /// How many pixels deep.
    pub depth: f32,
    /// Direction the depth runs in (a unit-ish vector, y points down).
    pub dir: (f32, f32),
}

/// A raised, rounded surface: the edge slopes up over `width` pixels and is
/// lit from one side, which gives metal, glass and glossy-plastic looks.
#[derive(Debug, Clone, Copy)]
pub struct Bevel {
    /// How far in from the edge the slope runs, in pixels.
    pub width: f32,
    /// How tall the slope is, in pixels. Bigger means steeper, shinier edges.
    pub height: f32,
    /// The direction the light comes from, in the picture plane (y points
    /// down), so (-0.6, -0.8) is from the top left.
    pub light: (f32, f32),
    /// The colour lit slopes move towards.
    pub highlight: C,
    /// The colour slopes facing away move towards.
    pub shade: C,
    /// 0..1, how strong the lighting is.
    pub shine: f32,
    /// How tight the white specular glint is (try 20 to 80).
    pub sharpness: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub fill: Fill,
    pub outline: Option<Outline>,
    pub glow: Option<Glow>,
    pub shadow: Option<Shadow>,
    pub extrude: Option<Extrude>,
    pub bevel: Option<Bevel>,
}

impl Look {
    /// How far past the shape's edge this look paints, in pixels. Leave at
    /// least this much room around a shape so nothing is cut off.
    pub fn reach(&self) -> f32 {
        self.glow
            .map_or(0.0, |g| g.radius * 3.0)
            .max(self.outline.map_or(0.0, |o| o.width + 1.0))
            .max(self.extrude.map_or(0.0, |e| e.depth + 1.0))
            .max(self.shadow.map_or(0.0, |s| {
                s.blur * 3.0 + s.offset.0.abs().max(s.offset.1.abs())
            }))
    }
}

fn normalize3(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let l = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(1e-6);
    (v.0 / l, v.1 / l, v.2 / l)
}

/// The surface height for a bevel: 0 at the edge rising to 1 `width` pixels
/// in, then softened so the lighting has no creases or stair-steps.
fn height_map(field: &Field, b: &Bevel) -> Vec<f32> {
    let (w, h) = (field.width as usize, field.height as usize);
    let mut m: Vec<f32> = field
        .d
        .iter()
        .map(|&d| smooth(0.0, b.width.max(0.5), (-d).max(0.0)))
        .collect();
    let r = ((b.width / 5.0).round() as usize).max(1);
    let mut line = Vec::new();
    for _ in 0..2 {
        blur_lines(&mut m, h, w, 1, w, r, &mut line);
        blur_lines(&mut m, w, h, w, 1, r, &mut line);
    }
    m
}

/// A box blur of radius `r` along one axis of `m`: `lines` lines of `len`
/// values, `step` apart within a line and `stride` apart between lines.
fn blur_lines(
    m: &mut [f32],
    lines: usize,
    len: usize,
    step: usize,
    stride: usize,
    r: usize,
    line: &mut Vec<f32>,
) {
    for l in 0..lines {
        let base = l * stride;
        line.clear();
        line.extend((0..len).map(|i| m[base + i * step]));
        for i in 0..len {
            let (a, z) = (i.saturating_sub(r), (i + r).min(len - 1));
            let sum: f32 = line[a..=z].iter().sum();
            m[base + i * step] = sum / (z - a + 1) as f32;
        }
    }
}

/// The fill colour at a pixel after bevel lighting.
fn bevel_lit(hm: &[f32], w: usize, h: usize, x: usize, y: usize, base: C, b: &Bevel) -> C {
    let at = |px: usize, py: usize| hm[py.min(h - 1) * w + px.min(w - 1)];
    let hx = (at(x + 1, y) - at(x.saturating_sub(1), y)) * 0.5 * b.height;
    let hy = (at(x, y + 1) - at(x, y.saturating_sub(1))) * 0.5 * b.height;
    let n = normalize3((-hx, -hy, 1.0));
    let l = normalize3((b.light.0, b.light.1, 0.9));
    // Compared with a flat surface, so only the slopes change.
    let lit = n.0 * l.0 + n.1 * l.1 + n.2 * l.2 - l.2;
    let half = normalize3((l.0, l.1, l.2 + 1.0));
    let spec = (n.0 * half.0 + n.1 * half.1 + n.2 * half.2)
        .max(0.0)
        .powf(b.sharpness.max(1.0));
    let flat_spec = half.2.powf(b.sharpness.max(1.0));
    let glint = (spec - flat_spec).max(0.0);

    let mut c = base;
    c = lerp_c(c, b.highlight, lit.clamp(0.0, 1.0) * b.shine);
    c = lerp_c(c, b.shade, (-lit).clamp(0.0, 1.0) * b.shine * 0.85);
    for v in c.iter_mut().take(3) {
        *v = (*v + glint * b.shine).min(1.0);
    }
    c
}

fn coverage(d: f32) -> f32 {
    // One pixel of antialiasing across the edge.
    (0.5 - d).clamp(0.0, 1.0)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Premultiplied "source over destination" of a straight-alpha colour.
fn over(dst: &mut [f32; 4], c: C, a: f32) {
    let a = a.clamp(0.0, 1.0) * c[3];
    for k in 0..3 {
        dst[k] = c[k] * a + dst[k] * (1.0 - a);
    }
    dst[3] = a + dst[3] * (1.0 - a);
}

fn lerp_c(a: C, b: C, t: f32) -> C {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Paints the field with a look. The result is premultiplied RGBA.
pub fn render(field: &Field, look: &Look) -> RgbaImage {
    let (w, h) = (field.width as usize, field.height as usize);
    let mut img = RgbaImage::new(field.width, field.height);
    // The rows the shape actually covers, so the fill gradient fits the shape.
    let (mut y0, mut y1) = (h, 0);
    for y in 0..h {
        if field.d[y * w..(y + 1) * w].iter().any(|&d| d < 0.0) {
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    let span = (y1.saturating_sub(y0)).max(1) as f32;
    let heights = look.bevel.as_ref().map(|b| height_map(field, b));
    let reach = look.reach();

    for y in 0..h {
        for x in 0..w {
            let d = field.d[y * w + x];
            let mut px = [0.0f32; 4];
            if d > reach {
                continue;
            }

            if let Some(g) = look.glow {
                if d > -1.0 {
                    let a = g.strength * (-(d.max(0.0)) / g.radius.max(0.5) * 2.2).exp();
                    over(&mut px, g.color, a);
                }
            }
            if let Some(s) = look.shadow {
                let ds = field.sample(x as f32 - s.offset.0, y as f32 - s.offset.1);
                over(&mut px, s.color, 1.0 - smooth(-s.blur, s.blur.max(0.5), ds));
            }
            if let Some(e) = look.extrude {
                let steps = (e.depth.ceil() as i32).clamp(1, 48);
                let len = (e.dir.0 * e.dir.0 + e.dir.1 * e.dir.1).sqrt().max(1e-6);
                let (ux, uy) = (e.dir.0 / len, e.dir.1 / len);
                for k in (1..=steps).rev() {
                    let off = k as f32 * e.depth / steps as f32;
                    let dk = field.sample(x as f32 - ux * off, y as f32 - uy * off);
                    let near = lerp_c(look.fill.bottom, e.color, 0.35);
                    let c = lerp_c(near, e.color, k as f32 / steps as f32);
                    over(&mut px, c, coverage(dk));
                }
            }
            if let Some(o) = look.outline {
                over(&mut px, o.color, coverage(d - o.width));
            }
            let t = ((y as f32 - y0 as f32) / span).clamp(0.0, 1.0);
            let mut fill = lerp_c(look.fill.top, look.fill.bottom, t);
            if let Some(b) = &look.bevel {
                if d < 1.0 {
                    if let Some(hm) = &heights {
                        fill = bevel_lit(hm, w, h, x, y, fill, b);
                    }
                }
            }
            over(&mut px, fill, coverage(d));

            let i = (y * w + x) * 4;
            for (out, v) in img.pixels[i..i + 4].iter_mut().zip(px) {
                *out = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A filled circle as an alpha mask.
    fn disc(size: u32, r: f32) -> (Vec<u8>, Field) {
        let c = size as f32 / 2.0;
        let a: Vec<u8> = (0..size * size)
            .map(|i| {
                let (x, y) = ((i % size) as f32 + 0.5, (i / size) as f32 + 0.5);
                let d = ((x - c).powi(2) + (y - c).powi(2)).sqrt() - r;
                ((0.5 - d).clamp(0.0, 1.0) * 255.0) as u8
            })
            .collect();
        let f = Field::from_alpha(&a, size, size);
        (a, f)
    }

    fn rgba(img: &RgbaImage, w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [
            img.pixels[i],
            img.pixels[i + 1],
            img.pixels[i + 2],
            img.pixels[i + 3],
        ]
    }

    #[test]
    fn distances_match_a_circle_to_within_a_pixel() {
        let (_, f) = disc(96, 20.0);
        let mut worst = 0.0f32;
        for y in 0..96 {
            for x in 0..96 {
                let exact = (((x as f32 + 0.5) - 48.0).powi(2) + ((y as f32 + 0.5) - 48.0).powi(2))
                    .sqrt()
                    - 20.0;
                // Far away the field is exact; check everything within 25 px.
                if exact.abs() < 25.0 {
                    worst = worst.max((f.d[y * 96 + x] - exact).abs());
                }
            }
        }
        assert!(worst < 0.8, "worst error {worst} px");
    }

    #[test]
    fn signs_are_negative_inside_and_positive_outside() {
        let (_, f) = disc(64, 14.0);
        assert!(f.d[32 * 64 + 32] < -10.0);
        assert!(f.d[2] > 10.0);
        assert!(f.d[32 * 64 + 32 + 14].abs() < 1.5, "near the edge");
    }

    #[test]
    fn an_empty_or_full_mask_does_not_break() {
        let f = Field::from_alpha(&vec![0u8; 16 * 16], 16, 16);
        assert!(f.d.iter().all(|d| d.is_finite() || *d > 1e9), "empty");
        let g = Field::from_alpha(&vec![255u8; 16 * 16], 16, 16);
        assert!(g.d.iter().all(|d| d.is_finite() || *d < -1e9), "full");
    }

    fn plain(fill: C) -> Look {
        Look {
            fill: Fill {
                top: fill,
                bottom: fill,
            },
            outline: None,
            glow: None,
            shadow: None,
            extrude: None,
            bevel: None,
        }
    }

    #[test]
    fn bevel_lights_one_side_and_shades_the_other() {
        let (_, f) = disc(128, 40.0);
        let mut look = plain([0.5, 0.5, 0.5, 1.0]);
        look.bevel = Some(Bevel {
            width: 14.0,
            height: 14.0,
            light: (-0.6, -0.8),
            highlight: [1.0, 1.0, 1.0, 1.0],
            shade: [0.0, 0.0, 0.0, 1.0],
            shine: 1.0,
            sharpness: 30.0,
        });
        let img = render(&f, &look);
        let luma = |x: u32, y: u32| rgba(&img, 128, x, y)[0] as i32;
        // Just inside the top-left rim (towards the light) against the
        // bottom-right rim (away from it).
        let (near, far) = (luma(64 - 26, 64 - 26), luma(64 + 26, 64 + 26));
        assert!(near > far + 40, "lit {near} against shaded {far}");
        // The flat middle is left alone.
        assert!((luma(64, 64) - 128).abs() < 6, "middle {}", luma(64, 64));
    }

    #[test]
    fn fill_is_solid_inside_and_clear_far_outside() {
        let (_, f) = disc(64, 16.0);
        let img = render(&f, &plain([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(rgba(&img, 64, 32, 32), [255, 0, 0, 255]);
        assert_eq!(rgba(&img, 64, 2, 2)[3], 0);
    }

    #[test]
    fn outline_has_the_asked_width() {
        let (_, f) = disc(96, 16.0);
        let mut look = plain([1.0, 1.0, 1.0, 1.0]);
        look.outline = Some(Outline {
            color: [0.0, 0.0, 1.0, 1.0],
            width: 6.0,
        });
        let img = render(&f, &look);
        // Along the row through the middle, count the pixels that are blue.
        let blue = (48..96)
            .filter(|&x| {
                let p = rgba(&img, 96, x, 48);
                p[2] > 200 && p[0] < 60
            })
            .count();
        assert!(
            (5..=7).contains(&blue),
            "{blue} blue pixels for a 6 px outline"
        );
        assert_eq!(rgba(&img, 96, 94, 48)[3], 0, "nothing beyond it");
    }

    #[test]
    fn glow_fades_smoothly_with_distance() {
        let (_, f) = disc(128, 14.0);
        let mut look = plain([1.0, 1.0, 1.0, 1.0]);
        look.glow = Some(Glow {
            color: [0.2, 0.6, 1.0, 1.0],
            radius: 10.0,
            strength: 1.0,
        });
        let img = render(&f, &look);
        let alphas: Vec<u8> = (0..8)
            .map(|k| rgba(&img, 128, 64 + 14 + 2 + k * 4, 64)[3])
            .collect();
        assert!(alphas.windows(2).all(|w| w[1] <= w[0]), "{alphas:?}");
        assert!(alphas[0] > 100 && alphas[7] < alphas[0] / 2, "{alphas:?}");
    }

    #[test]
    fn extrude_adds_depth_in_its_direction_only() {
        let (_, f) = disc(96, 14.0);
        let mut look = plain([1.0, 1.0, 1.0, 1.0]);
        look.extrude = Some(Extrude {
            color: [0.1, 0.0, 0.3, 1.0],
            depth: 10.0,
            dir: (1.0, 1.0),
        });
        let img = render(&f, &look);
        // Down-right of the disc there is depth; up-left there is none.
        assert!(
            rgba(&img, 96, 48 + 14 + 5, 48 + 14 + 5)[3] > 0
                || rgba(&img, 96, 48 + 12, 48 + 8)[3] > 0
        );
        assert_eq!(rgba(&img, 96, 48 - 14 - 4, 48 - 14 - 4)[3], 0);
    }

    #[test]
    fn shadow_lands_where_it_is_told() {
        let (_, f) = disc(96, 12.0);
        let mut look = plain([1.0, 1.0, 1.0, 1.0]);
        look.shadow = Some(Shadow {
            color: [0.0, 0.0, 0.0, 1.0],
            offset: (8.0, 8.0),
            blur: 2.0,
        });
        let img = render(&f, &look);
        assert!(rgba(&img, 96, 48 + 14, 48 + 14)[3] > 100);
        assert_eq!(rgba(&img, 96, 48 - 18, 48 - 18)[3], 0);
    }
}
