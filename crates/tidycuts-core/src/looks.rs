//! The Color tab's HSL, curves and LUT settings (HSL, Curves and
//! LUT panels). The renderer bakes them into lookup tables for the grade
//! shader; this module only holds the settings and the curve maths.

use crate::keyframe::Animatable;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The eight HSL color ranges and the hue (degrees) each is centred on.
pub const HSL_RANGES: [(&str, f64); 8] = [
    ("Red", 0.0),
    ("Orange", 30.0),
    ("Yellow", 60.0),
    ("Green", 120.0),
    ("Aqua", 180.0),
    ("Blue", 240.0),
    ("Purple", 270.0),
    ("Magenta", 300.0),
];

/// Names of the HSL values as `ColorAdjust::fields_mut` lists them.
const HSL_FIELDS: [[&str; 3]; 8] = [
    ["red_hue", "red_saturation", "red_luminance"],
    ["orange_hue", "orange_saturation", "orange_luminance"],
    ["yellow_hue", "yellow_saturation", "yellow_luminance"],
    ["green_hue", "green_saturation", "green_luminance"],
    ["aqua_hue", "aqua_saturation", "aqua_luminance"],
    ["blue_hue", "blue_saturation", "blue_luminance"],
    ["purple_hue", "purple_saturation", "purple_luminance"],
    ["magenta_hue", "magenta_saturation", "magenta_luminance"],
];

/// Changes to one color range, each -100..=100 with 0 meaning unchanged.
/// Hue moves the range towards its neighbours (±100 = ±30°).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HslBand {
    pub hue: Animatable,
    pub saturation: Animatable,
    pub luminance: Animatable,
}

impl Default for HslBand {
    fn default() -> Self {
        HslBand {
            hue: Animatable::constant(0.0),
            saturation: Animatable::constant(0.0),
            luminance: Animatable::constant(0.0),
        }
    }
}

impl HslBand {
    pub fn is_neutral(&self) -> bool {
        *self == HslBand::default()
    }

    pub fn values_mut(&mut self) -> [&mut Animatable; 3] {
        [&mut self.hue, &mut self.saturation, &mut self.luminance]
    }
}

/// Hue, saturation and luminance per color range, in [`HSL_RANGES`] order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hsl {
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub red: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub orange: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub yellow: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub green: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub aqua: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub blue: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub purple: HslBand,
    #[serde(skip_serializing_if = "HslBand::is_neutral")]
    pub magenta: HslBand,
}

impl Hsl {
    pub fn is_neutral(&self) -> bool {
        *self == Hsl::default()
    }

    pub fn bands(&self) -> [&HslBand; 8] {
        [
            &self.red,
            &self.orange,
            &self.yellow,
            &self.green,
            &self.aqua,
            &self.blue,
            &self.purple,
            &self.magenta,
        ]
    }

    pub fn bands_mut(&mut self) -> [&mut HslBand; 8] {
        [
            &mut self.red,
            &mut self.orange,
            &mut self.yellow,
            &mut self.green,
            &mut self.aqua,
            &mut self.blue,
            &mut self.purple,
            &mut self.magenta,
        ]
    }

    /// Every value with its name ("red_hue", "aqua_luminance"…).
    pub fn fields_mut(&mut self) -> impl Iterator<Item = (&'static str, &mut Animatable)> {
        self.bands_mut()
            .into_iter()
            .zip(HSL_FIELDS)
            .flat_map(|(b, names)| {
                let HslBand {
                    hue,
                    saturation,
                    luminance,
                } = b;
                names.into_iter().zip([hue, saturation, luminance])
            })
    }
}

/// A tone curve: points (input, output), both 0..=1, sorted by input.
/// Between points it follows a smooth monotone spline; before the first
/// and after the last point it stays flat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Curve {
    pub points: Vec<[f64; 2]>,
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        }
    }
}

