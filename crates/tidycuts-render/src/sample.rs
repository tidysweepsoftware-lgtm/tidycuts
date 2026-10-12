//! Sample pictures used for library thumbnails and previews of effects,
//! filters and transitions.
//!
//! They are photographs embedded in the app (`assets/photos/`), all released
//! into the public domain (CC0) and listed with their sources in
//! `THIRD-PARTY-NOTICES.md`: trees in low sun, a lake at sunset, a bird on
//! the water, a green mountain meadow, two cats, a city by night, a city by
//! day and a latte being poured. The portrait is a cut-out of the app's
//! author for effects that need a face, and the bokeh behind it is drawn.
//! Every picture is resolution independent: the same scene at any size.

use std::sync::OnceLock;

use crate::RgbaImage;

type Rgb = [f32; 3];

// ---- small maths helpers -------------------------------------------------

fn col(r: u8, g: u8, b: u8) -> Rgb {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mixc(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)]
}

fn scale(a: Rgb, k: f32) -> Rgb {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn add(a: Rgb, b: Rgb, k: f32) -> Rgb {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1 inside a shape, 0 outside, with an edge `w` wide; `d` is the signed
/// distance (negative inside).
fn soft(d: f32, w: f32) -> f32 {
    smooth(w, -w, d)
}

fn hash(ix: i32, iy: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x27d4_eb2d)
        ^ (iy as u32).wrapping_mul(0x1656_67b1)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h & 0x00ff_ffff) as f32 / 16_777_215.0
}

fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ix, iy) = (ix as i32, iy as i32);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    mix(mix(a, b, ux), mix(c, d, ux), uy)
}

/// Fractal noise in 0..1.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * vnoise(x * freq, y * freq, seed + o * 17);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

// ---- the canvas ------------------------------------------------------------

/// Draws `f(x, y)` for every pixel (x, y in 0..1) and adds film grain, which
/// also hides any banding in the gradients.
fn render(
    width: u32,
    height: u32,
    grain: f32,
    vignette: f32,
    f: impl Fn(f32, f32, f32) -> Rgb,
) -> RgbaImage {
    let (w, h) = (width.max(2), height.max(2));
    let aspect = w as f32 / h as f32;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for py in 0..h {
        for px in 0..w {
            let (x, y) = ((px as f32 + 0.5) / w as f32, (py as f32 + 0.5) / h as f32);
            let mut c = f(x, y, aspect);
            let (dx, dy) = ((x - 0.5) * aspect.min(1.4), y - 0.5);
            let v = 1.0 - vignette * (dx * dx + dy * dy) * 1.6;
            let n = (hash(px as i32, py as i32, 91) - 0.5) * grain
                + (hash(px as i32, py as i32, 92) - 0.5) * (1.0 / 255.0);
            for ch in &mut c {
                *ch = (*ch * v + n).clamp(0.0, 1.0);
            }
            pixels.extend_from_slice(&[
                (c[0] * 255.0 + 0.5) as u8,
                (c[1] * 255.0 + 0.5) as u8,
                (c[2] * 255.0 + 0.5) as u8,
                255,
            ]);
        }
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
        premultiplied: true,
    }
}

// ---- photographs -----------------------------------------------------------

/// A photograph held as premultiplied RGBA with smaller copies of itself
/// (each half the size of the one before), so small thumbnails are averaged
/// down instead of aliasing.
struct Photo {
    levels: Vec<Level>,
}

struct Level {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
}

impl Photo {
    fn new(width: usize, height: usize, pixels: Vec<[u8; 4]>) -> Photo {
        let mut levels = vec![Level {
            width,
            height,
            pixels,
        }];
        while levels
            .last()
            .is_some_and(|l| l.height >= 16 && l.width >= 16)
        {
            let last = levels.last().expect("at least one level");
            let (w, h) = (last.width / 2, last.height / 2);
            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [2u32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = last.pixels[(2 * y + dy) * last.width + 2 * x + dx];
                        for (s, v) in sum.iter_mut().zip(p) {
                            *s += v as u32;
                        }
                    }
                    pixels.push(sum.map(|s| (s / 4) as u8));
                }
            }
            levels.push(Level {
                width: w,
                height: h,
                pixels,
            });
        }
        Photo { levels }
    }

    /// A PNG with a transparent background.
    fn from_png(data: &[u8]) -> Photo {
        let png = tiny_skia::Pixmap::decode_png(data).expect("the embedded PNG is valid");
        let (rgba, _) = png.data().as_chunks::<4>();
        Photo::new(png.width() as usize, png.height() as usize, rgba.to_vec())
    }

    fn from_jpeg(data: &[u8]) -> Photo {
        use zune_core::bytestream::ZCursor;
        use zune_core::colorspace::ColorSpace;
        use zune_core::options::DecoderOptions;
        let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(data), opts);
        let pixels = d.decode().expect("the embedded JPEG is valid");
        let info = d.info().expect("the embedded JPEG has a size");
        let (rgba, _) = pixels.as_chunks::<4>();
        Photo::new(info.width as usize, info.height as usize, rgba.to_vec())
    }

    fn aspect(&self) -> f32 {
        self.levels[0].width as f32 / self.levels[0].height as f32
    }

    /// The (premultiplied) colour at `u`, `v` in 0..1 across the picture, drawn
    /// for an output where the whole picture is `rows` pixels tall.
    fn sample(&self, u: f32, v: f32, rows: f32) -> [f32; 4] {
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return [0.0; 4];
        }
        let mut level = &self.levels[0];
        for next in &self.levels[1..] {
            if next.height as f32 >= rows {
                level = next;
            }
        }
        let (fx, fy) = (
            (u * level.width as f32 - 0.5).max(0.0),
            (v * level.height as f32 - 0.5).max(0.0),
        );
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = (
            (x0 + 1).min(level.width - 1),
            (y0 + 1).min(level.height - 1),
        );
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let at = |x: usize, y: usize| level.pixels[y * level.width + x];
        let mut out = [0.0; 4];
        for (c, o) in out.iter_mut().enumerate() {
            let get = |x, y| at(x, y)[c] as f32 / 255.0;
            let top = mix(get(x0, y0), get(x1, y0), tx);
            let bottom = mix(get(x0, y1), get(x1, y1), tx);
            *o = mix(top, bottom, ty);
        }
        out
    }
}

