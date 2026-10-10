//! The graphics card's video-colour output (NV12) must match what a CPU
//! conversion of the ordinary RGBA picture gives. Skipped without a GPU.

use tidycuts_render::{Compositor, Frame, Item, Layer, Source};

const W: u32 = 64;
const H: u32 = 32;

fn frame(source: Source) -> Frame {
    Frame {
        canvas: [W as f32, H as f32],
        background: [0.0, 0.0, 0.0, 1.0],
        items: vec![Item::Layer(Layer::full(source, [W as f32, H as f32]))],
    }
}

/// Limited-range BT.709 from gamma-encoded RGB (0..255).
fn ycbcr(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = [rgb[0] / 255.0, rgb[1] / 255.0, rgb[2] / 255.0];
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    [
        16.0 + 219.0 * y,
        128.0 + 224.0 * (b - y) / 1.8556,
        128.0 + 224.0 * (r - y) / 1.5748,
    ]
}

fn check(comp: &mut Compositor, source: Source, name: &str) {
    let rgba_target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let rgba = comp.render_to_cpu(&frame(source.clone()), &rgba_target);
    let nv12_target = comp.make_nv12_target(W, H);
    let index = comp.submit_nv12(&frame(source), &nv12_target);
    let nv12 = comp.read_nv12(&nv12_target, index);
    assert_eq!(nv12.len(), (W * H * 3 / 2) as usize, "{name}: size");

    let px = |x: i32, y: i32| {
        let x = x.clamp(0, W as i32 - 1) as usize;
        let y = y.clamp(0, H as i32 - 1) as usize;
        let i = (y * W as usize + x) * 4;
        ycbcr([rgba[i] as f32, rgba[i + 1] as f32, rgba[i + 2] as f32])
    };
    let mut worst = 0.0f32;
    // Brightness: one value per pixel.
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let want = px(x, y)[0];
            let got = nv12[(y as u32 * W + x as u32) as usize] as f32;
            worst = worst.max((want - got).abs());
        }
    }
    assert!(worst <= 1.6, "{name}: brightness off by {worst}");
    // Colour: a 1-2-1 blend across, the two rows averaged, from the left pixel.
    let base = (W * H) as usize;
    let mut worst = 0.0f32;
    for by in 0..(H / 2) as i32 {
        for bx in 0..(W / 2) as i32 {
            let mut want = [0.0f32; 2];
            for (dx, w) in [(-1, 0.25), (0, 0.5), (1, 0.25)] {
                for dy in 0..2 {
                    let c = px(bx * 2 + dx, by * 2 + dy);
                    want[0] += 0.5 * w * c[1];
                    want[1] += 0.5 * w * c[2];
                }
            }
            let i = base + (by as u32 * W + bx as u32 * 2) as usize;
            worst = worst
                .max((want[0] - nv12[i] as f32).abs())
                .max((want[1] - nv12[i + 1] as f32).abs());
        }
    }
    assert!(worst <= 2.5, "{name}: colour off by {worst}");
}

#[test]
fn nv12_matches_the_rgb_picture() {
    let Ok(renderer) = tidycuts_render::gpu::headless() else {
        eprintln!("no GPU here; skipping");
        return;
    };
    let mut comp = Compositor::new(renderer);
    for (name, color) in [
        ("black", [0.0, 0.0, 0.0, 1.0]),
        ("white", [1.0, 1.0, 1.0, 1.0]),
        ("red", [1.0, 0.0, 0.0, 1.0]),
        ("green", [0.0, 1.0, 0.0, 1.0]),
        ("blue", [0.0, 0.0, 1.0, 1.0]),
        ("skin", [0.8, 0.55, 0.45, 1.0]),
    ] {
        check(&mut comp, Source::Solid(color), name);
    }
    // Colour that changes across the picture shows where the colour sits.
    check(
        &mut comp,
        Source::Gradient([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0], 90.0),
        "red to blue",
    );
    check(
        &mut comp,
        Source::Gradient([0.0, 1.0, 0.0, 1.0], [1.0, 0.0, 1.0, 1.0], 0.0),
        "green to magenta",
    );
}
