//! The properties panel shown in place of the library when a clip is
//! double-clicked, with a header with the clip's name and
//! type, a tab bar (Video, Audio, Color, Speed, Animation, Effects, and
//! Title or Sticker for those clips) and scrollable sections.
//!
//! Each tab edits a copy of the clip; the copy is committed once per frame
//! with `TidyCutsApp::commit_clip`, so a slider drag is one undo step.
//! Edits that move other clips (speed, duration) or need the whole project
//! (detach audio, animation presets) are queued as [`Action`]s and run
//! after the commit.

mod animation;
mod audio;
mod color;
mod effects;
mod kit;
mod looks;
mod speed;
mod sticker;
mod title;
mod video;
mod visualizer;
mod wheels;

use crate::app::TidyCutsApp;
use crate::theme;
use crate::widgets;
use egui::{Align2, Color32, CornerRadius, CursorIcon, Id, Rect, Sense, Stroke, Ui};
use tidycuts_core::{
    Animatable, Clip, ClipId, Generator, MediaItem, MediaKind, Time, TrackKind, VideoSettings,
};

/// A tab of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Title,
    Sticker,
    Video,
    Audio,
    Color,
    Speed,
    Animation,
    Effects,
    Visualizer,
}

impl Tab {
    const ALL: [Tab; 9] = [
        Tab::Title,
        Tab::Sticker,
        Tab::Visualizer,
        Tab::Video,
        Tab::Audio,
        Tab::Color,
        Tab::Speed,
        Tab::Animation,
        Tab::Effects,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Title => "Title",
            Tab::Sticker => "Sticker",
            Tab::Visualizer => "Visualizer",
            Tab::Video => "Video",
            Tab::Audio => "Audio",
            Tab::Color => "Color",
            Tab::Speed => "Speed",
            Tab::Animation => "Animation",
            Tab::Effects => "Effects",
        }
    }

    /// Looks a tab up by name, ignoring case ("Image" means Video).
    pub fn from_name(name: &str) -> Option<Tab> {
        if name.eq_ignore_ascii_case("image") || name.eq_ignore_ascii_case("transform") {
            return Some(Tab::Video);
        }
        Tab::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// What kind of clip the panel shows; decides the tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Video,
    Image,
    Audio,
    Title,
    Sticker,
    Adjustment,
    /// A solid color or gradient.
    Solid,
    /// An empty split-screen cell.
    Placeholder,
    /// An audio visualizer.
    Visualizer,
}

impl Kind {
    pub fn of(clip: &Clip, track: TrackKind, media: Option<&MediaItem>) -> Kind {
        match &clip.generator {
            Some(Generator::Title(_)) => return Kind::Title,
            Some(Generator::Sticker(_)) => return Kind::Sticker,
            Some(Generator::Adjustment) => return Kind::Adjustment,
            Some(Generator::Solid(_)) | Some(Generator::Gradient { .. }) => return Kind::Solid,
            Some(Generator::Placeholder) => return Kind::Placeholder,
            Some(Generator::Visualizer(_)) => return Kind::Visualizer,
            None => {}
        }
        if track == TrackKind::Audio {
            return Kind::Audio;
        }
        match media.map(|m| m.kind) {
            Some(MediaKind::Audio) => Kind::Audio,
            Some(MediaKind::Image) => Kind::Image,
            _ => Kind::Video,
        }
    }

    /// Whether the picture comes from a media file.
    pub fn is_media(self) -> bool {
        matches!(self, Kind::Video | Kind::Image)
    }
}

