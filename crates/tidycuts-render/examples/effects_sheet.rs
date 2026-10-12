//! Contact sheets of every catalog effect and filter with default
//! parameters, for eyeballing the library:
//! `cargo run -p tidycuts-render --example effects_sheet -- out_dir`.
//!
//! Shader effects are rendered at 1.3 s into the clip; motion effects show
//! the clip at 0.4 s with a faint ghost at 2.5 s so the direction of the
//! movement is visible. A checkerboard sits under every cell so transparent
//! pixels show. `effects_sheet out_dir <id> [landscape|sunset|portrait]`
//! renders a single effect or filter at 960x540 instead.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::Arc;
use tidycuts_render::catalog::{effects, filters, Sample};
use tidycuts_render::*;

const W: u32 = 320;
const H: u32 = 180;
const LABEL: u32 = 18;
const COLS: u32 = 4;
const PER_SHEET: usize = 24;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// Draws `text` in light grey with its baseline at (x, y).
fn draw_text(font: &FontRef, buf: &mut [u8], bw: u32, bh: u32, x: f32, y: f32, text: &str) {
    let scale = PxScale::from(13.0);
    let sf = font.as_scaled(scale);
    let mut pen = x;
    for ch in text.chars() {
        let id = sf.glyph_id(ch);
        let g = id.with_scale_and_position(scale, ab_glyph::point(pen, y));
        pen += sf.h_advance(id);
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|gx, gy, cov| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= bw as i32 || py >= bh as i32 {
                    return;
                }
                let i = ((py as u32 * bw + px as u32) * 4) as usize;
                for c in 0..3 {
                    let v = buf[i + c] as f32;
                    buf[i + c] = (v + (225.0 - v) * cov.min(1.0)) as u8;
                }
            });
        }
    }
}

fn sheet(font: &FontRef, cells: &[(String, Vec<u8>)]) -> (u32, u32, Vec<u8>) {
    let rows = (cells.len() as u32).div_ceil(COLS);
    let (cw, ch) = (W + 4, H + LABEL + 4);
    let (sw, sh) = (COLS * cw, rows * ch);
    let mut out = vec![24u8; (sw * sh * 4) as usize];
    for p in out.chunks_mut(4) {
        p[3] = 255;
    }
    for (i, (label, c)) in cells.iter().enumerate() {
        let (cx, cy) = (i as u32 % COLS, i as u32 / COLS);
        for y in 0..H {
            let src = &c[(y * W * 4) as usize..((y + 1) * W * 4) as usize];
            let dy = cy * ch + y;
            let start = ((dy * sw + cx * cw) * 4) as usize;
            out[start..start + (W * 4) as usize].copy_from_slice(src);
        }
        draw_text(
            font,
            &mut out,
            sw,
            sh,
            (cx * cw + 3) as f32,
            (cy * ch + H + 14) as f32,
            label,
        );
    }
    (sw, sh, out)
}

