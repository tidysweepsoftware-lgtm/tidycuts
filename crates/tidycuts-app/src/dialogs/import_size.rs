//! The question asked when the first video put in a project doesn't match
//! its size or frame rate: keep the video's size (say 4K), bring it down to
//! 1080p, or keep the project as it is.

use super::sizes::{downscale, rate_label, same_rate, size_label};
use super::{body, footer, header, primary, switch, window};
use crate::app::TidyCutsApp;
use crate::prefs::ImportSize;
use crate::{theme, widgets};
use egui::RichText;
use egui_phosphor::regular as ph;
use tidycuts_core::{FrameRate, MediaId, MediaItem, ProjectSettings};

pub struct ImportSizeDialog {
    media: MediaId,
    choice: usize,
    remember: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    /// The project takes the video's size and frame rate.
    MatchVideo,
    /// The project takes the video's shape and frame rate at a smaller size.
    Downscale(u32, u32),
    KeepProject,
}

#[derive(Debug, Clone, PartialEq)]
struct Choice {
    action: Action,
    icon: &'static str,
    title: String,
    detail: String,
    note: &'static str,
}

/// The answers worth offering for this video and project.
fn choices(m: &MediaItem, s: &ProjectSettings) -> Vec<Choice> {
    let fps = m.frame_rate.unwrap_or(s.frame_rate);
    let (w, h) = (m.width.max(2) & !1, m.height.max(2) & !1);
    let big = w.min(h) > 1080;
    let mut v = vec![Choice {
        action: Action::MatchVideo,
        icon: ph::FRAME_CORNERS,
        title: "Keep original size".into(),
        detail: format!("{} · {}", size_label(w, h), rate_label(fps)),
        note: if big {
            "Full quality. Editing at this size needs a fast computer."
        } else {
            "The project takes the video's size and frame rate."
        },
    }];
    if big {
        let (dw, dh) = downscale(w, h, 1080);
        let same_as_project = (dw, dh) == (s.width, s.height) && same_rate(fps, s.frame_rate);
        if !same_as_project {
            v.push(Choice {
                action: Action::Downscale(dw, dh),
                icon: ph::ARROWS_IN,
                title: "Use 1080p".into(),
                detail: format!("{dw}×{dh} · {}", rate_label(fps)),
                note: "Smoother editing and smaller files. The video is scaled down.",
            });
        }
    }
    v.push(Choice {
        action: Action::KeepProject,
        icon: ph::LOCK_SIMPLE,
        title: "Keep project settings".into(),
        detail: format!(
            "{} · {}",
            size_label(s.width, s.height),
            rate_label(s.frame_rate)
        ),
        note: "The video is scaled to fit the project.",
    });
    v
}

/// What "Remember my choice" saves for an answer, if it can be saved.
fn remembered(action: Action) -> Option<ImportSize> {
    match action {
        Action::MatchVideo => Some(ImportSize::MatchMedia),
        Action::KeepProject => Some(ImportSize::KeepProject),
        Action::Downscale(..) => Some(ImportSize::Downscale1080),
    }
}

