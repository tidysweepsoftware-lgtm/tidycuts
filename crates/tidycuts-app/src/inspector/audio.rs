//! Audio tab: Detach Audio, one-click Enhance Speech, volume in dB with
//! keyframes and leveling, fades, balance and pitch, Voice Clean-up,
//! hiss/wind/hum removal and de-essing, a 10-band equalizer with presets,
//! voice changer, ducking and the audio stream picker.

use super::kit::{self, Sec};
use super::{Action, Kind, Panel};
use crate::theme;
use crate::widgets::{self, db_to_percent, percent_to_db};
use egui::{Align2, Color32, CornerRadius, CursorIcon, FontId, Rect, Sense, Shape, Stroke, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{Animatable, AudioSettings, Clip, Time, TrackKind};
use tidycuts_engine::audio_fx::{
    clear_enhance, enhance_speech, is_enhanced, EQ_BANDS, EQ_PRESETS, VOICE_EFFECTS,
};
use tidycuts_engine::voice_clean::{self, Status};

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    if p.track == TrackKind::Video && p.kind == Kind::Video {
        detach(p, ui, c);
    }
    enhance(ui, c);
    volume(p, ui, c);
    fades(ui, c);
    balance(ui, c);
    voice_cleanup(p, ui, c);
    denoise(ui, &mut c.audio);
    equalizer(ui, &mut c.audio);
    voice(ui, &mut c.audio);
    auto_duck(p, ui, c);
    ducking(ui, &mut c.audio);
    if let Some(m) = p.media {
        if m.audio_streams > 1 {
            stream(ui, &mut c.audio, m.audio_streams);
        }
    }
}

fn detach(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let can = p.app.can_detach(c.id);
    let cmd = if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    };
    let detail = if can {
        format!("Move the sound to its own clip on an audio track ({cmd}+Alt+D)")
    } else {
        "This clip's own sound is muted or already detached".to_string()
    };
    if kit::wide_button(ui, ph::LINK_BREAK, "Detach Audio", &detail, can).clicked() {
        p.actions.push(Action::DetachAudio);
    }
    if !can && c.audio.muted {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            kit::hint(ui, "Unmute to hear the clip's own sound again.");
            if kit::text_button(ui, ph::SPEAKER_HIGH, "Unmute").clicked() {
                c.audio.muted = false;
            }
        });
    }
    ui.add_space(8.0);
}

fn enhance(ui: &mut Ui, c: &mut Clip) {
    let on = is_enhanced(&c.audio);
    let detail = if on {
        "On: noise removal, leveling, podcast tone, limiter. Click to switch off."
    } else {
        "Cleans up a voice in one click."
    };
    if kit::wide_button(ui, ph::MAGIC_WAND, "Enhance Speech", detail, true).clicked() {
        if on {
            clear_enhance(&mut c.audio);
        } else {
            enhance_speech(&mut c.audio);
        }
    }
    ui.add_space(8.0);
}

fn voice_cleanup(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let mut on = c.audio.voice_clean > 0.0;
    let stream = c
        .audio
        .stream
        .min(p.media.map_or(0, |m| m.audio_streams.saturating_sub(1)));
    let media = p.media;
    let a = &mut c.audio;
    let out = Sec::new("audio-voice-clean", "Voice Clean-up")
        .switch(&mut on)
        .show(ui, |ui| {
            kit::slider_row(ui, "Amount", &mut a.voice_clean, 0.0..=100.0, 0, "", 70.0);
            kit::row_hint(
                ui,
                &format!(
                    "Keeps the voice, turns down fans, hiss and room noise ({}).",
                    voice_clean::engine_name()
                ),
            );
            // Asking also starts the work, so the cleaned sound is ready by the
            // time the clip plays.
            if let Some(m) = media {
                match voice_clean::request(
                    &m.path,
                    m.duration.as_secs_f64(),
                    stream,
                    a.voice_clean,
                    false,
                ) {
                    Status::Ready(_) => kit::row_hint(ui, "Ready."),
                    Status::Working(f) => {
                        kit::row_hint(ui, &format!("Preparing the voice ({:.0}%)...", f * 100.0));
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(250));
                    }
                    Status::Failed => kit::row_hint(ui, "Could not clean this sound."),
                }
            }
        });
    if out.switched {
        c.audio.voice_clean = if on { 70.0 } else { 0.0 };
    }
}

