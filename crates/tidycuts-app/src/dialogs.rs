//! Windows over the editor: project settings, preferences, the import size
//! question, crop, export and about.
//!
//! Every window is an [`egui::Modal`] with the same chrome: a title bar
//! with an icon and a close button, a body and a footer with the buttons.
//! Esc cancels the top window and Enter confirms it (when no text field
//! was being typed in).

mod about;
mod captions;
mod crop;
mod export;
mod export_preview;
pub use export::job_fraction;
mod imagegen;
mod import_size;
mod loudness;
mod preferences;
mod project_settings;
mod proxy;
mod silence;
pub mod sizes;
mod thumbnail;
mod tour;
mod updates;
mod voiceover;

use crate::app::TidyCutsApp;
use crate::prefs::Prefs;
use crate::{theme, widgets};
use egui::{
    Align, Color32, CornerRadius, Id, Key, Layout, Margin, Modifiers, Response, RichText, Sense,
    Stroke, Ui,
};
use egui_phosphor::regular as ph;
use std::path::Path;
use tidycuts_core::{ClipId, MediaId, Project, Time};

#[derive(Default)]
pub struct DialogState {
    import_size: Option<import_size::ImportSizeDialog>,
    project_settings: Option<project_settings::ProjectSettingsDialog>,
    preferences: Option<preferences::PreferencesDialog>,
    export: Option<export::ExportDialog>,
    progress: Option<export::ProgressDialog>,
    crop: Option<crop::CropDialog>,
    about: Option<about::AboutDialog>,
    silence: Option<silence::SilenceDialog>,
    tour: Option<tour::TourDialog>,
    updates: Option<updates::UpdatesDialog>,
    captions: Option<captions::CaptionsDialog>,
    loudness: Option<loudness::LoudnessDialog>,
    voiceover: Option<voiceover::VoiceoverDialog>,
    thumbnail: Option<thumbnail::ThumbnailDialog>,
    proxy: Option<proxy::ProxyDialog>,
    imagegen: Option<imagegen::ImageGenDialog>,
    /// Settings of the last export, so a failed one can be retried.
    last_export: Option<export::ExportDialog>,
    /// Start time of the export whose end was already announced.
    announced: Option<std::time::Instant>,
}

impl DialogState {
    /// Asks whether the project should take this video's size.
    pub fn ask_import_size(&mut self, media: MediaId) {
        self.import_size = Some(import_size::ImportSizeDialog::new(media));
    }

    pub fn open_project_settings(&mut self, project: &Project) {
        self.project_settings = Some(project_settings::ProjectSettingsDialog::new(project));
    }

    pub fn open_preferences(&mut self, prefs: &Prefs) {
        self.preferences = Some(preferences::PreferencesDialog::new(prefs));
    }

    pub fn open_export(&mut self, project: &Project, prefs: &Prefs, project_path: Option<&Path>) {
        self.export = Some(export::ExportDialog::new(project, prefs, project_path));
    }

    /// Shows the progress of the running export again.
    pub fn show_export_progress(&mut self) {
        self.export = None;
        if self.progress.is_none() {
            self.progress = Some(export::ProgressDialog::default());
        }
    }

    /// Limits the open export window to a part of the timeline.
    pub fn set_export_range(&mut self, range: (Time, Time)) {
        if let Some(e) = &mut self.export {
            e.set_selected_range(range);
        }
    }

    pub fn open_crop(&mut self, clip: ClipId) {
        self.crop = Some(crop::CropDialog::new(clip));
    }

    pub fn open_voiceover(&mut self, ctx: &egui::Context) {
        self.voiceover = Some(voiceover::VoiceoverDialog::new(ctx));
    }

    pub fn open_silence(&mut self, clip: ClipId) {
        self.silence = Some(silence::SilenceDialog::new(clip));
    }

    /// Opens the Auto Captions window for a clip.
    pub fn open_captions(&mut self, clip: ClipId) {
        self.captions = Some(captions::CaptionsDialog::new(clip));
    }

