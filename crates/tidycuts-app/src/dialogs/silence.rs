//! Silence Detection: look at a clip's sound, find the quiet
//! stretches with a volume threshold, a shortest length and a softening
//! buffer, see them marked on the waveform, choose which to cut, and
//! export the cuts to the timeline.

use super::{body, footer, header, primary, scroll_middle, secondary, window};
use crate::app::TidyCutsApp;
use crate::detection::{Detection, Found};
use crate::theme;
use crate::thumbs::PEAKS_PER_SEC;
use crate::widgets;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind,
};
use egui_phosphor::regular as ph;
use std::sync::mpsc::{channel, Receiver};
use tidycuts_core::{ClipId, MediaItem, MediaKind};

/// A quiet stretch the search found, in the file's own seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seg {
    start: f64,
    end: f64,
    /// Will be cut out when exported.
    cut: bool,
}

enum Run {
    Idle,
    Working(Receiver<Result<Vec<(f64, f64)>, String>>),
    Done,
    Failed(String),
}

pub struct SilenceDialog {
    clip: ClipId,
    /// How quiet counts as silence, 0..100 (see [`threshold_db`]).
    percent: f64,
    /// Pauses at least this long (seconds) are found.
    min_secs: f64,
    /// Sound kept either side of each cut, so words aren't clipped.
    buffer: f64,
    segs: Vec<Seg>,
    run: Run,
    /// The moment shown in the preview, in file seconds.
    cursor: f64,
    zoom: f64,
    /// Which part is in view when zoomed, 0..1.
    scroll: f64,
}

/// The sound level a threshold of `percent` stands for: 0% is a very quiet
/// -60 dB, 100% a loud -0 dB.
pub fn threshold_db(percent: f64) -> f64 {
    -60.0 + 0.6 * percent.clamp(0.0, 100.0)
}

impl SilenceDialog {
    pub fn new(clip: ClipId) -> Self {
        SilenceDialog {
            clip,
            percent: 42.0,
            min_secs: 0.5,
            buffer: 0.1,
            segs: Vec::new(),
            run: Run::Idle,
            cursor: 0.0,
            zoom: 1.0,
            scroll: 0.0,
        }
    }

    /// What will be cut: every chosen stretch, less the buffer at each end.
    fn cuts(&self) -> Vec<(f64, f64)> {
        self.segs
            .iter()
            .filter(|s| s.cut)
            .map(|s| (s.start + self.buffer, s.end - self.buffer))
            .filter(|(a, b)| b - a > 0.02)
            .collect()
    }