fn volume(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    // Mute stays as it is: a detached clip's sound is muted on purpose.
    let can_reset = c.volume != Animatable::constant(100.0)
        || c.audio.normalize
        || c.audio.leveler > 0.0
        || c.audio.limiter;
    let out = Sec::new("volume", "Volume")
        .reset(can_reset)
        .show(ui, |ui| {
            kit::anim_row_mapped(
                ui,
                "Volume",
                &mut c.volume,
                local,
                percent_to_db,
                db_to_percent,
                -60.0..=10.0,
                1,
                " dB",
                0.0,
            );
            kit::toggle_row(ui, "Mute", &mut c.audio.muted);
            kit::toggle_row(ui, "Normalize", &mut c.audio.normalize);
            kit::row_hint(
                ui,
                "Normalize evens out the loudness to a standard level (-14 LUFS), so \
                 quiet and loud clips match.",
            );
            kit::slider_row(
                ui,
                "Leveler",
                &mut c.audio.leveler,
                0.0..=100.0,
                0,
                "",
                45.0,
            );
            kit::row_hint(ui, "Lifts quiet speech and holds loud parts.");
            kit::toggle_row(ui, "Peak Limiter", &mut c.audio.limiter);
            kit::row_hint(ui, "Stops loud peaks from distorting.");
        });
    if out.reset {
        c.volume = Animatable::constant(100.0);
        c.audio.normalize = false;
        c.audio.leveler = 0.0;
        c.audio.limiter = false;
    }
}

fn fades(ui: &mut Ui, c: &mut Clip) {
    let max = (c.duration().as_secs_f64() / 2.0).min(30.0);
    let a = &mut c.audio;
    let out = Sec::new("audio-fade", "Fade")
        .reset(a.fade_in != Time::ZERO || a.fade_out != Time::ZERO)
        .show(ui, |ui| {
            kit::time_row(ui, "Fade In", &mut a.fade_in, max);
            kit::time_row(ui, "Fade Out", &mut a.fade_out, max);
        });
    if out.reset {
        a.fade_in = Time::ZERO;
        a.fade_out = Time::ZERO;
    }
}

fn balance(ui: &mut Ui, c: &mut Clip) {
    let speed_changed = (c.speed - 1.0).abs() > 1e-9;
    let a = &mut c.audio;
    let out = Sec::new("audio-balance", "Balance & Pitch")
        .reset(a.pan != 0.0 || a.pitch != 0.0 || !a.keep_pitch)
        .closed()
        .show(ui, |ui| {
            kit::slider_row(ui, "Pan", &mut a.pan, -100.0..=100.0, 0, "", 0.0);
            kit::row_hint(ui, "Left (-100) to right (100).");
            kit::slider_row(ui, "Pitch", &mut a.pitch, -12.0..=12.0, 1, " st", 0.0);
            kit::row_hint(ui, "Semitones up or down without changing the length.");
            kit::toggle_row(ui, "Keep Pitch", &mut a.keep_pitch);
            kit::row_hint(
                ui,
                if speed_changed {
                    "Keeps the natural pitch at the clip's changed speed."
                } else {
                    "Applies when the clip's speed is changed."
                },
            );
        });
    if out.reset {
        a.pan = 0.0;
        a.pitch = 0.0;
        a.keep_pitch = true;
    }
}

