//! Animation tab: in and out presets as a grid of moving previews, their
//! length, and the clip's transitions.

use super::kit::{self, Sec};
use super::{Action, Kind, Panel};
use crate::theme;
use crate::widgets;
use egui::{Align2, Color32, FontId, Painter, Rect, Shape, Stroke, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{Clip, Time};
use tidycuts_render::catalog::animations::{self, AnimDef, Phase};
use tidycuts_render::catalog::transitions;

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let name = |a: &Option<tidycuts_core::ClipAnimation>| {
        a.as_ref()
            .map(|a| {
                animations::get(&a.kind)
                    .map(|d| d.name)
                    .unwrap_or(a.kind.as_str())
                    .to_string()
            })
            .unwrap_or_else(|| "None".into())
    };
    let labels = [
        format!("In  ·  {}", name(&c.anim_in)),
        format!("Out  ·  {}", name(&c.anim_out)),
    ];
    let mut phase = usize::from(p.st.anim_out);
    if widgets::segmented(ui, &mut phase, &[labels[0].as_str(), labels[1].as_str()]) {
        p.st.anim_out = phase == 1;
    }
    ui.add_space(10.0);
    let out = p.st.anim_out;
    let list: Vec<&AnimDef> = animations::ANIMATIONS
        .iter()
        .filter(|a| !a.titles_only || p.kind == Kind::Title)
        .collect();
    let slot = if out { &mut c.anim_out } else { &mut c.anim_in };
    let selected = match slot {
        None => 0,
        Some(a) => list
            .iter()
            .position(|d| d.id == a.kind)
            .map(|i| i + 1)
            .unwrap_or(0),
    };
    let ph_ = if out { Phase::Out } else { Phase::In };
    let clicked = widgets::tile_grid(
        ui,
        ("anim-grid", out),
        list.len() + 1,
        egui::vec2(88.0, 60.0),
        Some(selected),
        |i| {
            if i == 0 {
                "None".into()
            } else {
                list[i - 1].name.into()
            }
        },
        |painter, rect, i, hovered| {
            if i == 0 {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    ph::PROHIBIT,
                    FontId::proportional(24.0),
                    if hovered {
                        Color32::WHITE
                    } else {
                        theme::TEXT_DIM
                    },
                );
            } else {
                preview(painter, rect, list[i - 1].id, ph_, hovered, selected == i);
            }
        },
    );
    if let Some(i) = clicked {
        let kind = (i > 0).then(|| list[i - 1].id.to_string());
        if kind.as_deref() != slot.as_ref().map(|a| a.kind.as_str()) {
            p.actions.push(Action::SetAnimation { out, kind });
        }
    }
    ui.add_space(8.0);
    let len = c.duration();
    let slot = if out { &mut c.anim_out } else { &mut c.anim_in };
    if let Some(a) = slot {
        let max = len.as_secs_f64().clamp(0.1, 10.0);
        let mut secs = a.duration.as_secs_f64();
        let default = p.app.prefs.animation_duration.clamp(0.1, max);
        if kit::slider_row(ui, "Duration", &mut secs, 0.1..=max, 2, " s", default) {
            a.duration = Time::from_secs_f64(secs.clamp(0.1, max));
        }
        kit::row_hint(
            ui,
            if out {
                "The out animation ends exactly as the clip ends."
            } else {
                "The in animation starts as the clip appears."
            },
        );
    } else {
        kit::hint(
            ui,
            "Hover a preset to preview it; click to apply it to the clip.",
        );
    }
    ui.add_space(6.0);
    transitions_section(ui, c);
}

