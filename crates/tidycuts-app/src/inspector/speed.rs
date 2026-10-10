//! Speed tab: uniform speed with presets, duration, reverse and pitch.
//! Speed and length changes move the clips after this one, so they're
//! applied as one undo step when a drag ends rather than every frame.

use super::kit::{self, Sec};
use super::{trim_float, Action, Panel};
use crate::theme;
use crate::widgets;
use egui::{
    Color32, CornerRadius, CursorIcon, Pos2, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
};
use egui_phosphor::regular as ph;
use tidycuts_core::{Clip, RampPreset, SpeedRamp, Time, MAX_SPEED, MIN_SPEED, RAMP_MAX, RAMP_MIN};

/// Width of the small reset button after the speed value.
const RESET_W: f32 = 22.0;

/// The length box leaves room for the timecode beside it.
const DURATION_BOX_W: f32 = 84.0;

const PRESETS: [f64; 10] = [0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0, 8.0, 16.0];

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let speed = p.st.speed_drag.unwrap_or(c.speed);
    let out = Sec::new("uniform-speed", "Uniform Speed")
        .reset((c.speed - 1.0).abs() > 1e-9)
        .show(ui, |ui| {
            speed_row(p, ui, c);
            let labels: Vec<String> = PRESETS
                .iter()
                .map(|s| format!("{}x", trim_float(*s)))
                .collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let sel = PRESETS.iter().position(|s| (s - speed).abs() < 1e-6);
            if let Some(i) = kit::chips_row(ui, "Presets", &refs, sel) {
                p.st.speed_drag = None;
                p.actions.push(Action::SetSpeed(PRESETS[i]));
            }
            duration_row(p, ui, c);
            let orig = c.source_len();
            kit::row_hint(
                ui,
                &format!(
                    "Original length {}. Clips after this one move to make room.",
                    widgets::short_duration(orig)
                ),
            );
        });
    if out.reset {
        p.st.speed_drag = None;
        p.actions.push(Action::SetSpeed(1.0));
    }
    // A drag that ended without the slider noticing (released elsewhere).
    if !ui.ctx().input(|i| i.pointer.any_down()) {
        if let Some(v) = p.st.speed_drag.take() {
            p.actions.push(Action::SetSpeed(v));
        }
    }

    ramp_section(p, ui, c);

    let has_sound = p
        .media
        .is_some_and(|m| m.has_audio || m.kind == tidycuts_core::MediaKind::Audio);
    Sec::new("playback", "Playback").show(ui, |ui| {
        if c.media_id.is_some() {
            kit::toggle_row(ui, "Reverse", &mut c.reverse);
            kit::row_hint(ui, "Plays the clip backwards, sound included.");
        }
        if has_sound {
            kit::toggle_row(ui, "Keep Pitch", &mut c.audio.keep_pitch);
            kit::row_hint(
                ui,
                "Keeps voices sounding natural when the speed changes. Off, sped-up \
                 sound gets higher like a tape.",
            );
        }
    });
}

/// A logarithmic speed slider, a value box and a reset button.
fn speed_row(p: &mut Panel, ui: &mut Ui, c: &Clip) {
    let mut v = p.st.speed_drag.unwrap_or(c.speed);
    widgets::row(ui, "Speed", |ui| {
        let s = widgets::slim_slider(
            ui,
            &mut v,
            MIN_SPEED..=MAX_SPEED,
            // Room for the value and the reset button after it.
            kit::slider_width(ui.available_width() - RESET_W - widgets::ROW_GAP),
            true,
        );
        let d = widgets::readout(ui, &mut v, MIN_SPEED..=MAX_SPEED, 2, "x", widgets::BOX_W);
        let reset = ui
            .add_enabled_ui((c.speed - 1.0).abs() > 1e-9, |ui| {
                widgets::icon_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "Reset to 1x")
            })
            .inner;
        if reset.clicked() {
            p.st.speed_drag = None;
            p.actions.push(Action::SetSpeed(1.0));
        } else if s.dragged() || d.dragged() {
            p.st.speed_drag = Some(v);
        } else if s.drag_stopped() || d.drag_stopped() || s.changed() || d.changed() {
            p.st.speed_drag = None;
            p.actions.push(Action::SetSpeed(v));
        }
    });
}