fn denoise(ui: &mut Ui, a: &mut AudioSettings) {
    let mut on = a.denoise > 0.0 || a.wind_removal || a.hum_removal != 0 || a.de_ess;
    let out = Sec::new("audio-denoise", "Hiss, Wind & Hum")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            kit::slider_row(ui, "Hiss Removal", &mut a.denoise, 0.0..=100.0, 0, "", 50.0);
            kit::row_hint(ui, "For steady hiss. For voices, use Voice Clean-up.");
            kit::toggle_row(ui, "Wind Removal", &mut a.wind_removal);
            let sel = match a.hum_removal {
                50 => 1,
                60 => 2,
                _ => 0,
            };
            if let Some(i) =
                kit::chips_row(ui, "Hum Removal", &["Off", "50 Hz", "60 Hz"], Some(sel))
            {
                a.hum_removal = [0, 50, 60][i];
            }
            kit::row_hint(
                ui,
                "Removes mains hum: 50 Hz in Europe, Asia and Africa, 60 Hz in the Americas.",
            );
            kit::toggle_row(ui, "De-esser", &mut a.de_ess);
            kit::row_hint(ui, "Softens harsh \"s\" sounds in speech.");
        });
    if out.switched {
        if on {
            a.denoise = 50.0;
        } else {
            a.denoise = 0.0;
            a.wind_removal = false;
            a.hum_removal = 0;
            a.de_ess = false;
        }
    }
}

fn equalizer(ui: &mut Ui, a: &mut AudioSettings) {
    let mut on = a.eq.iter().any(|g| *g != 0.0);
    let out = Sec::new("audio-eq", "Equalizer")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            let mut names: Vec<&str> = EQ_PRESETS.iter().map(|(n, _)| *n).collect();
            let matching = EQ_PRESETS.iter().find(|(_, g)| *g == a.eq).map(|(n, _)| *n);
            let mut current = if !a.eq_preset.is_empty() && matching == Some(a.eq_preset.as_str()) {
                a.eq_preset.clone()
            } else {
                matching.unwrap_or("Custom").to_string()
            };
            if current == "Custom" {
                names.push("Custom");
            }
            let options: Vec<(String, &str)> = names.iter().map(|n| (n.to_string(), *n)).collect();
            if widgets::choice_row(ui, "Preset", &mut current, &options) {
                if let Some((n, g)) = EQ_PRESETS.iter().find(|(n, _)| *n == current) {
                    a.eq = *g;
                    a.eq_preset = if *n == "Default" {
                        String::new()
                    } else {
                        n.to_string()
                    };
                }
            }
            ui.add_space(4.0);
            if graphic_eq(ui, &mut a.eq) {
                a.eq_preset = EQ_PRESETS
                    .iter()
                    .find(|(_, g)| *g == a.eq)
                    .map(|(n, _)| {
                        if *n == "Default" {
                            String::new()
                        } else {
                            n.to_string()
                        }
                    })
                    .unwrap_or_else(|| "Custom".into());
            }
            kit::hint(
                ui,
                "Drag a band up or down (±12 dB). Double-click a band to flatten it.",
            );
        });
    if out.switched {
        if on {
            if let Some((n, g)) = EQ_PRESETS.iter().find(|(n, _)| *n == "Vocal") {
                a.eq = *g;
                a.eq_preset = n.to_string();
            }
        } else {
            a.eq = [0.0; 10];
            a.eq_preset.clear();
        }
    }
}

