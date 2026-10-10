//! Top-left panel: imported media and the built-in sounds, music, titles,
//! transitions, effects, filters, stickers and templates, each draggable
//! onto the timeline.

use crate::app::TidyCutsApp;
use crate::theme;
use crate::widgets;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, Sense, Stroke, StrokeKind,
    UiBuilder, Vec2,
};
use egui_phosphor::regular as ph;
use tidycuts_core::{MediaId, MediaKind, Time};
use tidycuts_render::catalog::{categories, effects, filters, stickers, titles, transitions};
use tidycuts_sounds::{SoundDef, SoundKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Media,
    Audio,
    Titles,
    Transitions,
    Effects,
    Filters,
    Stickers,
    Layouts,
    Templates,
}

impl LibraryTab {
    pub const ALL: [LibraryTab; 9] = [
        LibraryTab::Media,
        LibraryTab::Audio,
        LibraryTab::Titles,
        LibraryTab::Transitions,
        LibraryTab::Effects,
        LibraryTab::Filters,
        LibraryTab::Stickers,
        LibraryTab::Layouts,
        LibraryTab::Templates,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LibraryTab::Media => "Media",
            LibraryTab::Audio => "Audio",
            LibraryTab::Titles => "Titles",
            LibraryTab::Transitions => "Transitions",
            LibraryTab::Effects => "Effects",
            LibraryTab::Filters => "Filters",
            LibraryTab::Stickers => "Stickers",
            LibraryTab::Layouts => "Layouts",
            LibraryTab::Templates => "Templates",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            LibraryTab::Media => ph::FOLDER,
            LibraryTab::Audio => ph::MUSIC_NOTES,
            LibraryTab::Titles => ph::TEXT_T,
            LibraryTab::Transitions => ph::SWAP,
            LibraryTab::Effects => ph::SPARKLE,
            LibraryTab::Filters => ph::CIRCLE_HALF,
            LibraryTab::Stickers => ph::STICKER,
            LibraryTab::Layouts => ph::SQUARES_FOUR,
            LibraryTab::Templates => ph::LAYOUT,
        }
    }
}

