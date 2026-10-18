//! The keyboard shortcut list (Help menu): every shortcut in one place.

use super::{body, footer, header, scroll_middle, secondary, window};
use crate::app::TidyCutsApp;
use crate::theme;
use egui::{Color32, RichText};
use egui_phosphor::regular as ph;

#[derive(Default)]
pub struct ShortcutsDialog {
    filter: String,
}

/// The key that does what Ctrl does elsewhere: Cmd on a Mac.
const CMD: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

/// (group, [(keys, what it does)]). `{c}` stands for Ctrl (Cmd on a Mac).
const LIST: &[(&str, &[(&str, &str)])] = &[
    (
        "Project",
        &[
            ("{c}+N", "New project"),
            ("{c}+O", "Open a project"),
            ("{c}+S", "Save"),
            ("{c}+Shift+S", "Save as"),
            ("{c}+I", "Import media"),
            ("{c}+E", "Export"),
            ("{c}+,", "Preferences"),
            ("{c}+Shift+,", "Project settings"),
        ],
    ),
    (
        "Editing",
        &[
            ("{c}+Z", "Undo"),
            ("{c}+Y or {c}+Shift+Z", "Redo"),
            ("{c}+C", "Copy the selected clips"),
            ("{c}+X", "Cut the selected clips"),
            ("{c}+V", "Paste at the playhead"),
            ("{c}+D", "Duplicate"),
            ("{c}+A", "Select all"),
            ("Delete", "Delete the selected clips"),
            ("Shift+Delete", "Delete and close the gap (ripple delete)"),
            ("{c}+B", "Split at the playhead"),
            (
                "Alt+[  and  Alt+]",
                "Trim the start or the end to the playhead",
            ),
            ("{c}+Alt+D", "Detach the sound from a video"),
            ("{c}+Alt+F", "Freeze frame"),
            ("{c}+T", "Add a title"),
            ("{c}+R", "Speed of the selected clip"),
            ("Alt+C", "Crop and zoom"),
        ],
    ),
    (
        "Playing and moving",
        &[
            ("Space", "Play or pause"),
            ("Left or Right", "One frame back or forward"),
            ("Shift+Left or Right", "One second back or forward"),
            ("Up or Down", "Previous or next cut"),
            ("Home or End", "Go to the start or the end"),
            ("M", "Add a marker"),
            ("Alt+Left or Right", "Previous or next marker"),
            ("F or Alt+Enter", "Full screen"),
            ("Esc", "Leave full screen, or clear the selection"),
        ],
    ),
    (
        "Timeline",
        &[
            ("N", "Turn snapping on or off"),
            ("{c}+Plus or {c}+Minus", "Zoom the timeline"),
            ("Click the ruler", "Move the playhead there"),
            ("Drag the ruler", "Zoom the timeline"),
            (
                "Mouse over the timeline",
                "Skim: see that picture while paused",
            ),
            ("Double-click the sound meter", "Open the audio mixer"),
        ],
    ),
];

impl ShortcutsDialog {
    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, _app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let mut close = false;
        let (_, keys) = window(ctx, "shortcuts", 560.0, |ui| {
            close |= header(
                ui,
                ph::KEYBOARD,
                "Keyboard shortcuts",
                "Everything you can do from the keyboard",
            );
            scroll_middle(ui, |ui| {
                body(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Search, e.g. split")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);
                    let needle = self.filter.trim().to_lowercase();
                    for (group, items) in LIST {
                        let rows: Vec<_> = items
                            .iter()
                            .filter(|(k, what)| {
                                needle.is_empty()
                                    || what.to_lowercase().contains(&needle)
                                    || k.to_lowercase().contains(&needle)
                            })
                            .collect();
                        if rows.is_empty() {
                            continue;
                        }
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(*group)
                                .size(12.0)
                                .color(theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        egui::Grid::new(("shortcut-grid", *group))
                            .num_columns(2)
                            .spacing([24.0, 6.0])
                            .show(ui, |ui| {
                                for (keys, what) in rows {
                                    ui.label(
                                        RichText::new(keys.replace("{c}", CMD))
                                            .monospace()
                                            .color(Color32::WHITE),
                                    );
                                    ui.label(*what);
                                    ui.end_row();
                                }
                            });
                    }
                })
            });
            footer(
                ui,
                |_| {},
                |ui| {
                    if secondary(ui, "Close").clicked() {
                        close = true;
                    }
                },
            );
        });
        !(close || keys.escape)
    }
}