macro_rules! embedded {
    ($name:ident, $load:expr) => {
        fn $name() -> &'static Photo {
            static PHOTO: OnceLock<Photo> = OnceLock::new();
            PHOTO.get_or_init(|| $load)
        }
    };
}

embedded!(
    cutout,
    Photo::from_png(include_bytes!("../assets/portrait.png"))
);
embedded!(
    lake_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/lake.jpg"))
);
embedded!(
    bird_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/bird.jpg"))
);
embedded!(
    mountain_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/mountain.jpg"))
);
embedded!(
    pet_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/cat.jpg"))
);
embedded!(
    nap_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/cat_nap.jpg"))
);
embedded!(
    forest_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/forest.jpg"))
);
embedded!(
    night_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/city_night.jpg"))
);
embedded!(
    city_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/city.jpg"))
);
embedded!(
    food_photo,
    Photo::from_jpeg(include_bytes!("../assets/photos/food.jpg"))
);

/// A photograph filling the picture (cropped, never stretched), keeping the
/// part around `focus` (0..1 across and down the photo) when it has to crop.
fn photo_scene(
    width: u32,
    height: u32,
    photo: &'static Photo,
    focus: (f32, f32),
    grade: fn(Rgb) -> Rgb,
) -> RgbaImage {
    let pa = photo.aspect();
    render(width, height, 0.0, 0.0, move |x, y, a| {
        let (sx, sy) = if a > pa { (1.0, pa / a) } else { (a / pa, 1.0) };
        let u0 = (focus.0 - sx * 0.5).clamp(0.0, 1.0 - sx);
        let v0 = (focus.1 - sy * 0.5).clamp(0.0, 1.0 - sy);
        let p = photo.sample(u0 + x * sx, v0 + y * sy, height as f32 / sy);
        grade([p[0], p[1], p[2]])
    })
}

fn as_shot(c: Rgb) -> Rgb {
    c
}

/// Trees against the low sun beside a lake.
pub fn landscape(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, forest_photo(), (0.5, 0.45), as_shot)
}

/// A lake under an orange sunset sky.
pub fn sunset(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, lake_photo(), (0.5, 0.5), as_shot)
}

/// A city skyline at night with light trails on the road.
pub fn night(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, night_photo(), (0.5, 0.5), as_shot)
}

/// A duck swimming on blue water.
pub fn wildlife(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, bird_photo(), (0.55, 0.55), as_shot)
}

/// A green mountain valley with flowers.
pub fn meadow(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, mountain_photo(), (0.5, 0.5), as_shot)
}

/// A black cat on the back of a sofa.
pub fn pet(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, pet_photo(), (0.45, 0.4), as_shot)
}

/// A cat curled up asleep on a bed.
pub fn nap(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, nap_photo(), (0.5, 0.4), as_shot)
}

/// A bridge and the city skyline in the daytime.
pub fn city(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, city_photo(), (0.5, 0.45), as_shot)
}

/// A barista pouring milk into a latte.
pub fn food(width: u32, height: u32) -> RgbaImage {
    photo_scene(width, height, food_photo(), (0.5, 0.62), as_shot)
}

// ---- a portrait ---------------------------------------------------------------

