//! Renders a looping preview of stickers, one numbered PNG per frame, to turn
//! into a video:
//!
//! `cargo run --release -p tidycuts-render --example sticker_preview -- out_dir id-prefix [seconds] [fps] [cell_height] [columns]`
//!
//! then, for example:
//! `ffmpeg -framerate 30 -i out_dir/frame_%04d.png -c:v libx264 -pix_fmt yuv420p -crf 18 preview.mp4`
//!
//! The stickers are drawn on a dark backdrop, laid out in a grid in the order
//! they are listed in the catalog.

use tidycuts_core::StickerSpec;
use tidycuts_render::catalog::stickers::{render, StickerDef, STICKERS};

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn spec(d: &StickerDef) -> StickerSpec {
    StickerSpec {
        kind: d.id.into(),
        color: d.color,
        color2: d.color2,
        motion: d.motion.into(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| ".".into());
    let prefix = args.get(2).cloned().unwrap_or_default();
    let seconds: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let fps: u32 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(30);
    let cell_h: u32 = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(300);
    let cols: usize = args.get(6).and_then(|v| v.parse().ok()).unwrap_or(3);
    std::fs::create_dir_all(&out).unwrap();

    let defs: Vec<&StickerDef> = STICKERS
        .iter()
        .filter(|d| d.id.starts_with(prefix.as_str()))
        .collect();
    if defs.is_empty() {
        eprintln!("no sticker id starts with '{prefix}'");
        std::process::exit(1);
    }
    for (i, d) in defs.iter().enumerate() {
        println!("{i}: {} ({})", d.id, d.name);
    }

    // Every cell is as wide as the widest sticker, plus some breathing room.
    let cell_w = defs
        .iter()
        .map(|d| (cell_h as f32 * d.aspect).round() as u32)
        .max()
        .unwrap()
        + cell_h / 6;
    let rows = defs.len().div_ceil(cols);
    let (w, h) = (cell_w * cols.min(defs.len()) as u32, cell_h * rows as u32);
    let frames = (seconds * fps as f32).round() as u32;

    for n in 0..frames {
        let t = n as f32 / fps as f32;
        let mut sheet = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            // A dark backdrop with a slight vertical lift.
            let v = 16.0 + 16.0 * (y as f32 / h as f32);
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                sheet[i..i + 4].copy_from_slice(&[v as u8, (v + 2.0) as u8, (v + 9.0) as u8, 255]);
            }
        }
        for (k, d) in defs.iter().enumerate() {
            let img = render(&spec(d), cell_h, t);
            let (ox, oy) = (
                (k % cols) as u32 * cell_w + (cell_w - img.width) / 2,
                (k / cols) as u32 * cell_h,
            );
            for y in 0..img.height.min(cell_h) {
                for x in 0..img.width {
                    let s = ((y * img.width + x) * 4) as usize;
                    let dst = (((oy + y) * w + ox + x) * 4) as usize;
                    let a = img.pixels[s + 3] as u32;
                    for c in 0..3 {
                        // Premultiplied over the backdrop.
                        sheet[dst + c] = (img.pixels[s + c] as u32
                            + sheet[dst + c] as u32 * (255 - a) / 255)
                            .min(255) as u8;
                    }
                }
            }
        }
        save(&format!("{out}/frame_{n:04}.png"), w, h, &sheet);
    }
    println!("wrote {frames} frames of {w}x{h} to {out}");
}
