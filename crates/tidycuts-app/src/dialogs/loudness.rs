//! The Loudness window: how loud the whole timeline is (integrated loudness
//! in LUFS, the highest peak and the loudness range), and how that compares
//! with what streaming sites expect.

use super::{footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::theme;
use egui::{Color32, CornerRadius, RichText};
use egui_phosphor::regular as ph;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tidycuts_engine::loudness::Report;

type Shared = Arc<Mutex<Option<Result<Report, String>>>>;

/// What the sites that normalise loudness aim for, and how far off a reading is.
pub fn verdict(lufs: f64) -> (String, Color32) {
    let off = lufs - -14.0;
    if off.abs() <= 1.0 {
        (
            "Right on the streaming standard (-14 LUFS).".into(),
            theme::GOOD,
        )
    } else if off < 0.0 {
        (
            format!(
                "{:.1} dB quieter than the streaming standard (-14 LUFS). Sites may turn it up, \
                 or people may reach for the volume. Set a loudness target when exporting.",
                -off
            ),
            theme::WARNING,
        )
    } else {
        (
            format!(
                "{off:.1} dB louder than the streaming standard (-14 LUFS). Sites will turn it \
                 down, which can make it sound flatter. Set a loudness target when exporting."
            ),
            theme::WARNING,
        )
    }
}

pub struct LoudnessDialog {
    result: Shared,
    cancel: Arc<AtomicBool>,
}

impl LoudnessDialog {
    pub fn new(app: &TidyCutsApp, ctx: &egui::Context) -> LoudnessDialog {
        let d = LoudnessDialog {
            result: Arc::new(Mutex::new(None)),
            cancel: Arc::new(AtomicBool::new(false)),
        };
        d.measure(app, ctx);
        d
    }

    fn measure(&self, app: &TidyCutsApp, ctx: &egui::Context) {
        *self.result.lock().unwrap() = None;
        let project = app.project.clone();
        let (result, cancel, ctx) = (self.result.clone(), self.cancel.clone(), ctx.clone());
        cancel.store(false, Ordering::Relaxed);
        std::thread::spawn(move || {
            let r = tidycuts_engine::export::measure_project_loudness(&project, None, &cancel);
            *result.lock().unwrap() = Some(r);
            ctx.request_repaint();
        });
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let state = self.result.lock().unwrap().clone();
        let mut close = false;
        let mut again = false;
        let (_, keys) = window(ctx, "loudness", 460.0, |ui| {
            if header(
                ui,
                ph::SPEAKER_HIGH,
                "Loudness",
                "How loud the whole timeline is",
            ) {
                close = true;
            }
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(20, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    match &state {
                        None => {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Mixing the timeline and measuring it...")
                                        .color(theme::TEXT_DIM),
                                );
                            });
                            ui.add_space(8.0);
                            ctx.request_repaint_after(std::time::Duration::from_millis(200));
                        }
                        Some(Err(e)) => {
                            ui.add(
                                egui::Label::new(RichText::new(e).color(theme::WARNING)).wrap(),
                            );
                        }
                        Some(Ok(r)) => {
                            egui::Frame::new()
                                .fill(theme::PANEL_BG_2)
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(14, 12))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width() - 10.0);
                                    egui::Grid::new("loudness-grid")
                                        .num_columns(2)
                                        .spacing(egui::vec2(18.0, 10.0))
                                        .show(ui, |ui| {
                                            for (k, v, tip) in [
                                                (
                                                    "Loudness",
                                                    format!("{:.1} LUFS", r.integrated),
                                                    "How loud it sounds on average. Streaming sites aim for -14.",
                                                ),
                                                (
                                                    "Highest peak",
                                                    format!("{:.1} dBTP", r.true_peak),
                                                    "The loudest instant. Keep it under -1 so it cannot distort.",
                                                ),
                                                (
                                                    "Loudness range",
                                                    format!("{:.1} LU", r.range),
                                                    "How much louder the loud parts are than the quiet ones.",
                                                ),
                                            ] {
                                                ui.label(RichText::new(k).color(theme::TEXT_DIM));
                                                ui.label(
                                                    RichText::new(v)
                                                        .font(theme::heading_font(16.0))
                                                        .color(theme::TEXT),
                                                )
                                                .on_hover_text(tip);
                                                ui.end_row();
                                            }
                                        });
                                });
                            let (text, color) = verdict(r.integrated);
                            ui.add(egui::Label::new(RichText::new(text).color(color)).wrap());
                            if r.true_peak > -1.0 {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(
                                            "The peak is above -1 dBTP: it may distort. Lower the \
                                             volume or use a Peak Limiter.",
                                        )
                                        .color(theme::WARNING),
                                    )
                                    .wrap(),
                                );
                            }
                        }
                    }
                    ui.add_space(6.0);
                });
            footer(
                ui,
                |ui| {
                    if secondary(ui, &format!("{}  Measure again", ph::ARROWS_CLOCKWISE)).clicked()
                    {
                        again = true;
                    }
                },
                |ui| {
                    if primary(ui, "Close", true).clicked() {
                        close = true;
                    }
                },
            );
        });
        if again {
            self.measure(app, ctx);
        }
        let done = close || keys.escape || keys.enter;
        if done {
            self.cancel.store(true, Ordering::Relaxed);
        }
        !done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdict_compares_with_the_streaming_standard() {
        assert_eq!(verdict(-14.4).1, theme::GOOD);
        assert!(verdict(-24.0).0.contains("quieter"));
        assert!(verdict(-8.0).0.contains("louder"));
    }
}
