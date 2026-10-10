//! Contact sheets of every catalog transition plus an end-point check:
//! `cargo run -p tidycuts-render --example transitions_sheet -- out_dir [pip]`.
//!
//! Each transition is one row of three cells (25%, 50% and 75% progress)
//! with its id written on the left. Every transition is also rendered at 0%
//! and 100% and compared with the outgoing and incoming picture alone, in
//! three set-ups (full-frame 16:9, smaller pictures with transparency around
//! them over a grey background, and an odd-sized portrait canvas); the
//! program exits with an error if any differs by more than 2/255.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::Arc;
use tidycuts_render::catalog::transitions::{TransitionDef, TRANSITIONS};
use tidycuts_render::*;

const W: u32 = 240;
const H: u32 = 135;
const GAP: u32 = 4;
const LABEL: u32 = 190;
const ROWS_PER_SHEET: usize = 11;
const STEPS: [f32; 3] = [0.25, 0.5, 0.75];

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// Draws `text` in light grey with its baseline at (x, y).
#[allow(clippy::too_many_arguments)]
fn draw_text(
    img: &mut [u8],
    iw: u32,
    font: &FontRef,
    size: f32,
    x: f32,
    y: f32,
    text: &str,
    shade: u8,
) {
    let scale = PxScale::from(size);
    let sf = font.as_scaled(scale);
    let mut pen = x;
    let mut prev = None;
    for ch in text.chars() {
        let id = sf.glyph_id(ch);
        if let Some(p) = prev {
            pen += sf.kern(p, id);
        }
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(pen, y));
        pen += sf.h_advance(id);
        prev = Some(id);
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= iw as i32 {
                    return;
                }
                let i = ((py as u32 * iw + px as u32) * 4) as usize;
                if i + 3 < img.len() {
                    for k in 0..3 {
                        let v = img[i + k] as f32;
                        img[i + k] = (v + (shade as f32 - v) * c.min(1.0)) as u8;
                    }
                }
            });
        }
    }
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    // `pip` as a second argument draws the sheets with the smaller,
    // partly transparent pictures instead.
    let pip = std::env::args().nth(2).is_some_and(|a| a == "pip");
    let (sheet_setup, prefix) = if pip {
        (1, "transitions_pip")
    } else {
        (0, "transitions")
    };
    std::fs::create_dir_all(&out).unwrap();
    let r = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(r);
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let a = Arc::new(sample::landscape(640, 360));
    let b = Arc::new(sample::sunset(640, 360));
    let layer = |img: &Arc<RgbaImage>, canvas: [f32; 2]| {
        Layer::full(Source::Pixels(Pixels::Rgba(img.clone())), canvas)
    };
    let font = FontRef::try_from_slice(fonts::inter()).unwrap();

    // End-point set-ups: (canvas, background, A, B, target).
    let wide = [1920.0f32, 1080.0];
    let tall = [1080.0f32, 1920.0];
    let mut small_a = layer(&a, wide);
    small_a.scale = [0.55, 0.55];
    small_a.center = [700.0, 480.0];
    small_a.corner_radius = 60.0;
    let mut small_b = layer(&b, wide);
    small_b.scale = [0.6, 0.6];
    small_b.center = [1150.0, 600.0];
    small_b.rotation = 6.0;
    let tall_target = comp.make_target(101, 179, wgpu::TextureFormat::Rgba8Unorm, true);
    let black = [0.0, 0.0, 0.0, 1.0];
    let setups = [
        (wide, black, layer(&a, wide), layer(&b, wide), &target),
        (wide, [0.35, 0.4, 0.45, 1.0], small_a, small_b, &target),
        (tall, black, layer(&a, tall), layer(&b, tall), &tall_target),
    ];
    let render = |comp: &mut Compositor, def: &TransitionDef, s: usize, p: f32| {
        let (canvas, background, from, to, target) = &setups[s];
        let f = Frame {
            canvas: *canvas,
            background: *background,
            items: vec![Item::Transition(TransitionItem {
                from: Some(from.clone()),
                to: Some(to.clone()),
                kind: def.shader,
                progress: p,
                params: [def.params, [0.0; 4], [0.0; 4], [0.0; 4]],
                color: def.color,
                time: p * def.duration,
            })],
        };
        comp.render_to_cpu(&f, target)
    };
    let refs: Vec<(Vec<u8>, Vec<u8>)> = setups
        .iter()
        .map(|(canvas, background, from, to, target)| {
            let mut alone = |l: &Layer| {
                let f = Frame {
                    canvas: *canvas,
                    background: *background,
                    items: vec![Item::Layer(l.clone())],
                };
                comp.render_to_cpu(&f, target)
            };
            (alone(from), alone(to))
        })
        .collect();

    let mut failed = Vec::new();
    let mut rows: Vec<(String, String, Vec<Vec<u8>>)> = Vec::new();
    for def in TRANSITIONS {
        let (mut d0, mut d1) = (0, 0);
        for (s, (ref_a, ref_b)) in refs.iter().enumerate() {
            d0 = d0.max(max_diff(&render(&mut comp, def, s, 0.0), ref_a));
            d1 = d1.max(max_diff(&render(&mut comp, def, s, 1.0), ref_b));
        }
        let cells = STEPS
            .iter()
            .map(|&p| render(&mut comp, def, sheet_setup, p))
            .collect();
        let ok = d0 <= 2 && d1 <= 2;
        println!(
            "{:<22} shader {:>2}  diff t=0 {:>3}  t=1 {:>3}  {}",
            def.id,
            def.shader,
            d0,
            d1,
            if ok { "ok" } else { "FAIL" }
        );
        if !ok {
            failed.push(def.id);
        }
        rows.push((
            def.id.to_string(),
            format!("{} · {}", def.category, def.name),
            cells,
        ));
    }

    let sw = LABEL + 3 * (W + GAP);
    for (si, chunk) in rows.chunks(ROWS_PER_SHEET).enumerate() {
        let sh = chunk.len() as u32 * (H + GAP);
        let mut img = vec![24u8; (sw * sh * 4) as usize];
        for px in img.chunks_mut(4) {
            px[3] = 255;
        }
        for (ri, (id, name, cells)) in chunk.iter().enumerate() {
            let y0 = ri as u32 * (H + GAP);
            let n = si * ROWS_PER_SHEET + ri;
            draw_text(
                &mut img,
                sw,
                &font,
                17.0,
                8.0,
                y0 as f32 + 26.0,
                &format!("{n:>2} {id}"),
                235,
            );
            draw_text(&mut img, sw, &font, 12.0, 8.0, y0 as f32 + 46.0, name, 150);
            for (ci, c) in cells.iter().enumerate() {
                let x0 = LABEL + ci as u32 * (W + GAP);
                for y in 0..H {
                    let src = &c[(y * W * 4) as usize..((y + 1) * W * 4) as usize];
                    let start = (((y0 + y) * sw + x0) * 4) as usize;
                    img[start..start + (W * 4) as usize].copy_from_slice(src);
                }
            }
            println!("sheet {:02} row {:>2}: {}", si + 1, ri, id);
        }
        let path = format!("{out}/{prefix}_{:02}.png", si + 1);
        save(&path, sw, sh, &img);
        println!("wrote {path}");
    }

    if !failed.is_empty() {
        eprintln!("end points differ for: {}", failed.join(", "));
        std::process::exit(1);
    }
    println!("{} transitions, all end points match", TRANSITIONS.len());
}