impl ImportSizeDialog {
    pub fn new(media: MediaId) -> Self {
        ImportSizeDialog {
            media,
            choice: 0,
            remember: false,
        }
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let Some(m) = app.project.media(self.media).cloned() else {
            return false;
        };
        let settings = app.project.settings.clone();
        let options = choices(&m, &settings);
        self.choice = self.choice.min(options.len() - 1);
        let fps = m.frame_rate.unwrap_or(settings.frame_rate);
        let mut apply = false;
        let mut close = false;
        let (_, keys) = window(ctx, "import-size", 540.0, |ui| {
            close |= header(
                ui,
                ph::FILM_STRIP,
                "This video doesn't match your project",
                &m.name,
            );
            body(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(RichText::new("The video is ").color(theme::TEXT_DIM));
                    ui.label(
                        RichText::new(format!("{}×{} at {}", m.width, m.height, rate_label(fps)))
                            .color(theme::TEXT),
                    );
                    ui.label(RichText::new(". The project is ").color(theme::TEXT_DIM));
                    ui.label(
                        RichText::new(format!(
                            "{}×{} at {}",
                            settings.width,
                            settings.height,
                            rate_label(settings.frame_rate)
                        ))
                        .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(". Which size should the project use?")
                            .color(theme::TEXT_DIM),
                    );
                });
                ui.add_space(6.0);
                for (i, c) in options.iter().enumerate() {
                    let r = widgets::option_card(
                        ui,
                        self.choice == i,
                        c.icon,
                        &c.title,
                        &c.detail,
                        c.note,
                    );
                    if r.clicked() {
                        self.choice = i;
                    }
                    if r.double_clicked() {
                        apply = true;
                    }
                }
                ui.add_space(2.0);
                widgets::hint(
                    ui,
                    "You can change the size later in File › Project Settings.",
                );
            });
            let can_remember = remembered(options[self.choice].action).is_some();
            footer(
                ui,
                |ui| {
                    ui.add_enabled_ui(can_remember, |ui| {
                        let mut on = self.remember && can_remember;
                        if switch(ui, &mut on, "Remember my choice") {
                            self.remember = on;
                        }
                    })
                    .response
                    .on_hover_text("Don't ask again; change it in Preferences › General");
                },
                |ui| {
                    if primary(ui, "Apply", true).clicked() {
                        apply = true;
                    }
                },
            );
        });
        if keys.enter {
            apply = true;
        }
        if apply {
            let action = options[self.choice].action;
            match action {
                Action::MatchVideo => app.match_project_to(self.media),
                Action::Downscale(w, h) => {
                    let f: FrameRate = fps;
                    app.edit(|p| {
                        p.settings.width = w;
                        p.settings.height = h;
                        p.settings.frame_rate = f;
                        true
                    });
                    app.notify(
                        &format!("Project set to {w}×{h} at {}.", rate_label(f)),
                        false,
                    );
                }
                Action::KeepProject => {}
            }
            if self.remember {
                if let Some(r) = remembered(action) {
                    app.prefs.import_size = r;
                    app.prefs.save();
                }
            }
            return false;
        }
        !(close || keys.escape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tidycuts_core::{MediaKind, Time};

    fn video(w: u32, h: u32, fps: FrameRate) -> MediaItem {
        let mut m = MediaItem::new(
            PathBuf::from("/v.mp4"),
            MediaKind::Video,
            Time::from_secs_f64(3.0),
        );
        m.width = w;
        m.height = h;
        m.frame_rate = Some(fps);
        m
    }

    #[test]
    fn a_4k_video_offers_original_1080p_and_project() {
        let s = ProjectSettings::default();
        let c = choices(&video(3840, 2160, FrameRate::new(30, 1)), &s);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].action, Action::MatchVideo);
        assert_eq!(c[0].detail, "3840×2160 · 4K · 30 fps");
        assert_eq!(c[1].action, Action::Downscale(1920, 1080));
        assert_eq!(c[2].action, Action::KeepProject);
    }

    #[test]
    fn vertical_video_downscales_to_vertical_1080p() {
        let s = ProjectSettings::default();
        let c = choices(&video(2160, 3840, FrameRate::new(30, 1)), &s);
        assert_eq!(c[1].action, Action::Downscale(1080, 1920));
    }

    #[test]
    fn no_downscale_when_it_equals_the_project_or_video_is_small() {
        let s = ProjectSettings::default();
        // 4K at the project's frame rate: 1080p would be the project itself.
        let c = choices(&video(3840, 2160, s.frame_rate), &s);
        assert_eq!(c.len(), 2);
        let c = choices(&video(1280, 720, FrameRate::new(30, 1)), &s);
        assert_eq!(c.len(), 2);
        assert!(c.iter().all(|c| !matches!(c.action, Action::Downscale(..))));
    }

    #[test]
    fn every_answer_can_be_remembered() {
        assert_eq!(remembered(Action::MatchVideo), Some(ImportSize::MatchMedia));
        assert_eq!(
            remembered(Action::KeepProject),
            Some(ImportSize::KeepProject)
        );
        assert_eq!(
            remembered(Action::Downscale(1920, 1080)),
            Some(ImportSize::Downscale1080)
        );
    }
}
