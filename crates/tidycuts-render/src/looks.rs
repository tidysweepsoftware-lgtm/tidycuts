//! The Color tab's HSL, curves and LUT, resolved for one moment and baked
//! into what the grade pass reads: a small table (each channel's curve, and
//! the HSL change for every hue) plus a 3D LUT.

use crate::lut::{self, Lut3d};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tidycuts_core::{ColorAdjust, Time, Wheel, Wheels, HSL_RANGES};

/// Entries per row of the tone table.
pub const TONE_SIZE: usize = 256;

/// Largest hue shift of an HSL range, in degrees (at ±100).
pub const HSL_HUE_SHIFT: f32 = 30.0;

/// Two rows of `TONE_SIZE` RGBA values.
///
/// Row 0, entry i: the red, green and blue curves (after the RGB curve) at
/// input i/255, and the RGB curve alone. Row 1, entry i: for the hue i/256
/// of a turn, the hue shift in turns, the saturation change and the
/// luminance change (-1..1).
#[derive(Debug, Clone, PartialEq)]
pub struct ToneTable {
    /// Content hash, so the GPU keeps one copy of each table.
    pub key: u64,
    pub data: Vec<f32>,
}

impl ToneTable {
    fn new(data: Vec<f32>) -> ToneTable {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in &data {
            v.to_bits().hash(&mut h);
        }
        ToneTable {
            key: h.finish(),
            data,
        }
    }
}

/// The extra looks of one grade pass.
#[derive(Debug, Clone)]
pub struct Looks {
    pub tone: Option<ToneTable>,
    /// Whether the tone table's curves row is used.
    pub curves: bool,
    /// Whether the tone table's HSL row is used.
    pub hsl: bool,
    pub lut: Option<Arc<Lut3d>>,
    /// How much of the LUT shows, 0..1.
    pub lut_strength: f32,
}

impl PartialEq for Looks {
    fn eq(&self, o: &Looks) -> bool {
        self.tone == o.tone
            && self.curves == o.curves
            && self.hsl == o.hsl
            && self.lut.as_ref().map(|l| l.key) == o.lut.as_ref().map(|l| l.key)
            && self.lut_strength == o.lut_strength
    }
}

/// The HSL, curves and LUT of a clip's color settings at `lt` (time from
/// the clip's start), or `None` when they leave the picture unchanged. A
/// LUT whose file can't be read is skipped.
pub fn for_color(c: &ColorAdjust, lt: Time) -> Option<Arc<Looks>> {
    let hsl: Vec<[f32; 3]> = c
        .hsl
        .bands()
        .iter()
        .map(|b| {
            [
                b.hue.value_at(lt) as f32 / 100.0,
                b.saturation.value_at(lt) as f32 / 100.0,
                b.luminance.value_at(lt) as f32 / 100.0,
            ]
        })
        .collect();
    let use_hsl = hsl.iter().flatten().any(|v| *v != 0.0);
    let use_curves = !c.curves.is_identity() || !c.wheels.is_neutral();
    let lut = c.lut.as_ref().and_then(|l| {
        let strength = (l.strength.value_at(lt) / 100.0).clamp(0.0, 1.0) as f32;
        if strength <= 0.0 {
            return None;
        }
        lut::load(&l.source).ok().map(|lut| (lut, strength))
    });
    if !use_hsl && !use_curves && lut.is_none() {
        return None;
    }
    let tone = (use_hsl || use_curves).then(|| {
        let mut data = vec![0f32; TONE_SIZE * 2 * 4];
        if use_curves {
            curves_row(c, &mut data[..TONE_SIZE * 4]);
        }
        if use_hsl {
            hsl_row(&hsl, &mut data[TONE_SIZE * 4..]);
        }
        ToneTable::new(data)
    });
    let (lut, lut_strength) = match lut {
        Some((l, s)) => (Some(l), s),
        None => (None, 0.0),
    };
    Some(Arc::new(Looks {
        tone,
        curves: use_curves,
        hsl: use_hsl,
        lut,
        lut_strength,
    }))
}

/// How far each wheel can push (at the rim of the disc, or the end of the
/// master slider): lift and offset add, gain multiplies and gamma bends.
const LIFT_PUSH: f32 = 0.20;
const GAMMA_PUSH: f32 = 0.45;
const GAIN_PUSH: f32 = 0.50;
const OFFSET_PUSH: f32 = 0.10;
const LIFT_MASTER: f32 = 0.25;
const GAMMA_MASTER: f32 = 0.60;
const GAIN_MASTER: f32 = 0.60;
const OFFSET_MASTER: f32 = 0.12;

/// Push and master push of the Lift, Gamma, Gain and Offset wheels, in that
/// order (for showing what a wheel does).
pub const WHEEL_STRENGTH: [(f32, f32); 4] = [
    (LIFT_PUSH, LIFT_MASTER),
    (GAMMA_PUSH, GAMMA_MASTER),
    (GAIN_PUSH, GAIN_MASTER),
    (OFFSET_PUSH, OFFSET_MASTER),
];

