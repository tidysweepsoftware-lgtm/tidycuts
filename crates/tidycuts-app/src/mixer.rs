//! The audio mixer: a volume fader, mute, solo and level meter for every
//! track that can make sound, and a master fader. Volumes live in the
//! project, so they play and export the same.

use crate::app::TidyCutsApp;
use crate::theme;
use crate::widgets;
use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as ph;
use std::collections::HashMap;
use tidycuts_core::{MediaKind, Project, TrackId, TrackKind};

/// Fader range, percent. 100 leaves the sound as it is.
pub const MAX_VOLUME: f64 = 200.0;

/// Smoothed meter heights between frames.
#[derive(Default)]
pub struct MixerState {
    pub open: bool,
    tracks: HashMap<TrackId, [f32; 2]>,
    /// Smoothed left and right levels of everything playing (1.0 = full scale).
    pub master: [f32; 2],
    /// The highest recent level per channel, held for a moment, and when it
    /// was reached.
    pub peak_hold: [f32; 2],
    peak_at: [Option<std::time::Instant>; 2],
    /// Whether a channel went over full scale since the lamp was last cleared.
    pub clipped: [bool; 2],
    /// Momentary loudness of what is playing, LUFS (`None` when silent or stopped).
    pub lufs: Option<f32>,
    /// How far the hearing guard is turning the sound down (decibels, 0 or less).
    pub guard_db: f32,
}

impl MixerState {
    /// Clears the clip lamps and the held peaks.
    pub fn reset_peaks(&mut self) {
        self.peak_hold = [0.0; 2];
        self.peak_at = [None; 2];
        self.clipped = [false; 2];
    }
}

/// Reads the levels of what is playing and eases them down when it stops.
/// Called every frame (the timeline's sound meter is always on).
pub fn update_levels(app: &mut TidyCutsApp, ctx: &egui::Context) {
    let live = app
        .engine
        .as_ref()
        .filter(|e| e.playback.is_playing())
        .map(|e| e.playback.meters().read());
    let playing = live.is_some();
    let mut moving = false;
    {
        let state = live.unwrap_or_default();
        let mut ease = |old: &mut [f32; 2], new: [f32; 2]| {
            for i in 0..2 {
                old[i] = new[i].max(old[i] * 0.86);
                if old[i] < 1e-3 {
                    old[i] = 0.0;
                } else {
                    moving = true;
                }
            }
        };
        ease(&mut app.mixer.master, state.master);
        app.mixer.lufs = state.lufs;
        app.mixer.guard_db = state.guard_db;
        let ids: Vec<TrackId> = app.project.tracks.iter().map(|t| t.id).collect();
        for id in ids {
            let new = state.tracks.get(&id).copied().unwrap_or([0.0; 2]);
            ease(app.mixer.tracks.entry(id).or_default(), new);
        }
        // Peak hold: stays for 1.2 s, then falls away. A lamp lights when a
        // channel reaches full scale.
        let now = std::time::Instant::now();
        let m = &mut app.mixer;
        for i in 0..2 {
            let raw = state.master[i];
            if raw >= m.peak_hold[i] {
                m.peak_hold[i] = raw;
                m.peak_at[i] = Some(now);
            } else if m.peak_at[i].is_some_and(|t| t.elapsed().as_secs_f32() > 1.2) {
                m.peak_hold[i] *= 0.93;
                if m.peak_hold[i] < 1e-3 {
                    m.peak_hold[i] = 0.0;
                    m.peak_at[i] = None;
                } else {
                    moving = true;
                }
            } else if m.peak_at[i].is_some() {
                moving = true;
            }
            if raw >= 0.995 {
                m.clipped[i] = true;
            }
        }
    }
    if moving || playing {
        ctx.request_repaint();
    }
}

/// Decibels for a volume percent ("-∞" at zero).
pub fn volume_label(percent: f64) -> String {
    if percent <= 0.05 {
        return "-∞ dB".into();
    }
    format!("{:+.1} dB", 20.0 * (percent / 100.0).log10())
}

