//! 3D color lookup tables (LUTs): the .cube file format, the looks TidyCuts
//! generates itself, and a cache so each LUT is read only once.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tidycuts_core::LutSource;

/// Largest LUT accepted (points per side). Common sizes are 17, 33 and 65.
pub const MAX_SIZE: usize = 128;

/// Points per side of the built-in looks.
const BUILT_IN_SIZE: usize = 33;

/// A 3D LUT with inputs 0..1 on each axis.
#[derive(Debug)]
pub struct Lut3d {
    /// Points per side.
    pub size: usize,
    /// `size³` output colors; red changes fastest, then green, then blue
    /// (the .cube order, and x, y, z of a 3D texture).
    pub data: Vec<[f32; 3]>,
    /// The file's title, if it has one.
    pub title: String,
    /// Unique per loaded LUT, so the GPU uploads each one once.
    pub key: u64,
}

fn next_key() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Lut3d {
    /// Builds a LUT by running `f` on every grid point.
    pub fn from_fn(size: usize, f: impl Fn([f32; 3]) -> [f32; 3]) -> Lut3d {
        let n = size.max(2);
        let step = 1.0 / (n - 1) as f32;
        let mut data = Vec::with_capacity(n * n * n);
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    data.push(f([r as f32 * step, g as f32 * step, b as f32 * step]));
                }
            }
        }
        Lut3d {
            size: n,
            data,
            title: String::new(),
            key: next_key(),
        }
    }

    /// The output for `rgb` (0..1), interpolated between grid points.
    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let last = (n - 1) as f32;
        let mut i0 = [0usize; 3];
        let mut fr = [0f32; 3];
        for k in 0..3 {
            let p = rgb[k].clamp(0.0, 1.0) * last;
            let i = (p.floor() as usize).min(n - 2);
            i0[k] = i;
            fr[k] = p - i as f32;
        }
        let at = |r: usize, g: usize, b: usize| self.data[r + n * (g + n * b)];
        let mut out = [0f32; 3];
        for (corner, w) in (0..8).map(|c| {
            let (dr, dg, db) = (c & 1, (c >> 1) & 1, (c >> 2) & 1);
            let w = [dr, dg, db]
                .iter()
                .enumerate()
                .map(|(k, d)| if *d == 1 { fr[k] } else { 1.0 - fr[k] })
                .product::<f32>();
            (at(i0[0] + dr, i0[1] + dg, i0[2] + db), w)
        }) {
            for k in 0..3 {
                out[k] += corner[k] * w;
            }
        }
        out
    }

    /// The table as RGBA half floats, ready for a 3D texture.
    pub fn to_rgba_f16(&self) -> Vec<u16> {
        let one = f16_bits(1.0);
        self.data
            .iter()
            .flat_map(|c| [f16_bits(c[0]), f16_bits(c[1]), f16_bits(c[2]), one])
            .collect()
    }
}

/// Converts to IEEE half precision (round to nearest even).
pub fn f16_bits(v: f32) -> u16 {
    let sign = ((v.to_bits() >> 16) & 0x8000) as u16;
    let a = v.abs();
    if a.is_nan() {
        return sign | 0x7e00;
    }
    if a >= 65504.0 {
        return sign | 0x7bff;
    }
    if a < 6.103_515_6e-5 {
        // Subnormal: a multiple of 2^-24.
        return sign | (a * 16_777_216.0).round() as u16;
    }
    let bits = a.to_bits();
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x7f_ffff;
    let mut h = ((exp as u32) << 10) | (mant >> 13);
    let rest = mant & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && h & 1 == 1) {
        h += 1;
    }
    sign | h as u16
}

fn numbers<const N: usize>(parts: &[&str], line: usize) -> Result<[f32; N], String> {
    if parts.len() != N {
        return Err(format!("Line {line}: expected {N} numbers."));
    }
    let mut out = [0f32; N];
    for (o, p) in out.iter_mut().zip(parts) {
        *o = p
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("Line {line}: \"{p}\" isn't a number."))?;
    }
    Ok(out)
}