impl Curve {
    /// Smallest horizontal gap between two points.
    pub const MIN_GAP: f64 = 0.01;

    /// Whether the curve leaves values unchanged (every point on the
    /// diagonal, running from 0 to 1).
    pub fn is_identity(&self) -> bool {
        let p = &self.points;
        p.is_empty()
            || (p.iter().all(|q| (q[0] - q[1]).abs() < 1e-6)
                && p[0][0] <= 1e-6
                && p[p.len() - 1][0] >= 1.0 - 1e-6)
    }

    pub fn spline(&self) -> Spline {
        Spline::new(&self.points)
    }

    pub fn eval(&self, x: f64) -> f64 {
        self.spline().eval(x)
    }

    /// Adds a point, keeping the points sorted. Returns its index, or
    /// `None` when another point is too close.
    pub fn insert(&mut self, x: f64, y: f64) -> Option<usize> {
        let (x, y) = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if self.points.iter().any(|p| (p[0] - x).abs() < Self::MIN_GAP) {
            return None;
        }
        let i = self.points.partition_point(|p| p[0] < x);
        self.points.insert(i, [x, y]);
        Some(i)
    }

    /// Moves point `i`, keeping it between its neighbours.
    pub fn move_point(&mut self, i: usize, x: f64, y: f64) {
        let n = self.points.len();
        if i >= n {
            return;
        }
        let lo = if i == 0 {
            0.0
        } else {
            self.points[i - 1][0] + Self::MIN_GAP
        };
        let hi = if i + 1 == n {
            1.0
        } else {
            self.points[i + 1][0] - Self::MIN_GAP
        };
        self.points[i] = [x.clamp(lo, hi.max(lo)), y.clamp(0.0, 1.0)];
    }

    /// Whether point `i` may be removed (the first and last stay).
    pub fn can_remove(&self, i: usize) -> bool {
        i > 0 && i + 1 < self.points.len()
    }
}

/// A Fritsch–Carlson monotone cubic spline through a curve's points: it
/// never overshoots, so a rising curve keeps rising.
#[derive(Debug, Clone)]
pub struct Spline {
    xs: Vec<f64>,
    ys: Vec<f64>,
    ms: Vec<f64>,
}

impl Spline {
    pub fn new(points: &[[f64; 2]]) -> Spline {
        let mut pts: Vec<[f64; 2]> = points
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)])
            .collect();
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        pts.dedup_by(|b, a| b[0] - a[0] < 1e-6);
        if pts.is_empty() {
            pts = Curve::default().points;
        }
        let xs: Vec<f64> = pts.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p[1]).collect();
        let n = xs.len();
        let mut ms = vec![0.0; n];
        if n >= 2 {
            let d: Vec<f64> = (0..n - 1)
                .map(|k| (ys[k + 1] - ys[k]) / (xs[k + 1] - xs[k]))
                .collect();
            ms[0] = d[0];
            ms[n - 1] = d[n - 2];
            for k in 1..n - 1 {
                ms[k] = if d[k - 1] * d[k] <= 0.0 {
                    0.0
                } else {
                    (d[k - 1] + d[k]) / 2.0
                };
            }
            for k in 0..n - 1 {
                if d[k] == 0.0 {
                    ms[k] = 0.0;
                    ms[k + 1] = 0.0;
                    continue;
                }
                let a = ms[k] / d[k];
                let b = ms[k + 1] / d[k];
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    ms[k] = t * a * d[k];
                    ms[k + 1] = t * b * d[k];
                }
            }
        }
        Spline { xs, ys, ms }
    }

    pub fn eval(&self, x: f64) -> f64 {
        let (xs, ys, ms) = (&self.xs, &self.ys, &self.ms);
        let n = xs.len();
        if n == 1 || x <= xs[0] {
            return ys[0];
        }
        if x >= xs[n - 1] {
            return ys[n - 1];
        }
        let k = xs.partition_point(|v| *v <= x).saturating_sub(1).min(n - 2);
        let h = xs[k + 1] - xs[k];
        let t = (x - xs[k]) / h;
        let (t2, t3) = (t * t, t * t * t);
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        (h00 * ys[k] + h10 * h * ms[k] + h01 * ys[k + 1] + h11 * h * ms[k + 1]).clamp(0.0, 1.0)
    }
}

