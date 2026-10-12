//! The welcome tour: five short cards that show where things are. It opens
//! the first time TidyCuts starts and any time from Help > Take the Tour.

use super::{footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::theme;
use egui::{Color32, CornerRadius, RichText, Sense};
use egui_phosphor::regular as ph;

/// One card: an icon, a title and the lines under it.
struct Card {
    icon: &'static str,
    title: &'static str,
    lines: &'static [(&'static str, &'static str)],
}

const CARDS: [Card; 5] = [
    Card {
        icon: ph::FILM_SLATE,
        title: "Welcome to TidyCuts",
        lines: &[
            (ph::DOWNLOAD_SIMPLE, "Import your videos, photos and music with the Import button, or drag them into the window."),
            (ph::PLUS_CIRCLE, "Double-click anything in the library to add it at the playhead, or drag it onto the timeline."),
            (ph::PLAY, "Press Space to play. Everything you do can be undone."),
        ],
    },
    Card {
        icon: ph::SELECTION_ALL,
        title: "Build your video on the timeline",
        lines: &[
            (ph::SCISSORS, "Move the playhead and press the scissors to cut. The main track closes gaps by itself."),
            (ph::LIGHTNING, "The buttons beside the timeline are shortcuts: freeze frame, markers, smart cuts, rotate, voiceover and more."),
            (ph::MOUSE_RIGHT_CLICK, "Right-click a clip to see everything you can do with it."),
        ],
    },
    Card {
        icon: ph::SPARKLE,
        title: "Make it look good",
        lines: &[
            (ph::TEXT_T, "Titles, Transitions, Effects, Filters and Stickers are in the library tabs."),
            (ph::FUNNEL, "The row of chips (Business, Gaming, Travel...) shows only what suits your kind of video."),
            (ph::CLOCK_COUNTER_CLOCKWISE, "Turn on \"Old designs\" to bring back the earlier looks."),
        ],
    },
    Card {
        icon: ph::SPEAKER_HIGH,
        title: "Make it sound good",
        lines: &[
            (ph::MAGIC_WAND, "Select a clip and open the Audio tab: Enhance Speech removes background noise and evens out a voice in one click."),
            (ph::SPEAKER_SIMPLE_LOW, "\"Lower Under Speech\" turns music down wherever someone is talking."),
            (ph::CHART_BAR, "The sound meter at the right edge of the timeline is always on. Click the number under it to see how loud the whole video is."),
        ],
    },
    Card {
        icon: ph::SUBTITLES,
        title: "Captions and sharing",
        lines: &[
            (ph::SUBTITLES, "Auto Captions turns what is said into subtitles. Find it in the Titles tab, the Clip menu, or the button beside the timeline."),
            (ph::EXPORT, "Export (top right) has presets for YouTube, phones and the web, and can set the loudness for you."),
            (ph::QUESTION, "Help > Take the Tour shows this again."),
        ],
    },
];

#[derive(Default)]
pub struct TourDialog {
    step: usize,
}

impl TourDialog {
    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, _app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let mut close = false;
        let card = &CARDS[self.step.min(CARDS.len() - 1)];
        let last = self.step + 1 >= CARDS.len();
        let go = std::cell::Cell::new(0i32);
        let (_, keys) = window(ctx, "tour", 560.0, |ui| {
            if header(
                ui,
                card.icon,
                card.title,
                &format!("Step {} of {}", self.step + 1, CARDS.len()),
            ) {
                close = true;
            }
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(24, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 14.0);
                    for (icon, text) in card.lines {
                        ui.horizontal_top(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::hover());
                            ui.painter()
                                .rect_filled(r, CornerRadius::same(9), theme::ACCENT_BG);
                            ui.painter().text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                *icon,
                                egui::FontId::proportional(19.0),
                                theme::ACCENT,
                            );
                            ui.add(
                                egui::Label::new(RichText::new(*text).color(Color32::WHITE)).wrap(),
                            );
                        });
                    }
                    ui.add_space(4.0);
                    // Progress dots.
                    ui.horizontal(|ui| {
                        for i in 0..CARDS.len() {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(
                                r.center(),
                                if i == self.step { 4.5 } else { 3.0 },
                                if i == self.step {
                                    theme::ACCENT
                                } else {
                                    theme::PANEL_BG_3
                                },
                            );
                        }
                    });
                });
            footer(
                ui,
                |ui| {
                    if self.step > 0 && secondary(ui, "Back").clicked() {
                        go.set(-1);
                    }
                },
                |ui| {
                    if !last && secondary(ui, "Skip").clicked() {
                        close = true;
                    }
                    if primary(ui, if last { "Start editing" } else { "Next" }, true).clicked() {
                        if last {
                            close = true;
                        } else {
                            go.set(1);
                        }
                    }
                },
            );
        });
        if go.get() != 0 {
            self.step = (self.step as i32 + go.get()).clamp(0, CARDS.len() as i32 - 1) as usize;
        }
        !(close || keys.escape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_card_has_text_and_a_short_title() {
        for c in &CARDS {
            assert!(!c.lines.is_empty() && c.lines.len() <= 4);
            assert!(c.title.len() < 48);
            assert!(c.lines.iter().all(|(_, t)| t.len() > 20 && t.len() < 200));
        }
    }
}