/// Reads a .cube LUT (the common 3D LUT text format): `LUT_3D_SIZE`, optional
/// `TITLE`, `DOMAIN_MIN`/`DOMAIN_MAX` (or `LUT_3D_INPUT_RANGE`), `#`
/// comments, then one "r g b" line per grid point with red changing
/// fastest. 1D LUTs (`LUT_1D_SIZE`) are turned into 3D ones.
pub fn parse_cube(text: &str) -> Result<Lut3d, String> {
    let mut size3 = None;
    let mut size1 = None;
    let mut dmin = [0f32; 3];
    let mut dmax = [1f32; 3];
    let mut title = String::new();
    let mut data: Vec<[f32; 3]> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first.eq_ignore_ascii_case("TITLE") {
            title = line[first.len()..].trim().trim_matches('"').to_string();
            continue;
        }
        let body = line.split('#').next().unwrap_or("");
        let parts: Vec<&str> = body.split_whitespace().collect();
        let Some(&key) = parts.first() else {
            continue;
        };
        let rest = &parts[1..];
        let numeric = key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'));
        if numeric {
            data.push(numbers::<3>(&parts, line_no)?);
            continue;
        }
        let size = |rest: &[&str]| -> Result<usize, String> {
            rest.first()
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| format!("Line {line_no}: the LUT size isn't a whole number."))
        };
        match key.to_ascii_uppercase().as_str() {
            "LUT_3D_SIZE" => size3 = Some(size(rest)?),
            "LUT_1D_SIZE" => size1 = Some(size(rest)?),
            "DOMAIN_MIN" => dmin = numbers::<3>(rest, line_no)?,
            "DOMAIN_MAX" => dmax = numbers::<3>(rest, line_no)?,
            "LUT_3D_INPUT_RANGE" | "LUT_1D_INPUT_RANGE" => {
                let [lo, hi] = numbers::<2>(rest, line_no)?;
                dmin = [lo; 3];
                dmax = [hi; 3];
            }
            // Other keywords (LUT_IN_VIDEO_RANGE…) don't change the table.
            _ => {}
        }
    }
    if (0..3).any(|k| dmax[k] <= dmin[k]) {
        return Err("DOMAIN_MAX must be larger than DOMAIN_MIN.".into());
    }
    let mut lut = match (size3, size1) {
        (Some(n), _) => {
            if !(2..=MAX_SIZE).contains(&n) {
                return Err(format!(
                    "LUT_3D_SIZE {n} isn't supported (it must be 2 to {MAX_SIZE})."
                ));
            }
            if data.len() != n * n * n {
                return Err(format!(
                    "The file has {} colors, but a LUT of size {n} needs {}.",
                    data.len(),
                    n * n * n
                ));
            }
            Lut3d {
                size: n,
                data,
                title: String::new(),
                key: next_key(),
            }
        }
        (None, Some(n)) => {
            if !(2..=65536).contains(&n) || data.len() != n {
                return Err(format!(
                    "The file has {} colors, but its LUT_1D_SIZE is {n}.",
                    data.len()
                ));
            }
            let curve = |k: usize, v: f32| {
                let p = v.clamp(0.0, 1.0) * (n - 1) as f32;
                let i = (p.floor() as usize).min(n - 2);
                let f = p - i as f32;
                data[i][k] * (1.0 - f) + data[i + 1][k] * f
            };
            let u = |k: usize, v: f32| (v - dmin[k]) / (dmax[k] - dmin[k]);
            let lut = Lut3d::from_fn(BUILT_IN_SIZE, |c| {
                [
                    curve(0, u(0, c[0])),
                    curve(1, u(1, c[1])),
                    curve(2, u(2, c[2])),
                ]
            });
            return Ok(Lut3d { title, ..lut });
        }
        (None, None) => {
            return Err("This isn't a LUT file: it has no LUT_3D_SIZE line.".into());
        }
    };
    if dmin != [0.0; 3] || dmax != [1.0; 3] {
        // Resample so the shader can always assume inputs of 0..1.
        let src = lut;
        let u = |k: usize, v: f32| (v - dmin[k]) / (dmax[k] - dmin[k]);
        lut = Lut3d::from_fn(src.size, |c| {
            src.sample([u(0, c[0]), u(1, c[1]), u(2, c[2])])
        });
    }
    lut.title = title;
    Ok(lut)
}