/// Curves: one curve for all three channels plus one each.
/// The RGB curve applies first, then the channel's own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Curves {
    #[serde(skip_serializing_if = "Curve::is_identity")]
    pub rgb: Curve,
    #[serde(skip_serializing_if = "Curve::is_identity")]
    pub red: Curve,
    #[serde(skip_serializing_if = "Curve::is_identity")]
    pub green: Curve,
    #[serde(skip_serializing_if = "Curve::is_identity")]
    pub blue: Curve,
}

impl Curves {
    pub fn is_identity(&self) -> bool {
        self.rgb.is_identity()
            && self.red.is_identity()
            && self.green.is_identity()
            && self.blue.is_identity()
    }

    /// The curves in the editor's order: RGB, red, green, blue.
    pub fn all(&self) -> [&Curve; 4] {
        [&self.rgb, &self.red, &self.green, &self.blue]
    }

    pub fn all_mut(&mut self) -> [&mut Curve; 4] {
        [
            &mut self.rgb,
            &mut self.red,
            &mut self.green,
            &mut self.blue,
        ]
    }
}

/// Where a LUT comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LutSource {
    /// One of the looks TidyCuts generates itself, by id (e.g. "warm").
    BuiltIn(String),
    /// A .cube file on disk. Only projects saved before imported LUTs were
    /// kept inside the project use this; they still open.
    File(PathBuf),
    /// A .cube file's contents, kept inside the project so the look moves
    /// with it. `id` identifies the contents quickly (see [`LutSource::embedded`]).
    Embedded { name: String, id: u64, cube: String },
}

impl LutSource {
    /// A LUT kept inside the project: `name` is what the panel shows and
    /// `cube` the text of the .cube file.
    pub fn embedded(name: impl Into<String>, cube: String) -> LutSource {
        // FNV-1a over the text: cheap, and only used to tell contents apart.
        let id = cube.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        LutSource::Embedded {
            name: name.into(),
            id,
            cube,
        }
    }

    /// The name to show for a LUT that came from a file.
    pub fn file_label(&self) -> Option<String> {
        match self {
            LutSource::File(p) => Some(
                p.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.display().to_string()),
            ),
            LutSource::Embedded { name, .. } => Some(name.clone()),
            LutSource::BuiltIn(_) => None,
        }
    }
}

// Embedded LUTs are compared by name and `id`, never by their (large) text,
// because sources are looked up on every frame.
impl PartialEq for LutSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LutSource::BuiltIn(a), LutSource::BuiltIn(b)) => a == b,
            (LutSource::File(a), LutSource::File(b)) => a == b,
            (
                LutSource::Embedded {
                    name: n1, id: i1, ..
                },
                LutSource::Embedded {
                    name: n2, id: i2, ..
                },
            ) => n1 == n2 && i1 == i2,
            _ => false,
        }
    }
}

impl Eq for LutSource {}

impl std::hash::Hash for LutSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            LutSource::BuiltIn(id) => id.hash(state),
            LutSource::File(p) => p.hash(state),
            LutSource::Embedded { name, id, .. } => {
                name.hash(state);
                id.hash(state);
            }
        }
    }
}

fn hundred() -> Animatable {
    Animatable::constant(100.0)
}

/// A 3D LUT applied to the clip, mixed in at `strength` percent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorLut {
    pub source: LutSource,
    #[serde(default = "hundred")]
    pub strength: Animatable,
}

