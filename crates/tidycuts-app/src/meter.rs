//! The sound meter at the right edge of the timeline, always on: two level
//! bars (left and right) with a decibel scale, a held peak line, the highest
//! recent peak as a number and a lamp that lights when a channel reaches full
//! scale. Click it to clear the peak and the lamps.

use crate::app::TidyCutsApp;
use crate::mixer::meter_fraction;
use crate::theme;
use egui::{Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, Ui};

/// How wide the strip is.
pub const WIDTH: f32 = 60.0;
const BAR_W: f32 = 10.0;
const BAR_GAP: f32 = 3.0;
const SCALE_W: f32 = 24.0;
const FOOT_H: f32 = 40.0;
/// Marks on the scale, in decibels.
const TICKS: [f32; 8] = [0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0, -60.0];

/// A level (1.0 = full scale) in decibels, "-∞" when silent.
pub fn db_text(level: f32) -> String {
    if level < 1e-4 {
        "-∞".into()
    } else {
        format!("{:.1}", 20.0 * level.log10())
    }
}

/// The colour of the bar at a height, in decibels: green while quiet, yellow
/// up to -6, orange up to -1 and red above.
fn zone_color(db: f32) -> Color32 {
    if db < -18.0 {
        theme::GOOD
    } else if db < -6.0 {
        Color32::from_rgb(0xc9, 0xd9, 0x3f)
    } else if db < -1.0 {
        theme::WARNING
    } else {
        theme::DANGER
    }
}

/// Draws the bars from the bottom up to `level`, coloured by zone.
fn bar(ui: &Ui, bar: Rect, level: f32) {
    let painter = ui.painter();
    painter.rect_filled(bar, CornerRadius::same(2), theme::FIELD_BG);
    let top = meter_fraction(level);
    if top <= 0.0 {
        return;
    }
    // Zone edges in decibels, from the bottom: -60 .. -18 .. -6 .. -1 .. +3.
    let edges = [-60.0f32, -18.0, -6.0, -1.0, 3.0];
    let y_of = |frac: f32| bar.bottom() - frac * bar.height();
    for w in edges.windows(2) {
        let (lo, hi) = (meter_fraction_db(w[0]), meter_fraction_db(w[1]));
        let hi = hi.min(top);
        if hi <= lo {
            continue;
        }
        let r = Rect::from_min_max(
            Pos2::new(bar.left(), y_of(hi)),
            Pos2::new(bar.right(), y_of(lo)),
        );
        painter.rect_filled(r, CornerRadius::ZERO, zone_color(w[0]));
    }
}

fn meter_fraction_db(db: f32) -> f32 {
    ((db + 60.0) / 63.0).clamp(0.0, 1.0)
}

