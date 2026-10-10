//! Color tab, second half: HSL (eight color ranges), Curves (a draggable
//! tone-curve editor) and LUT (built-in looks or a .cube file).

use super::kit::{self, Sec};
use super::{Action, Panel};
use crate::theme;
use egui::{Color32, CornerRadius, CursorIcon, Pos2, Sense, Shape, Stroke, StrokeKind, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{Clip, ColorLut, Curve, LutSource, HSL_RANGES};
use tidycuts_render::lut;

const RANGE_COLORS: [Color32; 8] = [
    Color32::from_rgb(0xf0, 0x40, 0x40),
    Color32::from_rgb(0xf5, 0x8a, 0x2c),
    Color32::from_rgb(0xf2, 0xd4, 0x3a),
    Color32::from_rgb(0x40, 0xc8, 0x55),
    Color32::from_rgb(0x30, 0xc8, 0xd0),
    Color32::from_rgb(0x3d, 0x7b, 0xf0),
    Color32::from_rgb(0x90, 0x58, 0xe8),
    Color32::from_rgb(0xe0, 0x4a, 0xc8),
];

const CURVE_NAMES: [&str; 4] = ["RGB", "Red", "Green", "Blue"];
const CURVE_COLORS: [Color32; 4] = [
    Color32::WHITE,
    Color32::from_rgb(0xff, 0x5d, 0x5d),
    Color32::from_rgb(0x5d, 0xdb, 0x6a),
    Color32::from_rgb(0x5d, 0x9a, 0xff),
];

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    hsl(p, ui, c);
    curves(p, ui, c);
    lut_section(p, ui, c);
}

// ----- HSL ------------------------------------------------------------------

fn hsl(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    let dirty = !c.color.hsl.is_neutral();
    let out = Sec::new("color-hsl", "HSL").reset(dirty).closed().show(ui, |ui| {
        // The range being edited, with a dot on the ones already changed.
        let band = p.st.hsl_band.min(7);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (i, (name, _)) in HSL_RANGES.iter().enumerate() {
                let changed = !c.color.hsl.bands()[i].is_neutral();
                if swatch(ui, RANGE_COLORS[i], band == i, changed, name).clicked() {
                    p.st.hsl_band = i;
                }
            }
        });
        ui.add_space(6.0);
        let band = p.st.hsl_band.min(7);
        let b = c.color.hsl.bands_mut().into_iter().nth(band).unwrap();
        kit::anim_row(ui, "Hue", &mut b.hue, local, -100.0..=100.0, 0, "", 0.0);
        kit::anim_row(
            ui,
            "Saturation",
            &mut b.saturation,
            local,
            -100.0..=100.0,
            0,
            "",
            0.0,
        );
        kit::anim_row(
            ui,
            "Luminance",
            &mut b.luminance,
            local,
            -100.0..=100.0,
            0,
            "",
            0.0,
        );
        kit::hint(
            ui,
            "Pick a color range, then change only that color. Hue slides it towards its neighbours.",
        );
    });
    if out.reset {
        c.color.hsl = Default::default();
    }
}

/// A round color swatch; ringed when selected, with a dot when changed.
fn swatch(ui: &mut Ui, color: Color32, selected: bool, changed: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::click());
    let p = ui.painter();
    p.circle_filled(rect.center(), 9.0, color);
    if selected {
        p.circle_stroke(rect.center(), 11.5, Stroke::new(2.0, Color32::WHITE));
    } else if resp.hovered() {
        p.circle_stroke(rect.center(), 11.0, Stroke::new(1.0, theme::TEXT_DIM));
    }
    if changed {
        p.circle_filled(rect.center(), 2.5, Color32::BLACK.gamma_multiply(0.7));
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tip)
}

// ----- Curves ---------------------------------------------------------------

fn curves(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let dirty = !c.color.curves.is_identity();
    let out = Sec::new("color-curves", "Curves")
        .reset(dirty)
        .closed()
        .show(ui, |ui| {
            let labels: Vec<String> = c
                .color
                .curves
                .all()
                .iter()
                .zip(CURVE_NAMES)
                .map(|(cv, n)| {
                    if cv.is_identity() {
                        n.to_string()
                    } else {
                        format!("{n} •")
                    }
                })
                .collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            if let Some(i) = kit::chips(ui, &refs, Some(p.st.curve_ch.min(3))) {
                p.st.curve_ch = i;
                p.st.curve_drag = None;
            }
            ui.add_space(6.0);
            let ch = p.st.curve_ch.min(3);
            let curve = c.color.curves.all_mut().into_iter().nth(ch).unwrap();
            curve_editor(ui, curve, &mut p.st.curve_drag, CURVE_COLORS[ch]);
            kit::hint(
                ui,
                "Click the line to add a point and drag it. Double-click a point to remove it.",
            );
            if !curve.is_identity() && ui.small_button("Reset this curve").clicked() {
                *curve = Curve::default();
            }
        });
    if out.reset {
        c.color.curves = Default::default();
    }
}

/// How close (pixels) the pointer must be to grab a point.
const GRAB_PX: f32 = 11.0;

