//! Preferences (Ctrl+,): import behaviour, editing defaults, performance
//! and the preview cache. Changes are kept in a draft until OK.

use super::{body, field, footer, header, primary, secondary, switch, window};
use crate::app::TidyCutsApp;
use crate::prefs::{self, ImportSize, Prefs, PreviewQuality};
use crate::{theme, widgets};
use egui::{Margin, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tidycuts_core::Project;
use tidycuts_engine::export::{ExportSettings, VideoCodec};

const PAGES: [(&str, &str); 6] = [
    (ph::GEAR_SIX, "General"),
    (ph::SCISSORS, "Editing"),
    (ph::LIGHTNING, "Performance"),
    (ph::HARD_DRIVES, "Storage"),
    (ph::SPARKLE, "Image generator"),
    (ph::ROBOT, "AI assistants"),
];

/// Autosave choices in minutes; 0 is off.
const AUTOSAVE: [u32; 7] = [0, 1, 2, 5, 10, 15, 30];

pub struct PreferencesDialog {
    draft: Prefs,
    /// Bytes the preview cache takes on disk, once measured.
    usage: Arc<Mutex<Option<u64>>>,
    measuring: bool,
}

/// Remembers the last page shown for the rest of the session.
fn page_id() -> egui::Id {
    egui::Id::new("tidycuts-prefs-page")
}

/// Content shifted right to line up with the text of a switch above it.
fn under_switch(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .inner_margin(Margin {
            left: 42,
            right: 0,
            top: 0,
            bottom: 4,
        })
        .show(ui, add);
}

fn folder_text(path: &Option<PathBuf>, automatic: &str) -> String {
    match path {
        Some(p) => p.display().to_string(),
        None => automatic.to_string(),
    }
}

fn pick_folder(title: &str, start: Option<&PathBuf>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title);
    if let Some(s) = start.filter(|s| s.is_dir()) {
        d = d.set_directory(s);
    }
    d.pick_folder()
}

impl PreferencesDialog {
    pub fn new(prefs: &Prefs) -> Self {
        PreferencesDialog {
            draft: prefs.clone(),
            usage: Arc::new(Mutex::new(None)),
            measuring: false,
        }
    }

    /// Measures (and with `clear`, first trims) the preview cache on a
    /// worker thread.
    fn measure(&mut self, app: &TidyCutsApp, ctx: &egui::Context, clear: bool) {
        let Some(e) = &app.engine else {
            return;
        };
        let scrub = e.scrub.clone();
        let keep = app.project.media.clone();
        let slot = self.usage.clone();
        let ctx = ctx.clone();
        *slot.lock().unwrap() = None;
        self.measuring = true;
        std::thread::spawn(move || {
            if clear {
                scrub.clear_unused(&keep);
            }
            let n = scrub.disk_usage();
            *slot.lock().unwrap() = Some(n);
            ctx.request_repaint();
        });
    }