/// The clip's length on the timeline. For video and audio it changes the
/// speed; for images, titles and other generated clips it trims the end.
pub fn duration_row(p: &mut Panel, ui: &mut Ui, c: &Clip) {
    let fixed = p.fixed_length(c);
    let current = match (fixed, p.st.speed_drag) {
        (true, Some(s)) => c.source_len().as_secs_f64() / s,
        _ => c.duration().as_secs_f64(),
    };
    let mut v = p.st.duration_drag.unwrap_or(current);
    let (lo, hi) = if fixed {
        let src = c.source_len().as_secs_f64();
        (src / MAX_SPEED, src / MIN_SPEED)
    } else {
        (0.1, 3600.0)
    };
    widgets::row(ui, "Duration", |ui| {
        let d = ui.add_sized(
            [DURATION_BOX_W, 24.0],
            egui::DragValue::new(&mut v)
                .range(lo..=hi)
                .speed(0.05)
                .max_decimals(2)
                .min_decimals(2)
                .suffix(" s")
                .update_while_editing(false),
        );
        let tc = Time::from_secs_f64(v).timecode(p.app.project.settings.frame_rate);
        ui.label(
            RichText::new(tc)
                .monospace()
                .size(11.5)
                .color(theme::TEXT_DIM),
        );
        let apply = |v: f64, actions: &mut Vec<Action>| {
            if fixed {
                if let Some(ramp) = &c.ramp {
                    // A ramp keeps its shape and is stretched to the new length.
                    let k = c.duration().as_secs_f64() / v.max(1e-6);
                    actions.push(Action::SetRamp(Some(ramp.scaled(k))));
                } else {
                    let s =
                        (c.source_len().as_secs_f64() / v.max(1e-6)).clamp(MIN_SPEED, MAX_SPEED);
                    actions.push(Action::SetSpeed(s));
                }
            } else {
                actions.push(Action::SetDuration(Time::from_secs_f64(v)));
            }
        };
        if d.dragged() {
            p.st.duration_drag = Some(v);
        } else if d.drag_stopped()
            || d.changed()
            || (p.st.duration_drag.is_some() && !ui.ctx().input(|i| i.pointer.any_down()))
        {
            // Released, typed, or the release happened off the box.
            p.st.duration_drag = None;
            apply(v, &mut p.actions);
        }
    });
}

// ----- speed ramping ----------------------------------------------------------

/// How close (pixels) the pointer must be to grab a point.
const GRAB_PX: f32 = 10.0;
/// Speeds this close to 1x snap to it (as a fraction of the log scale).
const SNAP_ONE: f64 = 0.025;