/// What can be dragged from the library onto the timeline.
#[derive(Debug, Clone)]
pub enum LibraryDrag {
    Media(MediaId),
    Title(&'static str),
    Transition(&'static str),
    /// An effect or filter id.
    Effect(&'static str),
    Sticker(&'static str),
    /// A split-screen layout, by id.
    Layout(&'static str),
    /// An audio visualizer style, by id.
    Visualizer(&'static str),
    /// A template, by index into [`TEMPLATES`].
    Template(usize),
    /// A built-in sound effect or music bed, by id.
    Sound(&'static str),
}

/// Ready-made backgrounds and multi-clip recipes (see `templates.rs`).
pub use crate::templates::TEMPLATES;

pub struct LibraryState {
    pub tab: LibraryTab,
    pub search: String,
    /// Selected category per tab; `None` shows everything.
    pub category: Option<&'static str>,
    pub tile_size: f32,
    /// Tiles per row on the catalog tabs.
    pub columns: u32,
}

impl Default for LibraryState {
    fn default() -> Self {
        LibraryState {
            tab: LibraryTab::Media,
            search: String::new(),
            category: None,
            tile_size: 149.0,
            columns: 3,
        }
    }
}

/// One entry of a catalog tab.
#[derive(Clone)]
pub struct CatalogItem {
    pub name: &'static str,
    pub category: &'static str,
    pub drag: LibraryDrag,
}

/// The entries of a catalog tab, in catalog order.
pub fn catalog(tab: LibraryTab) -> Vec<CatalogItem> {
    match tab {
        LibraryTab::Titles => titles::TITLES
            .iter()
            .map(|t| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Title(t.id),
            })
            .collect(),
        LibraryTab::Transitions => transitions::TRANSITIONS
            .iter()
            .map(|t| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Transition(t.id),
            })
            .collect(),
        LibraryTab::Effects => effects::EFFECTS
            .iter()
            .map(|t| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Effect(t.id),
            })
            .collect(),
        LibraryTab::Filters => filters::FILTERS
            .iter()
            .map(|t| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Effect(t.id),
            })
            .collect(),
        LibraryTab::Stickers => stickers::STICKERS
            .iter()
            .map(|t| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Sticker(t.id),
            })
            .collect(),
        LibraryTab::Layouts => tidycuts_core::LAYOUTS
            .iter()
            .map(|l| CatalogItem {
                name: l.name,
                category: l.category,
                drag: LibraryDrag::Layout(l.id),
            })
            .chain(
                tidycuts_core::VISUALIZER_STYLES
                    .iter()
                    .map(|(id, name)| CatalogItem {
                        name,
                        category: "Audio visualizers",
                        drag: LibraryDrag::Visualizer(id),
                    }),
            )
            .collect(),
        LibraryTab::Templates => TEMPLATES
            .iter()
            .enumerate()
            .map(|(i, t)| CatalogItem {
                name: t.name,
                category: t.category,
                drag: LibraryDrag::Template(i),
            })
            .collect(),
        LibraryTab::Media | LibraryTab::Audio => Vec::new(),
    }
}

/// Applies a library item the quick way (double-click or the + button):
/// media and titles go to the playhead, effects and transitions to the
/// selected clip.
pub fn apply(app: &mut TidyCutsApp, drag: &LibraryDrag) {
    match drag {
        LibraryDrag::Media(m) => app.add_media_to_timeline(*m, None),
        LibraryDrag::Title(id) => app.add_title(id, None),
        LibraryDrag::Sticker(id) => app.add_sticker(id, None),
        LibraryDrag::Layout(id) => app.add_layout(id, None),
        LibraryDrag::Visualizer(id) => app.add_visualizer(id, None),
        LibraryDrag::Template(i) => app.add_template(*i, None),
        LibraryDrag::Sound(id) => app.add_sound(id, None),
        LibraryDrag::Effect(id) => {
            let sel = app.selection.clone();
            app.apply_effect(id, &sel, None);
        }
        LibraryDrag::Transition(id) => match app.selected_clip() {
            Some(c) => app.set_transition(c, id),
            None => app.notify(
                "Select a clip, or drag the transition onto a cut between two clips.",
                false,
            ),
        },
    }
}

pub fn show(app: &mut TidyCutsApp, ui: &mut egui::Ui) {
    let full = ui.max_rect();
    // 44 pixels of tabs and a line under them.
    let tabs = Rect::from_min_size(full.min, egui::vec2(full.width(), 44.0));
    tab_bar(app, ui, tabs);
    let body = Rect::from_min_max(egui::pos2(full.left(), tabs.bottom() + 1.0), full.max);
    // Left-menus layout: a clip opened with a double-click shows its properties
    // here, under the same tabs; a tab brings the library back.
    if props_shown(app) {
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(body);
        crate::inspector::show(app, &mut child);
        return;
    }
    let side_w = 150.0_f32.min(body.width() * 0.3);
    let side = Rect::from_min_size(body.min, egui::vec2(side_w, body.height()));
    let main = Rect::from_min_max(egui::pos2(side.right(), body.top()), body.max);
    sidebar(app, ui, side);
    // Room around the tiles: 4 on the left, 14 on the top and right.
    let inner = Rect::from_min_max(
        main.min + egui::vec2(4.0, 14.0),
        egui::pos2(main.right() - 14.0, main.bottom()),
    );
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
    child.set_clip_rect(main);
    // Gaps below are set exactly, not added to the default spacing.
    child.spacing_mut().item_spacing.y = 0.0;
    match app.library.tab {
        LibraryTab::Media => media_tab(app, &mut child, None),
        LibraryTab::Audio => audio_tab(app, &mut child),
        tab => catalog_tab(app, &mut child, tab),
    }
}

/// Whether the properties of the selected clip are shown in place of the
/// library (the left-menus layout).
fn props_shown(app: &TidyCutsApp) -> bool {
    app.prefs.layout_preset == crate::prefs::LayoutPreset::LeftMenus
        && app.left_shows_props
        && app.selected_clip().is_some()
}

fn tab_bar(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::TAB_BG);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme::BORDER),
    );
    // Nine tabs that share the width equally, except that none is narrower
    // than its label plus 10 pixels each side (like a flexible row).
    let all = LibraryTab::ALL;
    let n = all.len();
    let min_w: Vec<f32> = all
        .iter()
        .map(|t| {
            ui.painter()
                .layout_no_wrap(t.label().into(), theme::tab_font(11.0), Color32::WHITE)
                .size()
                .x
                + 20.0
        })
        .collect();
    let total = rect.width() - 1.0 - 12.0;
    // Too narrow for the words: the tabs share the width and show icons only.
    let icons_only = min_w.iter().sum::<f32>() > total;
    let mut widths = vec![total / n as f32; n];
    let mut fixed = vec![false; n];
    // Give any tab that needs more than its share what it needs, and share the
    // rest between the others.
    loop {
        if icons_only {
            break;
        }
        let free: f32 = total
            - min_w
                .iter()
                .zip(&fixed)
                .filter(|(_, f)| **f)
                .map(|(m, _)| *m)
                .sum::<f32>();
        let loose = fixed.iter().filter(|f| !**f).count().max(1) as f32;
        let share = free / loose;
        let mut changed = false;
        for i in 0..n {
            if !fixed[i] && min_w[i] > share {
                fixed[i] = true;
                changed = true;
            }
        }
        if !changed {
            for i in 0..n {
                widths[i] = if fixed[i] { min_w[i] } else { share };
            }
            break;
        }
    }
    let mut x = rect.left() + 6.0;
    for (i, tab) in all.into_iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(widths[i], rect.height()),
        );
        x += widths[i];
        let resp = ui.interact(r, Id::new(("lib-tab", i)), Sense::click());
        let on = app.library.tab == tab && !props_shown(app);
        let color = if on {
            Color32::WHITE
        } else if resp.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        if on {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(r.left() + 8.0, r.bottom() - 2.0),
                    egui::pos2(r.right() - 8.0, r.bottom()),
                ),
                CornerRadius::ZERO,
                theme::ACCENT,
            );
        }
        if icons_only {
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                tab.icon(),
                FontId::proportional(19.0),
                color,
            );
        } else {
            ui.painter().text(
                egui::pos2(r.center().x, r.top() + 13.8),
                Align2::CENTER_CENTER,
                tab.icon(),
                FontId::proportional(17.0),
                color,
            );
            ui.painter().text(
                egui::pos2(r.center().x, r.top() + 32.0),
                Align2::CENTER_CENTER,
                tab.label(),
                theme::tab_font(11.0),
                color,
            );
        }
        let resp = if icons_only {
            resp.on_hover_text(tab.label())
        } else {
            resp
        };
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            app.left_shows_props = false;
            if app.library.tab != tab {
                app.library.tab = tab;
                app.library.category = None;
                app.library.search.clear();
            }
        }
    }
}

