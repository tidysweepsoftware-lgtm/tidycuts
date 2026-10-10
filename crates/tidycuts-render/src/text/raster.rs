//! Pixel helpers for titles: alpha masks, a fast box blur, directional
//! dilation for extruded shadows and compositing of tinted masks onto
//! premultiplied RGBA8 buffers. Plus a few path shapes the styles use.

use tiny_skia::{Path, PathBuilder, Rect};

/// The alpha channel of a premultiplied RGBA8 buffer.
pub fn alpha_of(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
}

/// A soft copy of `a` blurred by about `radius` pixels. Wide blurs run on
/// a smaller copy of the mask and are scaled back up, which looks the same
/// (the result is smooth anyway) and costs a fraction of the time.
pub fn blurred(a: &[u8], w: usize, h: usize, radius: f32) -> Vec<u8> {
    let k = (radius / 2.5).floor().clamp(1.0, 8.0) as usize;
    if k == 1 || w < 4 * k || h < 4 * k {
        let mut out = a.to_vec();
        blur(&mut out, w, h, radius);
        return out;
    }
    let (sw, sh) = (w.div_ceil(k), h.div_ceil(k));
    // Box-average k x k blocks.
    let mut small = vec![0u8; sw * sh];
    let mut acc = vec![0u32; sw];
    for sy in 0..sh {
        acc.iter_mut().for_each(|v| *v = 0);
        for y in sy * k..((sy + 1) * k).min(h) {
            for (x, &v) in a[y * w..(y + 1) * w].iter().enumerate() {
                acc[x / k] += v as u32;
            }
        }
        let n = (k * k) as u32;
        for (o, &v) in small[sy * sw..(sy + 1) * sw].iter_mut().zip(&acc) {
            *o = ((v + n / 2) / n) as u8;
        }
    }
    blur(&mut small, sw, sh, radius / k as f32);
    // Bilinear back up: horizontally into a sw-high strip, then vertically.
    let taps = |n: usize, sn: usize| -> Vec<(usize, usize, u32)> {
        (0..n)
            .map(|i| {
                let f = ((i as f32 + 0.5) / k as f32 - 0.5).clamp(0.0, (sn - 1) as f32);
                let i0 = f.floor() as usize;
                let i1 = (i0 + 1).min(sn - 1);
                (i0, i1, ((f - i0 as f32) * 256.0) as u32)
            })
            .collect()
    };
    let (tx, ty) = (taps(w, sw), taps(h, sh));
    let mut wide = vec![0u8; w * sh];
    for sy in 0..sh {
        let row = &small[sy * sw..(sy + 1) * sw];
        for (o, &(i0, i1, f)) in wide[sy * w..(sy + 1) * w].iter_mut().zip(&tx) {
            *o = ((row[i0] as u32 * (256 - f) + row[i1] as u32 * f) >> 8) as u8;
        }
    }
    let mut out = vec![0u8; w * h];
    for (y, &(i0, i1, f)) in ty.iter().enumerate() {
        let (r0, r1) = (&wide[i0 * w..(i0 + 1) * w], &wide[i1 * w..(i1 + 1) * w]);
        for ((o, &p), &q) in out[y * w..(y + 1) * w].iter_mut().zip(r0).zip(r1) {
            *o = ((p as u32 * (256 - f) + q as u32 * f) >> 8) as u8;
        }
    }
    out
}

/// Approximately a Gaussian blur of `radius` pixels: three box blurs per
/// axis, each O(pixels) whatever the radius.
pub fn blur(a: &mut Vec<u8>, w: usize, h: usize, radius: f32) {
    let r = radius.round().max(0.0) as usize;
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let r = r.min(w.max(h));
    let mut tmp = vec![0u8; a.len()];
    for _ in 0..3 {
        blur_h(a, &mut tmp, w, h, r);
        std::mem::swap(a, &mut tmp);
    }
    for _ in 0..3 {
        blur_v(a, &mut tmp, w, h, r);
        std::mem::swap(a, &mut tmp);
    }
}

fn reciprocal(d: usize) -> u64 {
    ((1u64 << 24) + d as u64 / 2) / d as u64
}

#[inline]
fn scaled(sum: u32, inv: u64) -> u8 {
    ((sum as u64 * inv + (1 << 23)) >> 24).min(255) as u8
}

fn blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let inv = reciprocal(2 * r + 1);
    for (s, o) in src.chunks_exact(w).zip(dst.chunks_exact_mut(w)).take(h) {
        let mut sum: u32 = s[..(r + 1).min(w)].iter().map(|&v| v as u32).sum();
        for x in 0..w {
            o[x] = scaled(sum, inv);
            if x + r + 1 < w {
                sum += s[x + r + 1] as u32;
            }
            if x >= r {
                sum -= s[x - r] as u32;
            }
        }
    }
}