/// Where a level (1.0 = full scale) sits on a meter: -60 dB at the bottom,
/// +3 dB at the top.
pub fn meter_fraction(level: f32) -> f32 {
    if level <= 1e-4 {
        return 0.0;
    }
    ((20.0 * level.log10() + 60.0) / 63.0).clamp(0.0, 1.0)
}

/// Whether a track can make sound at all (shown in the mixer).
fn has_sound(project: &Project, track: &tidycuts_core::Track) -> bool {
    track.kind == TrackKind::Audio
        || track.clips.iter().any(|c| {
            c.media_id
                .and_then(|m| project.media(m))
                .is_some_and(|m| m.kind == MediaKind::Video && m.has_audio)
        })
}

pub fn show(app: &mut TidyCutsApp, ctx: &egui::Context) {
    if !app.mixer.open {
        return;
    }
    let strips: Vec<(TrackId, String, f64, bool, bool)> = app
        .project
        .tracks
        .iter()
        .filter(|t| has_sound(&app.project, t))
        .map(|t| (t.id, t.name.clone(), t.volume, t.muted, t.solo))
        .collect();
    let master = app.project.master_volume;
    let mut open = true;
    let mut change: Option<Change> = None;
    egui::Window::new("Audio Mixer")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL_BG)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::same(12))
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 28,
                    spread: 0,
                    color: Color32::from_black_alpha(120),
                }),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            ui.horizontal_top(|ui| {
                if strips.is_empty() {
                    ui.label(
                        RichText::new("Add video with sound or an audio track to mix it here.")
                            .color(theme::TEXT_DIM),
                    );
                }
                for (id, name, volume, muted, solo) in &strips {
                    let levels = app.mixer.tracks.get(id).copied().unwrap_or([0.0; 2]);
                    if let Some(c) = strip(ui, *id, name, *volume, *muted, *solo, levels) {
                        change = Some(c);
                    }
                }
                ui.separator();
                let levels = app.mixer.master;
                if let Some(c) = master_strip(ui, master, levels) {
                    change = Some(c);
                }
            });
        });
    if !open {
        app.mixer.open = false;
    }
    if let Some(c) = change {
        app.edit_mix(ctx, |p| match c {
            Change::Volume(id, v) => {
                if let Some(t) = p.track_mut(id) {
                    t.volume = v;
                }
            }
            Change::Mute(id) => {
                if let Some(t) = p.track_mut(id) {
                    t.muted = !t.muted;
                }
            }
            Change::Solo(id) => {
                if let Some(t) = p.track_mut(id) {
                    t.solo = !t.solo;
                }
            }
            Change::Master(v) => p.master_volume = v,
        });
    }
}

#[derive(Clone, Copy)]
enum Change {
    Volume(TrackId, f64),
    Mute(TrackId),
    Solo(TrackId),
    Master(f64),
}

const STRIP_H: f32 = 230.0;

/// Two thin bars (left and right) filling with the level.
fn meter(ui: &mut Ui, rect: Rect, levels: [f32; 2]) {
    let p = ui.painter();
    let w = (rect.width() - 3.0) / 2.0;
    for (i, level) in levels.iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * (w + 3.0), rect.top()),
            egui::vec2(w, rect.height()),
        );
        p.rect_filled(r, CornerRadius::same(2), theme::FIELD_BG);
        let f = meter_fraction(*level);
        if f > 0.0 {
            // Green up to about -12 dB, then amber, red past -3 dB.
            let h = r.height() * f;
            let fill = Rect::from_min_max(egui::pos2(r.left(), r.bottom() - h), r.max);
            let color = if *level > 0.7 {
                theme::DANGER
            } else if *level > 0.25 {
                theme::WARNING
            } else {
                theme::ACCENT
            };
            p.rect_filled(fill, CornerRadius::same(2), color);
        }
    }
}