/// The tabs a clip gets, in order.
fn tabs_for(kind: Kind, media: Option<&MediaItem>) -> Vec<Tab> {
    match kind {
        Kind::Video => {
            let mut t = vec![Tab::Video];
            if media.is_some_and(|m| m.has_audio) {
                t.push(Tab::Audio);
            }
            t.extend([Tab::Color, Tab::Speed, Tab::Animation, Tab::Effects]);
            t
        }
        Kind::Image | Kind::Solid => vec![Tab::Video, Tab::Color, Tab::Animation, Tab::Effects],
        Kind::Audio => vec![Tab::Audio, Tab::Speed],
        Kind::Title => vec![Tab::Title, Tab::Video, Tab::Animation, Tab::Effects],
        Kind::Sticker => vec![Tab::Sticker, Tab::Video, Tab::Animation, Tab::Effects],
        Kind::Adjustment => vec![Tab::Effects, Tab::Color, Tab::Video],
        Kind::Placeholder => vec![Tab::Video],
        Kind::Visualizer => vec![Tab::Visualizer, Tab::Video, Tab::Animation, Tab::Effects],
    }
}

/// The tab a text, sticker, visualizer or effect clip is edited in, by name:
/// double-clicking such a clip goes straight to it. `None` for clips from a
/// file, which keep whatever tab was open.
pub fn main_tab_of(app: &TidyCutsApp, clip: ClipId) -> Option<&'static str> {
    let (_, c) = app.project.clip(clip)?;
    match &c.generator {
        Some(Generator::Title(_)) => Some("Title"),
        Some(Generator::Sticker(_)) => Some("Sticker"),
        Some(Generator::Visualizer(_)) => Some("Visualizer"),
        Some(Generator::Adjustment) => Some("Effects"),
        _ => None,
    }
}

/// Something the panel asks the app to do after the clip is committed.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    DetachAudio,
    SetSpeed(f64),
    /// A speed ramp for the clip, or `None` to remove it.
    SetRamp(Option<tidycuts_core::SpeedRamp>),
    /// New length for a clip without a fixed length (images, titles…).
    SetDuration(Time),
    SetAnimation {
        out: bool,
        kind: Option<String>,
    },
    OpenCrop,
    /// Show the scopes window.
    OpenScopes,
    /// A message for the user (`true` for an error).
    Notify(String, bool),
}

pub struct InspectorState {
    /// Tab asked for by an action (e.g. "Speed" from the toolbar).
    requested: Option<String>,
    /// The tab the user picked last; used whenever the clip has it.
    want: Tab,
    /// The kind of clip shown last: a different kind starts on its own tab.
    last_kind: Option<Kind>,
    /// Video tab: 0 Basic, 1 Mask, 2 Enhance.
    video_sub: usize,
    /// Animation tab shows the out animation instead of the in one.
    anim_out: bool,
    /// Scale X and Y move together.
    scale_lock: bool,
    /// Speed shown while its slider is dragged (applied on release).
    speed_drag: Option<f64>,
    /// The ramp as it looks while a point is being dragged (applied on release).
    ramp_drag: Option<tidycuts_core::SpeedRamp>,
    /// Duration in seconds shown while it's being dragged.
    duration_drag: Option<f64>,
    /// The clip shown last, to drop half-finished drags on a switch.
    clip: Option<ClipId>,
    /// Sticker preview picture and the spec it shows.
    sticker_preview: Option<(tidycuts_core::StickerSpec, egui::TextureHandle)>,
    /// Color tab: the HSL range being edited (0 red … 7 magenta).
    hsl_band: usize,
    /// Color tab: the curve being edited (0 RGB, 1 red, 2 green, 3 blue)
    /// and the point being dragged.
    curve_ch: usize,
    curve_drag: Option<usize>,
}

impl Default for InspectorState {
    fn default() -> Self {
        InspectorState {
            requested: None,
            want: Tab::Video,
            last_kind: None,
            video_sub: 0,
            anim_out: false,
            scale_lock: true,
            speed_drag: None,
            ramp_drag: None,
            duration_drag: None,
            clip: None,
            sticker_preview: None,
            hsl_band: 0,
            curve_ch: 0,
            curve_drag: None,
        }
    }
}

