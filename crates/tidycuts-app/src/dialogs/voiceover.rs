//! "Add a voiceover": type what to say, pick one of the computer's voices
//! and how fast it speaks.

use super::{body, field, footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::RichText;
use egui_phosphor::regular as ph;
use std::sync::{Arc, Mutex};
use tidycuts_media::speech::{self, Voice};

pub struct VoiceoverDialog {
    text: String,
    /// The voice picked: its id, or empty for the system's own.
    voice: String,
    speed: f64,
    /// Read on a worker thread: asking the system can take a moment.
    voices: Arc<Mutex<Option<Vec<Voice>>>>,
}

impl VoiceoverDialog {
    pub fn new(ctx: &egui::Context) -> Self {
        let voices = Arc::new(Mutex::new(None));
        let (slot, ctx) = (voices.clone(), ctx.clone());
        std::thread::spawn(move || {
            *slot.lock().unwrap() = Some(speech::voices());
            ctx.request_repaint();
        });
        VoiceoverDialog {
            text: String::new(),
            voice: String::new(),
            speed: 1.0,
            voices,
        }
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let voices = self.voices.lock().unwrap().clone();
        if self.voice.is_empty() {
            if let Some(first) = voices.as_ref().and_then(|v| v.first()) {
                self.voice = first.id.clone();
            }
        }
        let ready = !self.text.trim().is_empty() && voices.as_ref().is_some_and(|v| !v.is_empty());
        let mut go = false;
        let mut cancel = false;
        let (_, keys) = window(ctx, "voiceover", 520.0, |ui| {
            cancel |= header(
                ui,
                ph::MICROPHONE_STAGE,
                "Add a voiceover",
                "The computer reads your text aloud; nothing is sent anywhere",
            );
            body(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.text)
                        .hint_text("Type what the voice should say…")
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                field(ui, "Voice", |ui| match &voices {
                    None => {
                        ui.label(RichText::new("Looking for voices…").color(theme::TEXT_DIM));
                    }
                    Some(list) if list.is_empty() => {
                        let text = if cfg!(windows) {
                            "No voices were found on this computer. Windows has its own voices (no download from TidyCuts): add one in Windows Settings > Time & language > Speech."
                        } else if cfg!(target_os = "macos") {
                            "No voices were found on this computer."
                        } else {
                            "No voices were found. Install eSpeak NG (espeak-ng)."
                        };
                        ui.add(egui::Label::new(RichText::new(text).color(theme::WARNING)).wrap());
                        if cfg!(windows)
                            && ui
                                .add(theme::outline_button("Open the Windows speech settings"))
                                .clicked()
                        {
                            ctx.open_url(egui::OpenUrl::new_tab("ms-settings:speech"));
                        }
                    }
                    Some(list) => {
                        let current = list
                            .iter()
                            .find(|v| v.id == self.voice)
                            .map_or("Choose…", |v| v.name.as_str());
                        egui::ComboBox::from_id_salt("voiceover-voice")
                            .selected_text(current)
                            .width(300.0)
                            .show_ui(ui, |ui| {
                                for v in list {
                                    ui.selectable_value(&mut self.voice, v.id.clone(), &v.name);
                                }
                            });
                    }
                });
                field(ui, "Speed", |ui| {
                    widgets::slim_slider(ui, &mut self.speed, 0.5..=2.0, 200.0, false);
                    widgets::readout(ui, &mut self.speed, 0.5..=2.0, 2, "×", 64.0);
                });
                widgets::hint(
                    ui,
                    "The voiceover goes on an audio track at the playhead. You can undo it like any edit.",
                );
            });
            footer(
                ui,
                |_| {},
                |ui| {
                    if primary(ui, &format!("{}  Create", ph::MICROPHONE_STAGE), ready).clicked() {
                        go = true;
                    }
                    if secondary(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
            );
        });
        if go {
            app.start_voiceover(self.text.clone(), self.voice.clone(), self.speed);
            return false;
        }
        !(cancel || keys.escape)
    }
}