// ---- Built-in looks --------------------------------------------------------

/// A look TidyCuts generates as a LUT, so the LUT list is useful without
/// any files.
pub struct BuiltInLut {
    pub id: &'static str,
    pub name: &'static str,
    look: fn([f32; 3]) -> [f32; 3],
}

pub static BUILT_IN: &[BuiltInLut] = &[
    BuiltInLut {
        id: "warm",
        name: "Warm",
        look: warm,
    },
    BuiltInLut {
        id: "cool",
        name: "Cool",
        look: cool,
    },
    BuiltInLut {
        id: "teal_orange",
        name: "Teal & Orange",
        look: teal_orange,
    },
    BuiltInLut {
        id: "bleach_bypass",
        name: "Bleach Bypass",
        look: bleach_bypass,
    },
    BuiltInLut {
        id: "vintage_fade",
        name: "Vintage Fade",
        look: vintage_fade,
    },
    BuiltInLut {
        id: "bw_contrast",
        name: "Black & White Contrast",
        look: bw_contrast,
    },
    BuiltInLut {
        id: "cross_process",
        name: "Cross Process",
        look: cross_process,
    },
    BuiltInLut {
        id: "golden_hour",
        name: "Golden Hour",
        look: golden_hour,
    },
];

/// The display name of a built-in look.
pub fn built_in_name(id: &str) -> Option<&'static str> {
    BUILT_IN.iter().find(|b| b.id == id).map(|b| b.name)
}

