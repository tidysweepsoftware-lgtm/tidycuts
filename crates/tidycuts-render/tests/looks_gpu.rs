//! Draws a solid color through the grade pass with HSL, curves and LUT
//! looks on a real GPU, so mistakes in the second bind group or the shader
//! show up here rather than as a blank preview. Skipped without a GPU.

use tidycuts_core::{Animatable, ColorAdjust, ColorLut, Curve, LutSource, Time};
use tidycuts_render::{looks, Compositor, Frame, Grade, Item, Layer, Source};

fn render(comp: &mut Compositor, color: [f32; 4], c: &ColorAdjust) -> [u8; 3] {
    let target = comp.make_target(4, 4, wgpu::TextureFormat::Rgba8Unorm, true);
    let mut layer = Layer::full(Source::Solid(color), [4.0, 4.0]);
    layer.grade = Some(Grade {
        looks: looks::for_color(c, Time::ZERO),
        ..Default::default()
    });
    let frame = Frame {
        canvas: [4.0, 4.0],
        background: [0.0, 0.0, 0.0, 1.0],
        items: vec![Item::Layer(layer)],
    };
    let px = comp.render_to_cpu(&frame, &target);
    let i = (2 * 4 + 2) * 4;
    [px[i], px[i + 1], px[i + 2]]
}

fn close(got: [u8; 3], want: [u8; 3], tol: i32) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| (*g as i32 - w as i32).abs() <= tol)
}

#[test]
fn looks_change_the_picture_on_the_gpu() {
    let Ok(renderer) = tidycuts_render::gpu::headless() else {
        eprintln!("no GPU here; skipping");
        return;
    };
    let mut comp = Compositor::new(renderer);
    let base = [0.6, 0.4, 0.2, 1.0];

    // Untouched.
    let plain = render(&mut comp, base, &ColorAdjust::default());
    assert!(close(plain, [153, 102, 51], 3), "plain {plain:?}");

    // An inverting RGB curve.
    let mut c = ColorAdjust::default();
    c.curves.rgb = Curve {
        points: vec![[0.0, 1.0], [1.0, 0.0]],
    };
    let inverted = render(&mut comp, base, &c);
    assert!(close(inverted, [102, 153, 204], 4), "inverted {inverted:?}");

    // Taking all the saturation out of the reds leaves a gray.
    let mut c = ColorAdjust::default();
    c.hsl.red.saturation = Animatable::constant(-100.0);
    let gray = render(&mut comp, [0.8, 0.2, 0.2, 1.0], &c);
    assert!(
        gray.iter().all(|v| (*v as i32 - gray[0] as i32).abs() <= 4),
        "gray {gray:?}"
    );
    // Other colors are left alone by that same setting (green stays green).
    let green = render(&mut comp, [0.2, 0.7, 0.2, 1.0], &c);
    assert!(close(green, [51, 179, 51], 5), "green {green:?}");

    // A black-and-white LUT removes the color.
    let mut c = ColorAdjust {
        lut: Some(ColorLut::new(LutSource::BuiltIn("bw_contrast".into()))),
        ..Default::default()
    };
    let bw = render(&mut comp, base, &c);
    assert!(
        bw.iter().all(|v| (*v as i32 - bw[0] as i32).abs() <= 3),
        "bw {bw:?}"
    );
    // At half strength it's halfway between the original and the LUT.
    c.lut.as_mut().unwrap().strength = Animatable::constant(50.0);
    let half = render(&mut comp, base, &c);
    assert!(half[0] != plain[0] && half[0] != bw[0], "half {half:?}");
}
