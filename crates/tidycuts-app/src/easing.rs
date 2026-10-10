//! The motion curve editor on a keyframe: pick a preset or drag the two
//! handles of a cubic bezier, like CSS `cubic-bezier()`. It sets how the
//! value moves from this keyframe to the next.

use crate::theme;
use egui::{Color32, CornerRadius, CursorIcon, Pos2, Sense, Shape, Stroke, StrokeKind, Ui};
use tidycuts_core::{Animatable, Easing, Time};

/// The plot shows progress from this much below 0 to this much above 1, so
/// an overshooting curve fits.
const Y_MIN: f64 = -0.5;
const Y_MAX: f64 = 1.5;

/// The menu shown by right-clicking a keyframe diamond. Edits the stretch
/// of animation playing at `local`. Returns whether anything changed.
pub fn menu(ui: &mut Ui, anim: &mut Animatable, local: Time) -> bool {
    let Some(i) = anim.segment_at(local) else {
        ui.label("Add a second keyframe to shape the motion.");
        return false;
    };
    let mut changed = false;
    ui.set_min_width(230.0);
    ui.label(
        egui::RichText::new(format!("Motion from keyframe {} to {}", i + 1, i + 2))
            .size(12.0)
            .color(theme::TEXT_DIM),
    );
    ui.add_space(4.0);
    let current = anim.keyframes[i].easing;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        for (preset, name) in Easing::PRESETS {
            if ui.selectable_label(current == preset, name).clicked() {
                anim.keyframes[i].easing = preset;
                changed = true;
            }
        }
    });
    ui.add_space(6.0);
    let mut handles = current.handles();
    if curve_plot(ui, &mut handles, current != Easing::Hold) {
        anim.keyframes[i].easing = Easing::bezier(handles);
        changed = true;
    }
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Drag the two dots to shape the curve.")
            .size(11.5)
            .color(theme::TEXT_FAINT),
    );
    if anim.keyframes.len() > 2 && ui.button("Use this curve for every keyframe").clicked() {
        let e = anim.keyframes[i].easing;
        for k in &mut anim.keyframes {
            k.easing = e;
        }
        changed = true;
    }
    changed
}

/// Draws the curve with its two handles; returns whether a handle moved.
fn curve_plot(ui: &mut Ui, h: &mut [f64; 4], editable: bool) -> bool {
    let side = 190.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click_and_drag());
    let to_screen = |x: f64, y: f64| -> Pos2 {
        egui::pos2(
            rect.left() + x as f32 * rect.width(),
            rect.bottom() - ((y - Y_MIN) / (Y_MAX - Y_MIN)) as f32 * rect.height(),
        )
    };
    let from_screen = |p: Pos2| -> (f64, f64) {
        (
            ((p.x - rect.left()) / rect.width()) as f64,
            Y_MIN + ((rect.bottom() - p.y) / rect.height()) as f64 * (Y_MAX - Y_MIN),
        )
    };
    let id = resp.id.with("grab");
    let mut changed = false;
    if editable {
        if resp.drag_started() {
            let origin = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .or(resp.interact_pointer_pos());
            if let Some(p) = origin {
                let d1 = to_screen(h[0], h[1]).distance(p);
                let d2 = to_screen(h[2], h[3]).distance(p);
                let pick = if d1 <= d2 { 0usize } else { 1 };
                ui.data_mut(|d| d.insert_temp(id, pick));
            }
        }
        if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
            let pick: usize = ui.data(|d| d.get_temp(id)).unwrap_or(0);
            let (x, y) = from_screen(p);
            let before = *h;
            let e = Easing::bezier(match pick {
                0 => [x, y, h[2], h[3]],
                _ => [h[0], h[1], x, y],
            });
            *h = e.handles();
            changed = *h != before;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
    }

    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(6),
        theme::FIELD_BG,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    // The 0 and 1 lines: where the value starts and ends.
    let guide = Stroke::new(1.0, theme::BORDER.gamma_multiply(1.4));
    for y in [0.0, 1.0] {
        painter.hline(rect.x_range(), to_screen(0.0, y).y, guide);
    }
    let easing = Easing::bezier(*h);
    let pts: Vec<Pos2> = (0..=80)
        .map(|i| {
            let t = i as f64 / 80.0;
            to_screen(t, easing.apply(t))
        })
        .collect();
    let on = if editable {
        theme::ACCENT
    } else {
        theme::TEXT_DIM
    };
    if editable {
        let (p0, p3) = (to_screen(0.0, 0.0), to_screen(1.0, 1.0));
        let (p1, p2) = (to_screen(h[0], h[1]), to_screen(h[2], h[3]));
        let arm = Stroke::new(1.0, theme::TEXT_FAINT);
        painter.line_segment([p0, p1], arm);
        painter.line_segment([p3, p2], arm);
        painter.add(Shape::line(pts, Stroke::new(2.0, on)));
        for p in [p1, p2] {
            painter.circle_filled(p, 6.0, Color32::WHITE);
            painter.circle_stroke(p, 6.0, Stroke::new(1.5, theme::ACCENT));
        }
    } else {
        // Hold jumps at the end: draw the step.
        let (a, b) = (to_screen(0.0, 0.0), to_screen(1.0, 1.0));
        painter.add(Shape::line(
            vec![a, egui::pos2(b.x, a.y), b],
            Stroke::new(2.0, on),
        ));
    }
    for p in [to_screen(0.0, 0.0), to_screen(1.0, 1.0)] {
        painter.circle_filled(p, 3.5, theme::KEYFRAME);
    }
    changed
}