/// Draws and edits one curve. Returns whether it changed.
fn curve_editor(ui: &mut Ui, curve: &mut Curve, drag: &mut Option<usize>, color: Color32) -> bool {
    let side = ui.available_width().clamp(120.0, 260.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click_and_drag());
    let to_screen = |x: f64, y: f64| -> Pos2 {
        egui::pos2(
            rect.left() + x as f32 * rect.width(),
            rect.bottom() - y as f32 * rect.height(),
        )
    };
    let to_curve = |p: Pos2| -> (f64, f64) {
        (
            ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64,
            ((rect.bottom() - p.y) / rect.height()).clamp(0.0, 1.0) as f64,
        )
    };
    let nearest = |curve: &Curve, p: Pos2| -> Option<usize> {
        curve
            .points
            .iter()
            .enumerate()
            .map(|(i, q)| (i, to_screen(q[0], q[1]).distance(p)))
            .filter(|(_, d)| *d <= GRAB_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let mut changed = false;
    if resp.drag_started() {
        let origin = ui
            .ctx()
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some(p) = origin {
            *drag = nearest(curve, p).or_else(|| {
                let (x, y) = to_curve(p);
                let i = curve.insert(x, y);
                changed |= i.is_some();
                i
            });
        }
    }
    if let (true, Some(i), Some(p)) = (resp.dragged(), *drag, resp.interact_pointer_pos()) {
        let (x, y) = to_curve(p);
        let before = curve.points.clone();
        curve.move_point(i, x, y);
        changed |= curve.points != before;
    }
    if resp.drag_stopped() {
        *drag = None;
    }
    if resp.double_clicked() || resp.secondary_clicked() {
        if let Some(i) = resp
            .interact_pointer_pos()
            .and_then(|p| nearest(curve, p))
            .filter(|i| curve.can_remove(*i))
        {
            curve.points.remove(i);
            *drag = None;
            changed = true;
        }
    }
    if resp.hovered() {
        let near = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| nearest(curve, p).is_some());
        ui.ctx().set_cursor_icon(if near {
            CursorIcon::Grab
        } else {
            CursorIcon::Crosshair
        });
    }

    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(6),
        theme::FIELD_BG,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    for q in 1..4 {
        let t = q as f32 / 4.0;
        let grid = Stroke::new(1.0, theme::BORDER.gamma_multiply(0.6));
        painter.vline(rect.left() + t * rect.width(), rect.y_range(), grid);
        painter.hline(rect.x_range(), rect.top() + t * rect.height(), grid);
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.0, theme::TEXT_FAINT),
    );
    let spline = curve.spline();
    let pts: Vec<Pos2> = (0..=96)
        .map(|i| {
            let x = i as f64 / 96.0;
            to_screen(x, spline.eval(x))
        })
        .collect();
    painter.add(Shape::line(pts, Stroke::new(2.0, color)));
    for (i, q) in curve.points.iter().enumerate() {
        let pos = to_screen(q[0], q[1]);
        let active = *drag == Some(i);
        painter.circle_filled(pos, if active { 6.0 } else { 5.0 }, color);
        painter.circle_stroke(
            pos,
            5.5,
            Stroke::new(1.5, Color32::BLACK.gamma_multiply(0.6)),
        );
    }
    changed
}

// ----- LUT --------------------------------------------------------------------

fn lut_section(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    let dirty = c.color.lut.is_some();
    let out = Sec::new("color-lut", "LUT")
        .reset(dirty)
        .closed()
        .show(ui, |ui| {
            let current = c.color.lut.as_ref().map(|l| l.source.clone());
            let mut labels: Vec<String> = vec!["None".into()];
            labels.extend(lut::BUILT_IN.iter().map(|b| b.name.to_string()));
            let mut selected = 0;
            match &current {
                Some(LutSource::BuiltIn(id)) => {
                    if let Some(i) = lut::BUILT_IN.iter().position(|b| b.id == *id) {
                        selected = i + 1;
                    }
                }
                Some(file @ (LutSource::File(_) | LutSource::Embedded { .. })) => {
                    labels.push(lut::label(file));
                    selected = labels.len() - 1;
                }
                None => {}
            }
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            if let Some(i) = kit::chips(ui, &refs, Some(selected)) {
                let source = match i {
                    0 => None,
                    i if i <= lut::BUILT_IN.len() => {
                        Some(LutSource::BuiltIn(lut::BUILT_IN[i - 1].id.to_string()))
                    }
                    _ => current.clone(),
                };
                set_lut(c, source);
            }
            ui.add_space(6.0);
            if kit::text_button(ui, ph::FOLDER_OPEN, "Import .cube file…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("3D LUT", &["cube"])
                    .pick_file()
                {
                    // The file's contents go into the project, so the look
                    // still works after the project (or the file) moves.
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "LUT".into());
                    let loaded = std::fs::read_to_string(&path)
                        .map_err(|e| format!("The file can't be read: {e}"))
                        .and_then(|text| {
                            lut::parse_cube(&text)?;
                            Ok(LutSource::embedded(name, text))
                        });
                    match loaded {
                        Ok(source) => set_lut(c, Some(source)),
                        Err(e) => p
                            .actions
                            .push(Action::Notify(format!("That LUT can't be used: {e}"), true)),
                    }
                }
            }
            if let Some(l) = &mut c.color.lut {
                ui.add_space(4.0);
                kit::anim_row(
                    ui,
                    "Strength",
                    &mut l.strength,
                    local,
                    0.0..=100.0,
                    0,
                    "%",
                    100.0,
                );
            }
            kit::hint(
            ui,
            "A LUT is a ready-made color look. Pick one of TidyCuts' own or import a .cube file.",
        );
        });
    if out.reset {
        c.color.lut = None;
    }
}

/// Switches the clip's LUT, keeping the strength when only the look changes.
fn set_lut(c: &mut Clip, source: Option<LutSource>) {
    match source {
        None => c.color.lut = None,
        Some(s) => match &mut c.color.lut {
            Some(l) => l.source = s,
            None => c.color.lut = Some(ColorLut::new(s)),
        },
    }
}
