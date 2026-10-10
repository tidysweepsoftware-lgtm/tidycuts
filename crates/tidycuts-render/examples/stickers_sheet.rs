//! Renders contact sheets of every sticker for eyeballing the drawings:
//! `cargo run -p tidycuts-render --example stickers_sheet -- out_dir`.
//!
//! Writes `stickers.png` (default colors), `stickers_recolored.png` (every
//! sticker with two other colors) and `stickers_animated.png` (each animated
//! sticker at t = 0, 0.33 and 0.66), and prints index -> id.
//!
//! `-- out_dir <height> <id-prefix>` instead writes `zoom.png` with the
//! matching stickers at that height, for a closer look.

use tidycuts_core::StickerSpec;
use tidycuts_render::catalog::stickers::{render, StickerDef, STICKERS};

static CELL_H: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(160);

fn cell_h() -> u32 {
    CELL_H.load(std::sync::atomic::Ordering::Relaxed)
}
const PAD: u32 = 14;
const PER_ROW: usize = 8;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

struct Cell {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

fn cell(def: &StickerDef, color: [f32; 4], color2: [f32; 4], t: f32) -> Cell {
    let spec = StickerSpec {
        kind: def.id.into(),
        color,
        color2,
        motion: def.motion.into(),
    };
    let img = render(&spec, cell_h(), t);
    Cell {
        w: img.width,
        h: img.height,
        px: img.pixels,
    }
}

/// Lays cells out in rows of `per_row` on a mid-gray checkerboard.
fn sheet(cells: &[Cell], per_row: usize) -> (u32, u32, Vec<u8>) {
    let rows: Vec<&[Cell]> = cells.chunks(per_row).collect();
    let sw = rows
        .iter()
        .map(|r| r.iter().map(|c| c.w + PAD).sum::<u32>() + PAD)
        .max()
        .unwrap_or(1);
    let sh = rows.len() as u32 * (cell_h() + PAD) + PAD;
    let mut out = vec![0u8; (sw * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            let v = if ((x / 16) + (y / 16)) % 2 == 0 {
                118
            } else {
                140
            };
            let i = ((y * sw + x) * 4) as usize;
            out[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    for (r, row) in rows.iter().enumerate() {
        let mut ox = PAD;
        let oy = PAD + r as u32 * (cell_h() + PAD);
        for c in row.iter() {
            for y in 0..c.h {
                for x in 0..c.w {
                    let s = ((y * c.w + x) * 4) as usize;
                    let d = (((oy + y) * sw + ox + x) * 4) as usize;
                    let a = c.px[s + 3] as u32;
                    for k in 0..3 {
                        // Premultiplied over opaque.
                        out[d + k] = (c.px[s + k] as u32 + out[d + k] as u32 * (255 - a) / 255)
                            .min(255) as u8;
                    }
                }
            }
            ox += c.w + PAD;
        }
    }
    (sw, sh, out)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&out).unwrap();
    let args: Vec<String> = std::env::args().collect();
    if let (Some(h), Some(prefix)) = (args.get(2), args.get(3)) {
        CELL_H.store(
            h.parse().unwrap_or(400),
            std::sync::atomic::Ordering::Relaxed,
        );
        let t: f32 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let cells: Vec<Cell> = STICKERS
            .iter()
            .filter(|d| d.id.starts_with(prefix.as_str()))
            .map(|d| cell(d, d.color, d.color2, t))
            .collect();
        let (w, h, px) = sheet(&cells, 4);
        save(&format!("{out}/zoom.png"), w, h, &px);
        println!("wrote {out}/zoom.png ({} stickers)", cells.len());
        return;
    }
    for (i, d) in STICKERS.iter().enumerate() {
        println!(
            "{i:2} {:<22} {:<15} {}{}",
            d.id,
            d.category,
            d.motion,
            if d.animated { " (animated)" } else { "" }
        );
    }

    let cells: Vec<Cell> = STICKERS
        .iter()
        .map(|d| cell(d, d.color, d.color2, 0.0))
        .collect();
    let (w, h, px) = sheet(&cells, PER_ROW);
    save(&format!("{out}/stickers.png"), w, h, &px);

    let (a, b) = ([0.55, 0.3, 1.0, 1.0], [0.1, 0.9, 0.7, 1.0]);
    let cells: Vec<Cell> = STICKERS.iter().map(|d| cell(d, a, b, 0.0)).collect();
    let (w, h, px) = sheet(&cells, PER_ROW);
    save(&format!("{out}/stickers_recolored.png"), w, h, &px);

    let mut cells = Vec::new();
    let animated: Vec<&StickerDef> = STICKERS.iter().filter(|d| d.animated).collect();
    for d in &animated {
        for t in [0.0, 0.33, 0.66] {
            cells.push(cell(d, d.color, d.color2, t));
        }
    }
    let (w, h, px) = sheet(&cells, 6);
    save(&format!("{out}/stickers_animated.png"), w, h, &px);
    println!(
        "wrote {out}/stickers.png, stickers_recolored.png, stickers_animated.png ({} animated, 2 per row, 3 frames each)",
        animated.len()
    );
}