/// Ten vertical band faders over a response curve. Returns true on a change.
fn graphic_eq(ui: &mut Ui, gains: &mut [f64; 10]) -> bool {
    let id = ui.id().with("graphic-eq");
    let w = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 176.0), Sense::hover());
    let plot = Rect::from_min_max(
        egui::pos2(rect.left() + 34.0, rect.top() + 10.0),
        egui::pos2(rect.right() - 8.0, rect.bottom() - 26.0),
    );
    let p = ui.painter().clone();
    p.rect_filled(
        Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() - 18.0)),
        CornerRadius::same(8),
        theme::FIELD_BG,
    );
    let y_of = |g: f64| plot.center().y - (g / 12.0).clamp(-1.0, 1.0) as f32 * plot.height() / 2.0;
    for g in [12.0, 6.0, 0.0, -6.0, -12.0] {
        let y = y_of(g);
        p.hline(
            plot.x_range(),
            y,
            Stroke::new(
                1.0,
                if g == 0.0 {
                    theme::BORDER.lerp_to_gamma(Color32::WHITE, 0.1)
                } else {
                    theme::BORDER.gamma_multiply(0.6)
                },
            ),
        );
        p.text(
            egui::pos2(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            if g > 0.0 {
                format!("+{g}")
            } else {
                format!("{g}")
            },
            FontId::proportional(10.5),
            theme::TEXT_FAINT,
        );
    }
    let col_w = plot.width() / 10.0;
    let x_of = |i: usize| plot.left() + col_w * (i as f32 + 0.5);
    // Response curve through the band points.
    let pts: Vec<egui::Pos2> = (0..10)
        .map(|i| egui::pos2(x_of(i), y_of(gains[i])))
        .collect();
    let mut curve = Vec::new();
    for i in 0..9usize {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(9)];
        for s in 0..12 {
            let t = s as f32 / 12.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            curve.push(egui::pos2(
                f(p0.x, p1.x, p2.x, p3.x),
                f(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    curve.push(pts[9]);
    let zero = y_of(0.0);
    // Fill under the curve in thin vertical slices (the shape isn't convex).
    for pair in curve.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        p.add(Shape::convex_polygon(
            vec![egui::pos2(a.x, zero), a, b, egui::pos2(b.x, zero)],
            theme::ACCENT.gamma_multiply(0.12),
            Stroke::NONE,
        ));
    }
    p.add(Shape::line(curve, Stroke::new(2.0, theme::ACCENT)));

    let mut changed = false;
    for i in 0..10 {
        let x = x_of(i);
        let hit = Rect::from_center_size(
            egui::pos2(x, plot.center().y),
            egui::vec2(col_w.min(44.0), plot.height() + 18.0),
        );
        let resp = ui
            .interact(hit, id.with(i), Sense::click_and_drag())
            .on_hover_cursor(CursorIcon::ResizeVertical);
        if resp.double_clicked() {
            gains[i] = 0.0;
            changed = true;
        } else if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let g = ((plot.center().y - pos.y) / (plot.height() / 2.0) * 12.0) as f64;
                let g = (g.clamp(-12.0, 12.0) * 2.0).round() / 2.0;
                if g != gains[i] {
                    gains[i] = g;
                    changed = true;
                }
            }
        }
        let active = resp.hovered() || resp.dragged();
        p.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(
                if active { 2.0 } else { 1.0 },
                if active {
                    theme::TEXT_FAINT
                } else {
                    theme::PANEL_BG_3
                },
            ),
        );
        let y = y_of(gains[i]);
        p.circle(
            egui::pos2(x, y),
            if active { 7.0 } else { 6.0 },
            if active {
                Color32::WHITE
            } else {
                theme::ACCENT
            },
            Stroke::new(2.0, theme::FIELD_BG),
        );
        if active || gains[i] != 0.0 {
            let label = if gains[i] > 0.0 {
                format!("+{:.1}", gains[i])
            } else {
                format!("{:.1}", gains[i])
            };
            let ty = if gains[i] >= 0.0 { y - 14.0 } else { y + 14.0 };
            p.text(
                egui::pos2(x, ty),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.5),
                if active {
                    Color32::WHITE
                } else {
                    theme::TEXT_DIM
                },
            );
        }
        let hz = EQ_BANDS[i];
        let label = if hz >= 1000.0 {
            format!("{}k", hz / 1000.0)
        } else {
            format!("{hz}")
        };
        p.text(
            egui::pos2(x, rect.bottom() - 8.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            if active { theme::TEXT } else { theme::TEXT_DIM },
        );
    }
    p.text(
        egui::pos2(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_CENTER,
        "Hz",
        FontId::proportional(10.5),
        theme::TEXT_FAINT,
    );
    changed
}

fn voice_icon(id: &str) -> &'static str {
    match id {
        "" => ph::PROHIBIT,
        "robot" => ph::ROBOT,
        "chipmunk" => ph::PAW_PRINT,
        "deep" => ph::ARROW_FAT_DOWN,
        "child" => ph::BABY,
        "monster" => ph::GHOST,
        "radio" => ph::RADIO,
        "telephone" => ph::PHONE,
        "megaphone" => ph::MEGAPHONE,
        "echo" => ph::WAVES,
        "hall" => ph::CHURCH,
        "underwater" => ph::DROP,
        "alien" => ph::ALIEN,
        "chorus" => ph::USERS,
        "flanger" => ph::WAVE_SINE,
        _ => ph::MICROPHONE,
    }
}

