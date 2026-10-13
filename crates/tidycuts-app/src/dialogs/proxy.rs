//! The question asked when a big video is put on the timeline: make a smaller
//! copy of it for smooth editing (a proxy), or use the original.

use super::{body, footer, header, primary, secondary, switch, window};
use crate::app::TidyCutsApp;
use crate::theme;
use egui::RichText;
use egui_phosphor::regular as ph;
use tidycuts_core::MediaId;

pub struct ProxyDialog {
    media: MediaId,
    remember: bool,
}

impl ProxyDialog {
    pub fn new(media: MediaId) -> Self {
        ProxyDialog {
            media,
            remember: false,
        }
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let Some(m) = app.project.media(self.media).cloned() else {
            return false;
        };
        let mut answer: Option<bool> = None;
        let mut close = false;
        let (_, keys) = window(ctx, "proxy-ask", 520.0, |ui| {
            close |= header(
                ui,
                ph::FILM_STRIP,
                "Make a smooth copy for editing?",
                &m.name,
            );
            body(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "This video is large ({}×{}). A smaller copy (a proxy) plays and scrubs smoothly while you edit, \
                             but it takes a few minutes to make and uses some disk space. Exports always use the original, \
                             so the quality does not change.",
                            m.width, m.height
                        ))
                        .color(theme::TEXT_DIM),
                    )
                    .wrap(),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Without it the video starts at once, but jumping to a new place in a big video can stutter for a moment.",
                        )
                        .size(12.5)
                        .color(theme::TEXT_FAINT),
                    )
                    .wrap(),
                );
            });
            footer(
                ui,
                |ui| {
                    switch(ui, &mut self.remember, "Remember my choice");
                },
                |ui| {
                    if primary(ui, "Make smooth copy", true).clicked() {
                        answer = Some(true);
                    }
                    if secondary(ui, "Use the original").clicked() {
                        answer = Some(false);
                    }
                },
            );
        });
        // Esc or the close button means "not now".
        if answer.is_none() && (close || keys.escape) {
            answer = Some(false);
        }
        let Some(yes) = answer else {
            return true;
        };
        if self.remember {
            app.prefs.proxy_ask = false;
            app.prefs.auto_proxies = yes;
            app.prefs.save();
        }
        if yes {
            app.make_proxy(self.media);
        }
        false
    }
}
