//! Auto Enhance: works out colour settings that make a picture look right,
//! from a sample of its pixels.
//!
//! It looks at four things, one after the other:
//! 1. *Colour balance.* The colour of the light is estimated from the whole
//!    picture with the "shades of gray" method (more robust than a plain
//!    average, which turns a sunset blue). How far to trust it depends on how
//!    much of the picture is neutral (greys, whites, walls). A colourful scene
//!    with nothing neutral is only corrected a little.
//! 2. *Brightness.* The picture's typical brightness (measured on a
//!    logarithmic scale, as the eye does) is moved towards a natural level, by
//!    at most one stop.
//! 3. *Contrast and tone.* Black and white points are stretched when the
//!    picture does not reach them, flat pictures get a little contrast, and
//!    blown highlights and crushed shadows are eased.
//! 4. *Colour strength.* Dull pictures get more vibrance; already vivid ones
//!    are left alone.
//!
//! Everything is gentle on purpose: a good picture should come out almost as
//! it went in.

use tidycuts_render::Grade;

/// How bright a typical picture is, as light (not as a screen value).
const TARGET_LOG_LUMA: f32 = 0.14;

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Saturation (HSV) of a colour.
fn saturation(c: [f32; 3]) -> f32 {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    if mx <= 1e-4 {
        0.0
    } else {
        (mx - mn) / mx
    }
}

/// The value at fraction `q` of a list that is sorted.
fn quantile(sorted: &[f32], q: f32) -> f32 {
    sorted[((sorted.len() - 1) as f32 * q) as usize]
}

/// The temperature and tint settings (the Color tab's, in `grade.wgsl`) that
/// multiply red, green and blue by `m` (green is 1). The shader uses
/// `r = 1 + .35T - .1t`, `g = 1 + .25t`, `b = 1 - .35T - .1t`, up to a common
/// scale, which is solved here.
fn temperature_tint(m: [f32; 3]) -> (f32, f32) {
    let (r, g, b) = (m[0], m[1], m[2]);
    let s = 3.5 / (g + 1.25 * (r + b));
    let tint = (2.0 - s * (r + b)) / 0.2;
    let temperature = s * (r - b) / 0.7;
    (temperature, tint)
}

/// The shader's white balance on a linear colour.
fn white_balance(lin: [f32; 3], temperature: f32, tint: f32) -> [f32; 3] {
    [
        lin[0] * (1.0 + 0.35 * temperature - 0.1 * tint),
        lin[1] * (1.0 + 0.25 * tint),
        lin[2] * (1.0 - 0.35 * temperature - 0.1 * tint),
    ]
}

/// How many different hues a picture has, 0 (all one colour) to 1 (all
/// colours present). A scene with many hues can be assumed to average out to
/// grey, so a lean in its colour is probably the light; a scene of one hue
/// (a sunset, a ginger cat) leans on purpose.
fn hue_variety(pixels: &[[f32; 3]]) -> f32 {
    const BINS: usize = 12;
    let mut hist = [0.0f32; BINS];
    let mut total = 0.0f32;
    for p in pixels {
        let y = luma(*p);
        let mx = p[0].max(p[1]).max(p[2]);
        let mn = p[0].min(p[1]).min(p[2]);
        let chroma = mx - mn;
        if !(0.08..=0.92).contains(&y) || chroma < 0.08 {
            continue;
        }
        let h = if mx == p[0] {
            ((p[1] - p[2]) / chroma).rem_euclid(6.0)
        } else if mx == p[1] {
            (p[2] - p[0]) / chroma + 2.0
        } else {
            (p[0] - p[1]) / chroma + 4.0
        };
        let bin = ((h / 6.0 * BINS as f32) as usize).min(BINS - 1);
        hist[bin] += chroma;
        total += chroma;
    }
    if total < 1.0 {
        return 0.0;
    }
    let entropy: f32 = hist
        .iter()
        .filter(|v| **v > 0.0)
        .map(|v| {
            let q = v / total;
            -q * q.ln()
        })
        .sum();
    entropy / (BINS as f32).ln()
}

