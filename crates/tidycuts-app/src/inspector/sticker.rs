//! Sticker tab: a preview, which sticker it is, its two colors and its
//! looping motion.

use super::kit::{self, Sec};
use super::Panel;
use crate::theme;
use crate::widgets;
use egui::{Color32, CornerRadius, Rect, RichText, Sense, Ui};
use tidycuts_core::{Clip, Generator, StickerSpec};
use tidycuts_render::catalog::animations::STICKER_MOTIONS;
use tidycuts_render::catalog::{categories, stickers};

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let Some(Generator::Sticker(s)) = &mut c.generator else {
        return;
    };
    preview(p, ui, s);
    ui.add_space(8.0);

    Sec::new("sticker-kind", "Sticker").show(ui, |ui| {
        let current = stickers::get(&s.kind);
        let mut picked = None;
        widgets::row(ui, "Sticker", |ui| {
            egui::ComboBox::from_id_salt("sticker-kind")
                .selected_text(current.map(|d| d.name).unwrap_or(s.kind.as_str()))
                .width(ui.available_width().min(240.0))
                .height(360.0)
                .show_ui(ui, |ui| {
                    for cat in categories(stickers::STICKERS, |d| d.category) {
                        ui.label(RichText::new(cat).size(11.5).color(theme::TEXT_FAINT));
                        for d in stickers::STICKERS.iter().filter(|d| d.category == cat) {
                            if ui.selectable_label(d.id == s.kind, d.name).clicked() {
                                picked = Some(d);
                            }
                        }
                    }
                });
        });
        if let Some(d) = current {
            kit::row_hint(ui, &format!("{} sticker", d.category));
        }
        if let Some(d) = picked {
            if d.id != s.kind {
                *s = StickerSpec {
                    kind: d.id.into(),
                    color: d.color,
                    color2: d.color2,
                    motion: d.motion.into(),
                };
            }
        }
    });

    let def = stickers::get(&s.kind);
    let colors_dirty = def.is_some_and(|d| d.color != s.color || d.color2 != s.color2);
    let out = Sec::new("sticker-colors", "Colors")
        .reset(colors_dirty)
        .show(ui, |ui| {
            widgets::color_row(ui, "Main Color", &mut s.color);
            widgets::color_row(ui, "Second Color", &mut s.color2);
        });
    if out.reset {
        if let Some(d) = def {
            s.color = d.color;
            s.color2 = d.color2;
        }
    }

    Sec::new("sticker-motion", "Motion").show(ui, |ui| {
        let labels: Vec<&str> = STICKER_MOTIONS.iter().map(|(_, n)| *n).collect();
        let current = if s.motion.is_empty() {
            "none"
        } else {
            s.motion.as_str()
        };
        let sel = STICKER_MOTIONS.iter().position(|(id, _)| *id == current);
        if let Some(i) = kit::chips(ui, &labels, sel) {
            s.motion = STICKER_MOTIONS[i].0.to_string();
        }
        kit::hint(
            ui,
            "A loop that plays for as long as the sticker shows. Use the Animation \
             tab for how it comes in and goes out.",
        );
    });
}

/// The sticker drawn on a checkerboard, re-rendered when its look changes.
fn preview(p: &mut Panel, ui: &mut Ui, s: &StickerSpec) {
    let h = 120.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), theme::FIELD_BG);
    // Checkerboard so light and transparent parts show.
    let cell = 10.0;
    let clip = painter.with_clip_rect(rect.shrink(1.0));
    let (cols, rows) = (
        (rect.width() / cell).ceil() as i32,
        (rect.height() / cell).ceil() as i32,
    );
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 0 {
                let r = Rect::from_min_size(
                    rect.min + egui::vec2(x as f32 * cell, y as f32 * cell),
                    egui::vec2(cell, cell),
                );
                clip.rect_filled(r, CornerRadius::ZERO, Color32::from_gray(0x1c));
            }
        }
    }
    let stale =
        p.st.sticker_preview
            .as_ref()
            .is_none_or(|(spec, _)| spec != s);
    if stale {
        let img = stickers::render(s, (h * 2.0) as u32, 0.5);
        let size = [img.width as usize, img.height as usize];
        let color = if img.premultiplied {
            egui::ColorImage::from_rgba_premultiplied(size, &img.pixels)
        } else {
            egui::ColorImage::from_rgba_unmultiplied(size, &img.pixels)
        };
        let tex = ui
            .ctx()
            .load_texture("inspector-sticker", color, egui::TextureOptions::LINEAR);
        p.st.sticker_preview = Some((s.clone(), tex));
    }
    if let Some((_, tex)) = &p.st.sticker_preview {
        let [w, th] = tex.size();
        let scale = ((h - 16.0) / th as f32).min((rect.width() - 16.0) / w as f32);
        let r = Rect::from_center_size(rect.center(), egui::vec2(w as f32, th as f32) * scale);
        clip.image(
            tex.id(),
            r,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

/// Puts a sticker's colors and motion back to the catalog's.
pub fn reset(c: &mut Clip) {
    if let Some(Generator::Sticker(s)) = &mut c.generator {
        if let Some(d) = stickers::get(&s.kind) {
            s.color = d.color;
            s.color2 = d.color2;
            s.motion = d.motion.into();
        }
    }
}