    /// Opens the welcome tour.
    pub fn open_tour(&mut self) {
        self.tour = Some(tour::TourDialog::default());
    }

    /// Asks whether to make a smooth-editing copy of a video.
    pub fn ask_proxy(&mut self, media: MediaId) {
        self.proxy = Some(proxy::ProxyDialog::new(media));
    }

    pub fn open_about(&mut self) {
        self.about = Some(about::AboutDialog);
    }

    /// Whether a window that should block editing shortcuts is open.
    pub fn is_modal_open(&self) -> bool {
        self.import_size.is_some()
            || self.project_settings.is_some()
            || self.preferences.is_some()
            || self.export.is_some()
            || self.progress.is_some()
            || self.crop.is_some()
            || self.about.is_some()
            || self.silence.is_some()
            || self.tour.is_some()
            || self.updates.is_some()
            || self.captions.is_some()
            || self.loudness.is_some()
            || self.voiceover.is_some()
            || self.thumbnail.is_some()
            || self.proxy.is_some()
            || self.imagegen.is_some()
    }
}

/// Opens the Loudness window and starts measuring the timeline.
pub fn open_loudness(app: &mut TidyCutsApp, ctx: &egui::Context) {
    let d = loudness::LoudnessDialog::new(app, ctx);
    app.dialogs.loudness = Some(d);
}

/// Opens Check for Updates (this is the only time TidyCuts asks the internet).
pub fn open_updates(app: &mut TidyCutsApp, ctx: &egui::Context) {
    app.dialogs.updates = Some(updates::UpdatesDialog::new(ctx));
}

/// Whether this copy updates itself (the Microsoft Store version does).
pub fn updates_come_from_store() -> bool {
    updates::is_store_build()
}

/// Opens the thumbnail maker.
pub fn open_thumbnail(app: &mut TidyCutsApp) {
    let d = thumbnail::ThumbnailDialog::new(app);
    app.dialogs.thumbnail = Some(d);
}

/// Opens the Image Thumbnail Generator (`models`: on the page that chooses,
/// downloads and deletes the models).
pub fn open_imagegen(app: &mut TidyCutsApp, models: bool) {
    let d = imagegen::ImageGenDialog::new(app, models);
    app.dialogs.imagegen = Some(d);
}

/// Draws whichever windows are open. Called once per frame.
pub fn show(app: &mut TidyCutsApp, ctx: &egui::Context) {
    export::watch_job(app);

    // Each window is taken out while it draws so it can change the app;
    // it goes back unless it closed (or the same window was reopened).
    macro_rules! run {
        ($field:ident) => {
            if let Some(mut d) = app.dialogs.$field.take() {
                if d.show(app, ctx) && app.dialogs.$field.is_none() {
                    app.dialogs.$field = Some(d);
                }
            }
        };
    }
    run!(project_settings);
    run!(preferences);
    run!(export);
    run!(progress);
    run!(crop);
    run!(about);
    run!(silence);
    run!(tour);
    run!(updates);
    run!(captions);
    run!(loudness);
    run!(voiceover);
    run!(thumbnail);
    run!(proxy);
    run!(imagegen);
    // The import question goes on top of everything else.
    run!(import_size);
}

/// Keys the top window reacts to this frame.
#[derive(Debug, Clone, Copy, Default)]
struct Keys {
    /// Esc: cancel and close.
    escape: bool,
    /// Enter: confirm.
    enter: bool,
}

/// Shows a dialog window of `width` points over the editor.
fn window<R>(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> (R, Keys) {
    // A text field (or number box) being typed in takes Enter for itself.
    let typing = ctx.memory(|m| m.focused().is_some());
    let resp = egui::Modal::new(Id::new(("tidycuts-dialog", id)))
        .backdrop_color(Color32::from_black_alpha(150))
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL_BG)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(1))
                .shadow(egui::Shadow {
                    offset: [0, 12],
                    blur: 48,
                    spread: 0,
                    color: Color32::from_black_alpha(150),
                }),
        )
        .show(ctx, |ui| {
            // Never wider than the app window.
            ui.set_width(width.min(ctx.content_rect().width() - DIALOG_MARGIN));
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            add(ui)
        });
    let free = resp.is_top_modal && !resp.any_popup_open;
    let escape = free && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    let enter = free && !typing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    (resp.inner, Keys { escape, enter })
}