pub fn show(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect, header_h: f32) {
    let painter = ui.painter().clone();
    painter.rect_filled(rect, CornerRadius::ZERO, theme::PANEL_BG);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(1.0, theme::BORDER),
    );
    let resp = ui.interact(rect, Id::new("sound-meter"), Sense::click());
    let levels = app.mixer.master;
    let peaks = app.mixer.peak_hold;
    let clipped = app.mixer.clipped;
    if resp.clicked() {
        app.mixer.reset_peaks();
    }
    if resp.double_clicked() {
        // Double-click: the audio settings (the mixer).
        app.mixer.open = true;
    }
    resp.on_hover_text(
        "Sound level of everything that plays (peak). Click to clear the peak and the red lamps. Double-click to open the audio settings.          \"SAFE\" means the hearing guard is turning loud sound down (Preferences > General > Listening).",
    );

    // The two clip lamps sit in the ruler's height.
    let bars_left = rect.left() + SCALE_W + 6.0;
    let lamp_y = rect.top() + header_h * 0.5;
    for (ch, &lit) in clipped.iter().enumerate() {
        let x = bars_left + BAR_W * 0.5 + ch as f32 * (BAR_W + BAR_GAP);
        painter.circle_filled(
            Pos2::new(x, lamp_y),
            3.5,
            if lit {
                theme::DANGER
            } else {
                theme::PANEL_BG_3
            },
        );
    }
    // The hearing guard says so when it is turning the sound down.
    let guarding = app.mixer.guard_db < -1.0;
    painter.text(
        Pos2::new(rect.left() + 6.0, lamp_y),
        Align2::LEFT_CENTER,
        if guarding { "SAFE" } else { "dB" },
        FontId::proportional(10.5),
        if guarding {
            theme::WARNING
        } else {
            theme::TEXT_FAINT
        },
    );

    // The bars and their scale.
    let area = Rect::from_min_max(
        Pos2::new(bars_left, rect.top() + header_h + 4.0),
        Pos2::new(bars_left + 2.0 * BAR_W + BAR_GAP, rect.bottom() - FOOT_H),
    );
    if area.height() < 20.0 {
        return;
    }
    for (ch, level) in levels.iter().enumerate() {
        let x = area.left() + ch as f32 * (BAR_W + BAR_GAP);
        let b = Rect::from_min_max(
            Pos2::new(x, area.top()),
            Pos2::new(x + BAR_W, area.bottom()),
        );
        bar(ui, b, *level);
        // The held peak.
        let f = meter_fraction(peaks[ch]);
        if f > 0.0 {
            let y = b.bottom() - f * b.height();
            painter.hline(
                b.x_range(),
                y,
                Stroke::new(
                    2.0,
                    zone_color(20.0 * peaks[ch].max(1e-4).log10()).gamma_multiply(1.25),
                ),
            );
        }
    }
    for db in TICKS {
        let y = area.bottom() - meter_fraction_db(db) * area.height();
        painter.hline(
            (area.left() - 3.0)..=area.left(),
            y,
            Stroke::new(1.0, theme::TEXT_FAINT),
        );
        painter.text(
            Pos2::new(area.left() - 5.0, y),
            Align2::RIGHT_CENTER,
            format!("{}", db as i32),
            FontId::proportional(9.5),
            theme::TEXT_FAINT,
        );
    }

    // The highest recent peak as a number.
    let top = peaks[0].max(peaks[1]);
    let over = top >= 0.995;
    painter.text(
        Pos2::new(area.center().x, rect.bottom() - FOOT_H + 9.0),
        Align2::CENTER_CENTER,
        db_text(top),
        FontId::monospace(11.0),
        if over { theme::DANGER } else { theme::TEXT_DIM },
    );
    // How loud it is right now (the last 400 ms), in LUFS. Click for the whole video.
    let lufs_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 2.0, rect.bottom() - FOOT_H + 18.0),
        Pos2::new(rect.right() - 2.0, rect.bottom() - 2.0),
    );
    let lufs_resp = ui
        .interact(lufs_rect, Id::new("sound-meter-lufs"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("How loud it sounds right now (LUFS). Click to measure the whole video.");
    if lufs_resp.hovered() {
        painter.rect_filled(lufs_rect, CornerRadius::same(4), theme::HOVER);
    }
    painter.text(
        Pos2::new(lufs_rect.center().x, lufs_rect.top() + 7.0),
        Align2::CENTER_CENTER,
        app.mixer
            .lufs
            .map_or("--".to_string(), |l| format!("{l:.1}")),
        FontId::monospace(11.0),
        theme::TEXT,
    );
    painter.text(
        Pos2::new(lufs_rect.center().x, lufs_rect.bottom() - 6.0),
        Align2::CENTER_CENTER,
        "LUFS",
        FontId::proportional(9.5),
        theme::TEXT_FAINT,
    );
    if lufs_resp.clicked() {
        crate::dialogs::open_loudness(app, ui.ctx());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibel_text_reads_naturally() {
        assert_eq!(db_text(0.0), "-∞");
        assert_eq!(db_text(1.0), "0.0");
        assert_eq!(db_text(0.5), "-6.0");
    }

    #[test]
    fn zones_follow_the_usual_colours() {
        assert_eq!(zone_color(-30.0), theme::GOOD);
        assert_eq!(zone_color(-3.0), theme::WARNING);
        assert_eq!(zone_color(0.0), theme::DANGER);
    }

    #[test]
    fn scale_marks_sit_inside_the_bar() {
        for db in TICKS {
            let f = meter_fraction_db(db);
            assert!((0.0..=1.0).contains(&f));
            // The same position the level maps to.
            let level = 10f32.powf(db / 20.0);
            assert!((meter_fraction(level) - f).abs() < 0.01);
        }
    }
}
