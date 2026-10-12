//! Deterministic noise for built-in assets drawn on the CPU (stickers, titles,
//! previews): value noise in 2D and 3D, fractal sums, domain warping and a
//! divergence-free "curl" field for smoke-like motion. The GPU has its own
//! copies in `shaders/common.wgsl`.
//!
//! The third coordinate is usually time, so a pattern can flow without ever
//! popping. Every function is a pure function of its arguments.

/// A well-mixed hash of three integers and a seed, evenly spread over 0..1.
/// Use this, and not the smooth noise below, whenever you need random numbers.
pub fn hash3(ix: i32, iy: i32, iz: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x8DA6_B343)
        ^ (iy as u32).wrapping_mul(0xD816_3841)
        ^ (iz as u32).wrapping_mul(0xCB1A_B31F)
        ^ seed.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA77);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smooth value noise in 0..1 over three coordinates.
pub fn value3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let (u, v, w) = (fade(x - fx), fade(y - fy), fade(z - fz));
    let c = |dx, dy, dz| hash3(ix + dx, iy + dy, iz + dz, seed);
    let x00 = mix(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = mix(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = mix(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = mix(c(0, 1, 1), c(1, 1, 1), u);
    mix(mix(x00, x10, v), mix(x01, x11, v), w)
}

/// Smooth value noise in 0..1 over two coordinates.
pub fn value2(x: f32, y: f32, seed: u32) -> f32 {
    value3(x, y, 0.0, seed)
}

/// Fractal noise: `octaves` layers, each twice as fine and half as strong,
/// scaled back to 0..1. Slightly different offsets per layer hide the grid.
pub fn fbm3(x: f32, y: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
    let (mut fx, mut fy, mut fz) = (x, y, z);
    for o in 0..octaves.max(1) {
        sum += amp * value3(fx, fy, fz, seed.wrapping_add(o * 101));
        norm += amp;
        amp *= 0.5;
        fx = fx * 2.03 + 17.1;
        fy = fy * 2.03 + 9.2;
        fz = fz * 2.03 + 3.7;
    }
    sum / norm
}

pub fn fbm2(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    fbm3(x, y, 0.0, octaves, seed)
}

/// Domain-warped fractal noise: the noise is sampled at points pushed around
/// by two other noises. It looks like drifting smoke or flame instead of
/// clouds. `t` makes it flow.
pub fn warped(x: f32, y: f32, t: f32, seed: u32) -> f32 {
    let qx = fbm3(x, y, t, 4, seed);
    let qy = fbm3(x + 5.2, y + 1.3, t, 4, seed.wrapping_add(7));
    fbm3(
        x + 3.5 * qx,
        y + 3.5 * qy,
        t * 0.8,
        4,
        seed.wrapping_add(13),
    )
}

/// A divergence-free vector field made from the noise: particles pushed by
/// it swirl around each other and never bunch up or drain away.
pub fn curl(x: f32, y: f32, t: f32, seed: u32) -> (f32, f32) {
    let e = 0.01;
    let n = |px, py| fbm3(px, py, t, 3, seed);
    let dx = n(x + e, y) - n(x - e, y);
    let dy = n(x, y + e) - n(x, y - e);
    (dy / (2.0 * e), -dx / (2.0 * e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic_and_in_range() {
        for i in 0..400 {
            let (x, y, z) = (i as f32 * 0.173 - 20.0, i as f32 * 0.311, i as f32 * 0.05);
            let v = value3(x, y, z, 3);
            assert!((0.0..=1.0).contains(&v), "value3 {v}");
            assert_eq!(v, value3(x, y, z, 3));
            let f = fbm3(x, y, z, 5, 3);
            assert!((0.0..=1.0).contains(&f), "fbm3 {f}");
            let w = warped(x, y, z, 3);
            assert!((0.0..=1.0).contains(&w), "warped {w}");
        }
        assert_ne!(value2(1.5, 2.5, 1), value2(1.5, 2.5, 2), "seed changes it");
    }

    #[test]
    fn noise_is_smooth_and_spread_out() {
        // Small steps never jump.
        let mut worst = 0.0f32;
        let mut prev = value2(0.0, 0.7, 9);
        for i in 1..2000 {
            let v = value2(i as f32 * 0.002, 0.7, 9);
            worst = worst.max((v - prev).abs());
            prev = v;
        }
        assert!(worst < 0.05, "largest step {worst}");
        // It uses most of its range, so effects built on it have contrast.
        let vals: Vec<f32> = (0..4000)
            .map(|i| fbm2(i as f32 * 0.37, (i / 7) as f32 * 0.61, 4, 5))
            .collect();
        let (lo, hi) = vals
            .iter()
            .fold((1.0f32, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi - lo > 0.4, "range {lo}..{hi}");
    }

    #[test]
    fn time_flows_without_popping() {
        let a = warped(1.0, 2.0, 0.0, 4);
        let b = warped(1.0, 2.0, 0.01, 4);
        let c = warped(1.0, 2.0, 5.0, 4);
        assert!(
            (a - b).abs() < 0.03,
            "tiny time step moved {}",
            (a - b).abs()
        );
        assert!((a - c).abs() > 0.001, "but it does flow");
    }

    #[test]
    fn curl_is_divergence_free() {
        let (mut div, mut mag) = (0.0f32, 0.0f32);
        let h = 0.05;
        for i in 0..200 {
            let (x, y) = (i as f32 * 0.137, (i % 17) as f32 * 0.29);
            let (cx1, _) = curl(x + h, y, 0.3, 2);
            let (cx0, _) = curl(x - h, y, 0.3, 2);
            let (_, cy1) = curl(x, y + h, 0.3, 2);
            let (_, cy0) = curl(x, y - h, 0.3, 2);
            div += ((cx1 - cx0) / (2.0 * h) + (cy1 - cy0) / (2.0 * h)).abs();
            let (cx, cy) = curl(x, y, 0.3, 2);
            mag += (cx * cx + cy * cy).sqrt();
        }
        assert!(mag > 1.0, "the field is not empty ({mag})");
        assert!(div < mag * 0.6, "divergence {div} against size {mag}");
    }
}