fn voice(ui: &mut Ui, a: &mut AudioSettings) {
    let mut on = !a.voice.is_empty();
    let out = Sec::new("audio-voice", "Voice Changer")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            let selected = VOICE_EFFECTS.iter().position(|(id, _)| *id == a.voice);
            let clicked = widgets::tile_grid(
                ui,
                "voice-effects",
                VOICE_EFFECTS.len(),
                egui::vec2(82.0, 48.0),
                selected,
                |i| VOICE_EFFECTS[i].1.to_string(),
                |painter, rect, i, hovered| {
                    let on = selected == Some(i);
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        voice_icon(VOICE_EFFECTS[i].0),
                        FontId::proportional(22.0),
                        if on {
                            theme::ACCENT
                        } else if hovered {
                            Color32::WHITE
                        } else {
                            theme::TEXT_DIM
                        },
                    );
                },
            );
            if let Some(i) = clicked {
                a.voice = VOICE_EFFECTS[i].0.to_string();
            }
        });
    if out.switched {
        a.voice = if on { "robot".into() } else { String::new() };
    }
}

/// One click that lowers this clip (the music) wherever someone is talking.
fn auto_duck(p: &mut Panel, ui: &mut Ui, c: &Clip) {
    if crate::autoduck::unsuitable(p.app, c.id).is_some() {
        return;
    }
    if kit::wide_button(
        ui,
        ph::SPEAKER_SIMPLE_LOW,
        "Lower Under Speech",
        "Finds where someone talks on the rest of the timeline and lowers this clip there.",
        true,
    )
    .clicked()
    {
        p.actions.push(Action::AutoDuck);
    }
    ui.add_space(8.0);
}

fn ducking(ui: &mut Ui, a: &mut AudioSettings) {
    let mut on = a.ducking > 0.0;
    let out = Sec::new("audio-ducking", "Audio Ducking")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            kit::slider_row(
                ui,
                "Lower Others",
                &mut a.ducking,
                0.0..=100.0,
                0,
                "%",
                50.0,
            );
            kit::row_hint(
                ui,
                "While this clip plays, every other clip gets quieter by this much, \
                 so music sits under a voice-over.",
            );
        });
    if out.switched {
        a.ducking = if on { 50.0 } else { 0.0 };
    }
}

fn stream(ui: &mut Ui, a: &mut AudioSettings, count: u32) {
    Sec::new("audio-stream", "Audio Track").show(ui, |ui| {
        let names: Vec<String> = (1..=count).map(|i| format!("Track {i}")).collect();
        let options: Vec<(u32, &str)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u32, n.as_str()))
            .collect();
        widgets::choice_row(ui, "Play", &mut a.stream, &options);
        kit::row_hint(
            ui,
            "This file has several sound tracks (languages, commentary, separate \
             microphones). Pick the one to hear.",
        );
    });
}