/// A soft out-of-focus background with bright discs.
fn bokeh_background(x: f32, y: f32, a: f32) -> Rgb {
    let t = y.clamp(0.0, 1.0);
    let base = mixc(col(62, 104, 92), col(158, 134, 104), smooth(0.25, 1.0, t));
    let blotch = fbm(x * a * 2.2, y * 2.2, 4, 40);
    let mut c = scale(
        mixc(base, col(36, 70, 60), smooth(0.35, 0.75, blotch)),
        0.85 + 0.3 * blotch,
    );
    for i in 0..22 {
        let fx = hash(i, 1, 41);
        let fy = hash(i, 2, 41);
        let r = 0.035 + 0.075 * hash(i, 3, 41);
        let (bx, by) = ((fx - 0.5) * a * 1.05 + a * 0.5, fy * 0.7);
        let d = (((x * a) - bx).powi(2) + (y - by).powi(2)).sqrt();
        let body = soft(d - r, r * 0.12);
        let ring = smooth(r * 0.35, r * 0.0, (d - r * 0.9).abs());
        let tint = mixc(col(255, 244, 190), col(190, 255, 200), hash(i, 4, 41));
        let k = 0.10 + 0.14 * hash(i, 5, 41);
        c = add(c, tint, body * k + ring * 0.07);
    }
    c
}

/// The author in front of a soft bokeh background, so colour looks and skin
/// tones show up on a real face.
pub fn portrait(width: u32, height: u32) -> RgbaImage {
    portrait_scene(width, height, false)
}

/// The same photograph in front of a flat green screen, for the chroma key.
pub fn portrait_green(width: u32, height: u32) -> RgbaImage {
    portrait_scene(width, height, true)
}

fn portrait_scene(width: u32, height: u32, green: bool) -> RgbaImage {
    // The photograph is a little taller than the picture, so the shoulders
    // run off the bottom, and hangs so the head sits in the middle.
    const SCALE: f32 = 1.12;
    const TOP: f32 = -0.02;
    const HEAD_X: f32 = 0.47;
    let photo = cutout();
    let wide = photo.aspect();
    render(width, height, 0.018, 0.3, move |x, y, a| {
        let bg = if green {
            // An evenly lit green screen, a little darker towards the floor.
            scale(col(24, 190, 72), 1.0 - 0.28 * smooth(0.3, 1.0, y))
        } else {
            bokeh_background(x, y, a)
        };
        let u = ((x - 0.5) * a / SCALE + HEAD_X * wide) / wide;
        let v = (y - TOP) / SCALE;
        let p = photo.sample(u, v, height as f32 * SCALE);
        // The cut-out is premultiplied: the background shows through what it
        // does not cover.
        [
            p[0] + bg[0] * (1.0 - p[3]),
            p[1] + bg[1] * (1.0 - p[3]),
            p[2] + bg[2] * (1.0 - p[3]),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_is_opaque_varied_and_any_size() {
        type Maker = fn(u32, u32) -> RgbaImage;
        let makers: [(&str, Maker); 8] = [
            ("landscape", landscape),
            ("sunset", sunset),
            ("portrait", portrait),
            ("night", night),
            ("wildlife", wildlife),
            ("meadow", meadow),
            ("pet", pet),
            ("nap", nap),
        ];
        for (name, make) in makers {
            for (w, h) in [(64, 36), (97, 55)] {
                let img = make(w, h);
                assert_eq!((img.width, img.height), (w, h), "{name}");
                assert_eq!(img.pixels.len(), (w * h * 4) as usize, "{name}");
                assert!(img.pixels.chunks(4).all(|p| p[3] == 255), "{name}");
                let lo = img.pixels.chunks(4).map(|p| p[0]).min().unwrap();
                let hi = img.pixels.chunks(4).map(|p| p[0]).max().unwrap();
                assert!(hi - lo > 60, "{name} is flat");
            }
        }
    }

    #[test]
    fn the_green_screen_is_green_around_the_person_only() {
        let img = portrait_green(320, 180);
        let at = |x: usize, y: usize| {
            let i = (y * 320 + x) * 4;
            [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
        };
        let is_green =
            |p: [u8; 3]| p[1] > p[0].saturating_add(60) && p[1] > p[2].saturating_add(60);
        assert!(is_green(at(8, 8)), "top left corner is green");
        assert!(is_green(at(311, 90)), "right edge is green");
        // The forehead and the shirt are the person, not the screen.
        assert!(!is_green(at(160, 62)), "forehead");
        assert!(!is_green(at(130, 165)), "shirt");
    }

    #[test]
    fn the_portrait_is_clean_at_thumbnail_size() {
        // Averaged down from the big photograph, not sampled sparsely: the
        // same picture at two sizes has the same overall brightness.
        let (a, b) = (portrait(64, 36), portrait(256, 144));
        let mean = |i: &RgbaImage| {
            i.pixels
                .chunks(4)
                .map(|p| p[0] as f64 + p[1] as f64 + p[2] as f64)
                .sum::<f64>()
                / (i.width * i.height) as f64
        };
        assert!((mean(&a) - mean(&b)).abs() < 12.0);
    }

    #[test]
    fn the_same_scene_at_two_sizes_looks_alike() {
        let (a, b) = (landscape(64, 36), landscape(128, 72));
        let mean = |i: &RgbaImage| {
            i.pixels
                .chunks(4)
                .map(|p| p[0] as f64 + p[1] as f64 + p[2] as f64)
                .sum::<f64>()
                / (i.width * i.height) as f64
        };
        assert!((mean(&a) - mean(&b)).abs() < 12.0);
    }
}
