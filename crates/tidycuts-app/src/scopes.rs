//! The scopes window: waveform, RGB parade, vectorscope and histogram of
//! the picture in the preview, as colorists use them. The counts
//! come from `tidycuts_engine::scopes`; this draws them.

use crate::app::TidyCutsApp;
use crate::theme;
use egui::{
    Align2, Color32, ColorImage, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions,
};
use std::sync::Arc;
use std::time::Duration;
use tidycuts_engine::scopes::{vector_position, Scopes, LEVELS, WAVE_COLS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Waveform,
    Parade,
    Vectorscope,
    Histogram,
}

impl Kind {
    const ALL: [Kind; 4] = [
        Kind::Waveform,
        Kind::Parade,
        Kind::Vectorscope,
        Kind::Histogram,
    ];

    fn label(self) -> &'static str {
        match self {
            Kind::Waveform => "Waveform",
            Kind::Parade => "Parade",
            Kind::Vectorscope => "Vectorscope",
            Kind::Histogram => "Histogram",
        }
    }
}

pub struct ScopesState {
    pub open: bool,
    kind: Kind,
    /// The picture drawn from the counts, and the counts it came from.
    texture: Option<(TextureHandle, Arc<Scopes>, Kind)>,
}

impl Default for ScopesState {
    fn default() -> Self {
        ScopesState {
            open: false,
            kind: Kind::Waveform,
            texture: None,
        }
    }
}

/// How bright a count shows: a few hits in a spot already glow, and busy
/// spots saturate.
fn glow(count: u32, scale: f32) -> f32 {
    (count as f32 / scale).min(1.0).powf(0.55)
}

fn waveform_image(s: &Scopes, planes: &[(&Vec<u32>, [f32; 3])]) -> ColorImage {
    let w = WAVE_COLS * planes.len();
    let scale = (s.height as f32 * 0.035).max(2.0);
    let mut px = vec![Color32::BLACK; w * LEVELS];
    for (n, (plane, tint)) in planes.iter().enumerate() {
        for row in 0..LEVELS {
            for col in 0..WAVE_COLS {
                let g = glow(plane[row * WAVE_COLS + col], scale);
                if g > 0.0 {
                    px[row * w + n * WAVE_COLS + col] = Color32::from_rgb(
                        (tint[0] * g * 255.0) as u8,
                        (tint[1] * g * 255.0) as u8,
                        (tint[2] * g * 255.0) as u8,
                    );
                }
            }
        }
    }
    ColorImage {
        size: [w, LEVELS],
        source_size: egui::vec2(w as f32, LEVELS as f32),
        pixels: px,
    }
}

fn vector_image(s: &Scopes) -> ColorImage {
    let scale = ((s.width * s.height) as f32 * 0.0008).max(2.0);
    let mut px = vec![Color32::BLACK; 256 * 256];
    for y in 0..256 {
        for x in 0..256 {
            let count = s.vector[y * 256 + x];
            if count == 0 {
                continue;
            }
            // The colour of that spot, bright enough to see.
            let cb = x as f32 / 255.0 - 0.5;
            let cr = 0.5 - y as f32 / 255.0;
            let luma = 0.62;
            let r = luma + 1.5748 * cr;
            let b = luma + 1.8556 * cb;
            let g = (luma - 0.2126 * r - 0.0722 * b) / 0.7152;
            let g_ = glow(count, scale);
            let mix = |c: f32| ((c.clamp(0.0, 1.0) * 0.55 + 0.45) * g_ * 255.0) as u8;
            px[y * 256 + x] = Color32::from_rgb(mix(r), mix(g), mix(b));
        }
    }
    ColorImage {
        size: [256, 256],
        source_size: egui::vec2(256.0, 256.0),
        pixels: px,
    }
}

pub fn show(app: &mut TidyCutsApp, ctx: &egui::Context) {
    if !app.scopes.open {
        return;
    }
    let playing = app.is_playing();
    let live = app.engine.as_mut().and_then(|e| e.scopes());
    let mut open = true;
    egui::Window::new("Scopes")
        .open(&mut open)
        .default_size([400.0, 330.0])
        .min_size([300.0, 240.0])
        .resizable(true)
        .collapsible(false)
        .default_pos(egui::pos2(ctx.content_rect().right() - 440.0, 60.0))
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
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
            ui.horizontal(|ui| {
                for k in Kind::ALL {
                    if ui
                        .selectable_label(app.scopes.kind == k, k.label())
                        .clicked()
                    {
                        app.scopes.kind = k;
                    }
                }
            });
            let kind = app.scopes.kind;
            let Some(s) = live else {
                ui.label(
                    egui::RichText::new("Nothing to show yet: the preview has no picture.")
                        .color(theme::TEXT_DIM),
                );
                return;
            };
            let avail = ui.available_size();
            let size = egui::vec2(avail.x, avail.y.clamp(180.0, 700.0));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(6),
                Color32::from_rgb(0x0a, 0x0b, 0x0c),
            );
            draw(ui, rect, kind, &s, &mut app.scopes.texture);
        });
    if !open {
        app.scopes.open = false;
    }
    if playing {
        ctx.request_repaint_after(Duration::from_millis(60));
    }
}