/// Paints a small frame with a card moving through animation `id`. While
/// hovered the preview loops.
fn preview(painter: &Painter, rect: Rect, id: &str, phase: Phase, hovered: bool, on: bool) {
    let canvas = rect.shrink(7.0);
    let progress = if hovered {
        painter.ctx().request_repaint();
        let t = painter.ctx().input(|i| i.time) as f32;
        let cycle = (t % 1.6) / 1.2;
        cycle.min(1.0)
    } else {
        // A pose part-way through, so the still tile hints at the motion.
        match phase {
            Phase::In => 0.3,
            Phase::Out => 0.6,
        }
    };
    let m = animations::evaluate(id, phase, progress, [canvas.width(), canvas.height()]);
    let clip = painter.with_clip_rect(canvas);
    clip.rect_stroke(
        canvas,
        egui::CornerRadius::same(3),
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let base = egui::vec2(canvas.width() * 0.5, canvas.height() * 0.56);
    // Where the card rests, as a faint outline.
    clip.rect_stroke(
        Rect::from_center_size(canvas.center(), base),
        egui::CornerRadius::same(2),
        Stroke::new(1.0, theme::TEXT_FAINT.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    let size = egui::vec2(
        base.x * m.scale * m.stretch[0],
        base.y * m.scale * m.stretch[1],
    );
    let center = canvas.center() + egui::vec2(m.dx, m.dy);
    let mut r = Rect::from_center_size(center, size);
    // Wipes cut the card from its sides.
    let w = r.width();
    let h = r.height();
    r.min.x += m.crop[0] * w;
    r.max.x -= m.crop[2] * w;
    r.min.y += m.crop[1] * h;
    r.max.y -= m.crop[3] * h;
    if r.width() <= 0.5 || r.height() <= 0.5 {
        return;
    }
    let alpha = m.opacity.clamp(0.0, 1.0) * (1.0 - m.blur * 0.5);
    let base_col = if on { theme::ACCENT } else { theme::TEXT_DIM };
    let fill = base_col.gamma_multiply(0.35 * alpha);
    let stroke = Stroke::new(1.4, base_col.gamma_multiply(alpha));
    let (s, co) = m.rotation.to_radians().sin_cos();
    let rot = |p: egui::Pos2| {
        let v = p - center;
        center + egui::vec2(v.x * co - v.y * s, v.x * s + v.y * co)
    };
    let pts: Vec<egui::Pos2> = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
    .into_iter()
    .map(rot)
    .collect();
    if m.blur > 0.05 {
        let grow: Vec<egui::Pos2> = pts
            .iter()
            .map(|p| center + (*p - center) * (1.0 + m.blur * 0.25))
            .collect();
        clip.add(Shape::convex_polygon(
            grow,
            base_col.gamma_multiply(0.12 * alpha),
            Stroke::NONE,
        ));
    }
    clip.add(Shape::convex_polygon(pts, fill, stroke));
    if m.reveal < 1.0 || id == "typewriter" {
        let text = "Text";
        let n = ((text.len() as f32) * m.reveal).round() as usize;
        clip.text(
            r.center(),
            Align2::CENTER_CENTER,
            &text[..n.min(text.len())],
            FontId::proportional(12.0),
            Color32::WHITE.gamma_multiply(alpha),
        );
    } else {
        // A little mountain, like a picture.
        let b = r.shrink2(egui::vec2(r.width() * 0.18, r.height() * 0.22));
        let mount = vec![
            rot(egui::pos2(b.left(), b.bottom())),
            rot(egui::pos2(
                b.left() + b.width() * 0.38,
                b.top() + b.height() * 0.25,
            )),
            rot(egui::pos2(
                b.left() + b.width() * 0.62,
                b.bottom() - b.height() * 0.3,
            )),
            rot(egui::pos2(b.right(), b.bottom())),
        ];
        clip.add(Shape::line(
            mount,
            Stroke::new(1.2, base_col.gamma_multiply(alpha)),
        ));
    }
}

fn transitions_section(ui: &mut Ui, c: &mut Clip) {
    if c.transition_in.is_none() && c.transition_out.is_none() {
        return;
    }
    let len = c.duration();
    Sec::new("clip-transitions", "Transitions").show(ui, |ui| {
        for (label, slot) in [
            ("Into Clip", &mut c.transition_in),
            ("To Black", &mut c.transition_out),
        ] {
            let Some(tr) = slot else {
                continue;
            };
            let name = transitions::get(&tr.kind)
                .map(|d| d.name)
                .unwrap_or("Transition");
            let mut remove = false;
            widgets::row(ui, label, |ui| {
                ui.label(egui::RichText::new(name).strong());
                if widgets::icon_button(ui, ph::TRASH, "Remove the transition").clicked() {
                    remove = true;
                }
            });
            let max = len.as_secs_f64().clamp(0.1, 10.0);
            let mut secs = tr.duration.as_secs_f64();
            if kit::slider_row(
                ui,
                "Duration",
                &mut secs,
                0.1..=max,
                2,
                " s",
                1.0_f64.min(max),
            ) {
                tr.duration = Time::from_secs_f64(secs.clamp(0.1, max));
            }
            if remove {
                *slot = None;
            }
        }
        kit::row_hint(
            ui,
            "Transitions are centred on the cut. Drag a new one from the Transitions \
             library onto the cut to replace it.",
        );
    });
}
