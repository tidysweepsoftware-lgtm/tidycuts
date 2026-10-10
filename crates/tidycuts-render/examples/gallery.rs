//! Renders contact sheets of every effect and transition to PNG files, for
//! eyeballing shader changes: `cargo run -p tidycuts-render --example gallery -- out_dir`.

use std::sync::Arc;
use tidycuts_render::*;

const W: u32 = 320;
const H: u32 = 180;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn sheet(cells: &[Vec<u8>], cols: u32) -> (u32, u32, Vec<u8>) {
    let rows = (cells.len() as u32).div_ceil(cols);
    let (sw, sh) = (cols * (W + 4), rows * (H + 4));
    let mut out = vec![30u8; (sw * sh * 4) as usize];
    for (i, c) in cells.iter().enumerate() {
        let (cx, cy) = (i as u32 % cols, i as u32 / cols);
        for y in 0..H {
            let src = &c[(y * W * 4) as usize..((y + 1) * W * 4) as usize];
            let dy = cy * (H + 4) + y;
            let start = ((dy * sw + cx * (W + 4)) * 4) as usize;
            out[start..start + (W * 4) as usize].copy_from_slice(src);
        }
    }
    (sw, sh, out)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let r = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(r);
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let canvas = [1920.0f32, 1080.0];
    let a = Arc::new(sample::landscape(640, 360));
    let b = Arc::new(sample::sunset(640, 360));
    let layer =
        |img: &Arc<RgbaImage>| Layer::full(Source::Pixels(Pixels::Rgba(img.clone())), canvas);

    // Composite test: background, base layer, a rotated picture-in-picture
    // with rounded corners and a screen-blended gradient.
    let mut pip = layer(&b);
    pip.scale = [0.4, 0.4];
    pip.center = [1400.0, 330.0];
    pip.rotation = -8.0;
    pip.corner_radius = 40.0;
    let mut grad = Layer::full(
        Source::Gradient([1.0, 0.2, 0.4, 1.0], [0.1, 0.3, 1.0, 1.0], 30.0),
        canvas,
    );
    grad.blend = 6;
    grad.opacity = 0.5;
    grad.scale = [0.5, 1.0];
    grad.center = [480.0, 540.0];
    let frame = Frame {
        canvas,
        background: [0.1, 0.1, 0.1, 1.0],
        items: vec![Item::Layer(layer(&a)), Item::Layer(pip), Item::Layer(grad)],
    };
    let px = comp.render_to_cpu(&frame, &target);
    save(&format!("{out}/composite.png"), W, H, &px);

    // Effects.
    let mut cells = Vec::new();
    for id in 0..=22u32 {
        let mut l = layer(&a);
        l.effects.push(Fx {
            shader: id,
            passes: if id == 1 { 2 } else { 1 },
            params: [[0.6, 0.5, 0.5, 0.5], [0.5; 4], [0.5; 4], [0.5; 4]],
            colors: [[0.1, 0.8, 0.2, 1.0], [1.0, 0.6, 0.2, 1.0]],
            time: 1.3,
        });
        let f = Frame {
            canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Layer(l)],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let (sw, sh, px) = sheet(&cells, 6);
    save(&format!("{out}/effects.png"), sw, sh, &px);

    // Transitions at 35%.
    let mut cells = Vec::new();
    for id in 1..=17u32 {
        let f = Frame {
            canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Transition(TransitionItem {
                from: Some(layer(&a)),
                to: Some(layer(&b)),
                kind: id,
                progress: 0.35,
                params: [[0.0; 4]; 4],
                color: [0.0, 0.0, 0.0, 1.0],
                time: 0.0,
            })],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let (sw, sh, px) = sheet(&cells, 6);
    save(&format!("{out}/transitions.png"), sw, sh, &px);

    // Grading and enhancement.
    let mut cells = Vec::new();
    let grades = [
        Grade::default(),
        Grade {
            contrast: 0.4,
            saturation: 0.3,
            shadow_tint: [0.0, 0.5, 0.6, 0.35],
            highlight_tint: [1.0, 0.6, 0.2, 0.3],
            ..Default::default()
        },
        Grade {
            mono: true,
            contrast: 0.5,
            vignette: 0.6,
            ..Default::default()
        },
        Grade {
            temperature: 0.6,
            fade: 0.5,
            saturation: -0.3,
            grain: 0.4,
            ..Default::default()
        },
        Grade {
            exposure: -0.5,
            temperature: -0.5,
            tint: 0.2,
            ..Default::default()
        },
        Grade {
            hue: 120.0,
            ..Default::default()
        },
    ];
    for g in grades {
        let mut l = layer(&a);
        l.grade = Some(g);
        let f = Frame {
            canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Layer(l)],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let small = Arc::new(sample::landscape(96, 54));
    for mode in 0..3u32 {
        let mut l = layer(&small);
        l.enhance = EnhanceParams {
            upscale: mode,
            denoise: if mode == 2 { 0.3 } else { 0.0 },
            ..Default::default()
        };
        let f = Frame {
            canvas,
            background: [0.0, 0.0, 0.0, 1.0],
            items: vec![Item::Layer(l)],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let (sw, sh, px) = sheet(&cells, 3);
    save(&format!("{out}/grades.png"), sw, sh, &px);
    // Masks, drop shadow and track mattes.
    let mut cells = Vec::new();
    for shape in 0..6u32 {
        let mut l = layer(&a);
        l.mask = Some(MaskParams {
            shape,
            center: [0.5, 0.5],
            size: [0.5, 0.6],
            rotation: if shape < 2 { 20.0 } else { 0.0 },
            roundness: 0.3,
            feather: 0.02,
            invert: shape == 5,
        });
        let f = Frame {
            canvas,
            background: [0.15, 0.15, 0.2, 1.0],
            items: vec![Item::Layer(l)],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let mut pip = layer(&b);
    pip.scale = [0.5, 0.5];
    pip.corner_radius = 30.0;
    pip.shadow = Some(ShadowParams {
        color: [0.0, 0.0, 0.0, 0.8],
        offset: [30.0, 30.0],
        blur: 40.0,
    });
    let f = Frame {
        canvas,
        background: [0.0; 4],
        items: vec![
            Item::Layer(Layer::full(Source::Solid([0.9, 0.9, 0.85, 1.0]), canvas)),
            Item::Layer(pip),
        ],
    };
    cells.push(comp.render_to_cpu(&f, &target));
    for mode in [1u32, 3] {
        let mut matte = Layer::full(
            Source::Gradient([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0], 0.0),
            canvas,
        );
        matte.scale = [0.6, 0.6];
        matte.rotation = 15.0;
        let f = Frame {
            canvas,
            background: [0.1, 0.1, 0.1, 1.0],
            items: vec![Item::Matte(MatteItem {
                layer: layer(&a),
                matte,
                mode,
            })],
        };
        cells.push(comp.render_to_cpu(&f, &target));
    }
    let (sw, sh, px) = sheet(&cells, 3);
    save(&format!("{out}/features.png"), sw, sh, &px);
    println!("wrote {out}/composite.png effects.png transitions.png grades.png features.png");
}