    fn general(&mut self, ui: &mut egui::Ui) {
        let d = &mut self.draft;
        widgets::group_title(ui, "Importing");
        ui.label("When imported video doesn't match the project's size or frame rate:");
        egui::Frame::new()
            .inner_margin(Margin {
                left: 12,
                right: 0,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                for v in ImportSize::ALL {
                    ui.radio_value(&mut d.import_size, v, v.label());
                }
            });
        ui.add_space(8.0);
        widgets::group_title(ui, "Saving and exporting");
        field(ui, "Autosave", |ui| {
            let mut options: Vec<(u32, String)> = AUTOSAVE
                .iter()
                .map(|m| {
                    let l = match m {
                        0 => "Off".to_string(),
                        1 => "Every minute".to_string(),
                        n => format!("Every {n} minutes"),
                    };
                    (*m, l)
                })
                .collect();
            if !AUTOSAVE.contains(&d.autosave_minutes) {
                options.push((
                    d.autosave_minutes,
                    format!("Every {} minutes", d.autosave_minutes),
                ));
            }
            let refs: Vec<(u32, &str)> = options.iter().map(|(m, l)| (*m, l.as_str())).collect();
            widgets::choice(ui, "pref-autosave", &mut d.autosave_minutes, &refs);
        });
        field(ui, "Export folder", |ui| {
            widgets::path_box(
                ui,
                &folder_text(&d.export_folder, "Automatic (the last folder used)"),
                250.0,
            );
            if ui.add(theme::outline_button("Browse…")).clicked() {
                if let Some(p) = pick_folder("Default export folder", d.export_folder.as_ref()) {
                    d.export_folder = Some(p);
                }
            }
            ui.add_enabled_ui(d.export_folder.is_some(), |ui| {
                if widgets::icon_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "Automatic").clicked() {
                    d.export_folder = None;
                }
            });
        });
        ui.add_space(8.0);
        widgets::group_title(ui, "Listening");
        {
            let labels = ["Off", "Normal (-10 LUFS)", "Careful (-16 LUFS)"];
            let mut i = usize::from(d.hearing_guard).min(2);
            ui.scope(|ui| {
                ui.set_max_width(460.0);
                if widgets::segmented(ui, &mut i, &labels) {
                    d.hearing_guard = i as u8;
                }
            });
            widgets::hint(
                ui,
                "Protects your hearing when you edit with headphones: while you play, the sound is turned down if it gets louder than this, and sudden peaks are held under -3 dB. It never changes an exported video.",
            );
        }
        ui.add_space(8.0);
        widgets::group_title(ui, "Timeline");
        switch(
            ui,
            &mut d.confirm_track_delete,
            "Ask before deleting a track that has clips on it",
        );
    }

    fn editing(&mut self, ui: &mut egui::Ui) {
        let d = &mut self.draft;
        let def = Prefs::default();
        widgets::group_title(ui, "Default durations");
        widgets::hint(
            ui,
            "How long new clips last when they are added to the timeline.",
        );
        let width = ui.available_width() - 28.0;
        ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
            widgets::slider_row(
                ui,
                "Images",
                &mut d.image_duration,
                0.5..=60.0,
                1,
                " s",
                def.image_duration,
            );
            widgets::slider_row(
                ui,
                "Titles",
                &mut d.title_duration,
                0.5..=60.0,
                1,
                " s",
                def.title_duration,
            );
            widgets::slider_row(
                ui,
                "Stickers",
                &mut d.sticker_duration,
                0.5..=60.0,
                1,
                " s",
                def.sticker_duration,
            );
            widgets::slider_row(
                ui,
                "Transitions",
                &mut d.transition_duration,
                0.1..=5.0,
                1,
                " s",
                def.transition_duration,
            );
            widgets::slider_row(
                ui,
                "Animations",
                &mut d.animation_duration,
                0.1..=3.0,
                1,
                " s",
                def.animation_duration,
            );
        });
        ui.add_space(8.0);
        widgets::group_title(ui, "Timeline");
        switch(
            ui,
            &mut d.snapping,
            "Snap clips to edges, markers and the playhead (N)",
        );
        switch(ui, &mut d.auto_scroll, "Follow the playhead while playing");
    }

    fn performance(&mut self, ui: &mut egui::Ui) {
        let d = &mut self.draft;
        widgets::group_title(ui, "Graphics card");
        switch(ui, &mut d.hardware_decode, "Hardware-accelerated decoding");
        under_switch(ui, |ui| {
            widgets::hint(
                ui,
                "Decodes video on the graphics card when it can. Turn this off if previews show glitches.",
            );
        });
        switch(ui, &mut d.hardware_encode, "Hardware-accelerated encoding");
        under_switch(ui, |ui| {
            widgets::hint(
                ui,
                "Exports with the graphics card's encoder (NVENC, Quick Sync, AMF or VideoToolbox) when there is one. Can be changed for each export.",
            );
        });
        ui.add_space(8.0);
        widgets::group_title(ui, "This computer");
        let ctx = ui.ctx().clone();
        field(ui, "Graphics card", |ui| {
            let text = match tidycuts_render::gpu::adapter() {
                Some(a) => format!("{} ({})", a.name, a.backend),
                None => "Not opened yet".to_string(),
            };
            ui.label(RichText::new(text).color(theme::TEXT));
        });
        let found = super::export::working_encoders(&ctx).is_some();
        for codec in [VideoCodec::H264, VideoCodec::Hevc, VideoCodec::Av1] {
            let mut settings = ExportSettings::for_project(&Project::new("x"), PathBuf::new());
            settings.codec = codec;
            settings.hardware_encode = d.hardware_encode;
            field(ui, codec.label(), |ui| {
                let text = if !found {
                    "Checking…".to_string()
                } else {
                    match tidycuts_engine::export::pick_encoder(&settings) {
                        Some(e) => super::export::encoder_label(&e),
                        None => "Not available".to_string(),
                    }
                };
                ui.label(RichText::new(text).color(theme::TEXT));
            });
        }
        under_switch(ui, |ui| {
            widgets::hint(
                ui,
                "The encoder each kind of video is exported with. The one on your graphics card is used first because it is fastest.",
            );
        });
        ui.add_space(8.0);
        widgets::group_title(ui, "Big videos");
        switch(ui, &mut d.use_proxies, "Edit with smaller copies (proxies)");
        under_switch(ui, |ui| {
            widgets::hint(
                ui,
                "A proxy is a small copy of a big video that plays and scrubs smoothly. Exports always use the original.",
            );
        });
        switch(
            ui,
            &mut d.proxy_ask,
            "Ask me when a large video is put on the timeline",
        );
        switch(
            ui,
            &mut d.auto_proxies,
            "When not asking, make proxies of large videos automatically",
        );
        ui.add_space(8.0);
        widgets::group_title(ui, "Preview");
        field(ui, "Preview quality", |ui| {
            let options: Vec<(PreviewQuality, &str)> = PreviewQuality::ALL
                .iter()
                .map(|q| (*q, q.label()))
                .collect();
            widgets::choice(ui, "pref-preview-q", &mut d.preview_quality, &options);
        });
        widgets::hint(
            ui,
            "Lower quality plays big projects more smoothly. Exports are always made at full quality.",
        );
    }

    /// Letting Claude connect: the switch, and what to give Claude.
    fn claude(&mut self, ui: &mut egui::Ui, app: &TidyCutsApp) {
        let d = &mut self.draft;
        widgets::group_title(ui, "AI assistants");
        switch(ui, &mut d.mcp_enabled, "Let an AI assistant edit with me");
        under_switch(ui, |ui| {
            widgets::hint(
                ui,
                "Claude or ChatGPT's Codex can then add clips, cut, change settings, look at the \
                 picture and export while you watch. Every change can be undone. Only programs on \
                 this computer that have the secret key below can connect, and nothing leaves your \
                 computer.",
            );
        });
        if !d.mcp_enabled {
            return;
        }
        if d.mcp_token.is_empty() {
            d.mcp_token = tidycuts_mcp::new_token();
        }
        ui.add_space(8.0);
        widgets::group_title(ui, "Connect Claude");
        let url = app.mcp.url().unwrap_or_else(|| {
            format!("http://127.0.0.1:{}{}", d.mcp_port, tidycuts_mcp::ENDPOINT)
        });
        widgets::hint(
            ui,
            "In a terminal, run this once. Claude Code then lists TidyCuts' tools.",
        );
        let mut command = format!(
            "claude mcp add --transport http tidycuts {url} --header \"Authorization: Bearer {}\"",
            d.mcp_token
        );
        ui.add(
            egui::TextEdit::multiline(&mut command)
                .font(egui::TextStyle::Monospace)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if secondary(ui, "Copy the command").clicked() {
            ui.ctx().copy_text(command.clone());
        }
        ui.add_space(8.0);
        widgets::group_title(ui, "Connect ChatGPT's Codex");
        widgets::hint(
            ui,
            "Codex runs on this computer. Add these lines to its settings file \
             (.codex/config.toml in your user folder), then start Codex again.",
        );
        let mut codex = format!(
            "[mcp_servers.tidycuts]\nurl = \"{url}\"\nhttp_headers = {{ Authorization = \"Bearer {}\" }}",
            d.mcp_token
        );
        ui.add(
            egui::TextEdit::multiline(&mut codex)
                .font(egui::TextStyle::Monospace)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if secondary(ui, "Copy the lines").clicked() {
                ui.ctx().copy_text(codex.clone());
            }
            if secondary(ui, "Make a new secret key")
                .on_hover_text("Programs using the old key stop working")
                .clicked()
            {
                d.mcp_token = tidycuts_mcp::new_token();
            }
        });
        widgets::hint(
            ui,
            "The ChatGPT website and phone app can't connect: they only reach servers on the \
             internet, and this one stays on your computer.",
        );
        if let Some(e) = &app.mcp.error {
            ui.colored_label(theme::DANGER, e);
        } else if app.mcp.url().is_some() && app.prefs.mcp_enabled {
            ui.label(
                RichText::new(format!("{}  Listening on {url}", ph::PLUGS_CONNECTED))
                    .color(theme::ACCENT),
            );
        } else {
            widgets::hint(ui, "Press OK to turn the connection on.");
        }
    }

    /// The Image Thumbnail Generator's models: what is downloaded, and a way
    /// to the window that downloads, switches and deletes them.
    fn image_models(&mut self, ui: &mut egui::Ui, open: &mut bool) {
        use tidycuts_engine::imagegen::{self, Quality};
        widgets::group_title(ui, "Image Thumbnail Generator");
        widgets::hint(
            ui,
            "Draws thumbnails from your idea, on this computer and for free. It needs model files that are downloaded once. You can keep both sizes and pick one each time you make a thumbnail.",
        );
        for q in Quality::ALL {
            field(ui, q.name(), |ui| {
                let status = if imagegen::installed(q) {
                    format!("{}  Downloaded", ph::CHECK_CIRCLE)
                } else {
                    format!("Not downloaded ({})", super::sizes::fmt_bytes(q.bytes()))
                };
                ui.label(RichText::new(status).color(theme::TEXT));
            });
        }
        ui.add_space(6.0);
        if ui
            .add(theme::outline_button(format!(
                "{}  Choose, download or delete models…",
                ph::SPARKLE
            )))
            .clicked()
        {
            *open = true;
        }
    }

    fn storage(&mut self, ui: &mut egui::Ui, app: &TidyCutsApp, ctx: &egui::Context) {
        let current_dir = app.prefs.cache_dir.clone();
        let d = &mut self.draft;
        widgets::group_title(ui, "Preview cache");
        widgets::hint(
            ui,
            "TidyCuts keeps small frames of your videos on disk so scrubbing and thumbnails are instant.",
        );
        field(ui, "Cache folder", |ui| {
            widgets::path_box(ui, &d.cache_dir().display().to_string(), 250.0);
            if ui.add(theme::outline_button("Browse…")).clicked() {
                if let Some(p) = pick_folder("Cache folder", Some(&d.cache_dir())) {
                    d.cache_dir = Some(p);
                }
            }
            ui.add_enabled_ui(d.cache_dir.is_some(), |ui| {
                if widgets::icon_button(
                    ui,
                    ph::ARROW_COUNTER_CLOCKWISE,
                    &format!(
                        "Use the system cache folder ({})",
                        prefs::default_cache_dir().display()
                    ),
                )
                .clicked()
                {
                    d.cache_dir = None;
                }
            });
        });
        if d.cache_dir != current_dir {
            ui.horizontal(|ui| {
                ui.add_space(132.0);
                ui.label(
                    RichText::new(format!("{}  Used after TidyCuts restarts", ph::INFO))
                        .size(12.0)
                        .color(theme::WARNING),
                );
            });
        }
        field(ui, "Size limit", |ui| {
            ui.scope(|ui| {
                ui.set_max_width(330.0);
                widgets::slider(ui, &mut d.cache_limit_gb, 1.0..=200.0, 0, " GB");
            });
        });
        let usage = *self.usage.lock().unwrap();
        let mut clear = false;
        field(ui, "In use", |ui| {
            let text = match (usage, app.engine.is_some()) {
                (_, false) => "Not available".to_string(),
                (Some(n), _) => super::sizes::fmt_bytes(n),
                (None, _) => "Measuring…".to_string(),
            };
            ui.label(RichText::new(text).color(theme::TEXT));
            ui.add_space(8.0);
            if ui
                .add_enabled(
                    app.engine.is_some() && usage.is_some(),
                    theme::outline_button(format!("{}  Clear cache", ph::BROOM)),
                )
                .on_hover_text("Deletes cached frames of media that isn't in this project")
                .clicked()
            {
                clear = true;
            }
        });
        if clear {
            self.measure(app, ctx, true);
        }
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        if !self.measuring {
            self.measure(app, ctx, false);
        }
        let mut page: usize = ctx.data(|d| d.get_temp(page_id())).unwrap_or(0);
        let mut ok = false;
        let mut cancel = false;
        let mut reset = false;
        let mut open_models = false;
        let (_, keys) = window(ctx, "preferences", 780.0, |ui| {
            cancel |= header(
                ui,
                ph::GEAR_SIX,
                "Preferences",
                "Settings that apply to every project",
            );
            let h = 440.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(12, 14))
                    .show(ui, |ui| {
                        ui.set_width(168.0);
                        ui.set_min_height(h - 28.0);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            for (i, (icon, label)) in PAGES.iter().enumerate() {
                                if widgets::list_item(ui, page == i, icon, label, "").clicked() {
                                    page = i;
                                }
                            }
                        });
                    });
                let (line, _) = ui.allocate_exact_size(egui::vec2(1.0, h), Sense::hover());
                ui.painter().vline(
                    line.center().x,
                    line.y_range(),
                    Stroke::new(1.0, theme::BORDER),
                );
                body(ui, |ui| {
                    ui.set_min_height(h - 36.0);
                    match page {
                        0 => self.general(ui),
                        1 => self.editing(ui),
                        2 => self.performance(ui),
                        3 => self.storage(ui, app, ctx),
                        4 => self.image_models(ui, &mut open_models),
                        _ => self.claude(ui, app),
                    }
                });
            });
            footer(
                ui,
                |ui| {
                    if secondary(ui, "Reset to defaults")
                        .on_hover_text(
                            "Recent projects, project defaults and the window layout are kept",
                        )
                        .clicked()
                    {
                        reset = true;
                    }
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
        ctx.data_mut(|d| d.insert_temp(page_id(), page));
        if open_models {
            super::open_imagegen(app, true);
        }
        if reset {
            let keep = &self.draft;
            self.draft = Prefs {
                project_defaults: keep.project_defaults.clone(),
                recent_projects: keep.recent_projects.clone(),
                layout: keep.layout,
                ..Prefs::default()
            };
        }
        if ok || keys.enter {
            self.apply(app, ctx);
            return false;
        }
        !(cancel || keys.escape)
    }

    fn apply(&self, app: &mut TidyCutsApp, ctx: &egui::Context) {
        let old = app.prefs.clone();
        // Things that change outside this window keep their current value.
        let new = Prefs {
            recent_projects: old.recent_projects.clone(),
            layout: old.layout,
            project_defaults: old.project_defaults.clone(),
            imagegen_models: old.imagegen_models.clone(),
            ..self.draft.clone()
        };
        app.timeline.snap = new.snapping;
        if new.hearing_guard != old.hearing_guard {
            if let Some(e) = &app.engine {
                e.set_hearing_guard(new.hearing_guard);
            }
        }
        if new.hardware_decode != old.hardware_decode {
            if let Some(e) = &mut app.engine {
                e.set_hardware_decoding(new.hardware_decode);
            }
        }
        if new.cache_dir != old.cache_dir {
            app.notify(
                "The new cache folder will be used after TidyCuts restarts.",
                false,
            );
        }
        if let Some(e) = &mut app.engine {
            e.invalidate();
        }
        app.prefs = new;
        app.prefs.save();
        app.sync_proxy_pref();
        app.mcp.sync(&app.prefs, ctx);
    }
}
