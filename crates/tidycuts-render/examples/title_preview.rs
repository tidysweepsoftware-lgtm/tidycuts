//! Renders a looping preview of the modern title styles, one numbered PNG
//! per frame, to turn into a video:
//!
//! `cargo run --release -p tidycuts-render --example title_preview -- out_dir [seconds] [fps] [mode]`
//!
//! `mode` is `text` (the default: styled text with letter animations) or
//! `cards` (lower thirds and labels that build themselves in stages).
//! Then, for example:
//! `ffmpeg -framerate 30 -i out_dir/frame_%04d.png -c:v libx264 -pix_fmt yuv420p -crf 18 preview.mp4`

use tidycuts_core::TitleSpec;
use tidycuts_render::catalog::animations::LetterKind;
use tidycuts_render::text::render_title_anim;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

struct Sample {
    style: &'static str,
    text: &'static str,
    font: &'static str,
    size: f32,
    spacing: f32,
    color: [f32; 4],
    accent: [f32; 4],
    kind: LetterKind,
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[allow(clippy::too_many_arguments)]
fn sample(
    style: &'static str,
    text: &'static str,
    font: &'static str,
    size: f32,
    spacing: f32,
    color: [f32; 4],
    accent: [f32; 4],
    kind: LetterKind,
) -> Sample {
    Sample {
        style,
        text,
        font,
        size,
        spacing,
        color,
        accent,
        kind,
    }
}

fn text_samples() -> Vec<Sample> {
    use LetterKind::*;
    vec![
        sample(
            "gold_bevel",
            "GO LIVE",
            "Poppins Black",
            104.0,
            0.0,
            [1.0, 0.82, 0.3, 1.0],
            [0.6, 0.3, 0.05, 1.0],
            Pop,
        ),
        sample(
            "neon_tube",
            "NEW DROP",
            "Poppins Black",
            104.0,
            0.0,
            [0.4, 0.95, 1.0, 1.0],
            [0.1, 0.6, 1.0, 1.0],
            Rise,
        ),
        sample(
            "pop_3d",
            "SALE 50%",
            "Poppins Black",
            104.0,
            0.0,
            [1.0, 0.4, 0.7, 1.0],
            [0.45, 0.15, 0.7, 1.0],
            Pop,
        ),
        sample(
            "glass_soft",
            "Summer in Lisbon",
            "Poppins SemiBold",
            96.0,
            0.0,
            [0.85, 0.93, 1.0, 1.0],
            [0.5, 0.7, 1.0, 1.0],
            Soft,
        ),
        sample(
            "clean_soft",
            "Simply better.",
            "Poppins SemiBold",
            108.0,
            -0.02,
            WHITE,
            WHITE,
            Soft,
        ),
        sample(
            "clean_gradient",
            "Made to move.",
            "Poppins SemiBold",
            108.0,
            -0.02,
            WHITE,
            [0.62, 0.68, 1.0, 1.0],
            WordsSoft,
        ),
        sample(
            "clean_glow",
            "COMING SOON",
            "Poppins",
            92.0,
            0.14,
            WHITE,
            [0.5, 0.7, 1.0, 1.0],
            Soft,
        ),
        sample(
            "clean_soft",
            "Honest review",
            "Poppins",
            96.0,
            0.0,
            [0.96, 0.9, 0.78, 1.0],
            WHITE,
            WordsSoft,
        ),
    ]
}

fn card_samples() -> Vec<Sample> {
    use LetterKind::Soft;
    vec![
        sample(
            "card_gradient",
            "Alex Morgan",
            "Poppins SemiBold",
            64.0,
            0.0,
            WHITE,
            [0.2, 0.55, 0.95, 1.0],
            Soft,
        ),
        sample(
            "card_gradient",
            "Season finale",
            "Poppins SemiBold",
            64.0,
            0.0,
            WHITE,
            [0.9, 0.3, 0.55, 1.0],
            Soft,
        ),
        sample(
            "card_bar",
            "BREAKING NEWS",
            "Poppins SemiBold",
            56.0,
            0.04,
            WHITE,
            [0.92, 0.18, 0.2, 1.0],
            Soft,
        ),
        sample(
            "card_bar",
            "Lisbon, Portugal",
            "Poppins",
            60.0,
            0.0,
            WHITE,
            [0.2, 0.8, 0.7, 1.0],
            Soft,
        ),
        sample(
            "card_pill",
            "Subscribe",
            "Poppins SemiBold",
            60.0,
            0.0,
            WHITE,
            [0.95, 0.35, 0.3, 1.0],
            Soft,
        ),
        sample(
            "card_pill",
            "New review",
            "Poppins SemiBold",
            60.0,
            0.0,
            WHITE,
            [0.55, 0.4, 0.95, 1.0],
            Soft,
        ),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| ".".into());
    let seconds: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let fps: u32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(30);
    let cards = args.get(4).is_some_and(|m| m == "cards");
    std::fs::create_dir_all(&out).unwrap();

    let titles = if cards {
        card_samples()
    } else {
        text_samples()
    };
    let (cw, ch) = if cards {
        (1000u32, 260u32)
    } else {
        (1200u32, 300u32)
    };
    let rows = titles.len().div_ceil(2) as u32;
    let (w, h) = (cw * 2, ch * rows);
    let frames = (seconds * fps as f32).round() as u32;

    for n in 0..frames {
        let t = n as f32 / fps as f32;
        // The letters animate in over the first 1.3 s, then hold; a card
        // takes 1.8 s to build.
        let p = (t / 1.3).clamp(0.0, 1.0);
        let build = (t / 1.8).clamp(0.0, 1.0);
        let mut sheet = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let v = 14.0 + 18.0 * (y as f32 / h as f32);
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                sheet[i..i + 4].copy_from_slice(&[v as u8, (v + 2.0) as u8, (v + 10.0) as u8, 255]);
            }
        }
        for (k, s) in titles.iter().enumerate() {
            let spec = TitleSpec {
                style: s.style.into(),
                text: s.text.into(),
                font: s.font.into(),
                size: s.size,
                letter_spacing: s.spacing,
                color: s.color,
                accent: s.accent,
                ..TitleSpec::default()
            };
            let r = if cards {
                render_title_anim(&spec, 1.0, build, None, None)
            } else {
                render_title_anim(&spec, 1.0, 1.0, None, Some((s.kind, p)))
            };
            let img = r.image;
            let (ox, oy) = (
                (k as u32 % 2) * cw + (cw as i32 - img.width as i32).max(0) as u32 / 2,
                (k as u32 / 2) * ch + (ch as i32 - img.height as i32).max(0) as u32 / 2,
            );
            for y in 0..img.height.min(ch) {
                for x in 0..img.width.min(cw) {
                    let si = ((y * img.width + x) * 4) as usize;
                    let d = (((oy + y) * w + ox + x) * 4) as usize;
                    let a = img.pixels[si + 3] as u32;
                    for c in 0..3 {
                        sheet[d + c] = (img.pixels[si + c] as u32
                            + sheet[d + c] as u32 * (255 - a) / 255)
                            .min(255) as u8;
                    }
                }
            }
        }
        save(&format!("{out}/frame_{n:04}.png"), w, h, &sheet);
    }
    println!("wrote {frames} frames of {w}x{h} to {out}");
}
