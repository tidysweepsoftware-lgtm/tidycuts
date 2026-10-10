//! Color tab: white balance and color, light (tone) and vignette, every
//! value keyframable, with a reset per section.

use super::kit::{self, Sec};
use super::Panel;
use crate::widgets;
use egui::{Color32, Mesh, Rect, Shape, Ui};
use tidycuts_core::{Animatable, Clip, Time};

/// How a slider's track is tinted to show what it does.
#[derive(Clone, Copy)]
enum Tint {
    None,
    Ramp(Color32, Color32),
    Rainbow,
}

struct Field {
    label: &'static str,
    get: fn(&mut tidycuts_core::ColorAdjust) -> &mut Animatable,
    tint: Tint,
}

const fn field(
    label: &'static str,
    get: fn(&mut tidycuts_core::ColorAdjust) -> &mut Animatable,
    tint: Tint,
) -> Field {
    Field { label, get, tint }
}

const GRAY: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);

const COLOR: [Field; 5] = [
    field(
        "Temperature",
        |c| &mut c.temperature,
        Tint::Ramp(
            Color32::from_rgb(0x3d, 0x8b, 0xff),
            Color32::from_rgb(0xff, 0xb3, 0x3d),
        ),
    ),
    field(
        "Tint",
        |c| &mut c.tint,
        Tint::Ramp(
            Color32::from_rgb(0x3c, 0xc4, 0x5a),
            Color32::from_rgb(0xe0, 0x4d, 0xd0),
        ),
    ),
    field(
        "Saturation",
        |c| &mut c.saturation,
        Tint::Ramp(GRAY, Color32::from_rgb(0xff, 0x40, 0x60)),
    ),
    field(
        "Vibrance",
        |c| &mut c.vibrance,
        Tint::Ramp(GRAY, Color32::from_rgb(0x40, 0xd0, 0xff)),
    ),
    field("Hue", |c| &mut c.hue, Tint::Rainbow),
];

const LIGHT: [Field; 7] = [
    field(
        "Exposure",
        |c| &mut c.exposure,
        Tint::Ramp(Color32::BLACK, Color32::WHITE),
    ),
    field(
        "Brightness",
        |c| &mut c.brightness,
        Tint::Ramp(Color32::BLACK, Color32::WHITE),
    ),
    field("Contrast", |c| &mut c.contrast, Tint::None),
    field("Highlights", |c| &mut c.highlights, Tint::None),
    field("Shadows", |c| &mut c.shadows, Tint::None),
    field("Whites", |c| &mut c.whites, Tint::None),
    field("Blacks", |c| &mut c.blacks, Tint::None),
];

const VIGNETTE: [Field; 1] = [field("Amount", |c| &mut c.vignette, Tint::None)];

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    group(ui, "color-color", "Color", &COLOR, c, local);
    group(ui, "color-light", "Light", &LIGHT, c, local);
    group(ui, "color-vignette", "Vignette", &VIGNETTE, c, local);
    super::wheels::show(p, ui, c);
    super::looks::show(p, ui, c);
    kit::hint(
        ui,
        "Values run from -100 to 100 (hue in degrees); 0 leaves the picture unchanged. \
         Use ◇ to animate a value from the playhead.",
    );
}

fn group(ui: &mut Ui, id: &str, title: &str, fields: &[Field], c: &mut Clip, local: Time) {
    let color = &mut c.color;
    let dirty = fields
        .iter()
        .any(|f| *(f.get)(color) != Animatable::constant(0.0));
    let out = Sec::new(id, title).reset(dirty).show(ui, |ui| {
        for f in fields {
            let a = (f.get)(color);
            let (range, decimals, suffix) = if f.label == "Hue" {
                (-180.0..=180.0, 0, "°")
            } else {
                (-100.0..=100.0, 0, "")
            };
            tint_strip(ui, f.tint);
            kit::anim_row(ui, f.label, a, local, range, decimals, suffix, 0.0);
        }
    });
    if out.reset {
        for f in fields {
            *(f.get)(color) = Animatable::constant(0.0);
        }
    }
}

/// A thin colored strip under the track of the slider row about to be
/// added (cool to warm, gray to saturated, a rainbow for hue). Painted
/// first so the slider's handle covers it; matches the slider geometry of
/// `widgets::anim_row`.
fn tint_strip(ui: &mut Ui, tint: Tint) {
    let (a, b) = match tint {
        Tint::None => return,
        Tint::Ramp(a, b) => (a, b),
        Tint::Rainbow => (Color32::RED, Color32::RED),
    };
    let gap = ui.spacing().item_spacing.x;
    let left = ui.cursor().min.x + widgets::LABEL_W + gap;
    let avail = ui.max_rect().right() - left;
    let w = kit::slider_width(avail);
    let y = ui.cursor().min.y + ui.spacing().interact_size.y / 2.0 + 4.0;
    let rect = Rect::from_min_max(
        egui::pos2(left + 2.0, y),
        egui::pos2(left + w - 2.0, y + 2.5),
    );
    let mut mesh = Mesh::default();
    let steps = if matches!(tint, Tint::Rainbow) { 12 } else { 1 };
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let col = match tint {
            Tint::Rainbow => hue_color(t * 360.0 - 180.0),
            _ => a.lerp_to_gamma(b, t),
        }
        .gamma_multiply(0.85);
        let x = egui::lerp(rect.left()..=rect.right(), t);
        mesh.colored_vertex(egui::pos2(x, rect.top()), col);
        mesh.colored_vertex(egui::pos2(x, rect.bottom()), col);
        if s > 0 {
            let i = (s * 2) as u32;
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i, i + 1);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
}

/// A fully saturated color for a hue offset in degrees (0 = red).
fn hue_color(deg: f32) -> Color32 {
    let h = deg.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
