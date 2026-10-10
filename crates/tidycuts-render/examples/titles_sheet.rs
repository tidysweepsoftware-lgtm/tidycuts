//! Renders contact sheets of every title preset, every title style and the
//! reveal / typewriter animation, for eyeballing the title renderer:
//! `cargo run -p tidycuts-render --example titles_sheet -- out_dir [cell_width]`.
//!
//! Each preset is rendered at half resolution, composited on the CPU over a
//! 960x540 sample picture at its preset position, and downscaled into a
//! grid cell labelled with its index (the index → id list is printed).

use std::time::Instant;
use tidycuts_core::{TextAlign, TitleSpec};
use tidycuts_render::catalog::titles::TITLES;
use tidycuts_render::text::{render_title, STYLES};
use tidycuts_render::{sample, RgbaImage};

const FW: u32 = 960;
const FH: u32 = 540;
const PER_SHEET: usize = 12;
const COLS: u32 = 3;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// Draws a premultiplied image over an opaque frame, centred at (cx, cy).
fn over(dst: &mut RgbaImage, src: &RgbaImage, cx: f32, cy: f32) {
    let x0 = (cx - src.width as f32 * 0.5).round() as i32;
    let y0 = (cy - src.height as f32 * 0.5).round() as i32;
    for y in 0..src.height as i32 {
        let dy = y0 + y;
        if dy < 0 || dy >= dst.height as i32 {
            continue;
        }
        for x in 0..src.width as i32 {
            let dx = x0 + x;
            if dx < 0 || dx >= dst.width as i32 {
                continue;
            }
            let s = ((y as u32 * src.width + x as u32) * 4) as usize;
            let d = ((dy as u32 * dst.width + dx as u32) * 4) as usize;
            let a = src.pixels[s + 3] as u32;
            if a == 0 {
                continue;
            }
            for k in 0..4 {
                let v = src.pixels[s + k] as u32 + dst.pixels[d + k] as u32 * (255 - a) / 255;
                dst.pixels[d + k] = v.min(255) as u8;
            }
        }
    }
}

/// Box-filters `src` down to `w` x `h` (integer factor).
fn downscale(src: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let f = (src.width / w).max(1);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for yy in 0..f {
                for xx in 0..f {
                    let s = (((y * f + yy) * src.width + x * f + xx) * 4) as usize;
                    for (a, &p) in acc.iter_mut().zip(&src.pixels[s..s + 4]) {
                        *a += p as u32;
                    }
                }
            }
            let d = ((y * w + x) * 4) as usize;
            for (o, a) in out.pixels[d..d + 4].iter_mut().zip(acc) {
                *o = (a / (f * f)) as u8;
            }
        }
    }
    out
}

fn background(i: usize) -> RgbaImage {
    if i.is_multiple_of(2) {
        sample::sunset(FW, FH)
    } else {
        sample::landscape(FW, FH)
    }
}

fn label(text: &str) -> RgbaImage {
    let spec = TitleSpec {
        text: text.into(),
        font: "Inter".into(),
        size: 22.0,
        style: "subtitle".into(),
        background_color: [0.0, 0.0, 0.0, 0.7],
        ..Default::default()
    };
    render_title(&spec, 1.0, 1.0, None).image
}

/// One cell: `spec` on a sample picture at `pos` (1920x1080 offsets).
fn cell(
    bg_index: usize,
    spec: &TitleSpec,
    pos: [f32; 2],
    reveal: f32,
    visible: Option<usize>,
    name: &str,
    cw: u32,
) -> RgbaImage {
    let mut frame = background(bg_index);
    let t = render_title(spec, 0.5, reveal, visible);
    over(
        &mut frame,
        &t.image,
        FW as f32 * 0.5 + pos[0] * 0.5,
        FH as f32 * 0.5 + pos[1] * 0.5,
    );
    let mut c = downscale(&frame, cw, cw * 9 / 16);
    let l = label(name);
    over(
        &mut c,
        &l,
        4.0 + l.width as f32 * 0.5,
        4.0 + l.height as f32 * 0.5,
    );
    c
}