fn ramp_section(p: &mut Panel, ui: &mut Ui, c: &Clip) {
    if c.media_id.is_none() {
        return;
    }
    let out = Sec::new("speed-ramp", "Speed Ramping")
        .reset(c.ramp.is_some())
        .show(ui, |ui| {
            let labels: Vec<&str> = RampPreset::ALL.iter().map(|r| r.label()).collect();
            let sel = c
                .ramp
                .as_ref()
                .and_then(|r| RampPreset::ALL.iter().position(|x| *x == r.preset));
            if let Some(i) = kit::chips(ui, &labels, sel) {
                // Lay the shape over the clip in the order it plays.
                let (a, b) = if c.reverse {
                    (c.source_out, c.source_in)
                } else {
                    (c.source_in, c.source_out)
                };
                p.st.ramp_drag = None;
                p.actions.push(Action::SetRamp(Some(SpeedRamp::preset(
                    RampPreset::ALL[i],
                    a,
                    b,
                ))));
            }
            ui.add_space(6.0);
            match c.ramp.clone() {
                Some(current) => {
                    let mut ramp = p.st.ramp_drag.clone().unwrap_or(current);
                    let playing = (p.local <= c.duration() && p.local >= Time::ZERO)
                        .then(|| c.source_secs(p.local.as_secs_f64()));
                    match ramp_graph(ui, &mut ramp, c.source_in, c.source_out, playing) {
                        GraphEdit::None => {}
                        GraphEdit::Dragging => p.st.ramp_drag = Some(ramp.clone()),
                        GraphEdit::Done => {
                            p.st.ramp_drag = None;
                            p.actions.push(Action::SetRamp(Some(ramp.clone())));
                        }
                    }
                    let len = Time::from_secs_f64(
                        ramp.play_secs(c.source_in.as_secs_f64(), c.source_out.as_secs_f64()),
                    );
                    kit::row_hint(
                        ui,
                        &format!(
                            "Plays {} of footage in {}. Click the line to add a point, drag \
                             to change it, double-click a point to remove it. Clips after \
                             this one move to make room.",
                            widgets::short_duration(c.source_len()),
                            widgets::short_duration(len)
                        ),
                    );
                }
                None => kit::row_hint(
                    ui,
                    "Pick a shape to change the speed within the clip: slow motion for the \
                     big moment, fast before and after.",
                ),
            }
        });
    if out.reset {
        p.st.ramp_drag = None;
        p.actions.push(Action::SetRamp(None));
    }
    // A drag that ended off the graph.
    if p.st.ramp_drag.is_some() && !ui.ctx().input(|i| i.pointer.any_down()) {
        if let Some(r) = p.st.ramp_drag.take() {
            p.actions.push(Action::SetRamp(Some(r)));
        }
    }
}

enum GraphEdit {
    None,
    /// A point is being dragged; the ramp shows the change.
    Dragging,
    /// The change is finished and should be applied.
    Done,
}