/// A short name for a LUT: the built-in look's name or the file name.
pub fn label(source: &LutSource) -> String {
    match source {
        LutSource::BuiltIn(id) => built_in_name(id).unwrap_or(id).to_string(),
        LutSource::File(_) | LutSource::Embedded { .. } => source.file_label().unwrap_or_default(),
    }
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn saturate(c: [f32; 3], amount: f32) -> [f32; 3] {
    let l = luma(c);
    mix3([l; 3], c, amount)
}

fn clamp01(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| v.clamp(0.0, 1.0))
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A gentle S-curve around mid grey; `k` > 1 adds contrast.
fn s_curve(v: f32, k: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v < 0.5 {
        0.5 * (2.0 * v).powf(k)
    } else {
        1.0 - 0.5 * (2.0 - 2.0 * v).powf(k)
    }
}

fn warm(c: [f32; 3]) -> [f32; 3] {
    let c = [c[0].powf(0.9), c[1].powf(0.97), c[2].powf(1.12) * 0.94];
    clamp01(saturate(c, 1.08))
}

fn cool(c: [f32; 3]) -> [f32; 3] {
    let c = [c[0].powf(1.12) * 0.96, c[1].powf(1.0), c[2].powf(0.88)];
    clamp01(saturate(c, 0.96))
}

fn teal_orange(c: [f32; 3]) -> [f32; 3] {
    let l = luma(c);
    let hi = smoothstep(0.25, 0.85, l);
    // Shadows towards teal, highlights and skin towards orange.
    let teal = [-0.07, 0.03, 0.06];
    let orange = [0.08, 0.02, -0.09];
    let mut out = [0f32; 3];
    for k in 0..3 {
        out[k] = s_curve(c[k] + teal[k] * (1.0 - hi) + orange[k] * hi, 1.15);
    }
    clamp01(saturate(out, 1.18))
}

fn bleach_bypass(c: [f32; 3]) -> [f32; 3] {
    // Overlay the picture with its own luminance, then drain most color.
    let l = luma(c);
    let overlay = |v: f32| {
        if l < 0.5 {
            2.0 * v * l
        } else {
            1.0 - 2.0 * (1.0 - v) * (1.0 - l)
        }
    };
    let o = [overlay(c[0]), overlay(c[1]), overlay(c[2])];
    clamp01(saturate(o, 0.45))
}

fn vintage_fade(c: [f32; 3]) -> [f32; 3] {
    let c = saturate(c, 0.72);
    // Lifted, slightly green blacks and soft, creamy whites.
    let lift = [0.09, 0.1, 0.07];
    let gain = [0.95, 0.92, 0.8];
    let mut out = [0f32; 3];
    for k in 0..3 {
        out[k] = lift[k] + c[k] * (gain[k] - lift[k]);
    }
    clamp01(out)
}

fn bw_contrast(c: [f32; 3]) -> [f32; 3] {
    let l = s_curve(luma(c), 1.45);
    [l; 3]
}

fn cross_process(c: [f32; 3]) -> [f32; 3] {
    clamp01([
        s_curve(c[0], 1.3),
        s_curve(c[1], 1.15) * 0.97 + 0.02,
        0.18 + c[2] * 0.64,
    ])
}

fn golden_hour(c: [f32; 3]) -> [f32; 3] {
    let l = luma(c);
    let glow = smoothstep(0.3, 1.0, l);
    let c = [
        c[0].powf(0.85) + 0.04 * glow,
        c[1].powf(0.95) + 0.015 * glow,
        c[2].powf(1.2) * 0.9,
    ];
    clamp01(saturate(c, 1.12))
}

// ---- Cache -----------------------------------------------------------------

struct Cached {
    lut: Result<Arc<Lut3d>, String>,
    at: Instant,
}

fn cache() -> &'static Mutex<HashMap<LutSource, Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<LutSource, Cached>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn read(source: &LutSource) -> Result<Lut3d, String> {
    match source {
        LutSource::BuiltIn(id) => {
            let b = BUILT_IN
                .iter()
                .find(|b| b.id == id)
                .ok_or_else(|| format!("There's no built-in look called \"{id}\"."))?;
            let mut lut = Lut3d::from_fn(BUILT_IN_SIZE, b.look);
            lut.title = b.name.to_string();
            Ok(lut)
        }
        LutSource::Embedded { name, cube, .. } => {
            parse_cube(cube).map_err(|e| format!("\"{name}\" isn't a LUT TidyCuts can use. {e}"))
        }
        LutSource::File(path) => {
            let name = source.file_label().unwrap_or_default();
            let text = std::fs::read_to_string(path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!("The LUT file \"{name}\" can't be found.")
                } else {
                    format!("The LUT file \"{name}\" can't be read: {e}")
                }
            })?;
            parse_cube(&text).map_err(|e| format!("\"{name}\" isn't a LUT TidyCuts can use. {e}"))
        }
    }
}

/// Loads a LUT, reading each one only once. Failures are remembered for a
/// couple of seconds, so a missing file isn't looked for on every frame.
pub fn load(source: &LutSource) -> Result<Arc<Lut3d>, String> {
    let mut map = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = map.get(source) {
        if c.lut.is_ok() || c.at.elapsed() < Duration::from_secs(2) {
            return c.lut.clone();
        }
    }
    let lut = read(source).map(Arc::new);
    if let Err(e) = &lut {
        log::warn!("{e}");
    }
    map.insert(
        source.clone(),
        Cached {
            lut: lut.clone(),
            at: Instant::now(),
        },
    );
    lut
}