fn sidebar(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect) {
    // 14 pixels above and 10 at the sides; items are 130 wide.
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 14.0))));
    child.set_clip_rect(rect);
    let ui = &mut child;
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    let tab = app.library.tab;
    let cats: Vec<&'static str> = match tab {
        LibraryTab::Media => vec!["Project Media"],
        LibraryTab::Audio => Vec::new(),
        _ => categories(&catalog(tab), |c| c.category),
    };
    let entry = |ui: &mut egui::Ui, icon: Option<&str>, label: &str, on: bool| -> bool {
        let avail = ui.available_width();
        let inset = 10.0;
        let icon_w = if icon.is_some() { 13.5 + 8.0 } else { 0.0 };
        let color = if on { theme::TEXT } else { theme::TEXT_DIM };
        let galley = ui.painter().layout(
            label.to_string(),
            theme::medium_font(13.5),
            color,
            avail - 2.0 * inset - icon_w,
        );
        let h = galley.size().y.max(17.0) + 16.0;
        let (r, resp) = ui.allocate_exact_size(egui::vec2(avail, h), Sense::click());
        if on {
            ui.painter()
                .rect_filled(r, CornerRadius::same(9), theme::SEL);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(9), theme::HOVER);
        }
        if let Some(icon) = icon {
            ui.painter().text(
                egui::pos2(r.left() + inset + 6.75, r.center().y),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(13.5),
                color,
            );
        }
        ui.painter().galley(
            egui::pos2(
                r.left() + inset + icon_w,
                r.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
    };
    egui::ScrollArea::vertical()
        .id_salt("lib-cats")
        .show(ui, |ui| {
            if tab != LibraryTab::Media && entry(ui, None, "All", app.library.category.is_none()) {
                app.library.category = None;
            }
            if tab == LibraryTab::Audio {
                let groups = [
                    (None, vec![IMPORTED_AUDIO]),
                    (Some(MUSIC), sound_categories(SoundKind::Music)),
                    (Some(SOUND_EFFECTS), sound_categories(SoundKind::Effect)),
                ];
                for (head, items) in groups {
                    if let Some(h) = head {
                        ui.add_space(8.0);
                        if entry(ui, None, h, app.library.category == Some(h)) {
                            app.library.category = Some(h);
                        }
                    }
                    for c in items {
                        let label = if head.is_some() {
                            format!("    {c}")
                        } else {
                            c.to_string()
                        };
                        if entry(ui, None, &label, app.library.category == Some(c)) {
                            app.library.category = Some(c);
                        }
                    }
                }
                return;
            }
            for c in cats {
                let icon = (tab == LibraryTab::Media).then_some(ph::FOLDER);
                if entry(
                    ui,
                    icon,
                    c,
                    app.library.category == Some(c) || tab == LibraryTab::Media,
                ) {
                    app.library.category = Some(c);
                }
            }
        });
}

fn search_row(app: &mut TidyCutsApp, ui: &mut egui::Ui, import: bool, catalog: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if import && widgets::pill_button(ui, Some(ph::DOWNLOAD_SIMPLE), "Import", true).clicked() {
            app.import_dialog();
        }
        if import
            && app.library.tab == LibraryTab::Audio
            && widgets::pill_button(ui, Some(ph::MICROPHONE_STAGE), "Voiceover", false)
                .on_hover_text("Have the computer read some text aloud")
                .clicked()
        {
            let ctx = ui.ctx().clone();
            app.dialogs.open_voiceover(&ctx);
        }
        // The search box takes what is left; the size slider is 70 wide.
        let w = (ui.available_width() - 70.0 - 10.0).max(40.0);
        widgets::search_field(ui, &mut app.library.search, w);
        if catalog {
            // Fewer tiles per row means bigger tiles: the slider is flipped.
            let mut size = f64::from(11 - app.library.columns.clamp(3, 8));
            let r = widgets::slider_with(
                ui,
                widgets::LIBRARY_SLIDER,
                &mut size,
                3.0..=8.0,
                70.0,
                false,
            )
            .on_hover_text("Thumbnail size");
            if r.changed() {
                app.library.columns = 11 - (size.round() as u32).clamp(3, 8);
            }
        } else {
            let mut t = f64::from(app.library.tile_size);
            let r = widgets::slider_with(
                ui,
                widgets::LIBRARY_SLIDER,
                &mut t,
                110.0..=240.0,
                70.0,
                false,
            )
            .on_hover_text("Thumbnail size");
            if r.changed() {
                app.library.tile_size = t as f32;
            }
        }
    });
    ui.add_space(14.0);
}

/// Size of a media or sound tile so that whole tiles fill the row with no
/// space left over on the right: as many as fit at `wanted` wide, stretched
/// to share the width. The picture is 16:9, then a line for the name.
fn fitted_tile(width: f32, wanted: f32, gap: f32) -> Vec2 {
    let cols = ((width + gap) / (wanted + gap)).floor().max(1.0);
    let w = ((width - gap * (cols - 1.0)) / cols).floor().max(60.0);
    Vec2::new(w, w * 9.0 / 16.0 + 21.0)
}

fn matches_search(app: &TidyCutsApp, name: &str) -> bool {
    let q = app.library.search.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

fn media_tab(app: &mut TidyCutsApp, ui: &mut egui::Ui, only: Option<MediaKind>) {
    search_row(app, ui, true, false);
    let ids: Vec<MediaId> = app
        .project
        .media
        .iter()
        .filter(|m| only.is_none_or(|k| m.kind == k))
        // The Audio tab lists the built-in sounds in their own sections.
        .filter(|m| only.is_none() || crate::sounds::sound_id_of(&m.path).is_none())
        .filter(|m| matches_search(app, &m.name))
        .map(|m| m.id)
        .collect();
    if ids.is_empty() && app.importing == 0 {
        empty_state(app, ui, only == Some(MediaKind::Audio));
        return;
    }
    let tile = app.library.tile_size;
    egui::ScrollArea::vertical()
        .id_salt(("lib-media", only.is_some()))
        .show(ui, |ui| {
            let size = fitted_tile(ui.available_width(), tile, 12.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 14.0);
                for id in ids {
                    media_tile(app, ui, id, size);
                }
                for _ in 0..app.importing {
                    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.min, egui::vec2(size.x, size.y - 21.0)),
                        CornerRadius::same(6),
                        theme::PANEL_BG_2,
                    );
                    ui.painter().text(
                        r.center() - egui::vec2(0.0, 10.0),
                        Align2::CENTER_CENTER,
                        "Importing…",
                        FontId::proportional(12.0),
                        theme::TEXT_DIM,
                    );
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(150));
                }
            });
        });
}

// ----- audio tab ---------------------------------------------------------

/// Audio tab categories besides the sound library's own.
const IMPORTED_AUDIO: &str = "Imported Audio";
const MUSIC: &str = "Music";
const SOUND_EFFECTS: &str = "Sound Effects";

fn sound_categories(kind: SoundKind) -> Vec<&'static str> {
    tidycuts_sounds::categories(kind)
}

