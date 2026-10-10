//! Project Settings (File menu, Ctrl+Shift+,): name, size, frame rate,
//! sample rate, background and the magnetic main track, with an option to
//! keep them as the defaults for new projects.

use super::sizes::{aspect_name, preset_for, rate_label, same_rate, FRAME_RATES, PROJECT_SIZES};
use super::{body, field, footer, header, primary, secondary, switch, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Stroke, StrokeKind};
use egui_phosphor::regular as ph;
use tidycuts_core::{Project, ProjectSettings};

/// Sample rates offered for the project's sound.
const SAMPLE_RATES: [u32; 3] = [44_100, 48_000, 96_000];

pub struct ProjectSettingsDialog {
    name: String,
    settings: ProjectSettings,
    save_default: bool,
}

/// Draws a picture of a frame shape inside a preset tile.
fn draw_shape(p: &egui::Painter, tile: Rect, aspect: Option<f32>, on: bool, hovered: bool) {
    let color = if on {
        theme::ACCENT
    } else if hovered {
        Color32::WHITE
    } else {
        theme::TEXT_DIM
    };
    let area = tile.shrink(11.0);
    match aspect {
        Some(a) => {
            let mut size = area.size();
            if size.x / size.y > a {
                size.x = size.y * a;
            } else {
                size.y = size.x / a;
            }
            let r = Rect::from_center_size(area.center(), size);
            p.rect_filled(
                r,
                CornerRadius::same(2),
                if on {
                    theme::ACCENT_BG
                } else {
                    Color32::TRANSPARENT
                },
            );
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.5, color),
                StrokeKind::Inside,
            );
        }
        None => {
            p.text(
                area.center(),
                Align2::CENTER_CENTER,
                ph::PENCIL_SIMPLE,
                FontId::proportional(18.0),
                color,
            );
        }
    }
}