/// Reads a LUT file again (after the user picks it), replacing any copy
/// or failure remembered for it.
pub fn reload(source: &LutSource) -> Result<Arc<Lut3d>, String> {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(source);
    load(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_cube(n: usize) -> String {
        let mut s = String::from("# made by a test\nTITLE \"Identity\"\n\n");
        s += &format!("LUT_3D_SIZE {n}\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1.0 1.0 1.0\n");
        let d = (n - 1) as f32;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    s += &format!("{} {} {}\n", r as f32 / d, g as f32 / d, b as f32 / d);
                }
            }
        }
        s
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    #[test]
    fn parses_an_identity_cube() {
        let lut = parse_cube(&identity_cube(5)).unwrap();
        assert_eq!(lut.size, 5);
        assert_eq!(lut.title, "Identity");
        assert_eq!(lut.data.len(), 125);
        // Red changes fastest.
        assert!(close(lut.data[1], [0.25, 0.0, 0.0]));
        assert!(close(lut.data[5], [0.0, 0.25, 0.0]));
        assert!(close(lut.data[25], [0.0, 0.0, 0.25]));
        for c in [[0.1, 0.5, 0.9], [0.33, 0.66, 0.0], [1.0, 1.0, 1.0]] {
            assert!(close(lut.sample(c), c));
        }
    }

    #[test]
    fn reads_comments_keywords_in_any_case_and_crlf() {
        let text = "#comment\r\nlut_3d_size 2\r\nLUT_IN_VIDEO_RANGE\r\n\
                    0 0 0\r\n1 0 0 # inline\r\n0 1 0\r\n1 1 0\r\n\
                    0 0 1\r\n1 0 1\r\n0 1 1\r\n1 1 1\r\n";
        let lut = parse_cube(text).unwrap();
        assert_eq!(lut.size, 2);
        assert!(close(lut.sample([0.5, 0.25, 0.75]), [0.5, 0.25, 0.75]));
    }

    #[test]
    fn applies_the_domain() {
        // An identity table over 0..2: an input of 1 sits halfway, so it
        // comes out as 1 and an input of 0.5 as 0.5.
        let mut s = String::from("LUT_3D_SIZE 3\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n");
        for b in 0..3 {
            for g in 0..3 {
                for r in 0..3 {
                    s += &format!("{r} {g} {b}\n");
                }
            }
        }
        let lut = parse_cube(&s).unwrap();
        assert!(close(lut.sample([0.5, 1.0, 0.25]), [0.5, 1.0, 0.25]));
    }

    #[test]
    fn turns_1d_luts_into_3d() {
        let lut = parse_cube("LUT_1D_SIZE 2\n1 1 1\n0 0 0\n").unwrap();
        assert!(close(lut.sample([0.0, 1.0, 0.25]), [1.0, 0.0, 0.75]));
    }

    #[test]
    fn rejects_broken_files() {
        assert!(parse_cube("").is_err());
        assert!(parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err());
        assert!(parse_cube("LUT_3D_SIZE 1\n0 0 0\n").is_err());
        assert!(parse_cube("LUT_3D_SIZE 999\n").is_err());
        assert!(parse_cube("LUT_3D_SIZE two\n").is_err());
        let bad = identity_cube(2).replace("1 1 1", "1 x 1");
        assert!(parse_cube(&bad).unwrap_err().contains("isn't a number"));
        let flipped = identity_cube(2).replace("DOMAIN_MAX 1.0 1.0 1.0", "DOMAIN_MAX 0 0 0");
        assert!(parse_cube(&flipped).is_err());
    }

    #[test]
    fn built_in_looks_change_the_picture() {
        for b in BUILT_IN {
            let lut = read(&LutSource::BuiltIn(b.id.into())).unwrap();
            assert_eq!(lut.size, BUILT_IN_SIZE);
            assert!(lut.data.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
            let probe = [0.8, 0.4, 0.2];
            assert!(!close(lut.sample(probe), probe), "{} does nothing", b.id);
        }
        let bw = read(&LutSource::BuiltIn("bw_contrast".into())).unwrap();
        let c = bw.sample([0.9, 0.1, 0.2]);
        assert!((c[0] - c[1]).abs() < 1e-3 && (c[1] - c[2]).abs() < 1e-3);
        assert!(read(&LutSource::BuiltIn("nope".into())).is_err());
    }

    #[test]
    fn missing_files_fail_without_panicking() {
        let src = LutSource::File("/no/such/folder/look.cube".into());
        let e = load(&src).unwrap_err();
        assert!(e.contains("look"), "{e}");
        assert!(load(&src).is_err());
    }

    #[test]
    fn half_floats() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(65504.0), 0x7bff);
        assert_eq!(f16_bits(1.0 / 3.0), 0x3555);
        assert_eq!(f16_bits(6.0e-8), 1);
    }
}