    fn analyze(&mut self, app: &TidyCutsApp, range: (f64, f64), ctx: &egui::Context) {
        let Some(path) = app
            .project
            .clip(self.clip)
            .and_then(|(_, c)| c.media_id)
            .and_then(|m| app.project.media(m))
            .map(|m| m.path.clone())
        else {
            return;
        };
        let kind = Detection::Silences {
            noise_db: threshold_db(self.percent),
            min_secs: self.min_secs,
        };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = crate::detection::search(kind, &path, range).and_then(|f| match f {
                Found::Silences(v) => Ok(v),
                _ => Err("Unexpected answer.".to_string()),
            });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.run = Run::Working(rx);
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let Some((media, range, stream)) = app.project.clip(self.clip).and_then(|(_, c)| {
            let m = c.media_id.and_then(|m| app.project.media(m))?;
            Some((
                m.clone(),
                (c.source_in.as_secs_f64(), c.source_out.as_secs_f64()),
                c.audio.stream,
            ))
        }) else {
            return false;
        };
        self.cursor = self.cursor.clamp(range.0, range.1);
        if let Run::Working(rx) = &self.run {
            match rx.try_recv() {
                Ok(Ok(found)) => {
                    self.segs = found
                        .into_iter()
                        .map(|(start, end)| Seg {
                            start,
                            end,
                            cut: true,
                        })
                        .collect();
                    self.run = Run::Done;
                }
                Ok(Err(e)) => self.run = Run::Failed(e),
                Err(_) => ctx.request_repaint_after(std::time::Duration::from_millis(120)),
            }
        }
        let working = matches!(self.run, Run::Working(_));
        let unsuitable = crate::detection::unsuitable(
            app,
            self.clip,
            Detection::Silences {
                noise_db: threshold_db(self.percent),
                min_secs: self.min_secs,
            },
        );
        let mut go = false;
        let mut cancel = false;
        let mut analyze = false;
        let (_, keys) = window(ctx, "silence", 1000.0, |ui| {
            cancel |= header(
                ui,
                ph::WAVEFORM,
                "Silence Detection",
                "Find the pauses in a clip and cut them out in one go",
            );
            scroll_middle(ui, |ui| {
                body(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 22.0;
                        self.preview(ui, app, &media);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;
                            self.settings(ui, working, unsuitable.is_none(), &mut analyze);
                            self.summary(ui, unsuitable);
                        });
                    });
                    ui.add_space(6.0);
                    self.strip(ui, app, &media, range, stream);
                });
            });
            let cuts = self.cuts();
            let total: f64 = cuts.iter().map(|(a, b)| b - a).sum();
            footer(
                ui,
                |ui| {
                    widgets::hint(
                        ui,
                        "Red parts are cut and the gaps closed. Click one to keep it.",
                    );
                },
                |ui| {
                    let label = format!("{}  Export to Timeline", ph::EXPORT);
                    if primary(ui, &label, !cuts.is_empty() && !working).clicked() {
                        go = true;
                    }
                    if secondary(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                    if !cuts.is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "{} cut{} · {:.1} s",
                                cuts.len(),
                                if cuts.len() == 1 { "" } else { "s" },
                                total
                            ))
                            .size(12.5)
                            .color(theme::TEXT_DIM),
                        );
                    }
                },
            );
        });
        if analyze {
            self.analyze(app, range, ctx);
        }
        if go {
            let cuts = self.cuts();
            match app.remove_silences(self.clip, cuts) {
                Ok(text) => app.notify(&text, false),
                Err(text) => app.notify(&text, true),
            }
            return false;
        }
        !(cancel || keys.escape)
    }

    /// The picture at the moment looked at.
    fn preview(&mut self, ui: &mut egui::Ui, app: &mut TidyCutsApp, media: &MediaItem) {
        let w = 400.0;
        let aspect = if media.width > 0 && media.height > 0 {
            media.width as f32 / media.height as f32
        } else {
            16.0 / 9.0
        };
        let size = if aspect >= 1.0 {
            egui::vec2(w, (w / aspect).min(260.0))
        } else {
            egui::vec2((260.0 * aspect).min(w), 260.0)
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 236.0), Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(8),
            Color32::from_rgb(0x0a, 0x0b, 0x0c),
        );
        let fit = {
            let s = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
            Rect::from_center_size(rect.center(), size * s)
        };
        let tex = (media.kind == MediaKind::Video)
            .then(|| app.engine.as_ref().and_then(|e| e.scrub.get(media)))
            .flatten()
            .and_then(|cache| app.thumbs.strip_frame(media, &cache, self.cursor));
        match tex {
            Some(t) => {
                ui.painter().image(
                    t.id(),
                    fit,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    if media.kind == MediaKind::Audio {
                        ph::SPEAKER_HIGH
                    } else {
                        ph::FILM_STRIP
                    },
                    FontId::proportional(34.0),
                    theme::TEXT_FAINT,
                );
            }
        }
        ui.painter().text(
            rect.left_bottom() + egui::vec2(10.0, -8.0),
            Align2::LEFT_BOTTOM,
            time_label(self.cursor),
            FontId::monospace(12.0),
            Color32::WHITE,
        );
    }

    fn settings(&mut self, ui: &mut egui::Ui, working: bool, can_run: bool, analyze: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Settings").font(theme::heading_font(14.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if working {
                    format!("{}  Analyzing…", ph::HOURGLASS)
                } else {
                    format!("{}  Analyze", ph::MAGNIFYING_GLASS)
                };
                if ui
                    .add_enabled(
                        can_run && !working,
                        theme::accent_button(&label).min_size(egui::vec2(124.0, 32.0)),
                    )
                    .clicked()
                {
                    *analyze = true;
                }
            });
        });
        let tip = format!(
            "Sound quieter than this ({:.0} dB) counts as silence. Raise it if background noise keeps pauses from being found.",
            threshold_db(self.percent)
        );
        row(ui, "Volume Threshold", |ui| {
            widgets::slim_slider(ui, &mut self.percent, 0.0..=100.0, 220.0, false)
                .on_hover_text(&tip);
            widgets::readout(ui, &mut self.percent, 0.0..=100.0, 0, " %", 62.0);
        });
        row(ui, "Minimum Duration", |ui| {
            widgets::slim_slider(ui, &mut self.min_secs, 0.1..=5.0, 220.0, false)
                .on_hover_text("Shorter pauses stay, so speech keeps its natural rhythm.");
            widgets::readout(ui, &mut self.min_secs, 0.1..=5.0, 1, " s", 62.0);
        });
        row(ui, "Softening Buffer", |ui| {
            widgets::slim_slider(ui, &mut self.buffer, 0.0..=1.0, 220.0, false).on_hover_text(
                "A little silence is kept either side of every cut so words aren't clipped.",
            );
            widgets::readout(ui, &mut self.buffer, 0.0..=1.0, 2, " s", 62.0);
        });
    }

    fn summary(&mut self, ui: &mut egui::Ui, unsuitable: Option<&'static str>) {
        if let Some(why) = unsuitable {
            ui.label(RichText::new(why).color(theme::WARNING));
            return;
        }
        let text = match &self.run {
            Run::Idle => "Press Analyze to look for the pauses.".to_string(),
            Run::Working(_) => "Listening to the clip…".to_string(),
            Run::Failed(e) => format!("The search failed: {e}"),
            Run::Done if self.segs.is_empty() => {
                "No pauses found. Try a higher threshold or a shorter minimum duration.".into()
            }
            Run::Done => {
                let total: f64 = self.segs.iter().map(|s| s.end - s.start).sum();
                format!(
                    "Found {} silent part{}, {:.1} s in all.",
                    self.segs.len(),
                    if self.segs.len() == 1 { "" } else { "s" },
                    total
                )
            }
        };
        let failed = matches!(self.run, Run::Failed(_));
        ui.add(
            egui::Label::new(RichText::new(text).size(12.5).color(if failed {
                theme::DANGER
            } else {
                theme::TEXT_DIM
            }))
            .wrap(),
        );
        if !self.segs.is_empty() {
            ui.horizontal(|ui| {
                if secondary(ui, "Cut all").clicked() {
                    self.segs.iter_mut().for_each(|s| s.cut = true);
                }
                if secondary(ui, "Keep all").clicked() {
                    self.segs.iter_mut().for_each(|s| s.cut = false);
                }
            });
        }
    }

    /// The clip's frames and sound across the window, with the quiet parts
    /// marked.
    fn strip(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut TidyCutsApp,
        media: &MediaItem,
        range: (f64, f64),
        stream: u32,
    ) {
        let total = (range.1 - range.0).max(0.05);
        // Zoom.
        ui.horizontal(|ui| {
            ui.label(RichText::new("Timeline").font(theme::heading_font(13.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, ph::MAGNIFYING_GLASS_PLUS, "Zoom in").clicked() {
                    self.zoom = (self.zoom * 1.5).min(40.0);
                }
                widgets::slim_slider(ui, &mut self.zoom, 1.0..=40.0, 110.0, true);
                if widgets::icon_button(ui, ph::MAGNIFYING_GLASS_MINUS, "Zoom out").clicked() {
                    self.zoom = (self.zoom / 1.5).max(1.0);
                }
            });
        });
        let view_len = total / self.zoom.max(1.0);
        let view_start = range.0 + (total - view_len) * self.scroll.clamp(0.0, 1.0);
        let width = ui.available_width();
        let ruler_h = 22.0;
        let film_h = if media.kind == MediaKind::Video {
            58.0
        } else {
            0.0
        };
        let wave_h = 120.0;
        let (area, resp) = ui.allocate_exact_size(
            egui::vec2(width, ruler_h + film_h + wave_h),
            Sense::click_and_drag(),
        );
        let painter = ui.painter().with_clip_rect(area);
        painter.rect_filled(
            area,
            CornerRadius::same(6),
            Color32::from_rgb(0x0e, 0x0f, 0x11),
        );
        let x_of = |t: f64| area.left() + ((t - view_start) / view_len) as f32 * area.width();
        let t_of = |x: f32| view_start + ((x - area.left()) / area.width()) as f64 * view_len;

        // The ruler.
        let step = [
            0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
        ]
        .into_iter()
        .find(|s| (*s / view_len) as f32 * area.width() >= 80.0)
        .unwrap_or(3600.0);
        let mut t = (view_start / step).floor() * step;
        while t <= view_start + view_len {
            let x = x_of(t);
            if x >= area.left() {
                painter.vline(
                    x,
                    area.top()..=area.top() + ruler_h,
                    Stroke::new(1.0, theme::BORDER),
                );
                painter.text(
                    egui::pos2(x + 4.0, area.top() + 4.0),
                    Align2::LEFT_TOP,
                    time_label(t),
                    FontId::monospace(10.5),
                    theme::TEXT_FAINT,
                );
            }
            t += step;
        }

        // Pictures.
        if film_h > 0.0 {
            let film = Rect::from_min_size(
                egui::pos2(area.left(), area.top() + ruler_h),
                egui::vec2(width, film_h),
            );
            let aspect = if media.width > 0 && media.height > 0 {
                media.width as f32 / media.height as f32
            } else {
                16.0 / 9.0
            };
            let tile_w = film_h * aspect;
            let cache = app.engine.as_ref().and_then(|e| e.scrub.get(media));
            if let Some(cache) = cache {
                let tiles = (width / tile_w).ceil() as i32;
                let p = painter.with_clip_rect(film);
                for i in 0..tiles {
                    let x = film.left() + i as f32 * tile_w;
                    let at = t_of(x + tile_w * 0.5).clamp(0.0, media.duration.as_secs_f64());
                    if let Some(tex) = app.thumbs.strip_frame(media, &cache, at) {
                        p.image(
                            tex.id(),
                            Rect::from_min_size(
                                egui::pos2(x, film.top()),
                                egui::vec2(tile_w, film_h),
                            ),
                            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                }
            }
        }

        // Sound.
        let wave = Rect::from_min_size(
            egui::pos2(area.left(), area.top() + ruler_h + film_h),
            egui::vec2(width, wave_h),
        );
        painter.rect_filled(
            wave,
            CornerRadius::ZERO,
            Color32::from_rgb(0x12, 0x2b, 0x27),
        );
        let plot = wave.shrink2(egui::vec2(0.0, 8.0));
        let mut scale = 1.0;
        match app.thumbs.waveform(media, stream) {
            Some(peaks) if !peaks.is_empty() => {
                scale = crate::wave::draw(
                    &painter,
                    &peaks,
                    PEAKS_PER_SEC as f64,
                    plot,
                    |x| t_of(x).max(0.0),
                    1.0,
                    Color32::from_rgb(0x3f, 0xd9, 0xb0),
                );
            }
            _ => {
                painter.text(
                    wave.center(),
                    Align2::CENTER_CENTER,
                    "Measuring the sound…",
                    FontId::proportional(12.5),
                    theme::TEXT_DIM,
                );
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(150));
            }
        }
        // Where the threshold sits: sound below this line counts as quiet.
        let level = 10f32.powf(threshold_db(self.percent) as f32 / 20.0) * scale;
        let y = plot.bottom() - crate::wave::height_of(level) * (plot.height() - 1.0);
        painter.hline(
            wave.x_range(),
            y,
            Stroke::new(1.0, Color32::from_white_alpha(110)),
        );

        // The quiet parts: red where they will be cut, the buffer left clear.
        let pointer = ui.ctx().pointer_hover_pos().filter(|p| area.contains(*p));
        let mut hovered = None;
        for (i, s) in self.segs.iter().enumerate() {
            let (x0, x1) = (x_of(s.start), x_of(s.end));
            if x1 < area.left() || x0 > area.right() {
                continue;
            }
            let full = Rect::from_min_max(
                egui::pos2(x0, area.top() + ruler_h),
                egui::pos2(x1.max(x0 + 2.0), area.bottom()),
            );
            let inner = Rect::from_min_max(
                egui::pos2(x_of(s.start + self.buffer).min(x1), full.top()),
                egui::pos2(x_of(s.end - self.buffer).max(x0), full.bottom()),
            );
            if s.cut {
                painter.rect_filled(full, CornerRadius::ZERO, theme::DANGER.gamma_multiply(0.18));
                if inner.width() > 0.5 {
                    painter.rect_filled(
                        inner,
                        CornerRadius::ZERO,
                        theme::DANGER.gamma_multiply(0.38),
                    );
                    painter.rect_stroke(
                        inner,
                        CornerRadius::ZERO,
                        Stroke::new(1.0, theme::DANGER),
                        StrokeKind::Inside,
                    );
                }
            } else {
                painter.rect_stroke(
                    full,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, Color32::from_white_alpha(60)),
                    StrokeKind::Inside,
                );
            }
            if pointer.is_some_and(|p| full.contains(p)) {
                hovered = Some(i);
            }
        }

        // The moment being looked at.
        let cx = x_of(self.cursor);
        if cx >= area.left() && cx <= area.right() {
            painter.vline(cx, area.y_range(), Stroke::new(2.0, theme::PLAYHEAD));
        }

        // Clicks: a quiet part switches between cut and kept; anywhere else
        // moves the moment looked at (dragging scrubs).
        if let Some(i) = hovered {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            let s = self.segs[i];
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                egui::Id::new("silence-tip"),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label(format!(
                    "{:.1} s of silence · {}",
                    s.end - s.start,
                    if s.cut {
                        "click to keep it"
                    } else {
                        "click to cut it"
                    }
                ));
            });
        }
        if resp.clicked() {
            match hovered {
                Some(i) => self.segs[i].cut = !self.segs[i].cut,
                None => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.cursor = t_of(p.x).clamp(range.0, range.1);
                    }
                }
            }
        } else if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.cursor = t_of(p.x).clamp(range.0, range.1);
            }
        }
        // Moving along when zoomed in.
        if self.zoom > 1.01 {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Position").size(12.0).color(theme::TEXT_DIM));
                widgets::slim_slider(
                    ui,
                    &mut self.scroll,
                    0.0..=1.0,
                    (width - 90.0).max(100.0),
                    false,
                );
            });
        }
    }
}

fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(130.0, 22.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
        add(ui);
    });
}

/// "1:05.3" for a time in seconds.
fn time_label(secs: f64) -> String {
    let m = (secs / 60.0).floor() as u64;
    format!("{m}:{:04.1}", secs - m as f64 * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(segs: &[(f64, f64, bool)], buffer: f64) -> SilenceDialog {
        let mut d = SilenceDialog::new(uuid::Uuid::new_v4());
        d.buffer = buffer;
        d.segs = segs
            .iter()
            .map(|(start, end, cut)| Seg {
                start: *start,
                end: *end,
                cut: *cut,
            })
            .collect();
        d
    }

    #[test]
    fn the_threshold_runs_from_very_quiet_to_loud() {
        assert_eq!(threshold_db(0.0), -60.0);
        assert!((threshold_db(50.0) + 30.0).abs() < 1e-9);
        assert_eq!(threshold_db(100.0), 0.0);
        assert_eq!(threshold_db(250.0), 0.0);
    }

    #[test]
    fn cuts_keep_the_buffer_and_skip_what_is_kept() {
        let d = dialog(
            &[(1.0, 3.0, true), (5.0, 5.15, true), (8.0, 9.0, false)],
            0.1,
        );
        let cuts = d.cuts();
        // The first loses 0.1 s each end; the second is too short to cut once
        // softened; the third was kept.
        assert_eq!(cuts.len(), 1);
        assert!((cuts[0].0 - 1.1).abs() < 1e-9 && (cuts[0].1 - 2.9).abs() < 1e-9);
    }

    #[test]
    fn times_read_as_minutes_and_seconds() {
        assert_eq!(time_label(0.0), "0:00.0");
        assert_eq!(time_label(65.25), "1:05.2");
    }
}