impl ProjectSettingsDialog {
    pub fn new(project: &Project) -> Self {
        ProjectSettingsDialog {
            name: project.name.clone(),
            settings: project.settings.clone(),
            save_default: false,
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui, defaults: &ProjectSettings) {
        field(ui, "Name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .desired_width(320.0)
                    .hint_text("Untitled"),
            );
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            widgets::group_title(ui, "Resolution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        *defaults != self.settings,
                        egui::Button::new(
                            RichText::new(format!(
                                "{}  Use my defaults",
                                ph::ARROW_COUNTER_CLOCKWISE
                            ))
                            .size(12.5)
                            .color(theme::ACCENT),
                        )
                        .frame(false),
                    )
                    .on_hover_text("Fill in the settings saved as the default for new projects")
                    .clicked()
                {
                    self.settings = defaults.clone();
                }
            });
        });
        let s = &mut self.settings;
        let current = preset_for(s.width, s.height);
        let custom = PROJECT_SIZES.len();
        let picked = widgets::tile_grid(
            ui,
            "project-sizes",
            custom + 1,
            egui::vec2(60.0, 52.0),
            Some(current.unwrap_or(custom)),
            |i| {
                PROJECT_SIZES
                    .get(i)
                    .map(|p| p.short.to_string())
                    .unwrap_or_else(|| "Custom".into())
            },
            |p, r, i, hovered| {
                let on = current.unwrap_or(custom) == i;
                let aspect = PROJECT_SIZES
                    .get(i)
                    .map(|p| p.width as f32 / p.height as f32);
                draw_shape(p, r, aspect, on, hovered);
            },
        );
        if let Some(p) = picked.and_then(|i| PROJECT_SIZES.get(i)) {
            s.width = p.width;
            s.height = p.height;
        }
        ui.add_space(4.0);
        field(ui, "Size", |ui| {
            let mut w = s.width;
            let mut h = s.height;
            let rw = ui.add_sized(
                [86.0, 26.0],
                egui::DragValue::new(&mut w)
                    .range(16..=8192)
                    .speed(2.0)
                    .suffix(" px"),
            );
            ui.label(RichText::new("×").color(theme::TEXT_DIM));
            let rh = ui.add_sized(
                [86.0, 26.0],
                egui::DragValue::new(&mut h)
                    .range(16..=8192)
                    .speed(2.0)
                    .suffix(" px"),
            );
            if rw.changed() || rh.changed() {
                s.width = w.max(16) & !1;
                s.height = h.max(16) & !1;
            }
            let name = match preset_for(s.width, s.height) {
                Some(i) => format!(
                    "{} · {}",
                    PROJECT_SIZES[i].name,
                    aspect_name(s.width, s.height)
                ),
                None => format!("Custom · {}", aspect_name(s.width, s.height)),
            };
            ui.add_space(6.0);
            ui.label(RichText::new(name).size(12.5).color(theme::TEXT_DIM));
        });
        ui.add_space(6.0);
        widgets::group_title(ui, "Playback");
        field(ui, "Frame rate", |ui| {
            let mut options: Vec<(tidycuts_core::FrameRate, String)> = FRAME_RATES
                .iter()
                .map(|(r, l)| (*r, l.to_string()))
                .collect();
            let shown = match options.iter().find(|(r, _)| same_rate(*r, s.frame_rate)) {
                Some((r, _)) => *r,
                None => {
                    options.push((
                        s.frame_rate,
                        format!("{} (custom)", rate_label(s.frame_rate)),
                    ));
                    s.frame_rate
                }
            };
            let mut pick = shown;
            let refs: Vec<(tidycuts_core::FrameRate, &str)> =
                options.iter().map(|(r, l)| (*r, l.as_str())).collect();
            if widgets::choice(ui, "ps-fps", &mut pick, &refs) {
                s.frame_rate = pick;
            }
        });
        field(ui, "Sample rate", |ui| {
            let mut i = SAMPLE_RATES
                .iter()
                .position(|r| *r == s.sample_rate)
                .unwrap_or(usize::MAX);
            ui.scope(|ui| {
                ui.set_max_width(300.0);
                if widgets::segmented(ui, &mut i, &["44.1 kHz", "48 kHz", "96 kHz"]) {
                    s.sample_rate = SAMPLE_RATES[i];
                }
            });
        });
        field(ui, "Background", |ui| {
            widgets::color_button(ui, &mut s.background);
            ui.label(
                RichText::new("Shown wherever no clip covers the frame")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
        });
        field(ui, "Magnetic main track", |ui| {
            widgets::toggle(ui, &mut s.magnetic);
            ui.label(
                RichText::new("Closes gaps on the main video track automatically")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
        });
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let defaults = app.prefs.project_defaults.clone();
        let mut ok = false;
        let mut cancel = false;
        let (_, keys) = window(ctx, "project-settings", 660.0, |ui| {
            cancel |= header(
                ui,
                ph::SLIDERS_HORIZONTAL,
                "Project Settings",
                "Size, frame rate and look of this project",
            );
            super::scroll_middle(ui, |ui| body(ui, |ui| self.contents(ui, &defaults)));
            footer(
                ui,
                |ui| {
                    switch(
                        ui,
                        &mut self.save_default,
                        "Save as default for new projects",
                    );
                },
                |ui| {
                    if primary(ui, "OK", true).clicked() {
                        ok = true;
                    }
                    if secondary(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
            );
        });
        if ok || keys.enter {
            self.apply(app);
            return false;
        }
        !(cancel || keys.escape)
    }

    /// Writes the settings to the project as one undo step.
    fn apply(&self, app: &mut TidyCutsApp) {
        let name = match self.name.trim() {
            "" => "Untitled".to_string(),
            n => n.to_string(),
        };
        let s = self.settings.clone();
        app.edit(|p| {
            let before = (p.name.clone(), p.settings.clone());
            p.name = name.clone();
            let magnetic = s.magnetic;
            p.settings = ProjectSettings {
                magnetic: before.1.magnetic,
                ..s.clone()
            };
            // Turning the magnetic track on closes its gaps.
            if magnetic != before.1.magnetic {
                p.set_magnetic(magnetic);
            }
            p.name != before.0 || p.settings != before.1
        });
        if self.save_default {
            app.prefs.project_defaults = s;
            app.prefs.save();
            app.notify("Saved as the default for new projects.", false);
        }
    }
}
