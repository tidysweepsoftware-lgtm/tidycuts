//! Renders transitions in motion, one numbered PNG per frame, to turn into
//! a video:
//!
//! `cargo run --release -p tidycuts-render --example transition_preview -- out_dir id-prefix [fps]`
//!
//! Each matching transition plays once between two sample pictures, with a
//! short hold of each picture before and after; the pictures swap places
//! for every second transition. Then, for example:
//! `ffmpeg -framerate 30 -i out_dir/frame_%05d.png -c:v libx264 -pix_fmt yuv420p -crf 18 preview.mp4`

use std::sync::Arc;
use tidycuts_render::catalog::transitions::TRANSITIONS;
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
    let prefix = args.get(2).cloned().unwrap_or_default();
    let fps: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(30.0);
    std::fs::create_dir_all(&out).unwrap();

    let defs: Vec<_> = TRANSITIONS
        .iter()
        .filter(|d| d.id.starts_with(prefix.as_str()))
        .collect();
    if defs.is_empty() {
        eprintln!("no transition id starts with '{prefix}'");
        std::process::exit(1);
    }

    let r = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(r);
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let a = Arc::new(sample::landscape(1280, 720));
    let b = Arc::new(sample::sunset(1280, 720));
    let canvas = [1920.0f32, 1080.0];
    let layer =
        |img: &Arc<RgbaImage>| Layer::full(Source::Pixels(Pixels::Rgba(img.clone())), canvas);

    let hold = (0.3 * fps) as u32;
    let mut n = 0u32;
    for (k, def) in defs.iter().enumerate() {
        println!("{k}: {} ({})", def.id, def.name);
        let (from, to) = if k % 2 == 0 {
            (layer(&a), layer(&b))
        } else {
            (layer(&b), layer(&a))
        };
        let steps = (def.duration * fps).round() as u32;
        let total = hold + steps + 1 + hold;
        for f in 0..total {
            // Hold the first picture, run the transition, hold the second.
            let p = if f < hold {
                0.0
            } else {
                ((f - hold) as f32 / steps as f32).clamp(0.0, 1.0)
            };
            let frame = Frame {
                canvas,
                background: [0.0, 0.0, 0.0, 1.0],
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
            let px = comp.render_to_cpu(&frame, &target);
            save(&format!("{out}/frame_{n:05}.png"), W, H, &px);
            n += 1;
        }
    }
    println!("wrote {n} frames of {W}x{H} to {out}");
}