fn checker() -> RgbaImage {
    let mut img = RgbaImage::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let v = if ((x / 10) + (y / 10)) % 2 == 0 {
                70
            } else {
                110
            };
            let i = ((y * W + x) * 4) as usize;
            img.pixels[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    img
}

/// The portrait sample in front of a green screen, for the chroma key.
fn green_screen(w: u32, h: u32) -> RgbaImage {
    sample::portrait_green(w, h)
}

struct Ctx {
    comp: Compositor,
    /// Picture to use instead of each entry's own sample.
    force: Option<Sample>,
    canvas: [f32; 2],
    pics: [Arc<RgbaImage>; 11],
    board: Arc<RgbaImage>,
}

impl Ctx {
    fn layer(&self, img: Arc<RgbaImage>) -> Layer {
        Layer::full(Source::Pixels(Pixels::Rgba(img)), self.canvas)
    }

    fn pic(&self, def_id: &str, s: Sample) -> Arc<RgbaImage> {
        if def_id == "basic.chroma_key" {
            return self.pics[3].clone();
        }
        match self.force.unwrap_or(s) {
            Sample::Landscape => self.pics[0].clone(),
            Sample::Sunset => self.pics[1].clone(),
            Sample::Portrait => self.pics[2].clone(),
            Sample::Wildlife => self.pics[4].clone(),
            Sample::Meadow => self.pics[5].clone(),
            Sample::Night => self.pics[6].clone(),
            Sample::Pet => self.pics[7].clone(),
            Sample::Nap => self.pics[8].clone(),
            Sample::City => self.pics[9].clone(),
            Sample::Food => self.pics[10].clone(),
        }
    }

    fn effect(&mut self, def: &effects::EffectDef, target: &Target) -> Vec<u8> {
        let value = |k: &str| effects::default_value(def, k);
        let mut items = Vec::new();
        match def.kind {
            effects::EffectKind::Shader { id, passes } => {
                let (params, colors) = effects::pack(def, value);
                let mut l = self.layer(self.pic(def.id, def.sample));
                l.time = 1.3;
                l.effects.push(Fx {
                    shader: id,
                    passes,
                    params,
                    colors,
                    time: 1.3,
                    audio: [0.0; 4],
                });
                items.push(Item::Layer(self.layer(self.board.clone())));
                items.push(Item::Layer(l));
            }
            effects::EffectKind::Motion => {
                // The clip at 0.4 s, then faint ghosts of later moments.
                for (t, opacity) in [
                    (0.4f32, 1.0f32),
                    (1.1, 0.22),
                    (1.8, 0.22),
                    (2.5, 0.22),
                    (3.2, 0.22),
                ] {
                    let m = effects::motion(def.id, t, value);
                    let mut l = self.layer(self.pic(def.id, def.sample));
                    l.center = [960.0 + m.dx, 540.0 + m.dy];
                    l.scale = [m.scale * m.stretch[0], m.scale * m.stretch[1]];
                    l.rotation = m.rotation;
                    l.opacity = opacity;
                    if m.blur > 0.0 {
                        l.effects.push(Fx {
                            shader: 1,
                            passes: 2,
                            params: [[m.blur, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
                            colors: [[0.0; 4]; 2],
                            time: t,
                            audio: [0.0; 4],
                        });
                    }
                    items.push(Item::Layer(l));
                }
            }
        }
        let f = Frame {
            canvas: self.canvas,
            background: [0.15, 0.15, 0.17, 1.0],
            items,
        };
        self.comp.render_to_cpu(&f, target)
    }

    fn filter(&mut self, def: &filters::FilterDef, target: &Target) -> Vec<u8> {
        let mut l = self.layer(self.pic(def.id, def.sample));
        l.grade = Some((def.grade)());
        l.time = 1.3;
        let f = Frame {
            canvas: self.canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Layer(l)],
        };
        self.comp.render_to_cpu(&f, target)
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let only = std::env::args().nth(2);
    std::fs::create_dir_all(&out).unwrap();
    let font = FontRef::try_from_slice(fonts::inter()).unwrap();
    let r = gpu::headless().expect("gpu");
    let comp = Compositor::new(r);
    let force = match std::env::args().nth(3).as_deref() {
        Some("landscape") => Some(Sample::Landscape),
        Some("sunset") => Some(Sample::Sunset),
        Some("portrait") => Some(Sample::Portrait),
        _ => None,
    };
    let mut ctx = Ctx {
        comp,
        force,
        canvas: [1920.0, 1080.0],
        pics: [
            Arc::new(sample::landscape(640, 360)),
            Arc::new(sample::sunset(640, 360)),
            Arc::new(sample::portrait(640, 360)),
            Arc::new(green_screen(640, 360)),
            Arc::new(sample::wildlife(640, 360)),
            Arc::new(sample::meadow(640, 360)),
            Arc::new(sample::night(640, 360)),
            Arc::new(sample::pet(640, 360)),
            Arc::new(sample::nap(640, 360)),
            Arc::new(sample::city(640, 360)),
            Arc::new(sample::food(640, 360)),
        ],
        board: Arc::new(checker()),
    };

    // `effects_sheet out_dir <id> [landscape|sunset|portrait]` renders one
    // effect or filter large.
    if let Some(id) = only {
        let pics = [
            Arc::new(sample::landscape(1280, 720)),
            Arc::new(sample::sunset(1280, 720)),
            Arc::new(sample::portrait(1280, 720)),
            Arc::new(green_screen(1280, 720)),
            Arc::new(sample::wildlife(1280, 720)),
            Arc::new(sample::meadow(1280, 720)),
            Arc::new(sample::night(1280, 720)),
            Arc::new(sample::pet(1280, 720)),
            Arc::new(sample::nap(1280, 720)),
            Arc::new(sample::city(1280, 720)),
            Arc::new(sample::food(1280, 720)),
        ];
        ctx.pics = pics;
        let (bw, bh) = (960, 540);
        let target = ctx
            .comp
            .make_target(bw, bh, wgpu::TextureFormat::Rgba8Unorm, true);
        let px = if let Some(def) = effects::get(&id) {
            ctx.effect(def, &target)
        } else if let Some(def) = filters::get(&id) {
            ctx.filter(def, &target)
        } else {
            panic!("unknown id {id}");
        };
        let name = match std::env::args().nth(3) {
            Some(s) => format!("{id}-{s}"),
            None => id.clone(),
        };
        save(&format!("{out}/{name}.png"), bw, bh, &px);
        println!("wrote {out}/{name}.png");
        return;
    }

    let target = ctx
        .comp
        .make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let mut cells = Vec::new();
    for (i, def) in effects::EFFECTS.iter().enumerate() {
        println!("effect {i:3}: {} ({})", def.id, def.category);
        cells.push((format!("{i} {}", def.id), ctx.effect(def, &target)));
    }
    for (n, chunk) in cells.chunks(PER_SHEET).enumerate() {
        let (sw, sh, px) = sheet(&font, chunk);
        save(&format!("{out}/effects_{}.png", n + 1), sw, sh, &px);
    }

    let mut cells = Vec::new();
    for (i, def) in filters::FILTERS.iter().enumerate() {
        println!("filter {i:3}: {} ({})", def.id, def.category);
        cells.push((format!("{i} {}", def.id), ctx.filter(def, &target)));
    }
    for (n, chunk) in cells.chunks(PER_SHEET).enumerate() {
        let (sw, sh, px) = sheet(&font, chunk);
        save(&format!("{out}/filters_{}.png", n + 1), sw, sh, &px);
    }
    println!("wrote sheets to {out}");
}