/// A fader with a meter beside it; returns the new volume when dragged.
fn fader(ui: &mut Ui, volume: f64, levels: [f32; 2]) -> Option<f64> {
    let mut v = volume;
    let mut out = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let r = widgets::slim_fader(ui, &mut v, 0.0..=MAX_VOLUME, STRIP_H - 40.0);
        if r.changed() {
            // A small notch at 100%: close values snap to unity.
            if (v - 100.0).abs() < 2.5 {
                v = 100.0;
            }
            out = Some(v);
        }
        if r.double_clicked() {
            out = Some(100.0);
        }
        let (m, _) = ui.allocate_exact_size(egui::vec2(16.0, STRIP_H - 40.0), Sense::hover());
        meter(ui, m, levels);
    });
    out
}

fn small_button(ui: &mut Ui, text: &str, on: bool, color: Color32, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(28.0, 24.0), Sense::click());
    let fill = if on {
        color
    } else if resp.hovered() {
        theme::PANEL_BG_3
    } else {
        theme::PANEL_BG_2
    };
    ui.painter().rect_filled(r, CornerRadius::same(5), fill);
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.5),
        if on {
            theme::ACCENT_TEXT
        } else {
            theme::TEXT_DIM
        },
    );
    resp.on_hover_text(tip).clicked()
}

fn strip(
    ui: &mut Ui,
    id: TrackId,
    name: &str,
    volume: f64,
    muted: bool,
    solo: bool,
    levels: [f32; 2],
) -> Option<Change> {
    let mut change = None;
    ui.allocate_ui(Vec2::new(86.0, STRIP_H + 34.0), |ui| {
        ui.vertical_centered(|ui| {
            ui.set_width(80.0);
            ui.label(RichText::new(name).size(12.5).color(theme::TEXT));
            if let Some(v) = fader(ui, volume, levels) {
                change = Some(Change::Volume(id, v));
            }
            ui.label(
                RichText::new(volume_label(volume))
                    .monospace()
                    .size(11.5)
                    .color(theme::TEXT_DIM),
            );
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if small_button(ui, "M", muted, theme::DANGER, "Mute this track") {
                    change = Some(Change::Mute(id));
                }
                if small_button(ui, "S", solo, theme::WARNING, "Solo: hear only this track") {
                    change = Some(Change::Solo(id));
                }
            });
        });
    });
    change
}

fn master_strip(ui: &mut Ui, volume: f64, levels: [f32; 2]) -> Option<Change> {
    let mut change = None;
    ui.allocate_ui(Vec2::new(86.0, STRIP_H + 34.0), |ui| {
        ui.vertical_centered(|ui| {
            ui.set_width(80.0);
            ui.label(
                RichText::new(format!("{}  Master", ph::SPEAKER_HIGH))
                    .size(12.5)
                    .color(theme::ACCENT),
            );
            if let Some(v) = fader(ui, volume, levels) {
                change = Some(Change::Master(v));
            }
            ui.label(
                RichText::new(volume_label(volume))
                    .monospace()
                    .size(11.5)
                    .color(theme::TEXT_DIM),
            );
        });
    });
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibel_labels_read_plainly() {
        assert_eq!(volume_label(100.0), "+0.0 dB");
        assert_eq!(volume_label(50.0), "-6.0 dB");
        assert_eq!(volume_label(200.0), "+6.0 dB");
        assert_eq!(volume_label(0.0), "-∞ dB");
    }

    #[test]
    fn meters_run_from_minus_sixty_to_plus_three() {
        assert_eq!(meter_fraction(0.0), 0.0);
        assert!(meter_fraction(1.0) > 0.9 && meter_fraction(1.0) < 1.0);
        assert!((meter_fraction(0.5) - (54.0 / 63.0)).abs() < 0.01);
        assert_eq!(meter_fraction(10.0), 1.0);
    }
}
