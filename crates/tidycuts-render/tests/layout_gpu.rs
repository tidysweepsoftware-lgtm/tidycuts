//! Draws a split-screen cell and an audio visualizer on a real GPU, so a
//! mistake in their shaders or bind groups shows up here instead of as a
//! blank preview. Skipped without a GPU.

use tidycuts_render::{CellParams, Compositor, Frame, Item, Layer, Source, VisualizerParams};

const W: u32 = 64;
const H: u32 = 36;

fn render(comp: &mut Compositor, layer: Layer) -> Vec<u8> {
    let target = comp.make_target(W, H, wgpu::TextureFormat::Rgba8Unorm, true);
    let frame = Frame {
        canvas: [W as f32, H as f32],
        background: [0.0, 0.0, 0.0, 1.0],
        items: vec![Item::Layer(layer)],
    };
    comp.render_to_cpu(&frame, &target)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2]]
}

#[test]
fn a_cell_shows_its_picture_only_inside_with_a_border_around_it() {
    let Ok(renderer) = tidycuts_render::gpu::headless() else {
        eprintln!("no GPU here; skipping");
        return;
    };
    let mut comp = Compositor::new(renderer);
    let mut layer = Layer::full(Source::Solid([1.0, 0.0, 0.0, 1.0]), [W as f32, H as f32]);
    // The left half, with a 4-pixel white border all round.
    layer.cell = Some(CellParams {
        outer: [[0.0, 0.0], [32.0, 0.0], [32.0, 36.0], [0.0, 36.0]],
        inner: [[4.0, 4.0], [28.0, 4.0], [28.0, 32.0], [4.0, 32.0]],
        radius: 0.0,
        border: [1.0, 1.0, 1.0, 1.0],
    });
    let img = render(&mut comp, layer);
    let inside = px(&img, 16, 18);
    assert!(
        inside[0] > 230 && inside[1] < 30 && inside[2] < 30,
        "inside {inside:?}"
    );
    let border = px(&img, 2, 18);
    assert!(border.iter().all(|v| *v > 225), "border {border:?}");
    let outside = px(&img, 48, 18);
    assert!(outside.iter().all(|v| *v < 10), "outside {outside:?}");
}

#[test]
fn a_visualizer_draws_bars_that_follow_the_levels() {
    let Ok(renderer) = tidycuts_render::gpu::headless() else {
        eprintln!("no GPU here; skipping");
        return;
    };
    let mut comp = Compositor::new(renderer);
    let draw = |comp: &mut Compositor, level: f32| {
        let layer = Layer::full(
            Source::Visualizer(VisualizerParams {
                style: 0,
                levels: vec![level; 8],
                colors: [[0.0, 1.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]],
                mirror: false,
                thickness: 0.8,
            }),
            [W as f32, H as f32],
        );
        let img = render(comp, layer);
        (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            // Only the solid bars: the glow and the reflection are dimmer.
            .filter(|(x, y)| px(&img, *x, *y)[1] > 200)
            .count()
    };
    let quiet = draw(&mut comp, 0.1);
    let loud = draw(&mut comp, 0.9);
    assert!(loud > quiet * 3, "quiet {quiet} loud {loud}");
    assert!(quiet > 0, "even quiet sound shows something");
}

/// Black and white stripes, so a blur or a mosaic has something to smear.
fn stripes() -> Layer {
    let mut pixels = Vec::new();
    for _y in 0..H {
        for x in 0..W {
            let v = if (x / 2) % 2 == 0 { 255 } else { 0 };
            pixels.extend_from_slice(&[v, v, v, 255]);
        }
    }
    Layer::full(
        Source::Pixels(tidycuts_render::Pixels::Rgba(std::sync::Arc::new(
            tidycuts_render::RgbaImage {
                width: W,
                height: H,
                pixels,
                premultiplied: true,
            },
        ))),
        [W as f32, H as f32],
    )
}

/// Blur Area / Pixelate Area: the picture changes inside the chosen area only.
#[test]
fn blur_and_pixelate_area_touch_only_their_area() {
    let Ok(renderer) = tidycuts_render::gpu::headless() else {
        eprintln!("no GPU here; skipping");
        return;
    };
    let mut comp = Compositor::new(renderer);
    let plain = render(&mut comp, stripes());
    for (id, shader) in [("blur.area", 72), ("blur.pixelate_area", 73)] {
        let def = tidycuts_render::catalog::effects::get(id).expect(id);
        let value = |k: &str| match k {
            // The middle third of the picture, with a hard edge.
            "center_x" | "center_y" => 0.5,
            "width" | "height" => 0.34,
            "feather" => 0.0,
            "blur" | "size" => 0.9,
            k => tidycuts_render::catalog::effects::default_value(def, k),
        };
        let (params, colors) = tidycuts_render::catalog::effects::pack(def, value);
        let mut layer = stripes();
        layer.effects.push(tidycuts_render::Fx {
            shader,
            passes: 2,
            params,
            colors,
            time: 0.0,
        });
        let img = render(&mut comp, layer);
        let mut same_outside = true;
        let mut changed_inside = 0;
        for y in 0..H {
            for x in 0..W {
                let (a, b) = (px(&plain, x, y), px(&img, x, y));
                let (dx, dy) = (
                    (x as f32 / W as f32 - 0.5).abs(),
                    (y as f32 / H as f32 - 0.5).abs(),
                );
                // Well outside the area (an ellipse 0.17 by 0.17 of the picture).
                if (dx / 0.17).powi(2) + (dy / 0.17).powi(2) > 1.7 {
                    same_outside &= a == b;
                } else if (dx / 0.17).powi(2) + (dy / 0.17).powi(2) < 0.5
                    && a.iter().zip(b).any(|(p, q)| p.abs_diff(q) > 40)
                {
                    changed_inside += 1;
                }
            }
        }
        assert!(same_outside, "{id} changed pixels outside its area");
        assert!(
            changed_inside > 20,
            "{id} left its area alone ({changed_inside})"
        );
    }
}