fn draw(
    ui: &mut egui::Ui,
    rect: Rect,
    kind: Kind,
    s: &Arc<Scopes>,
    cache: &mut Option<(TextureHandle, Arc<Scopes>, Kind)>,
) {
    let painter = ui.painter().with_clip_rect(rect);
    // Redraw the picture only when the counts or the kind changed.
    let stale = cache
        .as_ref()
        .is_none_or(|(_, old, k)| !Arc::ptr_eq(old, s) || *k != kind);
    if stale && kind != Kind::Histogram {
        let image = match kind {
            Kind::Waveform => waveform_image(s, &[(&s.luma, [0.80, 1.0, 0.80])]),
            Kind::Parade => waveform_image(
                s,
                &[
                    (&s.red, [1.0, 0.35, 0.35]),
                    (&s.green, [0.35, 1.0, 0.45]),
                    (&s.blue, [0.45, 0.6, 1.0]),
                ],
            ),
            _ => vector_image(s),
        };
        match cache {
            Some((tex, old, k)) => {
                tex.set(image, TextureOptions::LINEAR);
                *old = s.clone();
                *k = kind;
            }
            None => {
                let tex = ui
                    .ctx()
                    .load_texture("scopes", image, TextureOptions::LINEAR);
                *cache = Some((tex, s.clone(), kind));
            }
        }
    }
    let label = |p: Pos2, a: Align2, text: &str| {
        painter.text(p, a, text, FontId::proportional(10.0), theme::TEXT_FAINT);
    };
    let faint = Stroke::new(1.0, Color32::from_white_alpha(30));
    match kind {
        Kind::Waveform | Kind::Parade => {
            let plot = Rect::from_min_max(
                rect.min + egui::vec2(30.0, 8.0),
                rect.max - egui::vec2(8.0, 8.0),
            );
            if let Some((tex, _, _)) = cache {
                painter.image(
                    tex.id(),
                    plot,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            for pct in [0, 25, 50, 75, 100] {
                let y = plot.bottom() - plot.height() * pct as f32 / 100.0;
                painter.hline(plot.x_range(), y, faint);
                label(
                    egui::pos2(plot.left() - 4.0, y),
                    Align2::RIGHT_CENTER,
                    &pct.to_string(),
                );
            }
            if kind == Kind::Parade {
                for n in 1..3 {
                    let x = plot.left() + plot.width() * n as f32 / 3.0;
                    painter.vline(x, plot.y_range(), faint);
                }
            }
        }
        Kind::Vectorscope => {
            let side = rect.width().min(rect.height()) - 16.0;
            let plot = Rect::from_center_size(rect.center(), egui::vec2(side, side));
            if let Some((tex, _, _)) = cache {
                painter.image(
                    tex.id(),
                    plot,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let c = plot.center();
            painter.circle_stroke(c, side * 0.5, faint);
            painter.circle_stroke(c, side * 0.25, faint);
            painter.hline(plot.x_range(), c.y, faint);
            painter.vline(c.x, plot.y_range(), faint);
            // The skin tone line, and where the 75% colour bars should land.
            let a = 123f32.to_radians();
            painter.line_segment(
                [c, c + egui::vec2(a.cos(), -a.sin()) * side * 0.5],
                Stroke::new(1.0, Color32::from_rgb(0xe0, 0xa0, 0x60).gamma_multiply(0.7)),
            );
            for (name, rgb) in [
                ("R", [0.75, 0.0, 0.0]),
                ("Y", [0.75, 0.75, 0.0]),
                ("G", [0.0, 0.75, 0.0]),
                ("C", [0.0, 0.75, 0.75]),
                ("B", [0.0, 0.0, 0.75]),
                ("M", [0.75, 0.0, 0.75]),
            ] {
                let (fx, fy) = vector_position(rgb[0], rgb[1], rgb[2]);
                let p = plot.min + egui::vec2(fx, fy) * side;
                painter.rect_stroke(
                    Rect::from_center_size(p, egui::vec2(9.0, 9.0)),
                    1.0,
                    Stroke::new(1.0, Color32::from_white_alpha(90)),
                    StrokeKind::Middle,
                );
                label(p + egui::vec2(8.0, -8.0), Align2::LEFT_BOTTOM, name);
            }
        }
        Kind::Histogram => {
            let plot = Rect::from_min_max(
                rect.min + egui::vec2(10.0, 8.0),
                rect.max - egui::vec2(10.0, 20.0),
            );
            for v in [0, 64, 128, 192, 255] {
                let x = plot.left() + plot.width() * v as f32 / 255.0;
                painter.vline(x, plot.y_range(), faint);
                label(
                    egui::pos2(x, plot.bottom() + 3.0),
                    Align2::CENTER_TOP,
                    &v.to_string(),
                );
            }
            // Most bins are small next to a few big ones, so heights are
            // scaled to the second biggest-ish and rooted.
            let peak = s
                .hist
                .iter()
                .flat_map(|h| h.iter().copied())
                .max()
                .unwrap_or(1)
                .max(1) as f32;
            let bar_w = plot.width() / 256.0;
            for (ch, color) in [
                (0, Color32::from_rgba_unmultiplied(235, 235, 235, 70)),
                (1, Color32::from_rgba_unmultiplied(255, 80, 80, 110)),
                (2, Color32::from_rgba_unmultiplied(70, 230, 110, 110)),
                (3, Color32::from_rgba_unmultiplied(90, 130, 255, 110)),
            ] {
                for (bin, count) in s.hist[ch].iter().enumerate() {
                    if *count == 0 {
                        continue;
                    }
                    let h = (*count as f32 / peak).sqrt() * plot.height();
                    let x = plot.left() + bar_w * bin as f32;
                    painter.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x, plot.bottom() - h),
                            egui::pos2(x + bar_w.max(1.0), plot.bottom()),
                        ),
                        0.0,
                        color,
                    );
                }
            }
        }
    }
}