/// Imported audio, then the built-in music and sound effects, narrowed by
/// the sidebar category and the search.
fn audio_tab(app: &mut TidyCutsApp, ui: &mut egui::Ui) {
    let cat = app.library.category;
    if cat == Some(IMPORTED_AUDIO) {
        media_tab(app, ui, Some(MediaKind::Audio));
        return;
    }
    search_row(app, ui, true, false);
    ui.label(
        egui::RichText::new(
            "Click to listen. Drag to the timeline, or double-click to add at the playhead.",
        )
        .color(theme::TEXT_FAINT)
        .size(12.0),
    );
    ui.add_space(6.0);
    let imported: Vec<MediaId> = if cat.is_none() {
        app.project
            .media
            .iter()
            .filter(|m| m.kind == MediaKind::Audio)
            .filter(|m| crate::sounds::sound_id_of(&m.path).is_none())
            .filter(|m| matches_search(app, &m.name))
            .map(|m| m.id)
            .collect()
    } else {
        Vec::new()
    };
    let wanted = |d: &SoundDef| match cat {
        None => true,
        Some(MUSIC) => d.kind == SoundKind::Music,
        Some(SOUND_EFFECTS) => d.kind == SoundKind::Effect,
        Some(c) => d.category == c,
    };
    let q = app.library.search.trim().to_lowercase();
    let defs: Vec<&'static SoundDef> = tidycuts_sounds::catalog()
        .iter()
        .filter(|d| wanted(d))
        .filter(|d| {
            q.is_empty()
                || d.name.to_lowercase().contains(&q)
                || d.category.to_lowercase().contains(&q)
        })
        .collect();
    let tile = app.library.tile_size;
    let heading = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(text)
                .font(theme::heading_font(13.0))
                .color(theme::TEXT),
        );
        ui.add_space(2.0);
    };
    egui::ScrollArea::vertical()
        .id_salt("lib-audio")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = fitted_tile(ui.available_width(), tile, 12.0);
            if !imported.is_empty() || (cat.is_none() && app.importing > 0) {
                heading(ui, IMPORTED_AUDIO);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                    for id in imported {
                        media_tile(app, ui, id, size);
                    }
                });
                ui.add_space(10.0);
            }
            for kind in [SoundKind::Music, SoundKind::Effect] {
                let group: Vec<&'static SoundDef> =
                    defs.iter().copied().filter(|d| d.kind == kind).collect();
                if group.is_empty() {
                    continue;
                }
                if cat.is_none() {
                    heading(
                        ui,
                        if kind == SoundKind::Music {
                            MUSIC
                        } else {
                            SOUND_EFFECTS
                        },
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                    for d in group {
                        sound_tile(app, ui, d, size);
                    }
                });
                ui.add_space(10.0);
            }
            if defs.is_empty() && cat.is_some() {
                ui.label(egui::RichText::new("Nothing matches the search.").color(theme::TEXT_DIM));
            }
        });
}

/// Tile colour and icon for a sound category.
fn sound_art(category: &str) -> (Color32, &'static str) {
    match category {
        "Whoosh & Transitions" => (Color32::from_rgb(0x1f, 0x4e, 0x79), ph::WIND),
        "Impacts" => (Color32::from_rgb(0x7a, 0x2a, 0x2a), ph::LIGHTNING),
        "Pops & Clicks" => (Color32::from_rgb(0x5e, 0x3c, 0x8a), ph::CURSOR_CLICK),
        "Notifications" => (Color32::from_rgb(0x1d, 0x63, 0x56), ph::BELL_RINGING),
        "Countdown & Timers" => (Color32::from_rgb(0x7c, 0x58, 0x1c), ph::TIMER),
        "Cartoon & Comedy" => (Color32::from_rgb(0x83, 0x3b, 0x66), ph::SMILEY),
        "Drums" => (Color32::from_rgb(0x45, 0x48, 0x58), ph::METRONOME),
        "Ambience" => (Color32::from_rgb(0x2d, 0x58, 0x30), ph::CLOUD_RAIN),
        "Chill" => (Color32::from_rgb(0x3b, 0x3f, 0x86), ph::COFFEE),
        "Upbeat" => (Color32::from_rgb(0x93, 0x47, 0x18), ph::MUSIC_NOTES),
        "Cinematic" => (Color32::from_rgb(0x2a, 0x30, 0x3e), ph::FILM_SLATE),
        "Ambient" => (Color32::from_rgb(0x1d, 0x52, 0x6a), ph::MOON_STARS),
        _ => (Color32::from_rgb(0x33, 0x38, 0x40), ph::MUSIC_NOTE),
    }
}

