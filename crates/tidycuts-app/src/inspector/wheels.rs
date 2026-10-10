//! Color wheels: Lift, Gamma, Gain and Offset, each a
//! disc to drag a puck on (towards a colour to push the picture that way) and
//! a master slider beside it for brightness. Double-click either to put it
//! back. What they do to the picture is in `tidycuts_render::looks`.

use super::kit::Sec;
use super::Panel;
use crate::theme;
use egui::{epaint::Mesh, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{Clip, Wheel, Wheels};
use tidycuts_render::looks::{wheel_rgb, WHEEL_STRENGTH};

const NAMES: [(&str, &str); 4] = [
    ("Lift", "Shadows"),
    ("Gamma", "Mid tones"),
    ("Gain", "Highlights"),
    ("Offset", "Everything"),
];

/// Space between wheels, and the room the title and the numbers take.
const GAP: f32 = 12.0;
const TITLE_H: f32 = 18.0;
const NUMBERS_H: f32 = 22.0;
/// The master slider's width and its distance from the disc.
const MASTER_W: f32 = 14.0;
const MASTER_GAP: f32 = 8.0;

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let dirty = !c.color.wheels.is_neutral();
    let out = Sec::new("color-wheels", "Color Wheels")
        .reset(dirty)
        .button(
            ph::WAVEFORM,
            "Scopes: graphs of the colours in the picture",
            true,
        )
        .show(ui, |ui| {
            let wheels = &mut c.color.wheels;
            let avail = ui.available_width();
            // Four in a row when there is room (the left-menus layout), else two.
            let cols = if avail >= 520.0 { 4 } else { 2 };
            let cell_w = ((avail - GAP * (cols as f32 - 1.0)) / cols as f32).floor();
            let disc = (cell_w - MASTER_W - MASTER_GAP).clamp(70.0, 160.0);
            let cell_h = TITLE_H + disc + NUMBERS_H;
            let rows = 4 / cols;
            let (area, _) = ui.allocate_exact_size(
                egui::vec2(avail, rows as f32 * cell_h + (rows as f32 - 1.0) * GAP),
                Sense::hover(),
            );
            let list: [&mut Wheel; 4] = [
                &mut wheels.lift,
                &mut wheels.gamma,
                &mut wheels.gain,
                &mut wheels.offset,
            ];
            for (i, w) in list.into_iter().enumerate() {
                let (col, row) = ((i % cols) as f32, (i / cols) as f32);
                let cell = Rect::from_min_size(
                    area.min + egui::vec2(col * (cell_w + GAP), row * (cell_h + GAP)),
                    egui::vec2(cell_w, cell_h),
                );
                wheel(ui, cell, disc, i, w);
            }
        });
    if out.reset {
        c.color.wheels = Wheels::default();
    }
    if out.button == Some(0) {
        p.actions.push(super::Action::OpenScopes);
    }
}

