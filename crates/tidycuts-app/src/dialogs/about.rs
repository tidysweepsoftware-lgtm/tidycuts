//! The About box: version, graphics card, FFmpeg and encoders found.

use super::{footer, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::{Align2, Color32, CornerRadius, FontId, RichText, Sense};
use egui_phosphor::regular as ph;
use std::sync::{Mutex, Once};

#[derive(Default)]
pub struct AboutDialog;

static VERSION_PROBE: Once = Once::new();
static FFMPEG_VERSION: Mutex<Option<String>> = Mutex::new(None);

/// The FFmpeg version, read once on a worker thread.
fn ffmpeg_version(ctx: &egui::Context) -> Option<String> {
    VERSION_PROBE.call_once(|| {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let v = tidycuts_media::Tools::get()
                .ok()
                .and_then(|t| t.ffmpeg_cmd().arg("-version").output().ok())
                .map(|o| parse_version(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or_default();
            *FFMPEG_VERSION.lock().unwrap() = Some(v);
            ctx.request_repaint();
        });
    });
    FFMPEG_VERSION.lock().unwrap().clone()
}

/// "6.1.1" from "ffmpeg version 6.1.1-3ubuntu5 Copyright (c) …".
fn parse_version(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    let mut words = first.split_whitespace();
    while let Some(w) = words.next() {
        if w == "version" {
            if let Some(v) = words.next() {
                return v.split('-').next().unwrap_or(v).to_string();
            }
        }
    }
    String::new()
}

impl AboutDialog {
    fn details(app: &TidyCutsApp, ctx: &egui::Context) -> Vec<(&'static str, String, bool)> {
        let gpu = match (&app.engine, &app.gpu_error) {
            (Some(e), _) => (e.adapter.clone(), true),
            (None, Some(err)) => (err.clone(), false),
            (None, None) => ("Not available".into(), false),
        };
        let ffmpeg = match (&app.ffmpeg_error, tidycuts_media::Tools::get()) {
            (None, Ok(t)) => {
                let v = ffmpeg_version(ctx);
                let v = match v.as_deref() {
                    None => "checking…".to_string(),
                    Some("") => "found".to_string(),
                    Some(v) => v.to_string(),
                };
                (format!("{v} ({})", t.ffmpeg.display()), true)
            }
            (Some(e), _) => (e.clone(), false),
            (None, Err(e)) => (e.to_string(), false),
        };
        let encoders = match super::export::working_encoders(ctx) {
            None => ("checking…".to_string(), true),
            Some(list) => {
                let gpu: Vec<&str> = list
                    .iter()
                    .filter(|e| super::export::is_gpu(e))
                    .map(String::as_str)
                    .collect();
                if list.is_empty() {
                    ("none found".to_string(), false)
                } else if gpu.is_empty() {
                    (format!("{} available (CPU only)", list.len()), true)
                } else {
                    (
                        format!("{} available, GPU: {}", list.len(), gpu.join(", ")),
                        true,
                    )
                }
            }
        };
        vec![
            ("Graphics", gpu.0, gpu.1),
            ("FFmpeg", ffmpeg.0, ffmpeg.1),
            ("Encoders", encoders.0, encoders.1),
            (
                "System",
                format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
                true,
            ),
        ]
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let details = Self::details(app, ctx);
        let mut close = false;
        let mut copy = false;
        let (_, keys) = window(ctx, "about", 480.0, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 24,
                    right: 14,
                    top: 14,
                    bottom: 18,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if widgets::icon_button(ui, ph::X, "Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                    ui.vertical_centered(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), Sense::hover());
                        ui.painter()
                            .rect_filled(r, CornerRadius::same(18), theme::ACCENT);
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            ph::FILM_SLATE,
                            FontId::proportional(40.0),
                            theme::ACCENT_TEXT,
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("TidyCuts")
                                .font(theme::heading_font(24.0))
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                .color(theme::TEXT_DIM),
                        );
                        ui.add_space(8.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "A fast, friendly video editor. TidyCuts is open source and \
                                     built in Rust, with egui for the interface, wgpu for drawing \
                                     on the graphics card and FFmpeg for reading and writing media.",
                                )
                                .color(theme::TEXT),
                            )
                            .wrap()
                            .halign(egui::Align::Center),
                        );
                    });
                    ui.add_space(14.0);
                    egui::Frame::new()
                        .fill(theme::PANEL_BG_2)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width() - 10.0);
                            egui::Grid::new("about-grid")
                                .num_columns(2)
                                .spacing(egui::vec2(14.0, 8.0))
                                .show(ui, |ui| {
                                    for (k, v, ok) in &details {
                                        ui.label(RichText::new(*k).color(theme::TEXT_DIM));
                                        ui.add(
                                            egui::Label::new(RichText::new(v).color(if *ok {
                                                theme::TEXT
                                            } else {
                                                theme::WARNING
                                            }))
                                            .wrap(),
                                        );
                                        ui.end_row();
                                    }
                                });
                        });
                });
            footer(
                ui,
                |ui| {
                    if secondary(ui, &format!("{}  Copy details", ph::COPY))
                        .on_hover_text("Copy the version and system details, for bug reports")
                        .clicked()
                    {
                        copy = true;
                    }
                },
                |ui| {
                    if primary(ui, "Close", true).clicked() {
                        close = true;
                    }
                },
            );
        });
        if copy {
            let mut text = format!("TidyCuts {}\n", env!("CARGO_PKG_VERSION"));
            for (k, v, _) in &details {
                text.push_str(&format!("{k}: {v}\n"));
            }
            ctx.copy_text(text);
            app.notify("Details copied.", false);
        }
        !(close || keys.escape || keys.enter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_version_is_read_from_the_banner() {
        assert_eq!(
            parse_version("ffmpeg version 6.1.1-3ubuntu5 Copyright (c) 2000-2023\nbuilt with gcc"),
            "6.1.1"
        );
        assert_eq!(parse_version("ffmpeg version n7.0 Copyright"), "n7.0");
        assert_eq!(parse_version(""), "");
    }
}