fn sound_tile(app: &mut TidyCutsApp, ui: &mut egui::Ui, def: &'static SoundDef, size: Vec2) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let pic = Rect::from_min_size(rect.min, egui::vec2(size.x, size.y - 26.0));
    let ready = app.sounds.media(def.id).map(|m| m.id);
    let playing = ready.is_some()
        && app.preview.mode == crate::preview::PreviewMode::Source
        && app.preview.source_media == ready
        && app.is_playing();
    let pending = app.sounds.is_pending(def.id);
    if ui.is_rect_visible(rect) {
        let (color, icon) = sound_art(def.category);
        let p = ui.painter();
        p.rect_filled(pic, CornerRadius::same(6), color);
        // A big faint copy of the icon, like album art.
        p.with_clip_rect(pic).text(
            pic.right_bottom() + egui::vec2(-pic.height() * 0.28, -pic.height() * 0.3),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(pic.height() * 0.9),
            Color32::from_white_alpha(18),
        );
        p.text(
            pic.left_top() + egui::vec2(18.0, 18.0),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(17.0),
            Color32::from_white_alpha(200),
        );
        let label = if def.secs < 10.0 {
            format!("{:.1}s", def.secs)
        } else {
            duration_label(Time::from_secs_f64(def.secs as f64))
        };
        let g = p.layout_no_wrap(label, FontId::proportional(11.0), Color32::WHITE);
        let br = Rect::from_min_size(
            pic.right_bottom() - g.size() - egui::vec2(12.0, 8.0),
            g.size() + egui::vec2(8.0, 4.0),
        );
        p.rect_filled(br, CornerRadius::same(4), Color32::from_black_alpha(150));
        p.galley(br.min + egui::vec2(4.0, 2.0), g, Color32::WHITE);
        // Play / pause in the middle.
        let c = pic.center();
        if pending {
            egui::Spinner::new()
                .size(22.0)
                .color(Color32::WHITE)
                .paint_at(ui, Rect::from_center_size(c, egui::vec2(22.0, 22.0)));
        } else if playing || resp.hovered() {
            p.circle_filled(c, 17.0, Color32::from_black_alpha(150));
            p.text(
                c + egui::vec2(if playing { 0.0 } else { 1.5 }, 0.0),
                Align2::CENTER_CENTER,
                if playing { ph::PAUSE } else { ph::PLAY },
                FontId::proportional(17.0),
                Color32::WHITE,
            );
        }
        if playing {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let used = app.project.media.iter().any(|m| {
            crate::sounds::sound_id_of(&m.path) == Some(def.id)
                && app
                    .project
                    .tracks
                    .iter()
                    .any(|t| t.clips.iter().any(|c| c.media_id == Some(m.id)))
        });
        if used {
            let at = pic.left_bottom() + egui::vec2(14.0, -14.0);
            p.circle_filled(at, 8.0, theme::ACCENT);
            p.text(
                at,
                Align2::CENTER_CENTER,
                ph::CHECK,
                FontId::proportional(10.0),
                theme::ACCENT_TEXT,
            );
        }
        let stroke = if playing {
            Stroke::new(2.0, theme::ACCENT)
        } else if resp.hovered() {
            Stroke::new(1.0, theme::TEXT_DIM)
        } else {
            Stroke::NONE
        };
        p.rect_stroke(pic, CornerRadius::same(6), stroke, StrokeKind::Inside);
        let name_rect = Rect::from_min_max(egui::pos2(rect.left(), pic.bottom() + 4.0), rect.max);
        ui.painter().with_clip_rect(name_rect).text(
            name_rect.left_center(),
            Align2::LEFT_CENTER,
            def.name,
            FontId::proportional(12.0),
            if playing {
                Color32::WHITE
            } else {
                theme::TEXT_DIM
            },
        );
    }
    let mut added = false;
    if resp.hovered() {
        let plus = Rect::from_center_size(
            pic.right_top() + egui::vec2(-16.0, 16.0),
            egui::vec2(24.0, 24.0),
        );
        let pr = ui.interact(plus, Id::new(("sound-add", def.id)), Sense::click());
        ui.painter().circle_filled(
            plus.center(),
            12.0,
            if pr.hovered() {
                theme::ACCENT
            } else {
                Color32::from_black_alpha(180)
            },
        );
        ui.painter().text(
            plus.center(),
            Align2::CENTER_CENTER,
            ph::PLUS,
            FontId::proportional(13.0),
            if pr.hovered() {
                theme::ACCENT_TEXT
            } else {
                Color32::WHITE
            },
        );
        if pr.on_hover_text("Add at the playhead").clicked() {
            app.add_sound(def.id, None);
            added = true;
        }
    }
    if resp.double_clicked() {
        app.add_sound(def.id, None);
    } else if resp.clicked() && !added {
        app.preview_sound(def.id);
    }
    if resp.dragged() {
        resp.dnd_set_drag_payload(LibraryDrag::Sound(def.id));
        drag_ghost(ui, None, def.name);
    }
    resp.on_hover_text(format!("{}\n{}", def.name, def.category))
        .context_menu(|ui| {
            if ui.button("Add at the playhead").clicked() {
                app.add_sound(def.id, None);
                ui.close();
            }
            if ui.button(if playing { "Stop" } else { "Listen" }).clicked() {
                app.preview_sound(def.id);
                ui.close();
            }
        });
}

fn empty_state(app: &mut TidyCutsApp, ui: &mut egui::Ui, audio: bool) {
    let rect = ui.available_rect_before_wrap();
    let r = Rect::from_center_size(
        rect.center() - egui::vec2(0.0, 20.0),
        egui::vec2((rect.width() - 20.0).min(380.0), 170.0),
    );
    let resp = ui.interact(r, Id::new("lib-empty"), Sense::click());
    let stroke = Stroke::new(
        1.0,
        if resp.hovered() {
            theme::ACCENT
        } else {
            theme::TEXT_FAINT
        },
    );
    let pts = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    ui.painter()
        .extend(egui::Shape::dashed_line(&pts, stroke, 6.0, 4.0));
    ui.painter().text(
        r.center() - egui::vec2(0.0, 28.0),
        Align2::CENTER_CENTER,
        ph::DOWNLOAD_SIMPLE,
        FontId::proportional(30.0),
        theme::ACCENT,
    );
    ui.painter().text(
        r.center() + egui::vec2(0.0, 14.0),
        Align2::CENTER_CENTER,
        if audio {
            "Import music or sound effects"
        } else {
            "Import videos, images and audio"
        },
        theme::heading_font(14.0),
        theme::TEXT,
    );
    ui.painter().text(
        r.center() + egui::vec2(0.0, 38.0),
        Align2::CENTER_CENTER,
        "Click here or drop files anywhere in the window",
        FontId::proportional(12.0),
        theme::TEXT_DIM,
    );
    if resp.clicked() {
        app.import_dialog();
    }
}

pub fn duration_label(t: Time) -> String {
    let s = t.as_secs_f64().round() as i64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn media_tile(app: &mut TidyCutsApp, ui: &mut egui::Ui, id: MediaId, size: Vec2) {
    let Some(media) = app.project.media(id).cloned() else {
        return;
    };
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let pic = Rect::from_min_size(rect.min, egui::vec2(size.x, size.y - 21.0));
    let p = ui.painter();
    p.rect_filled(pic, CornerRadius::same(6), theme::FIELD_BG);
    let cache = app.engine.as_ref().and_then(|e| e.scrub.get(&media));
    let tex = app.thumbs.poster(&media, cache.clone());
    match (&tex, media.kind) {
        (Some(t), _) => {
            let fit = fit_rect(pic, t.size_vec2());
            ui.painter().with_clip_rect(pic).image(
                t.id(),
                fit,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        (None, MediaKind::Audio) => {
            p.rect_filled(
                pic,
                CornerRadius::same(6),
                Color32::from_rgb(0x16, 0x3d, 0x35),
            );
            p.text(
                pic.center(),
                Align2::CENTER_CENTER,
                ph::WAVEFORM,
                FontId::proportional(30.0),
                theme::AUDIO_WAVE,
            );
        }
        (None, _) => {
            p.text(
                pic.center(),
                Align2::CENTER_CENTER,
                ph::FILM_STRIP,
                FontId::proportional(24.0),
                theme::TEXT_FAINT,
            );
        }
    }
    let p = ui.painter();
    // Duration badge and kind icon.
    if media.kind != MediaKind::Image {
        let label = duration_label(media.duration);
        let g = p.layout_no_wrap(label, theme::heading_font(10.5), Color32::WHITE);
        // 6 pixels from the corner, padded 2 by 6, fully rounded.
        let br = Rect::from_min_size(
            pic.right_bottom() - egui::vec2(g.size().x + 12.0 + 6.0, g.size().y + 4.0 + 6.0),
            g.size() + egui::vec2(12.0, 4.0),
        );
        p.rect_filled(br, CornerRadius::same(6), Color32::from_black_alpha(0xaa));
        p.galley(br.min + egui::vec2(6.0, 2.0), g, Color32::WHITE);
    }
    if let Some(c) = &cache {
        if !c.is_complete() {
            // Scrub cache progress along the bottom.
            let f = c.progress();
            let bar = Rect::from_min_max(
                egui::pos2(pic.left() + 4.0, pic.bottom() - 3.0),
                egui::pos2(
                    pic.left() + 4.0 + (pic.width() - 8.0) * f,
                    pic.bottom() - 1.0,
                ),
            );
            p.rect_filled(bar, CornerRadius::same(1), theme::ACCENT);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }
    }
    let used = app
        .project
        .tracks
        .iter()
        .any(|t| t.clips.iter().any(|c| c.media_id == Some(id)));
    if used {
        // An 18-pixel violet square with a check, 6 pixels in.
        let chk = Rect::from_min_size(
            pic.left_top() + egui::vec2(6.0, 6.0),
            egui::vec2(18.0, 18.0),
        );
        p.rect_filled(chk, CornerRadius::same(4), theme::ACCENT);
        p.text(
            chk.center(),
            Align2::CENTER_CENTER,
            ph::CHECK,
            FontId::proportional(11.0),
            theme::ACCENT_TEXT,
        );
    }
    let selected = app.selected_media == Some(id);
    let stroke = if selected {
        Stroke::new(2.0, theme::ACCENT)
    } else if resp.hovered() {
        Stroke::new(1.0, theme::TEXT_DIM)
    } else {
        Stroke::NONE
    };
    p.rect_stroke(pic, CornerRadius::same(6), stroke, StrokeKind::Inside);
    // Quick add button.
    if resp.hovered() {
        let plus = Rect::from_center_size(
            pic.right_top() + egui::vec2(-16.0, 16.0),
            egui::vec2(24.0, 24.0),
        );
        let pr = ui.interact(plus, Id::new(("media-add", id)), Sense::click());
        ui.painter().circle_filled(
            plus.center(),
            12.0,
            if pr.hovered() {
                theme::ACCENT
            } else {
                Color32::from_black_alpha(180)
            },
        );
        ui.painter().text(
            plus.center(),
            Align2::CENTER_CENTER,
            ph::PLUS,
            FontId::proportional(13.0),
            if pr.hovered() {
                theme::ACCENT_TEXT
            } else {
                Color32::WHITE
            },
        );
        if pr.on_hover_text("Add to the timeline at the end").clicked() {
            app.add_media_to_timeline(id, None);
        }
    }
    let name_rect = Rect::from_min_max(egui::pos2(rect.left(), pic.bottom() + 6.0), rect.max);
    ui.painter().with_clip_rect(name_rect).text(
        name_rect.left_center(),
        Align2::LEFT_CENTER,
        &media.name,
        theme::medium_font(12.0),
        if selected {
            Color32::WHITE
        } else {
            theme::TEXT_DIM
        },
    );

    if resp.clicked() {
        app.selected_media = Some(id);
        app.show_source(id);
    }
    // Proxy status in the corner of the picture.
    if let Some(done) = app.proxy_progress(id) {
        chip(ui, pic, &format!("Proxy {:.0}%", done * 100.0), false);
    } else if media.proxy.as_ref().is_some_and(|p| p.is_file()) {
        chip(ui, pic, "Proxy", false);
    }
    if resp.double_clicked() {
        app.add_media_to_timeline(id, None);
    }
    if resp.dragged() {
        resp.dnd_set_drag_payload(LibraryDrag::Media(id));
        drag_ghost(ui, tex.as_ref(), &media.name);
    }
    let tip = match media.kind {
        MediaKind::Audio => format!("{}\n{}", media.name, duration_label(media.duration)),
        _ => format!(
            "{}\n{}×{}{}  ·  {}",
            media.name,
            media.width,
            media.height,
            media
                .frame_rate
                .map(|f| format!(" @ {:.2} fps", f.as_f64()))
                .unwrap_or_default(),
            duration_label(media.duration)
        ),
    };
    resp.on_hover_text(tip).context_menu(|ui| {
        if ui.button("Add to timeline").clicked() {
            app.add_media_to_timeline(id, None);
            ui.close();
        }
        if ui.button("Preview").clicked() {
            app.show_source(id);
            ui.close();
        }
        if let Some(c) = app.selected_clip() {
            if ui.button("Replace selected clip").clicked() {
                app.replace_clip_media(c, id);
                ui.close();
            }
        }
        if media.kind == MediaKind::Video
            && ui
                .button("Set project size and frame rate from this")
                .clicked()
        {
            app.match_project_to(id);
            ui.close();
        }
        if media.kind == MediaKind::Video {
            ui.separator();
            if app.proxy_progress(id).is_some() {
                if ui.button("Stop making the proxy").clicked() {
                    app.remove_proxy(id);
                    ui.close();
                }
            } else if media.proxy.is_some() {
                if ui.button("Remove the proxy").clicked() {
                    app.remove_proxy(id);
                    ui.close();
                }
            } else if ui.button("Make a proxy for smooth editing").clicked() {
                app.make_proxy(id);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(egui::RichText::new("Remove from project").color(theme::DANGER))
            .clicked()
        {
            app.remove_media(id);
            ui.close();
        }
    });
}

/// A small picture following the pointer while dragging.
pub fn drag_ghost(ui: &egui::Ui, tex: Option<&egui::TextureHandle>, name: &str) {
    let Some(p) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("drag-ghost"),
    ));
    let r = Rect::from_min_size(p + egui::vec2(10.0, 10.0), egui::vec2(120.0, 68.0));
    painter.rect_filled(r, CornerRadius::same(6), theme::PANEL_BG_3);
    if let Some(t) = tex {
        painter.image(
            t.id(),
            fit_rect(r, t.size_vec2()),
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::from_white_alpha(220),
        );
    }
    painter.rect_stroke(
        r,
        CornerRadius::same(6),
        Stroke::new(1.5, theme::ACCENT),
        StrokeKind::Inside,
    );
    painter.text(
        r.center_bottom() + egui::vec2(0.0, 10.0),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(11.5),
        Color32::WHITE,
    );
}

pub fn fit_rect(outer: Rect, size: Vec2) -> Rect {
    let s = (outer.width() / size.x.max(1.0)).min(outer.height() / size.y.max(1.0));
    Rect::from_center_size(outer.center(), size * s)
}

/// The catalog tabs, in the order a search lists them.
const CATALOG_TABS: [LibraryTab; 7] = [
    LibraryTab::Effects,
    LibraryTab::Filters,
    LibraryTab::Titles,
    LibraryTab::Transitions,
    LibraryTab::Stickers,
    LibraryTab::Templates,
    LibraryTab::Layouts,
];

/// The entries shown first under "Trending", by name.
fn trending(tab: LibraryTab) -> &'static [&'static str] {
    match tab {
        LibraryTab::Titles => &[
            "Karaoke",
            "Glass Card",
            "Poster Stack",
            "Word Pop",
            "Gold Foil",
            "Subscribe",
        ],
        LibraryTab::Transitions => &[
            "Whip Pan",
            "Zoom Punch",
            "Light Wipe",
            "Film Roll Up",
            "Flash",
            "Shadow Fade",
        ],
        LibraryTab::Effects => &[
            "Halation",
            "Pro-Mist",
            "Beat Flash",
            "Speed Lines",
            "Prism",
            "Camcorder",
        ],
        LibraryTab::Filters => &[
            "Soft Neutral",
            "Hazy Pastel",
            "Creator Clean",
            "Matte Calm",
            "Teal & Orange",
            "Golden Hour",
        ],
        LibraryTab::Stickers => &[
            "Subscribe",
            "Link in Bio",
            "Like Button",
            "Swipe Up",
            "Crown",
            "Rocket",
        ],
        LibraryTab::Layouts => &[
            "Side by Side",
            "Four Squares",
            "Picture in Picture",
            "Three Columns",
            "Bars",
            "Circle",
        ],
        LibraryTab::Templates => &[
            "Channel Intro",
            "Vlog Opener",
            "Lower Third",
            "Subscribe End Screen",
            "Countdown 3-2-1",
            "Sale Banner",
        ],
        LibraryTab::Media | LibraryTab::Audio => &[],
    }
}

/// A block of tiles under a heading.
struct Section {
    title: String,
    items: Vec<(LibraryTab, CatalogItem)>,
}

/// Whether `item` of `tab` matches every word of the search.
fn item_matches(tab: LibraryTab, item: &CatalogItem, query: &str) -> bool {
    let hay = format!("{} {} {}", item.name, item.category, tab.label()).to_lowercase();
    query.split_whitespace().all(|w| hay.contains(w))
}

/// What the catalog tab shows: a search across every tab, one category, or
/// Trending followed by each category.
fn sections(app: &TidyCutsApp, tab: LibraryTab) -> Vec<Section> {
    let query = app.library.search.trim().to_lowercase();
    if !query.is_empty() {
        return CATALOG_TABS
            .into_iter()
            .map(|t| Section {
                title: t.label().to_string(),
                items: catalog(t)
                    .into_iter()
                    .filter(|i| item_matches(t, i, &query))
                    .map(|i| (t, i))
                    .collect(),
            })
            .filter(|s| !s.items.is_empty())
            .collect();
    }
    let all = catalog(tab);
    if let Some(cat) = app.library.category {
        return vec![Section {
            title: cat.to_string(),
            items: all
                .into_iter()
                .filter(|i| i.category == cat)
                .map(|i| (tab, i))
                .collect(),
        }];
    }
    let mut out = Vec::new();
    let hot: Vec<_> = trending(tab)
        .iter()
        .filter_map(|n| all.iter().find(|i| i.name == *n))
        .map(|i| (tab, i.clone()))
        .collect();
    if !hot.is_empty() {
        out.push(Section {
            title: "Trending".to_string(),
            items: hot,
        });
    }
    for cat in categories(&all, |c| c.category) {
        out.push(Section {
            title: cat.to_string(),
            items: all
                .iter()
                .filter(|i| i.category == cat)
                .map(|i| (tab, i.clone()))
                .collect(),
        });
    }
    out
}

fn catalog_tab(app: &mut TidyCutsApp, ui: &mut egui::Ui, tab: LibraryTab) {
    search_row(app, ui, false, true);
    let searching = !app.library.search.trim().is_empty();
    let sections = sections(app, tab);
    if !searching {
        let hint = match tab {
            LibraryTab::Transitions => {
                "Drag onto a cut between two clips, or onto a clip's start or end."
            }
            LibraryTab::Effects | LibraryTab::Filters => {
                "Drag onto a clip, or onto an empty track to cover everything below."
            }
            LibraryTab::Layouts => {
                "Drag a layout to the timeline, then drop media onto each empty cell. \
                 Visualizers move with the sound."
            }
            _ => "Drag to the timeline, or double-click to add at the playhead.",
        };
        ui.label(
            egui::RichText::new(hint)
                .color(theme::TEXT_FAINT)
                .size(12.0),
        );
        ui.add_space(6.0);
    }
    if sections.is_empty() {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("Nothing matches the search.").color(theme::TEXT_DIM));
        return;
    }
    let gap = 10.0;
    let cols = app.library.columns.clamp(3, 8) as usize;
    egui::ScrollArea::vertical()
        .id_salt(("lib-cat", tab.label(), app.library.search.is_empty()))
        .show(ui, |ui| {
            // Leave room for the scroll bar so the last tile never wraps.
            let avail = ui.available_width() - 12.0;
            let w = ((avail - gap * (cols as f32 - 1.0)) / cols as f32)
                .floor()
                .max(60.0);
            let size = Vec2::new(w, w * 9.0 / 16.0 + 22.0);
            for (si, section) in sections.iter().enumerate() {
                ui.push_id(si, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&section.title)
                                .strong()
                                .size(14.0)
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new(section.items.len().to_string())
                                .size(12.0)
                                .color(theme::TEXT_FAINT),
                        );
                    });
                    ui.add_space(4.0);
                    for row in section.items.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (item_tab, item) in row {
                                catalog_tile(app, ui, *item_tab, item, size);
                            }
                        });
                        ui.add_space(gap - 2.0);
                    }
                    ui.add_space(10.0);
                });
            }
        });
}