impl ColorLut {
    pub fn new(source: LutSource) -> Self {
        ColorLut {
            source,
            strength: hundred(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_embedded_lut_travels_with_the_project() {
        let a = LutSource::embedded("Look", "TITLE \"x\"\n".into());
        let json = serde_json::to_string(&a).unwrap();
        let back: LutSource = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
        assert_eq!(back.file_label().as_deref(), Some("Look"));
        // Different contents under the same name are different looks.
        let b = LutSource::embedded("Look", "TITLE \"y\"\n".into());
        assert_ne!(a, b);
        // Old projects that point at a file still load.
        let old: LutSource = serde_json::from_str("{\"File\":\"/old/look.cube\"}").unwrap();
        assert_eq!(old.file_label().as_deref(), Some("look"));
    }

    #[test]
    fn default_curve_is_the_diagonal() {
        let c = Curve::default();
        assert!(c.is_identity());
        for i in 0..=20 {
            let x = i as f64 / 20.0;
            assert!((c.eval(x) - x).abs() < 1e-9);
        }
        let mid = Curve {
            points: vec![[0.0, 0.0], [0.5, 0.5], [1.0, 1.0]],
        };
        assert!(mid.is_identity());
    }

    #[test]
    fn spline_passes_through_points_and_stays_monotone() {
        let c = Curve {
            points: vec![[0.0, 0.0], [0.25, 0.15], [0.75, 0.85], [1.0, 1.0]],
        };
        let s = c.spline();
        for p in &c.points {
            assert!((s.eval(p[0]) - p[1]).abs() < 1e-9);
        }
        let mut last = -1.0;
        for i in 0..=200 {
            let y = s.eval(i as f64 / 200.0);
            assert!(y >= last - 1e-12, "not monotone at {i}");
            last = y;
        }
        // An S-curve darkens the shadows and brightens the highlights.
        assert!(s.eval(0.2) < 0.2);
        assert!(s.eval(0.8) > 0.8);
    }

    #[test]
    fn spline_does_not_overshoot_a_step() {
        let s = Spline::new(&[[0.0, 0.0], [0.4, 0.0], [0.6, 1.0], [1.0, 1.0]]);
        for i in 0..=100 {
            let y = s.eval(i as f64 / 100.0);
            assert!((0.0..=1.0).contains(&y));
        }
        assert_eq!(s.eval(0.2), 0.0);
        assert_eq!(s.eval(0.8), 1.0);
    }

    #[test]
    fn curve_is_flat_outside_its_points() {
        let s = Spline::new(&[[0.2, 0.1], [0.8, 0.9]]);
        assert_eq!(s.eval(0.0), 0.1);
        assert_eq!(s.eval(1.0), 0.9);
    }

    #[test]
    fn inserting_and_moving_points_keeps_them_sorted() {
        let mut c = Curve::default();
        assert_eq!(c.insert(0.5, 0.7), Some(1));
        assert_eq!(c.insert(0.505, 0.2), None);
        assert_eq!(c.insert(0.25, 0.2), Some(1));
        c.move_point(1, 0.9, 0.3);
        assert!(c.points[1][0] < c.points[2][0]);
        assert!(!c.can_remove(0) && c.can_remove(1) && !c.can_remove(3));
    }

    #[test]
    fn hsl_lists_every_value_once() {
        let mut h = Hsl::default();
        let names: Vec<_> = h.fields_mut().map(|(n, _)| n).collect();
        assert_eq!(names.len(), 24);
        assert_eq!(names[0], "red_hue");
        assert_eq!(names[23], "magenta_luminance");
    }

    #[test]
    fn neutral_settings_are_not_saved() {
        let mut h = Hsl::default();
        assert_eq!(serde_json::to_string(&h).unwrap(), "{}");
        h.aqua.saturation = Animatable::constant(20.0);
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains("aqua") && !json.contains("red"));
        let back: Hsl = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
        let curves: Curves = serde_json::from_str(r#"{"red":[[0,0],[0.5,0.6],[1,1]]}"#).unwrap();
        assert!(!curves.is_identity() && curves.rgb.is_identity());
    }
}