fn blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let inv = reciprocal(2 * r + 1);
    let mut acc = vec![0u32; w];
    for row in src.chunks_exact(w).take((r + 1).min(h)) {
        for (a, &v) in acc.iter_mut().zip(row) {
            *a += v as u32;
        }
    }
    for y in 0..h {
        for (o, &a) in dst[y * w..(y + 1) * w].iter_mut().zip(&acc) {
            *o = scaled(a, inv);
        }
        if y + r + 1 < h {
            for (a, &v) in acc.iter_mut().zip(&src[(y + r + 1) * w..(y + r + 2) * w]) {
                *a += v as u32;
            }
        }
        if y >= r {
            for (a, &v) in acc.iter_mut().zip(&src[(y - r) * w..(y - r + 1) * w]) {
                *a -= v as u32;
            }
        }
    }
}

/// Pushes a mask back along the diagonal (1, 1) by up to `n` pixels, as
/// for an extruded or long shadow. Returns the coverage (the strongest
/// alpha reaching each pixel) and the distance in steps to the nearest
/// part of the original mask, computed by doubling in O(pixels * log n).
pub fn extrude(a: &[u8], w: usize, h: usize, n: usize) -> (Vec<u8>, Vec<u16>) {
    let mut cov = a.to_vec();
    let mut dist: Vec<u16> = a
        .iter()
        .map(|&v| if v > 0 { 0 } else { u16::MAX })
        .collect();
    let n = n.min(u16::MAX as usize / 2);
    let target = n + 1;
    let mut covered = 1usize;
    while covered < target {
        let s = covered.min(target - covered);
        // In place, bottom-right to top-left, so every read is still the
        // previous pass's value.
        for y in (s..h).rev() {
            for x in (s..w).rev() {
                let q = (y - s) * w + x - s;
                let c = cov[q];
                if c == 0 {
                    continue;
                }
                let p = y * w + x;
                let d = dist[q].saturating_add(s as u16);
                if d < dist[p] {
                    dist[p] = d;
                }
                if c > cov[p] {
                    cov[p] = c;
                }
            }
        }
        covered += s;
    }
    (cov, dist)
}

/// Paints an extrusion: colors run from `near` to `far` with the distance;
/// `fade` makes the far end transparent.
#[allow(clippy::too_many_arguments)]
pub fn paint_extrusion(
    dst: &mut [u8],
    cov: &[u8],
    dist: &[u16],
    len: usize,
    near: [f32; 4],
    far: [f32; 4],
    fade: bool,
) {
    let len = len.max(1);
    let lut: Vec<[u32; 4]> = (0..=len)
        .map(|d| {
            let t = d as f32 / len as f32;
            let mut c = [0.0; 4];
            for k in 0..4 {
                c[k] = near[k] + (far[k] - near[k]) * t;
            }
            if fade {
                c[3] *= 1.0 - t * t;
            }
            [to8(c[0]), to8(c[1]), to8(c[2]), to8(c[3])]
        })
        .collect();
    for ((&m, &d), p) in cov
        .iter()
        .zip(dist)
        .zip(dst.as_chunks_mut::<4>().0.iter_mut())
    {
        if m == 0 {
            continue;
        }
        let c = lut[(d as usize).min(len)];
        let sa = div255(m as u32 * c[3]);
        if sa == 0 {
            continue;
        }
        let inv = 255 - sa;
        p[0] = (div255(c[0] * sa) + div255(p[0] as u32 * inv)) as u8;
        p[1] = (div255(c[1] * sa) + div255(p[1] as u32 * inv)) as u8;
        p[2] = (div255(c[2] * sa) + div255(p[2] as u32 * inv)) as u8;
        p[3] = (sa + div255(p[3] as u32 * inv)) as u8;
    }
}

#[inline]
fn div255(x: u32) -> u32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

fn to8(c: f32) -> u32 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// Paints `color` (straight alpha) through `mask` moved by (dx, dy) onto a
/// premultiplied RGBA8 buffer. `additive` adds light instead of covering.
#[allow(clippy::too_many_arguments)]
pub fn paint_mask(
    dst: &mut [u8],
    w: usize,
    h: usize,
    mask: &[u8],
    dx: i32,
    dy: i32,
    color: [f32; 4],
    additive: bool,
) {
    let ca = to8(color[3]);
    if ca == 0 {
        return;
    }
    let (cr, cg, cb) = (to8(color[0]), to8(color[1]), to8(color[2]));
    let (x0, x1) = (dx.max(0) as usize, (w as i32 + dx.min(0)).max(0) as usize);
    if x0 >= x1 {
        return;
    }
    for y in 0..h {
        let sy = y as i32 - dy;
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        let sx0 = (x0 as i32 - dx) as usize;
        let mrow = &mask[sy as usize * w + sx0..sy as usize * w + sx0 + (x1 - x0)];
        let drow = &mut dst[(y * w + x0) * 4..(y * w + x1) * 4];
        for (&m, p) in mrow.iter().zip(drow.as_chunks_mut::<4>().0.iter_mut()) {
            if m == 0 {
                continue;
            }
            let sa = div255(m as u32 * ca);
            if sa == 0 {
                continue;
            }
            let (sr, sg, sb) = (div255(cr * sa), div255(cg * sa), div255(cb * sa));
            if additive {
                let a = sa + div255(p[3] as u32 * (255 - sa));
                p[0] = (p[0] as u32 + sr).min(a) as u8;
                p[1] = (p[1] as u32 + sg).min(a) as u8;
                p[2] = (p[2] as u32 + sb).min(a) as u8;
                p[3] = a as u8;
            } else {
                let inv = 255 - sa;
                p[0] = (sr + div255(p[0] as u32 * inv)) as u8;
                p[1] = (sg + div255(p[1] as u32 * inv)) as u8;
                p[2] = (sb + div255(p[2] as u32 * inv)) as u8;
                p[3] = (sa + div255(p[3] as u32 * inv)) as u8;
            }
        }
    }
}