/// Length and number of clips of each template, worked out once.
fn template_facts(index: usize) -> Option<(f64, usize)> {
    static FACTS: std::sync::OnceLock<Vec<(f64, usize)>> = std::sync::OnceLock::new();
    FACTS
        .get_or_init(|| {
            TEMPLATES
                .iter()
                .map(|t| {
                    let p = crate::templates::preview_project(t, 5.0);
                    let clips = p.tracks.iter().map(|tr| tr.clips.len()).sum();
                    (crate::templates::length(t, 5.0), clips)
                })
                .collect()
        })
        .get(index)
        .copied()
}

/// A small dark label in a bottom corner of a tile's picture.
fn chip(ui: &egui::Ui, pic: Rect, text: &str, right: bool) {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(10.0), Color32::WHITE);
    let size = galley.size() + egui::vec2(8.0, 3.0);
    let x = if right {
        pic.right() - size.x - 4.0
    } else {
        pic.left() + 4.0
    };
    let r = Rect::from_min_size(egui::pos2(x, pic.bottom() - size.y - 4.0), size);
    ui.painter()
        .rect_filled(r, CornerRadius::same(3), Color32::from_black_alpha(165));
    ui.painter()
        .galley(r.min + egui::vec2(4.0, 1.5), galley, Color32::WHITE);
}

