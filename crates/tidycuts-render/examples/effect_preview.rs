//! Renders effects in motion, one numbered PNG per frame, to turn into a
//! video:
//!
//! `cargo run --release -p tidycuts-render --example effect_preview -- out_dir ids [picture] [seconds] [fps]`
//!
//! `ids` is one effect id or several separated by commas; each plays for
//! `seconds` in turn. `picture` is landscape, sunset, portrait, night,
//! wildlife or pet. Shader effects only (the moving ones need a clip's
//! transform). Then, for example:
//! `ffmpeg -framerate 30 -i out_dir/frame_%05d.png -c:v libx264 -pix_fmt yuv420p -crf 18 preview.mp4`

use std::sync::Arc;
use tidycuts_render::catalog::effects;
use tidycuts_render::*;

const W: u32 = 960;
const H: u32 = 540;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| ".".into());
    let ids = args.get(2).cloned().unwrap_or_default();
    let picture = args.get(3).cloned().unwrap_or_else(|| "landscape".into());
    let seconds: f32 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let fps: f32 = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(30.0);
    std::fs::create_dir_all(&out).unwrap();

    let img = Arc::new(match picture.as_str() {
        "sunset" => sample::sunset(1280, 720),
        "portrait" => sample::portrait(1280, 720),
        "night" => sample::night(1280, 720),
        "wildlife" => sample::wildlife(1280, 720),
        "pet" => sample::pet(1280, 720),
        _ => sample::landscape(1280, 720),
    });
    let r = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(r);
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let canvas = [1920.0f32, 1080.0];

    let mut n = 0u32;
    for id in ids.split(',').filter(|s| !s.is_empty()) {
        let Some(def) = effects::get(id) else {
            eprintln!("unknown effect '{id}'");
            continue;
        };
        let effects::EffectKind::Shader { id: shader, passes } = def.kind else {
            eprintln!("'{id}' is a motion effect, not a shader effect");
            continue;
        };
        println!("{id} ({})", def.name);
        let (params, colors) = effects::pack(def, |k| effects::default_value(def, k));
        for f in 0..(seconds * fps) as u32 {
            let t = f as f32 / fps;
            let mut l = Layer::full(Source::Pixels(Pixels::Rgba(img.clone())), canvas);
            l.time = t;
            l.effects.push(Fx {
                shader,
                passes,
                params,
                colors,
                time: t,
                audio: [0.0; 4],
            });
            let frame = Frame {
                canvas,
                background: [0.0, 0.0, 0.0, 1.0],
                items: vec![Item::Layer(l)],
            };
            let px = comp.render_to_cpu(&frame, &target);
            save(&format!("{out}/frame_{n:05}.png"), W, H, &px);
            n += 1;
        }
    }
    println!("wrote {n} frames of {W}x{H} to {out}");
}