impl InspectorState {
    /// Switches to a tab by name the next time the panel shows.
    pub fn request_tab(&mut self, name: &str) {
        self.requested = Some(name.to_string());
    }
}

/// What every tab gets: read access to the app, the panel's own state and
/// facts about the clip.
pub struct Panel<'a> {
    pub app: &'a TidyCutsApp,
    pub st: &'a mut InspectorState,
    pub kind: Kind,
    pub track: TrackKind,
    pub media: Option<&'a MediaItem>,
    /// Clip-local time of the playhead (clamped into the clip), where
    /// keyframes are added.
    pub local: Time,
    pub actions: Vec<Action>,
}

impl Panel<'_> {
    /// Project frame size in pixels.
    pub fn frame(&self) -> (f64, f64) {
        let s = &self.app.project.settings;
        (s.width as f64, s.height as f64)
    }

    /// Whether the clip has a natural length (video and audio files).
    pub fn fixed_length(&self, c: &Clip) -> bool {
        c.has_fixed_length(self.media)
    }
}

pub fn show(app: &mut TidyCutsApp, ui: &mut Ui) {
    let Some(id) = app.selected_clip() else {
        return;
    };
    let Some((track, clip)) = app.project.clip(id) else {
        return;
    };
    let original = clip.clone();
    let track_kind = track.kind;
    let mut st = std::mem::take(&mut app.inspector);
    if st.clip != Some(id) {
        st.clip = Some(id);
        st.speed_drag = None;
        st.duration_drag = None;
    }
    let mut c = original.clone();
    let actions = {
        let media = original.media_id.and_then(|m| app.project.media(m));
        let kind = Kind::of(&original, track_kind, media);
        let tabs = tabs_for(kind, media);
        // Moving from a video to a title (or the like) starts on the tab where
        // that kind of clip is edited, not on the Video tab left open.
        if st.last_kind != Some(kind) {
            st.last_kind = Some(kind);
            let own = match kind {
                Kind::Title => Some(Tab::Title),
                Kind::Sticker => Some(Tab::Sticker),
                Kind::Visualizer => Some(Tab::Visualizer),
                Kind::Adjustment => Some(Tab::Effects),
                _ => None,
            };
            if let Some(t) = own {
                st.want = t;
            }
        }
        if let Some(name) = st.requested.take() {
            if let Some(t) = Tab::from_name(&name) {
                st.want = t;
            }
        }
        let tab = if tabs.contains(&st.want) {
            st.want
        } else {
            tabs[0]
        };
        let local = app.playhead().max(original.start).min(original.end()) - original.start;
        let mut p = Panel {
            app: &*app,
            st: &mut st,
            kind,
            track: track_kind,
            media,
            local,
            actions: Vec::new(),
        };
        draw(&mut p, ui, &mut c, &tabs, tab);
        p.actions
    };
    app.inspector = st;
    if c != original {
        app.commit_clip(ui.ctx(), c);
    }
    for a in actions {
        run(app, id, a);
    }
}

fn run(app: &mut TidyCutsApp, id: ClipId, action: Action) {
    match action {
        Action::DetachAudio => {
            app.selection = vec![id];
            app.detach_audio();
        }
        Action::SetSpeed(s) => {
            let same = app
                .project
                .clip(id)
                .is_some_and(|(_, c)| (c.speed - s).abs() < 1e-9);
            if !same {
                app.set_speed(id, s);
            }
        }
        Action::SetRamp(ramp) => {
            if !app.edit(|p| p.set_speed_ramp(id, ramp)) {
                app.notify(
                    "The speed couldn't be changed (is the track locked?).",
                    false,
                );
            }
        }
        Action::SetDuration(len) => {
            let changed = app.edit(|p| {
                let before = p.clip(id).map(|(_, c)| c.duration());
                p.set_duration(id, len) && p.clip(id).map(|(_, c)| c.duration()) != before
            });
            if !changed {
                app.notify(
                    "The length couldn't be changed (is the track locked or the next clip in the way?).",
                    false,
                );
            }
        }
        Action::SetAnimation { out, kind } => app.set_animation(id, out, kind.as_deref()),
        Action::OpenCrop => app.dialogs.open_crop(id),
        Action::OpenScopes => app.scopes.open = true,
        Action::Notify(text, error) => app.notify(&text, error),
    }
}