/// The red, green and blue amounts a wheel adds: the puck's direction picks
/// the colour (pushing towards red adds red and takes a little green and
/// blue), its distance from the middle how much, and the master slider adds
/// the same to all three.
pub fn wheel_rgb(w: &Wheel, push: f32, master_push: f32) -> [f32; 3] {
    let (x, y) = (w.x as f32, w.y as f32);
    let amount = (x * x + y * y).sqrt().min(1.0);
    let hue = y.atan2(x).to_degrees().rem_euclid(360.0);
    // Hue to colour, then away from gray so equal amounts cancel out.
    let h = hue / 60.0;
    let f = |n: f32| (((h + n).rem_euclid(6.0) - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let rgb = [f(0.0), f(4.0), f(2.0)];
    let mean = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
    let m = w.master as f32 * master_push;
    [0, 1, 2].map(|k| (rgb[k] - mean) * 1.5 * amount * push + m)
}

/// What the four wheels do to one channel's value, 0..1 in and out: gain
/// stretches it, lift raises or lowers the darks, gamma bends the middle
/// and offset slides everything.
pub fn wheels_curve(wheels: &Wheels, channel: usize) -> impl Fn(f32) -> f32 {
    let lift = wheel_rgb(&wheels.lift, LIFT_PUSH, LIFT_MASTER)[channel];
    let gamma = wheel_rgb(&wheels.gamma, GAMMA_PUSH, GAMMA_MASTER)[channel];
    let gain = wheel_rgb(&wheels.gain, GAIN_PUSH, GAIN_MASTER)[channel];
    let offset = wheel_rgb(&wheels.offset, OFFSET_PUSH, OFFSET_MASTER)[channel];
    move |v: f32| {
        let v = v * (1.0 + gain);
        let v = v + lift * (1.0 - v);
        let v = v.max(0.0).powf(1.0 / (1.0 + gamma).max(0.2));
        (v + offset).clamp(0.0, 1.0)
    }
}

fn curves_row(c: &ColorAdjust, out: &mut [f32]) {
    let cv = &c.curves;
    let master = cv.rgb.spline();
    let channels = [cv.red.spline(), cv.green.spline(), cv.blue.spline()];
    let wheels = [0, 1, 2].map(|k| wheels_curve(&c.wheels, k));
    let wheels_on = !c.wheels.is_neutral();
    for i in 0..TONE_SIZE {
        let x = i as f64 / (TONE_SIZE - 1) as f64;
        let m = master.eval(x);
        for (k, s) in channels.iter().enumerate() {
            // The wheels come first, then the curves.
            let input = if wheels_on {
                wheels[k](x as f32) as f64
            } else {
                x
            };
            let m_k = if wheels_on { master.eval(input) } else { m };
            out[i * 4 + k] = s.eval(m_k) as f32;
        }
        out[i * 4 + 3] = m as f32;
    }
}

/// How much each HSL range affects `hue_deg`: full at its centre, fading
/// out (cosine-shaped) by the next range's centre. The weights add to 1.
pub fn range_weights(hue_deg: f32) -> [f32; 8] {
    let h = hue_deg.rem_euclid(360.0);
    let mut w = [0f32; 8];
    for k in 0..8 {
        let a = HSL_RANGES[k].1 as f32;
        let b = if k == 7 {
            360.0
        } else {
            HSL_RANGES[k + 1].1 as f32
        };
        if h >= a && h < b {
            let t = (h - a) / (b - a);
            let near = 0.5 + 0.5 * (std::f32::consts::PI * t).cos();
            w[k] = near;
            w[(k + 1) % 8] = 1.0 - near;
        }
    }
    w
}

fn hsl_row(bands: &[[f32; 3]], out: &mut [f32]) {
    for i in 0..TONE_SIZE {
        let w = range_weights(i as f32 * 360.0 / TONE_SIZE as f32);
        let mut d = [0f32; 3];
        for (b, wk) in bands.iter().zip(w) {
            for k in 0..3 {
                d[k] += b[k] * wk;
            }
        }
        out[i * 4] = d[0] * HSL_HUE_SHIFT / 360.0;
        out[i * 4 + 1] = d[1];
        out[i * 4 + 2] = d[2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_core::{Animatable, ColorLut, Curve, LutSource};

    #[test]
    fn neutral_settings_bake_nothing() {
        assert!(for_color(&ColorAdjust::default(), Time::ZERO).is_none());
        let mut c = ColorAdjust {
            lut: Some(ColorLut {
                source: LutSource::BuiltIn("warm".into()),
                strength: Animatable::constant(0.0),
            }),
            ..Default::default()
        };
        assert!(for_color(&c, Time::ZERO).is_none());
        // A missing file is skipped rather than failing.
        c.lut = Some(ColorLut::new(LutSource::File("/missing/x.cube".into())));
        assert!(for_color(&c, Time::ZERO).is_none());
    }

    #[test]
    fn weights_cover_every_hue() {
        for i in 0..720 {
            let w = range_weights(i as f32 * 0.5);
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        }
        assert_eq!(range_weights(0.0)[0], 1.0);
        assert_eq!(range_weights(120.0)[3], 1.0);
        let between = range_weights(330.0);
        assert!(between[7] > 0.4 && between[0] > 0.4);
    }

    #[test]
    fn red_hue_shift_lands_in_the_table() {
        let mut c = ColorAdjust::default();
        c.hsl.red.hue = Animatable::constant(100.0);
        let looks = for_color(&c, Time::ZERO).unwrap();
        assert!(looks.hsl && !looks.curves && looks.lut.is_none());
        let t = &looks.tone.as_ref().unwrap().data;
        let row = &t[TONE_SIZE * 4..];
        assert!((row[0] - 30.0 / 360.0).abs() < 1e-6);
        // Green (hue 120°, entry 85⅓) is untouched.
        assert_eq!(row[86 * 4], 0.0);
    }

    #[test]
    fn curves_row_composes_rgb_then_channel() {
        let mut c = ColorAdjust::default();
        c.curves.rgb = Curve {
            points: vec![[0.0, 0.2], [1.0, 1.0]],
        };
        c.curves.blue = Curve {
            points: vec![[0.0, 1.0], [1.0, 0.0]],
        };
        let looks = for_color(&c, Time::ZERO).unwrap();
        let t = &looks.tone.as_ref().unwrap().data;
        assert!((t[0] - 0.2).abs() < 1e-6); // red at 0 → master 0.2
        assert!((t[2] - 0.8).abs() < 1e-6); // blue: inverted master
        assert!((t[3] - 0.2).abs() < 1e-6);
        let last = (TONE_SIZE - 1) * 4;
        assert!((t[last] - 1.0).abs() < 1e-6 && t[last + 2].abs() < 1e-6);
    }

    #[test]
    fn wheels_push_the_colour_they_point_at() {
        use tidycuts_core::Wheel;
        // Neutral wheels change nothing and need no table.
        assert!(for_color(&ColorAdjust::default(), Time::ZERO).is_none());
        let mut c = ColorAdjust::default();
        // Gain towards red (right on the disc).
        c.wheels.gain = Wheel {
            x: 1.0,
            y: 0.0,
            master: 0.0,
        };
        let looks = for_color(&c, Time::ZERO).unwrap();
        assert!(looks.curves);
        let t = &looks.tone.as_ref().unwrap().data;
        let mid = 128 * 4;
        assert!(t[mid] > 0.55, "red went up: {}", t[mid]);
        assert!(
            t[mid + 1] < 0.5 && t[mid + 2] < 0.5,
            "green and blue went down"
        );
        // Black stays black for gain, and white clips at 1.
        assert_eq!(t[0], 0.0);
        assert_eq!(t[(TONE_SIZE - 1) * 4], 1.0);
    }

    #[test]
    fn each_wheel_works_where_it_should() {
        use tidycuts_core::{Wheel, Wheels};
        let up = |w: &mut Wheel| w.master = 1.0;
        // Lift moves the shadows far more than the highlights.
        let mut w = Wheels::default();
        up(&mut w.lift);
        let f = wheels_curve(&w, 0);
        assert!(f(0.0) > 0.2);
        assert!((f(1.0) - 1.0).abs() < 1e-6);
        // Gamma moves the middle, not the ends.
        let mut w = Wheels::default();
        up(&mut w.gamma);
        let f = wheels_curve(&w, 1);
        assert!(f(0.5) > 0.6);
        assert_eq!(f(0.0), 0.0);
        assert!((f(1.0) - 1.0).abs() < 1e-6);
        // Gain moves the top, leaving black alone.
        let mut w = Wheels::default();
        w.gain.master = -1.0;
        let f = wheels_curve(&w, 2);
        assert_eq!(f(0.0), 0.0);
        assert!(f(1.0) < 0.5);
        // Offset moves everything.
        let mut w = Wheels::default();
        up(&mut w.offset);
        let f = wheels_curve(&w, 0);
        assert!(f(0.0) > 0.1 && f(0.5) > 0.6);
    }

    #[test]
    fn keyframed_lut_strength_follows_time() {
        let mut c = ColorAdjust::default();
        let mut l = ColorLut::new(LutSource::BuiltIn("cool".into()));
        l.strength.set_keyframe(Time::ZERO, 0.0);
        l.strength.set_keyframe(Time::from_secs_f64(2.0), 100.0);
        c.lut = Some(l);
        let at = |s: f64| for_color(&c, Time::from_secs_f64(s)).map(|l| l.lut_strength);
        assert_eq!(at(0.0), None);
        assert!((at(1.0).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(at(3.0), Some(1.0));
    }
}