fn sheet(cells: &[RgbaImage], cols: u32, path: &str) {
    let (cw, ch) = (cells[0].width, cells[0].height);
    let rows = (cells.len() as u32).div_ceil(cols);
    let (sw, sh) = (cols * (cw + 4) + 4, rows * (ch + 4) + 4);
    let mut out = vec![24u8; (sw * sh * 4) as usize];
    for (i, c) in cells.iter().enumerate() {
        let (cx, cy) = (i as u32 % cols, i as u32 / cols);
        for y in 0..ch {
            let dy = 4 + cy * (ch + 4) + y;
            let start = ((dy * sw + 4 + cx * (cw + 4)) * 4) as usize;
            out[start..start + (cw * 4) as usize]
                .copy_from_slice(&c.pixels[(y * cw * 4) as usize..((y + 1) * cw * 4) as usize]);
        }
    }
    for p in out.as_chunks_mut::<4>().0.iter_mut() {
        p[3] = 255;
    }
    save(path, sw, sh, &out);
    println!("wrote {path}");
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let cw: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(480);
    std::fs::create_dir_all(&out).unwrap();

    // Presets.
    let mut index = 0;
    for (s, chunk) in TITLES.chunks(PER_SHEET).enumerate() {
        let mut cells = Vec::new();
        for t in chunk {
            println!(
                "{index:2} {:<26} {:<14} {}",
                t.id,
                t.category,
                (t.make)().style
            );
            cells.push(cell(
                index,
                &(t.make)(),
                t.position,
                1.0,
                None,
                &format!("{index} {}", t.id),
                cw,
            ));
            index += 1;
        }
        sheet(&cells, COLS, &format!("{out}/titles_{}.png", s + 1));
    }

    // Every style with the same text.
    for (reveal, file) in [(1.0, "styles.png"), (0.5, "styles_half.png")] {
        let mut cells = Vec::new();
        for (i, (id, _)) in STYLES.iter().enumerate() {
            let lower = matches!(*id, "lower_bar" | "lower_slant" | "callout");
            let spec = TitleSpec {
                style: id.to_string(),
                text: "Your Title Here\nSubtitle text".into(),
                size: 84.0,
                align: if lower {
                    TextAlign::Left
                } else {
                    TextAlign::Center
                },
                accent: [0.98, 0.33, 0.45, 1.0],
                ..Default::default()
            };
            let pos = if lower { [-420.0, 0.0] } else { [0.0, 0.0] };
            cells.push(cell(
                i + 1,
                &spec,
                pos,
                reveal,
                None,
                &format!("{i} {id} reveal {reveal}"),
                cw,
            ));
        }
        sheet(&cells, 4, &format!("{out}/{file}"));
    }

    // Text options on the plain style.
    let base = TitleSpec {
        text: "Text Options\nSecond line".into(),
        size: 90.0,
        ..Default::default()
    };
    let options: Vec<(&str, TitleSpec)> = vec![
        ("default", base.clone()),
        (
            "bold",
            TitleSpec {
                bold: true,
                ..base.clone()
            },
        ),
        (
            "italic",
            TitleSpec {
                italic: true,
                ..base.clone()
            },
        ),
        (
            "outline 6",
            TitleSpec {
                outline: 6.0,
                outline_color: [0.9, 0.1, 0.3, 1.0],
                ..base.clone()
            },
        ),
        (
            "shadow 100",
            TitleSpec {
                shadow: 100.0,
                ..base.clone()
            },
        ),
        (
            "background",
            TitleSpec {
                background: true,
                ..base.clone()
            },
        ),
        (
            "spacing 0.2 / 1.6",
            TitleSpec {
                letter_spacing: 0.2,
                line_spacing: 1.6,
                ..base.clone()
            },
        ),
        (
            "align left",
            TitleSpec {
                align: TextAlign::Left,
                ..base.clone()
            },
        ),
        (
            "align right",
            TitleSpec {
                align: TextAlign::Right,
                ..base.clone()
            },
        ),
        (
            "all on",
            TitleSpec {
                bold: true,
                italic: true,
                outline: 4.0,
                shadow: 60.0,
                background: true,
                ..base.clone()
            },
        ),
        (
            "Cyrillic fallback",
            TitleSpec {
                text: "Здравей, свят!\nBebas + Inter".into(),
                font: "Bebas Neue".into(),
                ..base.clone()
            },
        ),
        (
            "tiny 24px",
            TitleSpec {
                size: 24.0,
                ..base.clone()
            },
        ),
    ];
    let cells: Vec<RgbaImage> = options
        .iter()
        .enumerate()
        .map(|(i, (name, spec))| cell(i, spec, [0.0, 0.0], 1.0, None, name, cw))
        .collect();
    sheet(&cells, 3, &format!("{out}/options.png"));

    // Reveal and typewriter progress.
    let mut cells = Vec::new();
    for (i, (style, font)) in [
        ("lower_bar", "Poppins Bold"),
        ("outline_box", "Bebas Neue"),
        ("highlight", "Permanent Marker"),
    ]
    .iter()
    .enumerate()
    {
        let spec = TitleSpec {
            style: style.to_string(),
            text: "Reveal Test\nSecond line".into(),
            font: font.to_string(),
            size: 90.0,
            align: if i == 0 {
                TextAlign::Left
            } else {
                TextAlign::Center
            },
            color: if i == 2 {
                [0.1, 0.1, 0.12, 1.0]
            } else {
                [1.0; 4]
            },
            accent: if i == 2 {
                [1.0, 0.86, 0.2, 1.0]
            } else {
                [0.2, 0.75, 1.0, 1.0]
            },
            ..Default::default()
        };
        let n = spec.text.chars().count();
        for (j, (rev, vis)) in [
            (0.0, None),
            (0.25, None),
            (0.5, None),
            (1.0, None),
            (1.0, Some(n * 3 / 10)),
            (1.0, Some(n * 7 / 10)),
        ]
        .into_iter()
        .enumerate()
        {
            let name = match vis {
                Some(v) => format!("{style} chars {v}/{n}"),
                None => format!("{style} reveal {rev}"),
            };
            cells.push(cell(i + j, &spec, [0.0, 0.0], rev, vis, &name, cw));
        }
    }
    sheet(&cells, 6, &format!("{out}/reveal.png"));

    // Speed of a typical title while typing.
    let spec = TitleSpec {
        text: "Your Title Here\nSecond line of text".into(),
        shadow: 50.0,
        outline: 3.0,
        ..Default::default()
    };
    let n = 10;
    let t0 = Instant::now();
    for _ in 0..n {
        std::hint::black_box(render_title(&spec, 1.0, 1.0, None));
    }
    println!(
        "2-line 96px title with shadow: {:.2} ms",
        t0.elapsed().as_secs_f64() * 1000.0 / n as f64
    );
    for (what, s) in [
        (
            "no outline or shadow",
            TitleSpec {
                shadow: 0.0,
                outline: 0.0,
                ..spec.clone()
            },
        ),
        (
            "outline only",
            TitleSpec {
                shadow: 0.0,
                ..spec.clone()
            },
        ),
        (
            "shadow only",
            TitleSpec {
                outline: 0.0,
                ..spec.clone()
            },
        ),
    ] {
        let t0 = Instant::now();
        for _ in 0..n {
            std::hint::black_box(render_title(&s, 1.0, 1.0, None));
        }
        println!(
            "  {what:<22} {:6.2} ms",
            t0.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
    }
    for (id, _) in STYLES {
        let s = TitleSpec {
            style: id.to_string(),
            ..spec.clone()
        };
        let t0 = Instant::now();
        let r = render_title(&s, 1.0, 1.0, None);
        println!(
            "  {id:<12} {:6.2} ms  {}x{}",
            t0.elapsed().as_secs_f64() * 1000.0,
            r.image.width,
            r.image.height
        );
    }
}
