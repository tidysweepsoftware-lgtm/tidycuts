//! Title tab: the text, font, size, colors, alignment, decoration style,
//! outline, shadow, background box and ready-made presets.

use super::kit::{self, Sec};
use super::Panel;
use crate::theme;
use crate::widgets;
use egui::{RichText, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{Clip, TextAlign, TitleSpec};
use tidycuts_render::catalog::{categories, titles};
use tidycuts_render::{fonts, text};

pub fn show(_p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let Some(t) = c.title_mut() else {
        return;
    };
    kit::caption(ui, "Text");
    ui.add(
        egui::TextEdit::multiline(&mut t.text)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(16.0))
            .hint_text("Type your title"),
    );
    ui.add_space(8.0);

    let def = TitleSpec::default();
    let font_dirty = t.font != def.font
        || t.size != def.size
        || t.bold
        || t.italic
        || t.align != def.align
        || t.letter_spacing != def.letter_spacing
        || t.line_spacing != def.line_spacing
        || t.color != def.color;
    let out = Sec::new("title-font", "Font")
        .reset(font_dirty)
        .show(ui, |ui| {
            let options: Vec<(String, &str)> = fonts::FONTS
                .iter()
                .map(|f| (f.name.to_string(), f.name))
                .collect();
            let mut font = fonts::get(&t.font).name.to_string();
            if widgets::choice_row(ui, "Font", &mut font, &options) {
                t.font = font;
            }
            let mut size = t.size as f64;
            if kit::slider_row(ui, "Size", &mut size, 8.0..=400.0, 0, "", def.size as f64) {
                t.size = size as f32;
            }
            widgets::row(ui, "Style", |ui| {
                // Five buttons have to fit beside the label.
                ui.spacing_mut().item_spacing.x = 4.0;
                let bw = 30.0;
                if kit::tool_button(ui, ph::TEXT_B, "", t.bold, bw)
                    .on_hover_text("Bold")
                    .clicked()
                {
                    t.bold = !t.bold;
                }
                if kit::tool_button(ui, ph::TEXT_ITALIC, "", t.italic, bw)
                    .on_hover_text("Italic")
                    .clicked()
                {
                    t.italic = !t.italic;
                }
                ui.add_space(6.0);
                for (align, icon, tip) in [
                    (TextAlign::Left, ph::TEXT_ALIGN_LEFT, "Align left"),
                    (TextAlign::Center, ph::TEXT_ALIGN_CENTER, "Center"),
                    (TextAlign::Right, ph::TEXT_ALIGN_RIGHT, "Align right"),
                ] {
                    if kit::tool_button(ui, icon, "", t.align == align, bw)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        t.align = align;
                    }
                }
            });
            let mut ls = t.letter_spacing as f64 * 100.0;
            if kit::slider_row(ui, "Letter Spacing", &mut ls, -10.0..=60.0, 0, "", 0.0) {
                t.letter_spacing = (ls / 100.0) as f32;
            }
            let mut lh = t.line_spacing as f64;
            if kit::slider_row(
                ui,
                "Line Spacing",
                &mut lh,
                0.6..=3.0,
                2,
                "",
                def.line_spacing as f64,
            ) {
                t.line_spacing = lh as f32;
            }
            widgets::color_row(ui, "Text Color", &mut t.color);
        });
    if out.reset {
        t.font = def.font.clone();
        t.size = def.size;
        t.bold = false;
        t.italic = false;
        t.align = def.align;
        t.letter_spacing = def.letter_spacing;
        t.line_spacing = def.line_spacing;
        t.color = def.color;
    }

    let out = Sec::new("title-style", "Decoration")
        .reset(t.style != def.style || t.accent != def.accent)
        .show(ui, |ui| {
            let options: Vec<(String, &str)> = text::STYLES
                .iter()
                .map(|(id, name)| (id.to_string(), *name))
                .collect();
            widgets::choice_row(ui, "Style", &mut t.style, &options);
            widgets::color_row(ui, "Accent Color", &mut t.accent);
            kit::row_hint(
                ui,
                "Bars, boxes, underlines and glows drawn around the text use the accent \
                 color.",
            );
        });
    if out.reset {
        t.style = def.style.clone();
        t.accent = def.accent;
    }

    let mut on = t.outline > 0.0;
    let out = Sec::new("title-outline", "Outline")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            let mut w = t.outline as f64;
            if kit::slider_row(ui, "Width", &mut w, 0.0..=30.0, 1, "", 4.0) {
                t.outline = w as f32;
            }
            widgets::color_row(ui, "Color", &mut t.outline_color);
        });
    if out.switched {
        t.outline = if on { 4.0 } else { 0.0 };
    }

    let mut on = t.shadow > 0.0;
    let out = Sec::new("title-shadow", "Shadow")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            let mut s = t.shadow as f64;
            if kit::slider_row(ui, "Strength", &mut s, 0.0..=100.0, 0, "", 50.0) {
                t.shadow = s as f32;
            }
            widgets::color_row(ui, "Color", &mut t.shadow_color);
        });
    if out.switched {
        t.shadow = if on { 50.0 } else { 0.0 };
    }

    let mut on = t.background;
    let out = Sec::new("title-background", "Background Box")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            widgets::color_row(ui, "Color", &mut t.background_color);
            kit::row_hint(
                ui,
                "A box behind the text; lower the color's alpha to see through it.",
            );
        });
    if out.switched {
        t.background = on;
    }

    Sec::new("title-presets", "Presets")
        .closed()
        .show(ui, |ui| {
            let mut picked = None;
            widgets::row(ui, "Apply", |ui| {
                egui::ComboBox::from_id_salt("title-preset")
                    .selected_text("Choose a look…")
                    .width(ui.available_width().min(240.0))
                    .height(360.0)
                    .show_ui(ui, |ui| {
                        for cat in categories(titles::TITLES, |t| t.category) {
                            ui.label(RichText::new(cat).size(11.5).color(theme::TEXT_FAINT));
                            for p in titles::TITLES.iter().filter(|p| p.category == cat) {
                                if ui.selectable_label(false, p.name).clicked() {
                                    picked = Some(p);
                                }
                            }
                        }
                    });
            });
            kit::row_hint(
                ui,
                "Takes the font, colors and decoration of a preset and keeps your text.",
            );
            if let Some(p) = picked {
                let text = std::mem::take(&mut t.text);
                *t = TitleSpec { text, ..(p.make)() };
            }
        });
}

/// Puts a title's look back to the default, keeping its text.
pub fn reset(c: &mut Clip) {
    if let Some(t) = c.title_mut() {
        let text = std::mem::take(&mut t.text);
        *t = TitleSpec {
            text,
            ..TitleSpec::default()
        };
    }
}