const TABS_H: f32 = 40.0;
const SUBTABS_H: f32 = 38.0;
const PAD: f32 = 16.0;

fn draw(p: &mut Panel, ui: &mut Ui, c: &mut Clip, tabs: &[Tab], tab: Tab) {
    let full = ui.max_rect();
    // The tabs come first: 40 pixels high with a line under them.
    let bar = Rect::from_min_size(full.min, egui::vec2(full.width(), TABS_H));
    let mut index = tabs.iter().position(|t| *t == tab).unwrap_or(0);
    let items: Vec<&str> = tabs.iter().map(|t| t.name()).collect();
    let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar));
    bar_ui.set_clip_rect(bar.intersect(ui.clip_rect()));
    let (picked, reset) = panel_tabs(&mut bar_ui, &mut index, &items);
    if picked {
        p.st.want = tabs[index];
    }
    if let Some(i) = reset {
        reset_tab(p, c, tabs[i]);
    }
    let tab = tabs[index];

    let mut body = Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
    if tab == Tab::Video {
        let labels = video::subtabs(p.kind);
        if labels.len() > 1 {
            let r = Rect::from_min_size(
                body.min + egui::vec2(PAD - 8.0, 2.0),
                egui::vec2(body.width() - 2.0 * PAD + 16.0, SUBTABS_H),
            );
            let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(r));
            sub.set_clip_rect(r.intersect(ui.clip_rect()));
            p.st.video_sub = p.st.video_sub.min(labels.len() - 1);
            widgets::tabs(&mut sub, &mut p.st.video_sub, labels);
            body.min.y = r.bottom();
        } else {
            p.st.video_sub = 0;
        }
    }

    let mut inner = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(body)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    inner.set_clip_rect(body.intersect(ui.clip_rect()));
    let sub = if tab == Tab::Video { p.st.video_sub } else { 0 };
    egui::ScrollArea::vertical()
        .id_salt(("inspector-scroll", tab.name(), sub))
        .auto_shrink([false, false])
        .show(&mut inner, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: PAD as i8,
                    right: PAD as i8,
                    top: 6,
                    bottom: 18,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let room = ui.max_rect().right();
                    match tab {
                        Tab::Title => title::show(p, ui, c),
                        Tab::Sticker => sticker::show(p, ui, c),
                        Tab::Visualizer => visualizer::show(p, ui, c),
                        Tab::Video => video::show(p, ui, c),
                        Tab::Audio => audio::show(p, ui, c),
                        Tab::Color => color::show(p, ui, c),
                        Tab::Speed => speed::show(p, ui, c),
                        Tab::Animation => animation::show(p, ui, c),
                        Tab::Effects => effects::show(p, ui, c),
                    }
                    // Rows are made to fit the panel; say so when one doesn't.
                    let over = ui.min_rect().right() - room;
                    if over > 0.5 {
                        log::warn!("{} tab runs {over:.0}px past the panel", tab.name());
                    }
                });
        });
}

