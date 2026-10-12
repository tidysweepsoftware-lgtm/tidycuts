//! Renders a word in four distance-field looks, to check them by eye:
//! `cargo run -p tidycuts-render --example sdf_demo -- out_dir [text]`
//! Writes `sdf_demo.png`.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use tidycuts_render::sdf::{render, Bevel, Extrude, Field, Fill, Glow, Look, Outline, Shadow};

const PAD: u32 = 80;

/// The alpha mask of `text`, with padding on every side.
fn mask(font_name: &str, text: &str, size: f32) -> (Vec<u8>, u32, u32) {
    let font = FontRef::try_from_slice(tidycuts_render::fonts::get(font_name).data).unwrap();
    let scale = PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let width: f32 = text
        .chars()
        .map(|c| scaled.h_advance(font.glyph_id(c)))
        .sum();
    let (w, h) = (width.ceil() as u32 + PAD * 2, size.ceil() as u32 + PAD * 2);
    let mut a = vec![0u8; (w * h) as usize];
    let mut x = PAD as f32;
    let base = PAD as f32 + scaled.ascent();
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let g = id.with_scale_and_position(scale, point(x, base));
        if let Some(og) = font.outline_glyph(g) {
            let b = og.px_bounds();
            og.draw(|px, py, c| {
                let (gx, gy) = (b.min.x as i32 + px as i32, b.min.y as i32 + py as i32);
                if gx >= 0 && gy >= 0 && (gx as u32) < w && (gy as u32) < h {
                    let i = (gy as u32 * w + gx as u32) as usize;
                    a[i] = a[i].max((c * 255.0) as u8);
                }
            });
        }
        x += scaled.h_advance(id);
    }
    (a, w, h)
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
    let text = std::env::args().nth(2).unwrap_or_else(|| "GO!".into());
    std::fs::create_dir_all(&out).unwrap();

    let (a, w, h) = mask("Poppins Black", &text, 190.0);
    let field = Field::from_alpha(&a, w, h);

    let looks: [Look; 4] = [
        // Gold block.
        Look {
            fill: Fill {
                top: [1.0, 0.93, 0.55, 1.0],
                bottom: [0.9, 0.55, 0.1, 1.0],
            },
            outline: Some(Outline {
                color: [0.35, 0.17, 0.02, 1.0],
                width: 5.0,
            }),
            glow: None,
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.55],
                offset: (14.0, 30.0),
                blur: 12.0,
            }),
            extrude: Some(Extrude {
                color: [0.3, 0.14, 0.02, 1.0],
                depth: 22.0,
                dir: (0.45, 1.0),
            }),
            bevel: Some(Bevel {
                width: 16.0,
                height: 16.0,
                light: (-0.55, -0.85),
                highlight: [1.0, 1.0, 0.86, 1.0],
                shade: [0.55, 0.22, 0.0, 1.0],
                shine: 0.95,
                sharpness: 40.0,
            }),
        },
        // Neon tube.
        Look {
            fill: Fill {
                top: [0.92, 1.0, 1.0, 1.0],
                bottom: [0.75, 0.97, 1.0, 1.0],
            },
            outline: Some(Outline {
                color: [0.1, 0.75, 1.0, 1.0],
                width: 7.0,
            }),
            glow: Some(Glow {
                color: [0.1, 0.6, 1.0, 1.0],
                radius: 22.0,
                strength: 0.95,
            }),
            shadow: None,
            extrude: None,
            bevel: None,
        },
        // Pop sticker: white rim and a purple block.
        Look {
            fill: Fill {
                top: [1.0, 0.55, 0.75, 1.0],
                bottom: [0.95, 0.2, 0.5, 1.0],
            },
            outline: Some(Outline {
                color: [1.0, 1.0, 1.0, 1.0],
                width: 9.0,
            }),
            glow: None,
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.5],
                offset: (6.0, 22.0),
                blur: 14.0,
            }),
            extrude: Some(Extrude {
                color: [0.35, 0.1, 0.55, 1.0],
                depth: 26.0,
                dir: (0.2, 1.0),
            }),
            bevel: Some(Bevel {
                width: 22.0,
                height: 20.0,
                light: (-0.4, -0.9),
                highlight: [1.0, 0.85, 0.95, 1.0],
                shade: [0.55, 0.0, 0.3, 1.0],
                shine: 0.9,
                sharpness: 60.0,
            }),
        },
        // Soft glass.
        Look {
            fill: Fill {
                top: [1.0, 1.0, 1.0, 0.95],
                bottom: [0.7, 0.85, 1.0, 0.85],
            },
            outline: Some(Outline {
                color: [1.0, 1.0, 1.0, 0.35],
                width: 4.0,
            }),
            glow: Some(Glow {
                color: [0.8, 0.9, 1.0, 1.0],
                radius: 30.0,
                strength: 0.35,
            }),
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.1, 0.4],
                offset: (0.0, 16.0),
                blur: 18.0,
            }),
            extrude: None,
            bevel: Some(Bevel {
                width: 12.0,
                height: 10.0,
                light: (-0.5, -0.9),
                highlight: [1.0, 1.0, 1.0, 1.0],
                shade: [0.45, 0.6, 0.9, 1.0],
                shine: 0.8,
                sharpness: 80.0,
            }),
        },
    ];

    // Lay the four looks out 2 x 2 on a dark background.
    let (sw, sh) = (w * 2, h * 2);
    let mut sheet = vec![0u8; (sw * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            let v = 18.0 + 14.0 * (y as f32 / sh as f32);
            let i = ((y * sw + x) * 4) as usize;
            sheet[i..i + 4].copy_from_slice(&[v as u8, (v + 2.0) as u8, (v + 8.0) as u8, 255]);
        }
    }
    for (n, look) in looks.iter().enumerate() {
        let img = render(&field, look);
        let (ox, oy) = ((n as u32 % 2) * w, (n as u32 / 2) * h);
        for y in 0..h {
            for x in 0..w {
                let s = ((y * w + x) * 4) as usize;
                let d = (((oy + y) * sw + ox + x) * 4) as usize;
                let a = img.pixels[s + 3] as u32;
                for k in 0..3 {
                    sheet[d + k] = (img.pixels[s + k] as u32
                        + sheet[d + k] as u32 * (255 - a) / 255)
                        .min(255) as u8;
                }
            }
        }
    }
    save(&format!("{out}/sdf_demo.png"), sw, sh, &sheet);
    println!("wrote {out}/sdf_demo.png ({sw}x{sh})");
}
