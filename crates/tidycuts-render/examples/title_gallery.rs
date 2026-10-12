//! Draws title presets from the catalog on a contact sheet, each at a chosen
//! point of its in-animation, to judge them by eye:
//!
//! `cargo run --release -p tidycuts-render --example title_gallery -- out.png "prefix1,prefix2" [progress] [first] [count]`
//!
//! A preset is drawn when its id starts with one of the comma-separated
//! prefixes (`all` draws everything). `progress` is 0..1 along the in
//! animation (1 = finished).

use tidycuts_render::catalog::animations::{self, Phase};
use tidycuts_render::catalog::titles::TITLES;
use tidycuts_render::text::render_title_anim;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "titles.png".into());
    let prefixes: Vec<&str> = args
        .get(2)
        .map(|s| s.split(',').collect())
        .unwrap_or_default();
    let progress: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let first: usize = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
    let count: usize = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(24);
    let chosen: Vec<_> = TITLES
        .iter()
        .filter(|t| prefixes.iter().any(|p| *p == "all" || t.id.starts_with(p)))
        .skip(first)
        .take(count)
        .collect();
    let (cw, ch, cols) = (900u32, 210u32, 2u32);
    let rows = (chosen.len() as u32).div_ceil(cols).max(1);
    let (w, h) = (cw * cols, ch * rows);
    let mut sheet = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            // A soft dark photo-like backdrop so light and dark text both read.
            let v = 30.0 + 40.0 * (x as f32 / w as f32) + 20.0 * (y as f32 / h as f32);
            let i = ((y * w + x) * 4) as usize;
            sheet[i..i + 4].copy_from_slice(&[(v * 0.8) as u8, (v * 0.9) as u8, v as u8, 255]);
        }
    }
    let mut blit = |img: &tidycuts_render::RgbaImage, ox: i32, oy: i32, scale: f32| {
        let (sw, sh) = (
            (img.width as f32 * scale) as i32,
            (img.height as f32 * scale) as i32,
        );
        for y in 0..sh {
            for x in 0..sw {
                let (dx, dy) = (ox + x, oy + y);
                if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                    continue;
                }
                let (sx, sy) = ((x as f32 / scale) as u32, (y as f32 / scale) as u32);
                if sx >= img.width || sy >= img.height {
                    continue;
                }
                let si = ((sy * img.width + sx) * 4) as usize;
                let di = ((dy as u32 * w + dx as u32) * 4) as usize;
                let a = img.pixels[si + 3] as u32;
                for c in 0..3 {
                    sheet[di + c] = (img.pixels[si + c] as u32
                        + sheet[di + c] as u32 * (255 - a) / 255)
                        .min(255) as u8;
                }
            }
        }
    };
    for (k, t) in chosen.iter().enumerate() {
        let (cx, cy) = ((k as u32 % cols) * cw, (k as u32 / cols) * ch);
        let spec = (t.make)();
        let (letters, reveal) = match t.anim_in {
            Some((id, _)) => {
                let m = animations::evaluate(id, Phase::In, progress, [1920.0, 1080.0]);
                (m.letters, progress)
            }
            None => (None, 1.0),
        };
        let r = render_title_anim(&spec, 1.0, reveal, None, letters);
        let img = r.image;
        let scale = (((cw - 20) as f32 / img.width as f32)
            .min((ch - 36) as f32 / img.height as f32))
        .min(1.0);
        let (iw, ih) = (
            (img.width as f32 * scale) as i32,
            (img.height as f32 * scale) as i32,
        );
        blit(
            &img,
            cx as i32 + (cw as i32 - iw) / 2,
            cy as i32 + 26 + (ch as i32 - 30 - ih) / 2,
            scale,
        );
        // Label in small plain text.
        let label = tidycuts_core::TitleSpec {
            text: format!("{}  ·  {}", t.id, t.name),
            font: "Inter".into(),
            size: 14.0,
            color: [0.7, 0.75, 0.85, 1.0],
            ..tidycuts_core::TitleSpec::default()
        };
        let l = render_title_anim(&label, 1.0, 1.0, None, None).image;
        blit(&l, cx as i32 + 8, cy as i32 + 4, 1.0);
    }
    let file = std::fs::File::create(&out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&sheet)
        .unwrap();
    println!("wrote {out}: {} presets", chosen.len());
}
