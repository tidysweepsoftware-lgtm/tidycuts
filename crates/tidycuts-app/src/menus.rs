//! The menu bar: File, Edit, Clip, View and Help, laid out the usual way.

use crate::app::TidyCutsApp;
use crate::theme;
use egui::{Button, RichText, Ui};
use tidycuts_core::{Edge, Generator, TrackKind};

fn cmd() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    }
}

fn item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let clicked = ui
        .add_enabled(enabled, Button::new(label).shortcut_text(shortcut))
        .clicked();
    if clicked {
        ui.close();
    }
    clicked
}

pub fn menus(app: &mut TidyCutsApp, ui: &mut Ui) {
    let c = cmd();
    let has_sel = !app.selection.is_empty();
    ui.menu_button(
        egui::RichText::new("File").font(theme::medium_font(13.5)),
        |ui| {
            ui.set_min_width(250.0);
            if item(ui, "New Project", &format!("{c}N"), true) {
                app.request_new();
            }
            if item(ui, "Open Project…", &format!("{c}O"), true) {
                app.request_open(None);
            }
            let recent = app.prefs.recent_projects.clone();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Open Recent", |ui| {
                    for p in recent {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if ui
                            .button(name)
                            .on_hover_text(p.display().to_string())
                            .clicked()
                        {
                            app.request_open(Some(p));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Clear list").clicked() {
                        app.prefs.recent_projects.clear();
                        app.prefs.save();
                        ui.close();
                    }
                });
            });
            ui.separator();
            if item(ui, "Save Project", &format!("{c}S"), true) {
                app.save();
            }
            if item(ui, "Save Project As…", &format!("{c}Shift+S"), true) {
                app.save_as();
            }
            ui.separator();
            if item(ui, "Project Settings…", &format!("{c}Shift+,"), true) {
                app.dialogs.open_project_settings(&app.project);
            }
            if item(ui, "Preferences…", &format!("{c},"), true) {
                app.dialogs.open_preferences(&app.prefs);
            }
            ui.separator();
            if item(ui, "Import Media…", &format!("{c}I"), true) {
                app.import_dialog();
            }
            if item(ui, "Export…", &format!("{c}E"), true) {
                app.dialogs
                    .open_export(&app.project, &app.prefs, app.project_path.as_deref());
            }
            ui.separator();
            if item(ui, "Exit", "", true) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        },
    );
    ui.menu_button(
        egui::RichText::new("Edit").font(theme::medium_font(13.5)),
        |ui| {
            ui.set_min_width(250.0);
            if item(ui, "Undo", &format!("{c}Z"), app.can_undo()) {
                app.undo();
            }
            if item(ui, "Redo", &format!("{c}Y"), app.can_redo()) {
                app.redo();
            }
            ui.separator();
            if item(ui, "Cut", &format!("{c}X"), has_sel) {
                app.cut_selection();
            }
            if item(ui, "Copy", &format!("{c}C"), has_sel) {
                app.copy_selection();
            }
            if item(ui, "Paste", &format!("{c}V"), !app.clipboard.is_empty()) {
                app.paste();
            }
            if item(ui, "Duplicate", &format!("{c}D"), has_sel) {
                app.duplicate_selection();
            }
            if item(ui, "Delete", "Del", has_sel) {
                app.delete_selection(false);
            }
            if item(ui, "Ripple Delete", "Shift+Del", has_sel) {
                app.delete_selection(true);
            }
            ui.separator();
            if item(ui, "Select All", &format!("{c}A"), true) {
                app.select_all();
            }
            ui.separator();
            if item(ui, "Split", &format!("{c}B"), true) {
                app.split_at_playhead();
            }
            if item(ui, "Trim Start to Playhead", "Alt+[", true) {
                app.trim_to_playhead(Edge::Start);
            }
            if item(ui, "Trim End to Playhead", "Alt+]", true) {
                app.trim_to_playhead(Edge::End);
            }
            if item(ui, "Detach Audio", &format!("{c}Alt+D"), has_sel) {
                app.detach_audio();
            }
            if item(ui, "Freeze Frame", &format!("{c}Alt+F"), has_sel) {
                app.freeze_frame();
            }
            ui.separator();
            if item(ui, "Add Marker", "M", true) {
                app.add_marker_at_playhead();
            }
            if item(
                ui,
                "Previous Marker",
                "Alt+Left",
                !app.project.markers.is_empty(),
            ) {
                app.jump_to_marker(false);
            }
            if item(
                ui,
                "Next Marker",
                "Alt+Right",
                !app.project.markers.is_empty(),
            ) {
                app.jump_to_marker(true);
            }
            if item(ui, "Take Snapshot", "", true) {
                app.take_snapshot();
            }
            if item(ui, "Image Thumbnail Generator…", "", true) {
                crate::dialogs::open_imagegen(app, false);
            }
            if item(ui, "Make Thumbnail…", "", true) {
                crate::dialogs::open_thumbnail(app);
            }
        },
    );
    ui.menu_button(
        egui::RichText::new("Clip").font(theme::medium_font(13.5)),
        |ui| {
            ui.set_min_width(250.0);
            if item(ui, "Properties", "Double-click", has_sel) {
                app.open_inspector();
            }
            if item(ui, "Speed…", &format!("{c}R"), has_sel) {
                app.inspector.request_tab("Speed");
                app.open_inspector();
            }
            if item(ui, "Crop and Zoom…", "Alt+C", has_sel) {
                if let Some(id) = app.selected_clip() {
                    app.dialogs.open_crop(id);
                }
            }
            if item(ui, "Crop to Fit", "", has_sel) {
                app.crop_selection_to_fit();
            }
            if item(ui, "Rotate 90° Clockwise", "", has_sel) {
                app.rotate_selection_90();
            }
            if item(ui, "Reverse", "", has_sel) {
                app.toggle_reverse();
            }
            if item(ui, "Mute", "", has_sel) {
                app.toggle_mute();
            }
            if item(ui, "Enhance Speech (clean up a voice)", "", has_sel) {
                app.for_selected(|c| tidycuts_engine::audio_fx::enhance_speech(&mut c.audio));
                app.notify(
                    "Enhance Speech is on. Open the Audio tab to adjust it.",
                    false,
                );
            }
            if item(ui, "Disable Clip", "", has_sel) {
                app.toggle_disable();
            }
            ui.separator();
            {
                use crate::detection::Detection;
                let clip = app.selected_clip();
                let ok = |app: &TidyCutsApp, kind| {
                    clip.is_some_and(|c| crate::detection::unsuitable(app, c, kind).is_none())
                };
                let silent = Detection::Silences {
                    noise_db: -35.0,
                    min_secs: 0.5,
                };
                let scenes = Detection::Scenes { threshold: 0.3 };
                if item(ui, "Silence Detection…", "", ok(app, silent)) {
                    if let Some(id) = clip {
                        app.dialogs.open_silence(id);
                    }
                }
                if item(ui, "Split at Scene Changes", "", ok(app, scenes)) {
                    if let Some(id) = clip {
                        app.start_detection(id, scenes);
                    }
                }
                if item(ui, "Add Beat Markers", "", ok(app, Detection::Beats)) {
                    if let Some(id) = clip {
                        app.start_detection(id, Detection::Beats);
                    }
                }
                let can_caption =
                    clip.is_some_and(|c| crate::captions::unsuitable(app, c).is_none());
                if item(ui, "Auto Captions…", "", can_caption) {
                    app.open_auto_captions();
                }
                let can_duck = clip.is_some_and(|c| crate::autoduck::unsuitable(app, c).is_none());
                if item(ui, "Lower Music Under Speech", "", can_duck) {
                    if let Some(id) = clip {
                        app.start_auto_duck(id, crate::autoduck::LOWER_DB);
                    }
                }
                if item(ui, "Line Up by Sound", "", app.selection.len() >= 2) {
                    let ids = app.selection.clone();
                    app.start_sync(&ids);
                }
            }
            ui.separator();
            if item(ui, "Add Title", &format!("{c}T"), true) {
                app.add_title("", None);
            }
            if item(ui, "Add Voiceover…", "", true) {
                let ctx = ui.ctx().clone();
                app.dialogs.open_voiceover(&ctx);
            }
            if item(ui, "Add Color Background", "", true) {
                app.add_solid(Generator::Solid([0.1, 0.12, 0.16, 1.0]), None);
            }
            ui.separator();
            if item(
                ui,
                "Add Video Track",
                "",
                app.project.can_add_track(TrackKind::Video),
            ) {
                app.add_track(TrackKind::Video, None);
            }
            if item(
                ui,
                "Add Audio Track",
                "",
                app.project.can_add_track(TrackKind::Audio),
            ) {
                app.add_track(TrackKind::Audio, None);
            }
        },
    );
    ui.menu_button(
        egui::RichText::new("View").font(theme::medium_font(13.5)),
        |ui| {
            ui.set_min_width(250.0);
            if item(ui, "Full Screen Preview", "F", true) {
                app.preview.toggle_fullscreen(ui.ctx());
            }
            if item(ui, "Audio Mixer", "", true) {
                app.mixer.open = !app.mixer.open;
            }
            if item(ui, "Loudness (how loud is the whole video)…", "", true) {
                crate::dialogs::open_loudness(app, ui.ctx());
            }
            if item(ui, "Scopes (waveform, vectorscope…)", "", true) {
                app.scopes.open = !app.scopes.open;
            }
            ui.checkbox(&mut app.preview.show_grid, "Preview Grid (rule of thirds)");
            ui.checkbox(
                &mut app.preview.show_safe_zones,
                "Title and Action Safe Zones",
            );
            if app.previews.rendering() {
                if item(ui, "Stop Rendering Previews", "", true) {
                    app.cancel_previews();
                }
            } else if item(ui, "Render Preview", "", true) {
                app.render_previews();
            }
            if item(
                ui,
                "Delete Preview Files",
                "",
                app.previews.disk_bytes > 0 || app.previews.ready_count() > 0,
            ) {
                app.delete_previews();
            }
            ui.separator();
            if item(ui, "Zoom In Timeline", &format!("{c}="), true) {
                app.timeline.zoom_by(1.5);
            }
            if item(ui, "Zoom Out Timeline", &format!("{c}-"), true) {
                app.timeline.zoom_by(1.0 / 1.5);
            }
            if item(ui, "Zoom to Fit", "Shift+Z", true) {
                app.timeline.fit_requested = true;
            }
            ui.separator();
            let mut snap = app.timeline.snap;
            if ui.checkbox(&mut snap, "Snapping  (N)").changed() {
                app.timeline.snap = snap;
                app.prefs.snapping = snap;
            }
            let mut magnetic = app.project.settings.magnetic;
            if ui.checkbox(&mut magnetic, "Magnetic Main Track").changed() {
                app.set_magnetic(magnetic);
            }
            ui.separator();
            ui.menu_button("Layout", |ui| {
                ui.set_min_width(300.0);
                for preset in crate::prefs::LayoutPreset::ALL {
                    let on = app.prefs.layout_preset == preset;
                    let label = format!("{}    {}", preset.label(), preset.hint());
                    if ui.radio(on, label).clicked() {
                        app.prefs.layout_preset = preset;
                        app.left_shows_props = false;
                        app.prefs.save();
                        ui.close();
                    }
                }
                ui.separator();
                if app.prefs.layout_preset.has_dock() {
                    let mut open = app.inspector_open;
                    if ui.checkbox(&mut open, "Properties panel").changed() {
                        app.inspector_open = open;
                    }
                }
                if item(ui, "Reset Layout", "", true) {
                    app.prefs.reset_layout();
                    app.prefs.save();
                }
            });
        },
    );
    ui.menu_button(
        egui::RichText::new("Help").font(theme::medium_font(13.5)),
        |ui| {
            ui.set_min_width(320.0);
            ui.label(RichText::new("Keyboard shortcuts").font(theme::heading_font(13.5)));
            ui.add_space(4.0);
            for (k, v) in [
                ("Space".to_string(), "Play / pause"),
                ("← / →".into(), "Previous / next frame"),
                ("Shift+← / →".into(), "Back / forward one second"),
                ("↑ / ↓".into(), "Previous / next cut"),
                (format!("{c}B"), "Split at the playhead"),
                ("Del / Shift+Del".into(), "Delete / ripple delete"),
                (format!("{c}Alt+D"), "Detach audio"),
                ("Alt+[ / Alt+]".into(), "Trim start / end to playhead"),
                (format!("{c}C / X / V"), "Copy / cut / paste"),
                (format!("{c}D"), "Duplicate"),
                (format!("{c}Z / {c}Y"), "Undo / redo"),
                (format!("{c}T"), "Add a title"),
                (format!("{c}R"), "Speed"),
                ("F".into(), "Full screen preview"),
                ("N".into(), "Snapping on or off"),
                (format!("{c}+wheel"), "Zoom the timeline"),
                ("Drag the ruler".into(), "Zoom the timeline"),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [140.0, 18.0],
                        egui::Label::new(RichText::new(k).color(theme::ACCENT)),
                    );
                    ui.label(v);
                });
            }
            ui.separator();
            if ui.button("Keyboard Shortcuts").clicked() {
                app.dialogs.open_shortcuts();
                ui.close();
            }
            if ui.button("Take the Tour").clicked() {
                app.dialogs.open_tour();
                ui.close();
            }
            if !crate::dialogs::updates_come_from_store()
                && ui.button("Check for Updates…").clicked()
            {
                crate::dialogs::open_updates(app, ui.ctx());
                ui.close();
            }
            if ui.button("About TidyCuts").clicked() {
                app.dialogs.open_about();
                ui.close();
            }
        },
    );
}