/// The colour a direction on the disc stands for (full strength).
fn hue(angle: f32, value: f32) -> Color32 {
    let h = angle.to_degrees().rem_euclid(360.0) / 60.0;
    let f = |n: f32| (((h + n).rem_euclid(6.0) - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let to = |v: f32| ((v * value) * 255.0).round() as u8;
    Color32::from_rgb(to(f(0.0)), to(f(4.0)), to(f(2.0)))
}

/// A disc of colours: gray in the middle, fully coloured at the rim.
fn disc_mesh(center: Pos2, radius: f32) -> Mesh {
    const STEPS: usize = 90;
    let mut mesh = Mesh::default();
    let mid = Color32::from_rgb(0x3c, 0x3d, 0x42);
    mesh.colored_vertex(center, mid);
    for i in 0..=STEPS {
        let a = i as f32 / STEPS as f32 * std::f32::consts::TAU;
        let p = center + egui::vec2(a.cos(), -a.sin()) * radius;
        mesh.colored_vertex(p, hue(a, 0.8));
    }
    for i in 0..STEPS as u32 {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    mesh
}

fn wheel(ui: &mut Ui, cell: Rect, diameter: f32, index: usize, w: &mut Wheel) {
    let (name, hint) = NAMES[index];
    let painter = ui.painter().clone();
    let disc = Rect::from_min_size(
        cell.min + egui::vec2(0.0, TITLE_H),
        egui::vec2(diameter, diameter),
    );
    let (center, radius) = (disc.center(), diameter / 2.0 - 1.0);

    // The name, and what it works on.
    painter.text(
        cell.left_top() + egui::vec2(2.0, 0.0),
        Align2::LEFT_TOP,
        name,
        theme::bold_font(12.0),
        theme::TEXT,
    );
    let name_w = painter
        .layout_no_wrap(name.to_string(), theme::bold_font(12.0), theme::TEXT)
        .size()
        .x;
    painter.text(
        cell.left_top() + egui::vec2(name_w + 8.0, 1.0),
        Align2::LEFT_TOP,
        hint,
        FontId::proportional(11.0),
        theme::TEXT_FAINT,
    );

    // The disc, a ring halfway out and cross hairs.
    painter.add(Shape::mesh(disc_mesh(center, radius)));
    let faint = Stroke::new(1.0, Color32::from_white_alpha(28));
    painter.circle_stroke(center, radius * 0.5, faint);
    painter.hline(center.x - radius..=center.x + radius, center.y, faint);
    painter.vline(center.x, center.y - radius..=center.y + radius, faint);
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, Color32::from_black_alpha(120)),
    );

    // Dragging the puck.
    let id = ui.id().with(("wheel", index));
    let resp = ui.interact(disc, id, Sense::click_and_drag());
    if resp.double_clicked() {
        w.x = 0.0;
        w.y = 0.0;
    } else if resp.dragged() {
        if let Some(p) = resp.interact_pointer_pos() {
            let v = (p - center) / radius;
            let (mut x, mut y) = (v.x as f64, -v.y as f64);
            let len = (x * x + y * y).sqrt();
            if len > 1.0 {
                x /= len;
                y /= len;
            }
            // Close to the middle sticks to it.
            if len < 0.04 {
                x = 0.0;
                y = 0.0;
            }
            w.x = x;
            w.y = y;
        }
    }
    let resp = resp.on_hover_text("Drag towards a colour. Double-click to reset.");
    let puck = center + egui::vec2(w.x as f32, -w.y as f32) * radius;
    let tint = if w.x == 0.0 && w.y == 0.0 {
        Color32::WHITE
    } else {
        hue((w.y as f32).atan2(w.x as f32), 1.0)
    };
    painter.circle_filled(puck, 6.5, Color32::from_black_alpha(160));
    painter.circle_filled(puck, 5.0, tint);
    painter.circle_stroke(
        puck,
        5.0,
        Stroke::new(
            1.5,
            if resp.dragged() {
                theme::ACCENT
            } else {
                Color32::WHITE
            },
        ),
    );

    // The master slider: up brightens, down darkens.
    let slider = Rect::from_min_size(
        egui::pos2(disc.right() + MASTER_GAP, disc.top()),
        egui::vec2(MASTER_W, diameter),
    );
    let track = Rect::from_center_size(slider.center(), egui::vec2(6.0, diameter - 12.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(track.left_top(), Color32::from_gray(235));
    mesh.colored_vertex(track.right_top(), Color32::from_gray(235));
    mesh.colored_vertex(track.right_bottom(), Color32::from_gray(10));
    mesh.colored_vertex(track.left_bottom(), Color32::from_gray(10));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        track,
        1.0,
        Stroke::new(1.0, Color32::from_black_alpha(120)),
        egui::StrokeKind::Outside,
    );
    // A tick at zero.
    painter.hline(
        slider.left() + 1.0..=slider.right() - 1.0,
        slider.center().y,
        Stroke::new(1.0, Color32::from_black_alpha(160)),
    );
    let half = track.height() / 2.0;
    let sresp = ui.interact(slider, id.with("master"), Sense::click_and_drag());
    if sresp.double_clicked() {
        w.master = 0.0;
    } else if sresp.dragged() {
        if let Some(p) = sresp.interact_pointer_pos() {
            let v = ((slider.center().y - p.y) / half).clamp(-1.0, 1.0) as f64;
            w.master = if v.abs() < 0.03 { 0.0 } else { v };
        }
    }
    let sresp = sresp.on_hover_text(format!(
        "Brightness {:+.2}. Double-click to reset.",
        w.master
    ));
    let handle = egui::pos2(
        slider.center().x,
        slider.center().y - w.master as f32 * half,
    );
    painter.circle_filled(handle, 6.5, Color32::from_black_alpha(160));
    painter.circle_filled(handle, 5.5, Color32::WHITE);
    if sresp.dragged() {
        painter.circle_stroke(handle, 5.5, Stroke::new(1.5, theme::ACCENT));
    }

    // What the wheel does to red, green and blue, in percent.
    let (push, master_push) = WHEEL_STRENGTH[index];
    let rgb = wheel_rgb(w, push, master_push);
    let colors = [
        Color32::from_rgb(0xff, 0x6b, 0x6b),
        Color32::from_rgb(0x4d, 0xdc, 0x8a),
        Color32::from_rgb(0x6b, 0x9c, 0xff),
    ];
    let y = disc.bottom() + 5.0;
    let step = (disc.width() + MASTER_W + MASTER_GAP) / 3.0;
    for k in 0..3 {
        painter.text(
            egui::pos2(cell.left() + 2.0 + step * k as f32, y),
            Align2::LEFT_TOP,
            format!("{:+.0}", rgb[k] * 100.0),
            FontId::monospace(11.0),
            if rgb[k].abs() < 0.005 {
                theme::TEXT_FAINT
            } else {
                colors[k]
            },
        );
    }
}