/// "1.5", "2", "0.25" (no trailing zeros).
pub fn trim_float(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Puts a tab's settings back to a fresh clip's.
fn reset_tab(p: &mut Panel, c: &mut Clip, tab: Tab) {
    match tab {
        Tab::Video => match p.st.video_sub {
            1 => c.video.mask = None,
            2 => c.enhance = Default::default(),
            _ => {
                c.transform = Default::default();
                c.crop = Default::default();
                c.blend = Default::default();
                let mask = c.video.mask.take();
                c.video = VideoSettings {
                    mask,
                    ..Default::default()
                };
            }
        },
        Tab::Audio => {
            c.volume = Animatable::constant(100.0);
            c.audio = tidycuts_core::AudioSettings {
                muted: c.audio.muted,
                stream: c.audio.stream,
                ..Default::default()
            };
        }
        Tab::Color => c.color = Default::default(),
        Tab::Speed => {
            c.reverse = false;
            p.st.speed_drag = None;
            p.actions.push(Action::SetSpeed(1.0));
        }
        Tab::Animation => {
            c.anim_in = None;
            c.anim_out = None;
        }
        Tab::Effects => {
            for e in &mut c.effects {
                e.params.clear();
                e.enabled = true;
            }
        }
        Tab::Title => title::reset(c),
        Tab::Sticker => sticker::reset(c),
        Tab::Visualizer => visualizer::reset(c),
    }
}

/// The tab strip: plain 12.5-pixel labels, the selected one white with an
/// accent underline, the rest faint. Padding shrinks to fit narrow panels.
fn panel_tabs(ui: &mut Ui, selected: &mut usize, items: &[&str]) -> (bool, Option<usize>) {
    let rect = ui.max_rect();
    let font = theme::heading_font(12.5);
    let widths: Vec<f32> = items
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap(l.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
        .collect();
    let avail = rect.width() - 16.0;
    let pad = ((avail - widths.iter().sum::<f32>()) / (2.0 * items.len() as f32)).clamp(2.0, 9.0);
    let mut changed = false;
    let mut reset = None;
    let mut x = rect.left() + 8.0;
    for (i, (label, text_w)) in items.iter().zip(&widths).enumerate() {
        let w = text_w + 2.0 * pad;
        let r = Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height()));
        x += w;
        let resp = ui.interact(r, Id::new(("insp-tab", *label)), Sense::click());
        let on = *selected == i;
        let color = if on {
            Color32::WHITE
        } else if resp.hovered() {
            theme::TEXT_DIM
        } else {
            theme::TEXT_FAINT
        };
        ui.painter().text(
            egui::pos2(r.left() + pad, r.center().y),
            Align2::LEFT_CENTER,
            *label,
            font.clone(),
            color,
        );
        if on {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(r.left() + 6.0_f32.min(pad), rect.bottom() - 2.0),
                    egui::pos2(r.right() - 6.0_f32.min(pad), rect.bottom()),
                ),
                CornerRadius::ZERO,
                theme::ACCENT,
            );
        }
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        resp.context_menu(|ui| {
            if ui.button(format!("Reset {label} settings")).clicked() {
                reset = Some(i);
                ui.close();
            }
        });
        if resp.clicked() && !on {
            *selected = i;
            changed = true;
        }
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    (changed, reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_tab_names_resolve() {
        for name in ["Video", "Color", "Speed", "Audio", "Effects", "animation"] {
            assert!(Tab::from_name(name).is_some(), "{name}");
        }
        assert_eq!(Tab::from_name("Image"), Some(Tab::Video));
        assert_eq!(Tab::from_name("nonsense"), None);
    }

    #[test]
    fn request_tab_is_remembered() {
        let mut st = InspectorState::default();
        st.request_tab("Speed");
        assert_eq!(st.requested.as_deref(), Some("Speed"));
    }

    #[test]
    fn audio_clips_get_audio_and_speed_tabs_only() {
        assert_eq!(tabs_for(Kind::Audio, None), vec![Tab::Audio, Tab::Speed]);
        assert!(!tabs_for(Kind::Image, None).contains(&Tab::Speed));
        assert_eq!(tabs_for(Kind::Title, None)[0], Tab::Title);
    }

    #[test]
    fn trims_trailing_zeros() {
        assert_eq!(trim_float(2.0), "2");
        assert_eq!(trim_float(0.25), "0.25");
        assert_eq!(trim_float(1.50), "1.5");
    }
}