/// Colour balance from the picture's own pixels (sRGB, 0..1): the
/// temperature (within 0.25 either way) and tint (within 0.15) to apply.
fn balance(pixels: &[[f32; 3]]) -> (f32, f32) {
    // Shades of gray: the 6th-power mean of each channel in linear light, over
    // pixels that are neither black nor blown out.
    let mut acc = [0.0f64; 3];
    let mut n = 0usize;
    for p in pixels {
        let y = luma(*p);
        if !(0.05..=0.95).contains(&y) {
            continue;
        }
        n += 1;
        for (a, c) in acc.iter_mut().zip(p) {
            *a += (srgb_to_linear(*c) as f64).powi(6);
        }
    }
    if n < 50 {
        return (0.0, 0.0);
    }
    let e: Vec<f32> = acc
        .iter()
        .map(|a| (a / n as f64).powf(1.0 / 6.0) as f32)
        .collect();
    if e.iter().any(|v| *v < 1e-4) {
        return (0.0, 0.0);
    }
    // What to multiply each channel by to make the light neutral.
    let mut m = [e[1] / e[0], 1.0, e[1] / e[2]];
    // Second look: pixels that come out neutral under that guess are probably
    // grey surfaces, and show the colour of the light better than the whole
    // picture does. Lean on them when there are enough.
    let mut sum = [0.0f64; 3];
    let mut count = 0usize;
    for p in pixels {
        if !(0.1..=0.9).contains(&luma(*p)) {
            continue;
        }
        let lin = [
            srgb_to_linear(p[0]),
            srgb_to_linear(p[1]),
            srgb_to_linear(p[2]),
        ];
        let c = [
            linear_to_srgb(lin[0] * m[0]),
            linear_to_srgb(lin[1]),
            linear_to_srgb(lin[2] * m[2]),
        ];
        if saturation(c) < 0.08 {
            count += 1;
            for (s, l) in sum.iter_mut().zip(lin) {
                *s += l as f64;
            }
        }
    }
    let share = count as f32 / n as f32;
    if count >= 40 && sum.iter().all(|v| *v > 0.0) {
        let m2 = [(sum[1] / sum[0]) as f32, 1.0, (sum[1] / sum[2]) as f32];
        let w = (share * 2.0).clamp(0.0, 0.8);
        for k in [0, 2] {
            m[k] = m[k].powf(1.0 - w) * m2[k].powf(w);
        }
    }
    // How far to trust that: only when a good part of the picture comes out
    // neutral. A colourful scene with nothing grey in it (a sunset, a field of
    // flowers) is left almost alone, because its colour is meant.
    let variety = hue_variety(pixels);
    let trust = ((variety - 0.45) / 0.35).clamp(0.0, 1.0) * 0.6 + 0.1;
    // Small differences are not worth touching.
    let size = (m[0].ln().abs() + m[2].ln().abs()).min(1.0);
    let gate = ((size - 0.08) / 0.2).clamp(0.0, 1.0);
    for v in &mut m {
        *v = (1.0 + (*v - 1.0) * trust * gate).clamp(0.78, 1.3);
    }
    let (t, tint) = temperature_tint(m);
    (t.clamp(-0.25, 0.25), tint.clamp(-0.15, 0.15))
}

