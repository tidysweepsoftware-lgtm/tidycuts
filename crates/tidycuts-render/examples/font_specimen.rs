//! Draws every embedded font as a specimen sheet, to check by eye that each
//! one renders (some variable fonts open on a light weight):
//!
//! `cargo run --release -p tidycuts-render --example font_specimen -- out.png [first] [count]`

use tidycuts_core::TitleSpec;
use tidycuts_render::fonts::FONTS;
use tidycuts_render::text::render_title_anim;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "specimen.png".into());
    let first: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
    let count: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(40);
    let fonts: Vec<_> = FONTS.iter().skip(first).take(count).collect();
    let (cw, ch, cols) = (620u32, 110u32, 3u32);
    let rows = (fonts.len() as u32).div_ceil(cols);
    let (w, h) = (cw * cols, ch * rows);
    let mut sheet = vec![0u8; (w * h * 4) as usize];
    for px in sheet.chunks_mut(4) {
        px.copy_from_slice(&[18, 20, 28, 255]);
    }
    let mut blit = |img: &tidycuts_render::RgbaImage, ox: i32, oy: i32| {
        for y in 0..img.height as i32 {
            for x in 0..img.width as i32 {
                let (dx, dy) = (ox + x, oy + y);
                if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                    continue;
                }
                let si = ((y as u32 * img.width + x as u32) * 4) as usize;
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
    for (k, f) in fonts.iter().enumerate() {
        let (cx, cy) = ((k as u32 % cols) * cw, (k as u32 / cols) * ch);
        let label = TitleSpec {
            text: f.name.into(),
            font: "Inter".into(),
            size: 15.0,
            color: [0.55, 0.6, 0.7, 1.0],
            ..TitleSpec::default()
        };
        let l = render_title_anim(&label, 1.0, 1.0, None, None).image;
        blit(&l, cx as i32 + 8, cy as i32 + 4);
        let spec = TitleSpec {
            text: "Aa Quick Fox 2026".into(),
            font: f.name.into(),
            size: 54.0,
            ..TitleSpec::default()
        };
        let r = render_title_anim(&spec, 1.0, 1.0, None, None).image;
        blit(
            &r,
            cx as i32 + 8,
            cy as i32 + 22 + ((ch as i32 - 26 - r.height as i32) / 2).max(0),
        );
    }
    let file = std::fs::File::create(&out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&sheet)
        .unwrap();
    println!("wrote {out}: {} fonts", fonts.len());
}