/// The speed curve over the clip's part of the source: time across, speed
/// up (logarithmic, 0.1x to 10x).
fn ramp_graph(
    ui: &mut Ui,
    ramp: &mut SpeedRamp,
    lo: Time,
    hi: Time,
    playing: Option<f64>,
) -> GraphEdit {
    let (lo_s, hi_s) = (
        lo.as_secs_f64(),
        hi.as_secs_f64().max(lo.as_secs_f64() + 0.01),
    );
    let size = egui::vec2(ui.available_width().min(380.0), 150.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let plot = rect
        .shrink2(egui::vec2(30.0, 10.0))
        .translate(egui::vec2(12.0, 0.0));
    let span = (RAMP_MAX / RAMP_MIN).ln();
    let to_screen = |x: f64, v: f64| -> Pos2 {
        egui::pos2(
            plot.left() + ((x - lo_s) / (hi_s - lo_s)) as f32 * plot.width(),
            plot.bottom() - ((v / RAMP_MIN).ln() / span) as f32 * plot.height(),
        )
    };
    let from_screen = |p: Pos2| -> (f64, f64) {
        let x = lo_s + ((p.x - plot.left()) / plot.width()) as f64 * (hi_s - lo_s);
        let f = ((plot.bottom() - p.y) / plot.height()) as f64;
        (x, RAMP_MIN * (f.clamp(0.0, 1.0) * span).exp())
    };
    let inside: Vec<usize> = ramp
        .points
        .iter()
        .enumerate()
        .filter(|(_, q)| q.at >= lo && q.at <= hi)
        .map(|(i, _)| i)
        .collect();
    let nearest = |ramp: &SpeedRamp, p: Pos2| -> Option<usize> {
        inside
            .iter()
            .map(|i| {
                let q = ramp.points[*i];
                (*i, to_screen(q.at.as_secs_f64(), q.speed).distance(p))
            })
            .filter(|(_, d)| *d <= GRAB_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let id = resp.id.with("grab");
    let mut result = GraphEdit::None;
    if resp.drag_started() {
        let origin = ui
            .ctx()
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some(pos) = origin.filter(|q| plot.expand(GRAB_PX).contains(*q)) {
            let hit = nearest(ramp, pos).or_else(|| {
                // A new point on the line where it was pressed.
                let (x, _) = from_screen(pos);
                let at = Time::from_secs_f64(x.clamp(lo_s, hi_s));
                if ramp
                    .points
                    .iter()
                    .any(|q| (q.at - at).abs() < Time::from_millis(30))
                {
                    return None;
                }
                let speed = ramp.speed_at(at.as_secs_f64());
                ramp.points.push(tidycuts_core::RampPoint { at, speed });
                ramp.points.sort_by_key(|q| q.at);
                ramp.preset = RampPreset::Custom;
                ramp.points.iter().position(|q| q.at == at)
            });
            ui.data_mut(|d| d.insert_temp(id, hit));
            if hit.is_some() {
                result = GraphEdit::Dragging;
            }
        }
    }
    if let (true, Some(pos)) = (resp.dragged(), resp.interact_pointer_pos()) {
        if let Some(Some(i)) = ui.data(|d| d.get_temp::<Option<usize>>(id)) {
            if i < ramp.points.len() {
                let (x, mut v) = from_screen(pos);
                if (v.ln() / span).abs() < SNAP_ONE {
                    v = 1.0;
                }
                let gap = 0.02;
                let min = if i == 0 {
                    lo_s
                } else {
                    ramp.points[i - 1].at.as_secs_f64() + gap
                };
                let max = if i + 1 == ramp.points.len() {
                    hi_s
                } else {
                    ramp.points[i + 1].at.as_secs_f64() - gap
                };
                ramp.points[i].at = Time::from_secs_f64(x.clamp(min, max.max(min)));
                ramp.points[i].speed = v.clamp(RAMP_MIN, RAMP_MAX);
                ramp.preset = RampPreset::Custom;
                result = GraphEdit::Dragging;
            }
        }
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<Option<usize>>(id));
        ramp.tidy();
        return GraphEdit::Done;
    }
    if resp.double_clicked() || resp.secondary_clicked() {
        if let Some(i) = resp
            .interact_pointer_pos()
            .and_then(|pos| nearest(ramp, pos))
            .filter(|_| ramp.points.len() > 2)
        {
            ramp.points.remove(i);
            ramp.preset = RampPreset::Custom;
            return GraphEdit::Done;
        }
    }
    if resp.hovered() {
        let near = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|pos| nearest(ramp, pos).is_some());
        ui.ctx().set_cursor_icon(if near {
            CursorIcon::Grab
        } else {
            CursorIcon::Crosshair
        });
    }

    // Drawing.
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(6),
        theme::FIELD_BG,
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    for v in [0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 10.0] {
        let y = to_screen(lo_s, v).y;
        let stroke = if v == 1.0 {
            Stroke::new(1.0, theme::TEXT_FAINT)
        } else {
            Stroke::new(1.0, theme::BORDER.gamma_multiply(0.7))
        };
        painter.hline(plot.x_range(), y, stroke);
        painter.text(
            egui::pos2(rect.left() + 6.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{}x", trim_float(v)),
            egui::FontId::proportional(10.0),
            theme::TEXT_FAINT,
        );
    }
    let steps = 120;
    let line: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let x = lo_s + (hi_s - lo_s) * i as f64 / steps as f64;
            to_screen(x, ramp.speed_at(x))
        })
        .collect();
    painter.add(Shape::line(line, Stroke::new(2.0, theme::ACCENT)));
    if let Some(x) = playing.filter(|x| *x >= lo_s && *x <= hi_s) {
        let sx = to_screen(x, 1.0).x;
        painter.vline(sx, plot.y_range(), Stroke::new(1.0, theme::PLAYHEAD));
    }
    for i in inside {
        let q = ramp.points[i];
        let pos = to_screen(q.at.as_secs_f64(), q.speed);
        painter.circle_filled(pos, 5.5, Color32::WHITE);
        painter.circle_stroke(pos, 5.5, Stroke::new(1.5, theme::ACCENT));
    }
    result
}