fn catalog_tile(
    app: &mut TidyCutsApp,
    ui: &mut egui::Ui,
    tab: LibraryTab,
    item: &CatalogItem,
    size: Vec2,
) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let pic = Rect::from_min_size(rect.min, egui::vec2(size.x, size.y - 22.0));
    if ui.is_rect_visible(rect) {
        crate::library_thumbs::draw(app, ui, tab, item, pic, resp.hovered());
        if let LibraryDrag::Template(i) = item.drag {
            if let Some((len, clips)) = template_facts(i) {
                let secs = len.round() as u32;
                chip(ui, pic, &format!("{}:{:02}", secs / 60, secs % 60), false);
                let unit = if clips == 1 { "clip" } else { "clips" };
                chip(ui, pic, &format!("{clips} {unit}"), true);
            }
        }
        let stroke = if resp.hovered() {
            Stroke::new(1.5, theme::ACCENT)
        } else {
            Stroke::new(1.0, theme::BORDER)
        };
        ui.painter()
            .rect_stroke(pic, CornerRadius::same(6), stroke, StrokeKind::Inside);
        let name_rect = Rect::from_min_max(egui::pos2(rect.left(), pic.bottom() + 3.0), rect.max);
        ui.painter().with_clip_rect(name_rect).text(
            name_rect.left_center(),
            Align2::LEFT_CENTER,
            item.name,
            FontId::proportional(12.0),
            theme::TEXT_DIM,
        );
    }
    if resp.double_clicked() {
        apply(app, &item.drag);
    }
    if resp.dragged() {
        resp.dnd_set_drag_payload(item.drag.clone());
        let tex = crate::library_thumbs::texture_for(ui.ctx(), item);
        drag_ghost(ui, tex.as_ref(), item.name);
    }
    resp.on_hover_text(item.name).context_menu(|ui| {
        let label = match item.drag {
            LibraryDrag::Effect(_) => "Apply to selected clips",
            LibraryDrag::Transition(_) => "Apply to selected clip",
            _ => "Add at the playhead",
        };
        if ui.button(label).clicked() {
            apply(app, &item.drag);
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trending_entry_exists_and_none_repeat() {
        for tab in CATALOG_TABS {
            let names = trending(tab);
            assert!(!names.is_empty(), "{tab:?} has nothing trending");
            let all = catalog(tab);
            for (i, n) in names.iter().enumerate() {
                assert!(all.iter().any(|c| c.name == *n), "{tab:?}: no {n}");
                assert!(!names[..i].contains(n), "{tab:?}: {n} twice");
            }
        }
    }

    #[test]
    fn templates_report_their_length_and_clips() {
        for i in 0..TEMPLATES.len() {
            let (len, clips) = template_facts(i).expect("facts");
            assert!(len > 0.0 && clips >= 1, "template {i}");
        }
        assert!(template_facts(TEMPLATES.len()).is_none());
    }
}
