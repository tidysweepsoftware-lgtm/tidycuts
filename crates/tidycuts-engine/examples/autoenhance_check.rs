//! Checks Auto Enhance on the sample pictures with colour casts, dark and
//! hazy versions added, using the real renderer. Prints how far each
//! result's colour is from neutral and saves a contact sheet per picture
//! (top row: before, bottom row: after).
//!
//! `cargo run --release -p tidycuts-engine --example autoenhance_check -- out_dir`

use std::sync::Arc;
use tidycuts_engine::autograde::grade_from_pixels;
use tidycuts_render::*;

const W: u32 = 384;
const H: u32 = 216;

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Changes the picture in linear light: per-channel multipliers and a
/// flattening toward grey (`haze` 0..1).
fn variant(img: &RgbaImage, mul: [f32; 3], haze: f32) -> RgbaImage {
    let mut out = img.clone();
    for px in out.pixels.as_chunks_mut::<4>().0.iter_mut() {
        for k in 0..3 {
            let v = to_linear(px[k] as f32 / 255.0) * mul[k];
            let v = to_srgb(v);
            let v = v * (1.0 - haze) + 0.55 * haze;
            px[k] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out
}

fn sample_pixels(img: &RgbaImage) -> Vec<[f32; 3]> {
    img.pixels
        .as_chunks::<4>()
        .0
        .iter()
        .step_by(3)
        .map(|p| {
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
            ]
        })
        .collect()
}

/// The average colour as (ln R/G, ln B/G) in linear light.
fn chroma(px: &[u8]) -> [f32; 2] {
    let mut s = [0.0f64; 3];
    for p in px.as_chunks::<4>().0.iter() {
        for k in 0..3 {
            s[k] += to_linear(p[k] as f32 / 255.0) as f64;
        }
    }
    [(s[0] / s[1]).ln() as f32, (s[2] / s[1]).ln() as f32]
}

/// How far the colour of `px` is from that of `reference`.
fn away(px: &[u8], reference: &[u8]) -> f32 {
    let (a, b) = (chroma(px), chroma(reference));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn mean_luma(px: &[u8]) -> f32 {
    let n = (px.len() / 4) as f32;
    px.as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
        .sum::<f32>()
        / n
        / 255.0
}

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&out).unwrap();
    let r = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(r);
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let canvas = [W as f32, H as f32];
    let pictures: Vec<(&str, RgbaImage)> = vec![
        ("landscape", sample::landscape(640, 360)),
        ("sunset", sample::sunset(640, 360)),
        ("night", sample::night(640, 360)),
        ("wildlife", sample::wildlife(640, 360)),
        ("pet", sample::pet(640, 360)),
        ("portrait", sample::portrait(640, 360)),
    ];
    let variants: [(&str, [f32; 3], f32); 6] = [
        ("as shot", [1.0, 1.0, 1.0], 0.0),
        ("warm", [1.4, 1.0, 0.65], 0.0),
        ("cool", [0.7, 1.0, 1.4], 0.0),
        ("green", [0.85, 1.25, 0.85], 0.0),
        ("dark", [0.3, 0.3, 0.3], 0.0),
        ("hazy", [1.0, 1.0, 1.0], 0.45),
    ];
    let render = |comp: &mut Compositor, img: &RgbaImage, grade: Option<Grade>| -> Vec<u8> {
        let mut l = Layer::full(Source::Pixels(Pixels::Rgba(Arc::new(img.clone()))), canvas);
        l.grade = grade;
        let frame = Frame {
            canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Layer(l)],
        };
        comp.render_to_cpu(&frame, &target)
    };
    println!(
        "{:10} {:8} {:>16} {:>14}",
        "picture", "variant", "colour error", "brightness"
    );
    for (name, base) in &pictures {
        let mut sheet = vec![0u8; (W * H * 4) as usize * variants.len() * 2];
        let sw = W as usize * variants.len();
        for (i, (vname, mul, haze)) in variants.iter().enumerate() {
            let img = variant(base, *mul, *haze);
            let g = grade_from_pixels(&sample_pixels(&img));
            let before = render(&mut comp, &img, None);
            let after = render(&mut comp, &img, Some(g.clone()));
            let reference = render(&mut comp, base, None);
            println!(
                "{:10} {:8} {:6.2} -> {:5.2}   {:5.2} -> {:5.2}   (as shot {:.2})  T{:+.2} t{:+.2} exp{:+.2} con{:+.2} blk{:+.2} wht{:+.2} vib{:.2}",
                name,
                vname,
                away(&before, &reference),
                away(&after, &reference),
                mean_luma(&before),
                mean_luma(&after),
                mean_luma(&reference),
                g.temperature,
                g.tint,
                g.exposure,
                g.contrast,
                g.blacks,
                g.whites,
                g.vibrance
            );
            for y in 0..H as usize {
                for (row, src) in [(0usize, &before), (1usize, &after)] {
                    let d = ((row * H as usize + y) * sw + i * W as usize) * 4;
                    let s = y * W as usize * 4;
                    sheet[d..d + W as usize * 4].copy_from_slice(&src[s..s + W as usize * 4]);
                }
            }
        }
        save(&format!("{out}/{name}.png"), sw as u32, H * 2, &sheet);
    }
}