/// Draws premultiplied `src` over `dst`, each row moved sideways by
/// `shift(row)` pixels (the glitch slices).
pub fn over_rows(dst: &mut [u8], src: &[u8], w: usize, h: usize, shift: impl Fn(usize) -> i32) {
    for y in 0..h {
        let dx = shift(y);
        let (x0, x1) = (dx.max(0) as usize, (w as i32 + dx.min(0)).max(0) as usize);
        if x0 >= x1 {
            continue;
        }
        let sx0 = (x0 as i32 - dx) as usize;
        let srow = &src[(y * w + sx0) * 4..(y * w + sx0 + (x1 - x0)) * 4];
        let drow = &mut dst[(y * w + x0) * 4..(y * w + x1) * 4];
        for (s, d) in srow
            .as_chunks::<4>()
            .0
            .iter()
            .zip(drow.as_chunks_mut::<4>().0.iter_mut())
        {
            let sa = s[3] as u32;
            if sa == 0 {
                continue;
            }
            if sa == 255 {
                d.copy_from_slice(s);
                continue;
            }
            let inv = 255 - sa;
            for k in 0..4 {
                d[k] = (s[k] as u32 + div255(d[k] as u32 * inv)).min(255) as u8;
            }
        }
    }
}

// ----- Shapes -----

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    if !(w > 0.05 && h > 0.05) {
        return None;
    }
    Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?))
}

/// A rectangle with rounded corners (radius clamped to fit).
pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if !(w > 0.05 && h > 0.05) {
        return None;
    }
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    if r < 0.05 {
        return rect(x, y, w, h);
    }
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn circle(x: f32, y: f32, r: f32) -> Option<Path> {
    if r <= 0.05 {
        return None;
    }
    PathBuilder::from_circle(x, y, r)
}

pub fn polygon(pts: &[(f32, f32)]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish()
}

/// The first `frac` (0..1) of a polyline, measured along its length.
pub fn partial_polyline(pts: &[(f32, f32)], frac: f32) -> Option<Path> {
    if pts.len() < 2 || frac <= 0.001 {
        return None;
    }
    let total: f32 = pts.windows(2).map(|s| dist(s[0], s[1])).sum();
    let mut left = total * frac.min(1.0);
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for s in pts.windows(2) {
        let l = dist(s[0], s[1]);
        if l >= left {
            let t = if l > 0.0 { left / l } else { 0.0 };
            pb.line_to(
                s[0].0 + (s[1].0 - s[0].0) * t,
                s[0].1 + (s[1].1 - s[0].1) * t,
            );
            break;
        }
        pb.line_to(s[1].0, s[1].1);
        left -= l;
    }
    pb.finish()
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_energy_and_spreads() {
        let (w, h) = (40, 30);
        let mut a = vec![0u8; w * h];
        a[15 * w + 20] = 255;
        blur(&mut a, w, h, 2.0);
        let sum: u32 = a.iter().map(|&v| v as u32).sum();
        assert!(sum > 150 && sum < 360, "{sum}");
        assert!(a[15 * w + 23] > 0);
        assert_eq!(a[0], 0);
    }

    #[test]
    fn scaled_blur_matches_full_blur() {
        let (w, h) = (120, 80);
        let mut a = vec![0u8; w * h];
        for y in 30..50 {
            for x in 40..80 {
                a[y * w + x] = 255;
            }
        }
        let fast = blurred(&a, w, h, 9.0);
        let mut slow = a.clone();
        blur(&mut slow, w, h, 9.0);
        let diff = fast
            .iter()
            .zip(&slow)
            .map(|(&p, &q)| (p as i32 - q as i32).abs())
            .max()
            .unwrap();
        assert!(diff < 40, "{diff}");
        assert!(fast[40 * w + 60] > 100 && fast[2 * w + 2] == 0);
    }

    #[test]
    fn extrusion_covers_every_step_with_its_distance() {
        let (w, h) = (20, 20);
        let mut a = vec![0u8; w * h];
        a[2 * w + 2] = 200;
        let (cov, dist) = extrude(&a, w, h, 9);
        for k in 0..=9 {
            assert_eq!(cov[(2 + k) * w + 2 + k], 200, "step {k}");
            assert_eq!(dist[(2 + k) * w + 2 + k], k as u16, "step {k}");
        }
        assert_eq!(cov[12 * w + 12], 0);
    }
}