/// The title bar: an icon badge, the title, an optional subtitle and a
/// close button. Returns true when the close button was clicked.
fn header(ui: &mut Ui, icon: &str, title: &str, subtitle: &str) -> bool {
    let mut close = false;
    egui::Frame::new()
        .inner_margin(Margin {
            left: 20,
            right: 14,
            top: 16,
            bottom: 14,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (badge, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::hover());
                ui.painter()
                    .rect_filled(badge, CornerRadius::same(9), theme::ACCENT_BG);
                ui.painter().text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    icon,
                    egui::FontId::proportional(19.0),
                    theme::ACCENT,
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new(title)
                            .font(theme::heading_font(16.5))
                            .color(Color32::WHITE),
                    );
                    if !subtitle.is_empty() {
                        ui.label(RichText::new(subtitle).size(12.5).color(theme::TEXT_DIM));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if widgets::icon_button(ui, ph::X, "Close (Esc)").clicked() {
                        close = true;
                    }
                });
            });
        });
    rule(ui);
    close
}

/// Space a dialog's title bar and button bar take, plus its border.
const DIALOG_CHROME_H: f32 = 142.0;
/// Space kept free around a dialog at the edges of the app window.
const DIALOG_MARGIN: f32 = 48.0;

/// Scrolls the middle of a dialog when the app window is too short to show
/// all of it, so the title bar and the buttons stay in view.
fn scroll_middle<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let room = ui.ctx().content_rect().height() - DIALOG_MARGIN - DIALOG_CHROME_H;
    egui::ScrollArea::vertical()
        .max_height(room.max(160.0))
        .auto_shrink([false, true])
        .show(ui, add)
        .inner
}

/// A full-width line between parts of a window.
fn rule(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::BORDER));
}

/// The window's content, with roomy margins.
fn body<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(Margin::symmetric(22, 18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            // The parent may be a row (a sidebar beside the body).
            ui.with_layout(Layout::top_down(Align::Min), add).inner
        })
        .inner
}

/// The button bar at the bottom. `left` fills from the left; `right` is
/// laid out right to left, so add the main button first.
fn footer(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    rule(ui);
    egui::Frame::new()
        .fill(Color32::from_rgb(0x15, 0x17, 0x1a))
        .corner_radius(CornerRadius {
            nw: 0,
            ne: 0,
            sw: 11,
            se: 11,
        })
        .inner_margin(Margin::symmetric(20, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
            ui.horizontal(|ui| {
                left(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), right);
            });
        });
}

/// The main button of a window.
fn primary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    ui.add_enabled(
        enabled,
        theme::accent_button(text).min_size(egui::vec2(104.0, 34.0)),
    )
}

/// A secondary button (Cancel, Reset…).
fn secondary(ui: &mut Ui, text: &str) -> Response {
    ui.add(theme::outline_button(text).min_size(egui::vec2(92.0, 34.0)))
}

/// A row with a label column, as used in all the dialogs.
fn field<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    widgets::labeled_row(ui, label, 132.0, add)
}

/// A switch with a label to its right. Returns true when it changed.
fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> bool {
    ui.horizontal(|ui| {
        let changed = widgets::toggle(ui, on).changed();
        let r = ui.add(egui::Label::new(label).sense(Sense::click()));
        if r.clicked() {
            *on = !*on;
            return true;
        }
        changed
    })
    .inner
}

/// Opens the system file browser at a file (selected where the system can)
/// or a folder.
fn reveal(path: &Path) -> std::io::Result<()> {
    use std::process::Command;
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("explorer");
        if path.is_file() {
            c.arg(format!("/select,{}", path.display()));
        } else {
            c.arg(path);
        }
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        if path.is_file() {
            c.arg("-R");
        }
        c.arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        let dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        c.arg(dir);
        c
    };
    cmd.spawn().map(|_| ())
}