/// The settings for a picture, from a sample of its pixels (sRGB, 0..1).
pub fn grade_from_pixels(pixels: &[[f32; 3]]) -> Grade {
    if pixels.len() < 100 {
        return Grade::default();
    }
    let (temperature, tint) = balance(pixels);

    // Brightness: the log-average of the light after the balance.
    let mut log_sum = 0.0f64;
    let mut count = 0usize;
    for p in pixels {
        let lin = white_balance(
            [
                srgb_to_linear(p[0]),
                srgb_to_linear(p[1]),
                srgb_to_linear(p[2]),
            ],
            temperature,
            tint,
        );
        let y = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
        if y > 0.002 {
            log_sum += (y as f64).ln();
            count += 1;
        }
    }
    let mean_log = if count > 0 {
        (log_sum / count as f64).exp() as f32
    } else {
        TARGET_LOG_LUMA
    };
    let mut stops = (TARGET_LOG_LUMA / mean_log.max(1e-4)).log2() * 0.6;
    stops = stops.clamp(-1.0, 1.0);
    if stops.abs() < 0.12 {
        stops = 0.0;
    }
    // The shader multiplies the light by 2^(2 * exposure).
    let exposure = stops / 2.0;

    // What the picture looks like after balance and exposure.
    let mut after: Vec<[f32; 3]> = pixels
        .iter()
        .map(|p| {
            let lin = white_balance(
                [
                    srgb_to_linear(p[0]),
                    srgb_to_linear(p[1]),
                    srgb_to_linear(p[2]),
                ],
                temperature,
                tint,
            );
            let k = 2f32.powf(stops);
            [
                linear_to_srgb(lin[0] * k),
                linear_to_srgb(lin[1] * k),
                linear_to_srgb(lin[2] * k),
            ]
        })
        .collect();
    let mut lumas: Vec<f32> = after.iter().map(|p| luma(*p)).collect();
    lumas.sort_by(|a, b| a.total_cmp(b));
    let (lo, hi) = (quantile(&lumas, 0.005), quantile(&lumas, 0.995));
    let spread = quantile(&lumas, 0.9) - quantile(&lumas, 0.1);

    // Black and white points: only when the picture stops short of them.
    // `blacks` below 0 raises the black point; `whites` above 0 lowers the
    // white point (grade.wgsl: black point = -0.12 * blacks, white = 1 - 0.18 * whites).
    let blacks = if lo < 0.05 {
        0.0
    } else {
        (-(lo - 0.03) / 0.12 * 0.8).clamp(-1.0, 0.0)
    };
    let whites = if hi > 0.94 {
        0.0
    } else {
        ((0.97 - hi) / 0.18 * 0.8).clamp(0.0, 1.0)
    };
    // A flat picture gets a little contrast.
    let contrast = ((0.30 - spread) * 0.9).clamp(0.0, 0.3);

    // Blown highlights and crushed shadows.
    let blown = lumas.iter().filter(|y| **y > 0.97).count() as f32 / lumas.len() as f32;
    let crushed = lumas.iter().filter(|y| **y < 0.03).count() as f32 / lumas.len() as f32;
    let highlights = if blown > 0.02 {
        -(blown * 6.0).min(0.35)
    } else {
        0.0
    };
    let shadows = if crushed > 0.1 && stops <= 0.3 {
        (crushed * 0.8).min(0.25)
    } else {
        0.0
    };

    // Colour strength: lift dull pictures, leave vivid ones.
    after.retain(|p| luma(*p) > 0.12);
    let mean_sat = if after.is_empty() {
        0.3
    } else {
        after.iter().map(|p| saturation(*p)).sum::<f32>() / after.len() as f32
    };
    let vibrance = ((0.34 - mean_sat) * 1.6).clamp(0.0, 0.4);
    let saturation_boost = if mean_sat < 0.12 { 0.06 } else { 0.0 };

    Grade {
        exposure,
        contrast,
        blacks,
        whites,
        highlights,
        shadows,
        temperature,
        tint,
        vibrance,
        saturation: saturation_boost,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey-ish test picture: a ramp from dark to light with a few
    /// saturated colours, `n` pixels.
    fn picture(n: usize, f: impl Fn(f32, usize) -> [f32; 3]) -> Vec<[f32; 3]> {
        (0..n).map(|i| f(i as f32 / n as f32, i)).collect()
    }

    /// Applies a colour cast the way a camera in warm or cool light does.
    fn cast(p: [f32; 3], r: f32, b: f32) -> [f32; 3] {
        [
            linear_to_srgb(srgb_to_linear(p[0]) * r),
            p[1],
            linear_to_srgb(srgb_to_linear(p[2]) * b),
        ]
    }

    fn natural(t: f32, i: usize) -> [f32; 3] {
        // Mostly neutral surfaces with a few colours, spread through the tones.
        let v = 0.02 + 0.95 * t;
        match i % 7 {
            0 => [v, v * 0.6, v * 0.5],
            1 => [v * 0.55, v, v * 0.6],
            _ => [v, v * 0.98, v * 0.97],
        }
    }

    #[test]
    fn a_good_neutral_picture_is_left_nearly_alone() {
        let px = picture(5000, natural);
        let g = grade_from_pixels(&px);
        assert!(g.temperature.abs() < 0.08, "{g:?}");
        assert!(g.tint.abs() < 0.08, "{g:?}");
        assert!(g.exposure.abs() < 0.2, "{g:?}");
        assert!(
            g.contrast < 0.1 && g.blacks > -0.4 && g.whites < 0.4,
            "{g:?}"
        );
    }

    #[test]
    fn a_warm_cast_is_pulled_back_towards_neutral() {
        let px = picture(5000, |t, i| cast(natural(t, i), 1.35, 0.7));
        let g = grade_from_pixels(&px);
        // Warm light: the picture should be cooled (temperature below 0).
        assert!(g.temperature < -0.1, "{g:?}");
        // Never beyond the safety limit.
        assert!(g.temperature >= -0.25, "{g:?}");
    }

    #[test]
    fn a_cool_cast_is_warmed() {
        let px = picture(5000, |t, i| cast(natural(t, i), 0.7, 1.35));
        let g = grade_from_pixels(&px);
        assert!(g.temperature > 0.1, "{g:?}");
    }

    #[test]
    fn the_correction_moves_the_average_colour_closer_to_neutral() {
        for (r, b) in [(1.3, 0.7), (0.75, 1.3)] {
            let px = picture(6000, |t, i| cast(natural(t, i), r, b));
            let g = grade_from_pixels(&px);
            let mean = |f: &dyn Fn([f32; 3]) -> [f32; 3]| {
                let mut s = [0.0f32; 3];
                for p in &px {
                    let q = f(*p);
                    for k in 0..3 {
                        s[k] += srgb_to_linear(q[k]);
                    }
                }
                let n = px.len() as f32;
                [s[0] / n, s[1] / n, s[2] / n]
            };
            let off = |m: [f32; 3]| (m[0] / m[1]).ln().abs() + (m[2] / m[1]).ln().abs();
            let before = off(mean(&|p| p));
            let after = off(mean(&|p| {
                let lin = white_balance(
                    [
                        srgb_to_linear(p[0]),
                        srgb_to_linear(p[1]),
                        srgb_to_linear(p[2]),
                    ],
                    g.temperature,
                    g.tint,
                );
                [lin[0], lin[1], lin[2]].map(linear_to_srgb)
            }));
            assert!(
                after < before * 0.92,
                "cast ({r}, {b}): {before} -> {after}"
            );
        }
    }

    #[test]
    fn a_colourful_scene_without_neutrals_is_only_corrected_a_little() {
        // A sunset: oranges and reds everywhere, no grey at all.
        let px = picture(5000, |t, i| {
            let v = 0.2 + 0.7 * t;
            [v, v * (0.45 + 0.1 * ((i % 5) as f32 / 5.0)), v * 0.18]
        });
        let g = grade_from_pixels(&px);
        assert!(g.temperature.abs() <= 0.25 && g.tint.abs() <= 0.15, "{g:?}");
        // The scene is warm on purpose: never push it past what the clamp allows
        // and never more than a partial correction.
        let (lo, hi) = (0.78f32, 1.3f32);
        let (t, tint) = temperature_tint([hi, 1.0, lo]);
        assert!(g.temperature.abs() <= t.abs().max(tint.abs()) + 0.1);
    }

    #[test]
    fn dark_pictures_are_brightened_and_bright_ones_toned_down_by_at_most_a_stop() {
        let dark = picture(5000, |t, i| {
            let c = natural(t, i);
            [c[0] * 0.25, c[1] * 0.25, c[2] * 0.25]
        });
        let g = grade_from_pixels(&dark);
        assert!(g.exposure > 0.1 && g.exposure <= 0.5, "{g:?}");
        let bright = picture(5000, |t, i| {
            let c = natural(t, i);
            [
                (c[0] * 0.5 + 0.5).min(1.0),
                (c[1] * 0.5 + 0.5).min(1.0),
                (c[2] * 0.5 + 0.5).min(1.0),
            ]
        });
        let g = grade_from_pixels(&bright);
        assert!(g.exposure < -0.05 && g.exposure >= -0.5, "{g:?}");
    }

    #[test]
    fn flat_hazy_pictures_get_their_black_and_white_points_stretched() {
        let flat = picture(5000, |t, i| {
            let c = natural(t, i);
            [0.3 + 0.35 * c[0], 0.3 + 0.35 * c[1], 0.3 + 0.35 * c[2]]
        });
        let g = grade_from_pixels(&flat);
        assert!(g.blacks < -0.1, "{g:?}");
        assert!(g.whites > 0.05, "{g:?}");
        assert!(g.contrast > 0.0, "{g:?}");
    }

    #[test]
    fn pale_pictures_get_vibrance_and_vivid_ones_do_not() {
        let pale = picture(5000, |t, _| {
            let v = 0.1 + 0.8 * t;
            [v, v * 0.97, v * 0.95]
        });
        assert!(grade_from_pixels(&pale).vibrance > 0.15);
        let vivid = picture(5000, |t, i| {
            let v = 0.2 + 0.7 * t;
            match i % 3 {
                0 => [v, v * 0.2, v * 0.2],
                1 => [v * 0.2, v, v * 0.2],
                _ => [v * 0.2, v * 0.2, v],
            }
        });
        assert!(grade_from_pixels(&vivid).vibrance < 0.05);
    }

    #[test]
    fn the_temperature_and_tint_solution_reproduces_the_multipliers() {
        for m in [
            [1.2, 1.0, 0.85],
            [0.9, 1.0, 1.15],
            [1.1, 1.0, 1.1],
            [1.0, 1.0, 1.0],
        ] {
            let (t, tint) = temperature_tint(m);
            let got = white_balance([1.0; 3], t, tint);
            // Equal up to one common scale.
            let s = got[1] / m[1];
            for k in 0..3 {
                assert!((got[k] / s - m[k]).abs() < 0.01, "{m:?} -> {got:?}");
            }
        }
    }

    #[test]
    fn too_few_pixels_change_nothing() {
        assert_eq!(grade_from_pixels(&[[0.5; 3]; 10]).temperature, 0.0);
    }
}
