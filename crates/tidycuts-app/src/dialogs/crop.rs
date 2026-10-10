//! Crop and Zoom: a picture of the clip with a crop frame to drag, aspect
//! ratio presets, exact numbers, and what to do with the cropped part:
//! leave it where it is, or centre it and zoom it to fit or fill the
//! project frame.

use super::sizes::aspect_name;
use super::{footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::{
    Align2, Color32, ColorImage, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, TextureHandle, TextureOptions,
};
use egui_phosphor::regular as ph;
use std::sync::mpsc::{channel, Receiver};
use tidycuts_core::{Animatable, ClipId, Crop, MediaKind};

/// Smallest crop, as a fraction of the picture (the renderer keeps 5%).
const MIN: f64 = 0.05;

/// The visible part of the picture, in 0..1 picture coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NRect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl NRect {
    pub const FULL: NRect = NRect {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    pub fn from_crop(c: Crop) -> NRect {
        let c = c.clamped();
        NRect {
            x0: c.left,
            y0: c.top,
            x1: 1.0 - c.right,
            y1: 1.0 - c.bottom,
        }
    }

    pub fn to_crop(self) -> Crop {
        let r = |v: f64| (v * 10_000.0).round() / 10_000.0;
        Crop {
            left: r(self.x0),
            top: r(self.y0),
            right: r(1.0 - self.x1),
            bottom: r(1.0 - self.y1),
        }
        .clamped()
    }

    pub fn w(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn h(&self) -> f64 {
        self.y1 - self.y0
    }

    fn center(&self) -> (f64, f64) {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }

    /// A `w`×`h` rectangle centred on `c`, moved to lie inside the picture.
    fn centered(c: (f64, f64), w: f64, h: f64) -> NRect {
        let (w, h) = (w.clamp(MIN, 1.0), h.clamp(MIN, 1.0));
        let x0 = (c.0 - w / 2.0).clamp(0.0, 1.0 - w);
        let y0 = (c.1 - h / 2.0).clamp(0.0, 1.0 - h);
        NRect {
            x0,
            y0,
            x1: x0 + w,
            y1: y0 + h,
        }
    }
}

/// The part of the crop frame being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Move,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    fn left(self) -> bool {
        matches!(self, Handle::W | Handle::NW | Handle::SW)
    }
    fn right(self) -> bool {
        matches!(self, Handle::E | Handle::NE | Handle::SE)
    }
    fn top(self) -> bool {
        matches!(self, Handle::N | Handle::NE | Handle::NW)
    }
    fn bottom(self) -> bool {
        matches!(self, Handle::S | Handle::SE | Handle::SW)
    }
    fn is_corner(self) -> bool {
        matches!(self, Handle::NE | Handle::NW | Handle::SE | Handle::SW)
    }

    fn cursor(self) -> CursorIcon {
        match self {
            Handle::Move => CursorIcon::Move,
            Handle::N | Handle::S => CursorIcon::ResizeVertical,
            Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
        }
    }
}

/// The largest rectangle of aspect `an` (width over height, in picture
/// units) that fits the picture, centred where `r` is.
pub fn with_aspect(r: NRect, an: f64) -> NRect {
    let (w, h) = if an >= 1.0 {
        (1.0, 1.0 / an)
    } else {
        (an, 1.0)
    };
    NRect::centered(r.center(), w, h)
}

/// The crop after dragging handle `h` of `start` by `d` (picture units),
/// keeping aspect `an` when given.
pub fn drag(start: NRect, h: Handle, d: (f64, f64), an: Option<f64>) -> NRect {
    let (dx, dy) = d;
    if h == Handle::Move {
        let (w, hh) = (start.w(), start.h());
        let x0 = (start.x0 + dx).clamp(0.0, 1.0 - w);
        let y0 = (start.y0 + dy).clamp(0.0, 1.0 - hh);
        return NRect {
            x0,
            y0,
            x1: x0 + w,
            y1: y0 + hh,
        };
    }
    let Some(an) = an else {
        let mut r = start;
        if h.left() {
            r.x0 = (start.x0 + dx).clamp(0.0, start.x1 - MIN);
        }
        if h.right() {
            r.x1 = (start.x1 + dx).clamp(start.x0 + MIN, 1.0);
        }
        if h.top() {
            r.y0 = (start.y0 + dy).clamp(0.0, start.y1 - MIN);
        }
        if h.bottom() {
            r.y1 = (start.y1 + dy).clamp(start.y0 + MIN, 1.0);
        }
        return r;
    };
    // Sizes grow away from the fixed side or corner.
    let sx = if h.right() { 1.0 } else { -1.0 };
    let sy = if h.bottom() { 1.0 } else { -1.0 };
    let ax = if h.right() { start.x0 } else { start.x1 };
    let ay = if h.bottom() { start.y0 } else { start.y1 };
    let room_x = if sx > 0.0 { 1.0 - ax } else { ax };
    let room_y = if sy > 0.0 { 1.0 - ay } else { ay };
    let span = |a: f64, b: f64| (a.min(b), a.max(b));
    if h.is_corner() {
        let px = (if h.right() { start.x1 } else { start.x0 }) + dx;
        let py = (if h.bottom() { start.y1 } else { start.y0 }) + dy;
        let mut w = ((px - ax) * sx).max(MIN);
        let mut hh = ((py - ay) * sy).max(MIN);
        // Follow whichever way the pointer went further.
        if w / hh > an {
            hh = w / an;
        } else {
            w = hh * an;
        }
        let k = (room_x / w).min(room_y / hh).min(1.0);
        let (w, hh) = (w * k, hh * k);
        let (x0, x1) = span(ax, ax + sx * w);
        let (y0, y1) = span(ay, ay + sy * hh);
        NRect { x0, y0, x1, y1 }
    } else if h.left() || h.right() {
        let px = (if h.right() { start.x1 } else { start.x0 }) + dx;
        let w = ((px - ax) * sx).max(MIN);
        let hh = w / an;
        let cy = start.center().1;
        let k = (room_x / w).min(2.0 * cy.min(1.0 - cy) / hh).min(1.0);
        let (w, hh) = (w * k, hh * k);
        let (x0, x1) = span(ax, ax + sx * w);
        NRect {
            x0,
            y0: cy - hh / 2.0,
            x1,
            y1: cy + hh / 2.0,
        }
    } else {
        let py = (if h.bottom() { start.y1 } else { start.y0 }) + dy;
        let hh = ((py - ay) * sy).max(MIN);
        let w = hh * an;
        let cx = start.center().0;
        let k = (room_y / hh).min(2.0 * cx.min(1.0 - cx) / w).min(1.0);
        let (w, hh) = (w * k, hh * k);
        let (y0, y1) = span(ay, ay + sy * hh);
        NRect {
            x0: cx - w / 2.0,
            y0,
            x1: cx + w / 2.0,
            y1,
        }
    }
}

/// Which handle of the on-screen crop frame `r` is at `p`, if any.
fn hit(r: Rect, p: Pos2) -> Option<Handle> {
    let near = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol;
    let (l, ri, t, b) = (
        near(p.x, r.left(), 12.0),
        near(p.x, r.right(), 12.0),
        near(p.y, r.top(), 12.0),
        near(p.y, r.bottom(), 12.0),
    );
    let in_x = p.x >= r.left() - 12.0 && p.x <= r.right() + 12.0;
    let in_y = p.y >= r.top() - 12.0 && p.y <= r.bottom() + 12.0;
    Some(match (l, ri, t, b) {
        (true, _, true, _) => Handle::NW,
        (_, true, true, _) => Handle::NE,
        (true, _, _, true) => Handle::SW,
        (_, true, _, true) => Handle::SE,
        (true, _, _, _) if in_y => Handle::W,
        (_, true, _, _) if in_y => Handle::E,
        (_, _, true, _) if in_x => Handle::N,
        (_, _, _, true) if in_x => Handle::S,
        _ if r.contains(p) => Handle::Move,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ratio {
    Free,
    Original,
    Fixed(u32, u32),
}

const RATIOS: [(&str, Ratio); 8] = [
    ("Free", Ratio::Free),
    ("Original", Ratio::Original),
    ("16:9", Ratio::Fixed(16, 9)),
    ("9:16", Ratio::Fixed(9, 16)),
    ("4:3", Ratio::Fixed(4, 3)),
    ("1:1", Ratio::Fixed(1, 1)),
    ("4:5", Ratio::Fixed(4, 5)),
    ("21:9", Ratio::Fixed(21, 9)),
];

/// The aspect a ratio keeps in picture units (where the whole picture is
/// 1×1), for a source of `src` pixels.
fn picture_aspect(r: Ratio, src: (f64, f64)) -> Option<f64> {
    let px = match r {
        Ratio::Free => return None,
        Ratio::Original => src.0 / src.1,
        Ratio::Fixed(a, b) => a as f64 / b as f64,
    };
    Some(px * src.1 / src.0)
}

/// What happens to the clip's size and place after cropping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    /// The picture stays where it is; only the cropped part shows.
    Keep,
    /// The cropped part is centred and made as big as fits in the frame.
    Fit,
    /// The cropped part is centred and covers the whole frame.
    Fill,
}

const ZOOMS: [(Zoom, &str, &str); 3] = [
    (
        Zoom::Keep,
        "Keep",
        "The rest of the picture is hidden; size and position don't change.",
    ),
    (
        Zoom::Fit,
        "Fit",
        "The cropped part is centred and enlarged until it touches the frame's edges.",
    ),
    (
        Zoom::Fill,
        "Fill",
        "The cropped part is centred and enlarged to cover the whole frame, with no bars.",
    ),
];

/// Where a clip's picture sits, for working out the zoom.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    /// The media's size in pixels.
    media: (f64, f64),
    /// The project frame's size.
    canvas: (f64, f64),
    /// Degrees, clockwise.
    rotation: f64,
    /// Offset from the picture's centre that it scales around.
    anchor: [f64; 2],
}

/// Scale (percent) and position (from the frame centre) that put the
/// cropped part in the middle of the frame, fitting inside it or covering
/// it. None for [`Zoom::Keep`].
///
/// At 100% the whole (uncropped) picture fits the frame and the crop keeps
/// the visible part where it was, as the renderer draws it.
fn zoom_transform(zoom: Zoom, g: &Geometry, crop: Crop) -> Option<(f64, f64, f64)> {
    let cover = match zoom {
        Zoom::Keep => return None,
        Zoom::Fit => false,
        Zoom::Fill => true,
    };
    let (mw, mh) = (g.media.0.max(1.0), g.media.1.max(1.0));
    let (cw, ch) = (g.canvas.0.max(1.0), g.canvas.1.max(1.0));
    let fit = (cw / mw).min(ch / mh);
    let base = (mw * fit, mh * fit);
    let c = crop.clamped();
    let (fw, fh) = (
        (1.0 - c.left - c.right).max(0.01),
        (1.0 - c.top - c.bottom).max(0.01),
    );
    let (mut vw, mut vh) = (base.0 * fw, base.1 * fh);
    if (g.rotation.rem_euclid(180.0) - 90.0).abs() < 1.0 {
        std::mem::swap(&mut vw, &mut vh);
    }
    let k = if cover {
        (cw / vw).max(ch / vh)
    } else {
        (cw / vw).min(ch / vh)
    };
    // The cropped part's centre, from the picture's centre.
    let off = (
        ((c.left + 1.0 - c.right) / 2.0 - 0.5) * base.0,
        ((c.top + 1.0 - c.bottom) / 2.0 - 0.5) * base.1,
    );
    let rel = ((off.0 - g.anchor[0]) * k, (off.1 - g.anchor[1]) * k);
    let (sin, cos) = g.rotation.to_radians().sin_cos();
    let turned = (rel.0 * cos - rel.1 * sin, rel.0 * sin + rel.1 * cos);
    Some((k * 100.0, -g.anchor[0] - turned.0, -g.anchor[1] - turned.1))
}

/// Which zoom the clip's current size and place match, so reopening the
/// window shows what was chosen before.
fn current_zoom(g: &Geometry, crop: Crop, tr: &tidycuts_core::Transform) -> Zoom {
    if tr.scale_x.is_animated()
        || tr.scale_y.is_animated()
        || tr.position_x.is_animated()
        || tr.position_y.is_animated()
    {
        return Zoom::Keep;
    }
    for z in [Zoom::Fit, Zoom::Fill] {
        if let Some((pct, x, y)) = zoom_transform(z, g, crop) {
            if (tr.scale_x.value - pct).abs() < 0.5
                && (tr.scale_y.value - pct).abs() < 0.5
                && (tr.position_x.value - x).abs() < 1.0
                && (tr.position_y.value - y).abs() < 1.0
            {
                return z;
            }
        }
    }
    Zoom::Keep
}

pub struct CropDialog {
    clip: ClipId,
    rect: Option<NRect>,
    ratio: usize,
    /// Chosen when the window first draws, from the clip's transform.
    zoom: Option<Zoom>,
    drag: Option<(Handle, NRect, Pos2)>,
    picture: Option<TextureHandle>,
    /// The sharp picture from the decoder replaced the quick one.
    sharp: bool,
    loading: Option<Receiver<Option<ColorImage>>>,
    /// The sharp picture was asked for (once).
    requested: bool,
}

/// What the dialog shows the crop on.
struct Source {
    size: (f64, f64),
    name: String,
    is_media: bool,
    locked: bool,
    geometry: Geometry,
}

impl CropDialog {
    pub fn new(clip: ClipId) -> Self {
        CropDialog {
            clip,
            rect: None,
            ratio: 0,
            zoom: None,
            drag: None,
            picture: None,
            sharp: false,
            loading: None,
            requested: false,
        }
    }

    /// Finds a picture of the clip at the playhead (or its first frame):
    /// a quick one from the scrub cache or library poster right away, then
    /// a sharp one decoded on a worker thread.
    fn load_picture(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) {
        if let Some(rx) = &self.loading {
            if let Ok(img) = rx.try_recv() {
                self.loading = None;
                if let Some(img) = img {
                    self.picture =
                        Some(ctx.load_texture("crop-picture", img, TextureOptions::LINEAR));
                    self.sharp = true;
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(60));
            }
        }
        if self.sharp {
            return;
        }
        let Some((_, clip)) = app.project.clip(self.clip) else {
            return;
        };
        let Some(m) = clip.media_id.and_then(|id| app.project.media(id)).cloned() else {
            return;
        };
        let t = app.playhead();
        let at = if clip.contains(t) { t } else { clip.start };
        let secs = clip
            .source_time(at)
            .as_secs_f64()
            .clamp(0.0, (m.duration.as_secs_f64() - 0.05).max(0.0));
        if self.picture.is_none() {
            let cache = app.engine.as_ref().and_then(|e| e.scrub.get(&m));
            let quick = cache.as_ref().and_then(|c| {
                let i = c.nearest(secs, (c.fps * 10.0) as usize)?;
                c.image(i)
            });
            if let Some(img) = quick {
                let ci = ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &img.pixels,
                );
                self.picture = Some(ctx.load_texture("crop-quick", ci, TextureOptions::LINEAR));
            } else if m.kind == MediaKind::Image {
                self.picture = app.thumbs.poster(&m, None);
            }
        }
        if !self.requested {
            self.requested = true;
            let (tx, rx) = channel();
            let ctx = ctx.clone();
            let path = m.path.clone();
            let at = if m.kind == MediaKind::Image {
                0.0
            } else {
                secs
            };
            std::thread::spawn(move || {
                let img = tidycuts_media::grab_frame(&path, at, (m.width, m.height), 1280, 720)
                    .ok()
                    .map(|f| {
                        ColorImage::from_rgba_unmultiplied(
                            [f.width as usize, f.height as usize],
                            &f.pixels,
                        )
                    });
                let _ = tx.send(img);
                ctx.request_repaint();
            });
            self.loading = Some(rx);
        }
    }

    fn source(&self, app: &TidyCutsApp) -> Option<Source> {
        let (track, clip) = app.project.clip(self.clip)?;
        let media = clip.media_id.and_then(|id| app.project.media(id));
        let size = match media {
            Some(m) if m.width > 0 && m.height > 0 => (m.width as f64, m.height as f64),
            _ => (
                app.project.settings.width.max(1) as f64,
                app.project.settings.height.max(1) as f64,
            ),
        };
        let s = &app.project.settings;
        Some(Source {
            size,
            name: crate::timeline::clip_name(app, clip),
            is_media: media.is_some_and(|m| m.kind != MediaKind::Audio),
            locked: track.locked,
            geometry: Geometry {
                media: size,
                canvas: (s.width.max(1) as f64, s.height.max(1) as f64),
                rotation: clip.transform.rotation.value,
                anchor: clip.video.anchor,
            },
        })
    }

    fn canvas(&mut self, ui: &mut egui::Ui, src: &Source, rect: &mut NRect) {
        let (area, resp) =
            ui.allocate_exact_size(egui::vec2(600.0, 420.0), Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(area);
        p.rect_filled(
            area,
            CornerRadius::same(10),
            Color32::from_rgb(0x0a, 0x0b, 0x0d),
        );
        let a = (src.size.0 / src.size.1) as f32;
        let room = area.shrink(22.0);
        let mut size = room.size();
        if size.x / size.y > a {
            size.x = size.y * a;
        } else {
            size.y = size.x / a;
        }
        let pic = Rect::from_center_size(room.center(), size);
        match &self.picture {
            Some(t) => {
                p.image(
                    t.id(),
                    pic,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                // A checkerboard for generated clips (and while loading).
                let n = 16.0;
                let cell = pic.width() / n;
                let rows = (pic.height() / cell).ceil() as i32;
                for y in 0..rows {
                    for x in 0..n as i32 {
                        let c = if (x + y) % 2 == 0 {
                            Color32::from_rgb(0x2a, 0x2d, 0x33)
                        } else {
                            Color32::from_rgb(0x22, 0x25, 0x2a)
                        };
                        let r = Rect::from_min_size(
                            pic.min + egui::vec2(x as f32 * cell, y as f32 * cell),
                            egui::vec2(cell, cell),
                        )
                        .intersect(pic);
                        p.rect_filled(r, CornerRadius::ZERO, c);
                    }
                }
                let label = if self.loading.is_some() {
                    "Loading picture…".to_string()
                } else {
                    src.name.clone()
                };
                p.text(
                    pic.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(14.0),
                    theme::TEXT_DIM,
                );
            }
        }
        let to_screen = |x: f64, y: f64| pic.lerp_inside(egui::vec2(x as f32, y as f32));
        let cr = Rect::from_min_max(to_screen(rect.x0, rect.y0), to_screen(rect.x1, rect.y1));

        // Interaction.
        let pointer = resp.hover_pos();
        if let Some(h) = self
            .drag
            .map(|d| d.0)
            .or_else(|| pointer.and_then(|q| hit(cr, q)))
        {
            ui.ctx().set_cursor_icon(h.cursor());
        }
        if resp.drag_started() {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(h) = hit(cr, origin) {
                    self.drag = Some((h, *rect, origin));
                }
            }
        }
        if let (Some((h, start, origin)), Some(now)) = (self.drag, resp.interact_pointer_pos()) {
            if resp.dragged() {
                let d = now - origin;
                let an = picture_aspect(RATIOS[self.ratio].1, src.size);
                *rect = drag(
                    start,
                    h,
                    ((d.x / pic.width()) as f64, (d.y / pic.height()) as f64),
                    an,
                );
            }
        }
        if resp.drag_stopped() {
            self.drag = None;
        }

        // The frame, with the cut-away parts dimmed.
        let cr = Rect::from_min_max(to_screen(rect.x0, rect.y0), to_screen(rect.x1, rect.y1));
        let shade = Color32::from_black_alpha(165);
        for r in [
            Rect::from_min_max(pic.min, egui::pos2(pic.max.x, cr.min.y)),
            Rect::from_min_max(egui::pos2(pic.min.x, cr.max.y), pic.max),
            Rect::from_min_max(
                egui::pos2(pic.min.x, cr.min.y),
                egui::pos2(cr.min.x, cr.max.y),
            ),
            Rect::from_min_max(
                egui::pos2(cr.max.x, cr.min.y),
                egui::pos2(pic.max.x, cr.max.y),
            ),
        ] {
            if r.is_positive() {
                p.rect_filled(r, CornerRadius::ZERO, shade);
            }
        }
        let thirds = Stroke::new(1.0, Color32::from_white_alpha(60));
        for k in [1.0 / 3.0, 2.0 / 3.0] {
            p.vline(cr.left() + cr.width() * k, cr.y_range(), thirds);
            p.hline(cr.x_range(), cr.top() + cr.height() * k, thirds);
        }
        p.rect_stroke(
            cr,
            CornerRadius::ZERO,
            Stroke::new(1.5, Color32::WHITE),
            StrokeKind::Middle,
        );
        let bold = Stroke::new(3.5, Color32::WHITE);
        let len = 16.0f32.min(cr.width() / 3.0).min(cr.height() / 3.0);
        for (c, sx, sy) in [
            (cr.left_top(), 1.0, 1.0),
            (cr.right_top(), -1.0, 1.0),
            (cr.left_bottom(), 1.0, -1.0),
            (cr.right_bottom(), -1.0, -1.0),
        ] {
            p.line_segment([c, c + egui::vec2(sx * len, 0.0)], bold);
            p.line_segment([c, c + egui::vec2(0.0, sy * len)], bold);
        }
        let half = 9.0f32.min(cr.width() / 4.0).min(cr.height() / 4.0);
        for (c, horizontal) in [
            (cr.center_top(), true),
            (cr.center_bottom(), true),
            (cr.left_center(), false),
            (cr.right_center(), false),
        ] {
            let d = if horizontal {
                egui::vec2(half, 0.0)
            } else {
                egui::vec2(0.0, half)
            };
            p.line_segment([c - d, c + d], bold);
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, src: &Source, rect: &mut NRect) {
        widgets::group_title(ui, "Aspect ratio");
        let mut picked = None;
        egui::Grid::new("crop-ratios")
            .num_columns(2)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
                for (i, (label, _)) in RATIOS.iter().enumerate() {
                    let on = self.ratio == i;
                    let b = egui::Button::new(RichText::new(*label).color(if on {
                        theme::ACCENT
                    } else {
                        theme::TEXT
                    }))
                    .fill(if on {
                        theme::ACCENT_BG
                    } else {
                        theme::PANEL_BG_2
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if on { theme::ACCENT } else { theme::BORDER },
                    ))
                    .min_size(egui::vec2(104.0, 30.0));
                    if ui.add(b).clicked() {
                        picked = Some(i);
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        if let Some(i) = picked {
            self.ratio = i;
            if let Some(an) = picture_aspect(RATIOS[i].1, src.size) {
                *rect = with_aspect(*rect, an);
            }
        }
        ui.add_space(10.0);
        widgets::group_title(ui, "Crop area");
        let (sw, sh) = src.size;
        let an = picture_aspect(RATIOS[self.ratio].1, src.size);
        let mut x = (rect.x0 * sw).round();
        let mut y = (rect.y0 * sh).round();
        let mut w = (rect.w() * sw).round();
        let mut h = (rect.h() * sh).round();
        let num = |ui: &mut egui::Ui, label: &str, v: &mut f64, max: f64| {
            widgets::labeled_row(ui, label, 22.0, |ui| {
                ui.add_sized(
                    [80.0, 26.0],
                    egui::DragValue::new(v)
                        .range(0.0..=max)
                        .speed(1.0)
                        .max_decimals(0)
                        .suffix(" px"),
                )
                .changed()
            })
        };
        let mut changed = None;
        ui.horizontal(|ui| {
            if num(ui, "X", &mut x, sw) {
                changed = Some('x');
            }
            if num(ui, "Y", &mut y, sh) {
                changed = Some('y');
            }
        });
        ui.horizontal(|ui| {
            if num(ui, "W", &mut w, sw) {
                changed = Some('w');
            }
            if num(ui, "H", &mut h, sh) {
                changed = Some('h');
            }
        });
        if let Some(which) = changed {
            let (mut nw, mut nh) = ((w / sw).max(MIN), (h / sh).max(MIN));
            match (which, an) {
                ('w', Some(a)) => nh = nw / a,
                ('h', Some(a)) => nw = nh * a,
                _ => {}
            }
            let (nx, ny) = (x / sw, y / sh);
            // Shrink to fit (keeping the shape), then keep inside.
            let k = (1.0 / nw).min(1.0 / nh).min(1.0);
            let (nw, nh) = (nw * k, nh * k);
            let x0 = nx.clamp(0.0, 1.0 - nw);
            let y0 = ny.clamp(0.0, 1.0 - nh);
            *rect = NRect {
                x0,
                y0,
                x1: x0 + nw,
                y1: y0 + nh,
            };
        }
        let (rw, rh) = (
            (rect.w() * sw).round() as u32,
            (rect.h() * sh).round() as u32,
        );
        ui.label(
            RichText::new(format!("Result: {rw}×{rh} px · {}", aspect_name(rw, rh)))
                .size(12.5)
                .color(theme::TEXT_DIM),
        );
        if src.is_media {
            ui.add_space(10.0);
            widgets::group_title(ui, "After cropping");
            let zoom = self.zoom.get_or_insert(Zoom::Keep);
            let mut i = ZOOMS.iter().position(|z| z.0 == *zoom).unwrap_or(0);
            ui.scope(|ui| {
                ui.set_max_width(264.0);
                if widgets::segmented(ui, &mut i, &ZOOMS.map(|z| z.1)) {
                    *zoom = ZOOMS[i].0;
                }
            });
            ui.scope(|ui| {
                ui.set_max_width(264.0);
                widgets::hint(ui, ZOOMS[i].2);
            });
        }
        if src.locked {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{}  The clip's track is locked.", ph::LOCK_SIMPLE))
                    .color(theme::WARNING),
            );
        }
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let Some(src) = self.source(app) else {
            return false;
        };
        let Some((_, clip)) = app.project.clip(self.clip) else {
            return false;
        };
        let mut rect = self.rect.unwrap_or_else(|| NRect::from_crop(clip.crop));
        if self.zoom.is_none() {
            self.zoom = Some(current_zoom(&src.geometry, clip.crop, &clip.transform));
        }
        self.load_picture(app, ctx);
        let mut ok = false;
        let mut cancel = false;
        let mut reset = false;
        let (_, keys) = window(ctx, "crop", 920.0, |ui| {
            cancel |= header(ui, ph::CROP, "Crop and Zoom", &src.name);
            super::scroll_middle(ui, |ui| {
                super::body(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 20.0;
                        self.canvas(ui, &src, &mut rect);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;
                            self.sidebar(ui, &src, &mut rect);
                        });
                    });
                    widgets::hint(
                        ui,
                        "Drag the corners or edges to crop, or drag inside the frame to move it.",
                    );
                })
            });
            footer(
                ui,
                |ui| {
                    if secondary(ui, &format!("{}  Reset", ph::ARROW_COUNTER_CLOCKWISE)).clicked() {
                        reset = true;
                    }
                },
                |ui| {
                    if primary(ui, "OK", !src.locked).clicked() {
                        ok = true;
                    }
                    if secondary(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                },
            );
        });
        if reset {
            rect = NRect::FULL;
            self.ratio = 0;
            self.zoom = Some(Zoom::Fit);
        }
        self.rect = Some(rect);
        if (ok || keys.enter) && !src.locked {
            let crop = rect.to_crop();
            let id = self.clip;
            let zoom = if src.is_media {
                zoom_transform(self.zoom.unwrap_or(Zoom::Keep), &src.geometry, crop)
            } else {
                None
            };
            app.edit(|p| {
                let Some(c) = p.clip_mut(id) else {
                    return false;
                };
                let before = (c.crop, c.transform.clone());
                c.crop = crop;
                if let Some((pct, x, y)) = zoom {
                    let t = &mut c.transform;
                    t.scale_x = Animatable::constant(pct);
                    t.scale_y = Animatable::constant(pct);
                    t.position_x = Animatable::constant(x);
                    t.position_y = Animatable::constant(y);
                }
                before != (c.crop, c.transform.clone())
            });
            return false;
        }
        !(cancel || keys.escape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn crop_and_rect_convert_both_ways() {
        let c = Crop {
            left: 0.1,
            top: 0.2,
            right: 0.3,
            bottom: 0.05,
        };
        let r = NRect::from_crop(c);
        assert!(close(r.x1, 0.7) && close(r.y1, 0.95));
        assert_eq!(r.to_crop(), c);
        assert!(NRect::FULL.to_crop().is_none());
    }

    #[test]
    fn aspect_presets_give_the_largest_centred_frame() {
        // 16:9 picture: a 1:1 crop is 9/16 of the width, full height.
        let src = (1920.0, 1080.0);
        let an = picture_aspect(Ratio::Fixed(1, 1), src).unwrap();
        let r = with_aspect(NRect::FULL, an);
        assert!(close(r.w(), 0.5625) && close(r.h(), 1.0));
        assert!(close(r.x0, (1.0 - 0.5625) / 2.0));
        // 9:16 out of 16:9.
        let an = picture_aspect(Ratio::Fixed(9, 16), src).unwrap();
        let r = with_aspect(NRect::FULL, an);
        let px = r.w() * 1920.0 / (r.h() * 1080.0);
        assert!(close(px, 9.0 / 16.0));
        // Original keeps the whole picture.
        let an = picture_aspect(Ratio::Original, src).unwrap();
        assert_eq!(with_aspect(NRect::FULL, an), NRect::FULL);
        assert_eq!(picture_aspect(Ratio::Free, src), None);
    }

    #[test]
    fn free_dragging_moves_edges_and_stays_inside() {
        let r = drag(NRect::FULL, Handle::W, (0.2, 0.0), None);
        assert!(close(r.x0, 0.2) && close(r.x1, 1.0));
        let r = drag(NRect::FULL, Handle::SE, (-0.5, -0.25), None);
        assert!(close(r.x1, 0.5) && close(r.y1, 0.75));
        // Can't cross over or leave the picture.
        let r = drag(NRect::FULL, Handle::W, (2.0, 0.0), None);
        assert!(close(r.w(), MIN));
        let r = drag(NRect::FULL, Handle::E, (0.5, 0.0), None);
        assert!(close(r.x1, 1.0));
        // Moving keeps the size and stays inside.
        let small = NRect {
            x0: 0.25,
            y0: 0.25,
            x1: 0.75,
            y1: 0.75,
        };
        let r = drag(small, Handle::Move, (1.0, -1.0), None);
        assert!(close(r.x1, 1.0) && close(r.y0, 0.0) && close(r.w(), 0.5));
    }

    #[test]
    fn locked_dragging_keeps_the_aspect() {
        let an = 1.0;
        let start = NRect {
            x0: 0.25,
            y0: 0.25,
            x1: 0.75,
            y1: 0.75,
        };
        let r = drag(start, Handle::SE, (0.1, 0.0), Some(an));
        assert!(close(r.w() / r.h(), an));
        assert!(close(r.x0, 0.25) && close(r.y0, 0.25));
        // Growing past the picture shrinks to fit, keeping the shape.
        let r = drag(start, Handle::SE, (0.9, 0.2), Some(an));
        assert!(close(r.w() / r.h(), an) && r.x1 <= 1.0 + 1e-9 && r.y1 <= 1.0 + 1e-9);
        // An edge grows the other side around the centre.
        let r = drag(start, Handle::E, (0.1, 0.0), Some(an));
        assert!(close(r.w(), 0.6) && close(r.h(), 0.6));
        assert!(close((r.y0 + r.y1) / 2.0, 0.5));
        let r = drag(start, Handle::N, (0.0, -0.1), Some(2.0));
        assert!(close(r.w() / r.h(), 2.0) && close(r.y1, 0.75));
    }

    #[test]
    fn handles_are_found_around_the_frame() {
        let r = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 200.0));
        assert_eq!(hit(r, egui::pos2(101.0, 99.0)), Some(Handle::NW));
        assert_eq!(hit(r, egui::pos2(299.0, 201.0)), Some(Handle::SE));
        assert_eq!(hit(r, egui::pos2(200.0, 102.0)), Some(Handle::N));
        assert_eq!(hit(r, egui::pos2(305.0, 150.0)), Some(Handle::E));
        assert_eq!(hit(r, egui::pos2(200.0, 150.0)), Some(Handle::Move));
        assert_eq!(hit(r, egui::pos2(500.0, 150.0)), None);
    }

    fn geometry(media: (f64, f64), canvas: (f64, f64)) -> Geometry {
        Geometry {
            media,
            canvas,
            rotation: 0.0,
            anchor: [0.0, 0.0],
        }
    }

    #[test]
    fn no_crop_fits_at_100_percent() {
        let g = geometry((1920.0, 1080.0), (1920.0, 1080.0));
        let (pct, x, y) = zoom_transform(Zoom::Fit, &g, Crop::default()).unwrap();
        assert!(close(pct, 100.0) && close(x, 0.0) && close(y, 0.0));
        assert_eq!(zoom_transform(Zoom::Keep, &g, Crop::default()), None);
    }

    #[test]
    fn an_off_centre_strip_is_centred_and_fills_the_frame() {
        // A 608 px wide strip starting at x = 1008 of a 1080p frame.
        let g = geometry((1920.0, 1080.0), (1920.0, 1080.0));
        let crop = Crop {
            left: 1008.0 / 1920.0,
            top: 0.0,
            right: 1.0 - 1616.0 / 1920.0,
            bottom: 0.0,
        };
        let (pct, x, y) = zoom_transform(Zoom::Fill, &g, crop).unwrap();
        let k = 1920.0 / 608.0;
        assert!(close(pct, k * 100.0));
        // The strip's centre (352 px right of the middle) moves to the middle.
        assert!((x + 352.0 * k).abs() < 1e-6, "{x}");
        assert!(close(y, 0.0));
        // Fit stops when the strip touches the top and bottom.
        let (pct, x, _) = zoom_transform(Zoom::Fit, &g, crop).unwrap();
        assert!(close(pct, 100.0));
        assert!((x + 352.0).abs() < 1e-6);
    }

    #[test]
    fn fill_covers_a_vertical_frame() {
        // A square from the middle of a 16:9 video in a 9:16 project.
        let g = geometry((1920.0, 1080.0), (1080.0, 1920.0));
        let crop = Crop {
            left: 420.0 / 1920.0,
            top: 0.0,
            right: 420.0 / 1920.0,
            bottom: 0.0,
        };
        let (pct, x, y) = zoom_transform(Zoom::Fill, &g, crop).unwrap();
        // At 100% the video is 1080×607.5, so the square is 607.5 px.
        let side = 1080.0 * (1080.0 / 1920.0);
        assert!((pct / 100.0 * side - 1920.0).abs() < 1e-6);
        assert!(close(x, 0.0) && close(y, 0.0));
    }

    #[test]
    fn the_current_zoom_is_recognised() {
        let g = geometry((1920.0, 1080.0), (1920.0, 1080.0));
        let mut tr = tidycuts_core::Transform::default();
        assert_eq!(current_zoom(&g, Crop::default(), &tr), Zoom::Fit);
        let crop = Crop {
            left: 0.25,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
        let (pct, x, y) = zoom_transform(Zoom::Fill, &g, crop).unwrap();
        tr.scale_x = Animatable::constant(pct);
        tr.scale_y = Animatable::constant(pct);
        tr.position_x = Animatable::constant(x);
        tr.position_y = Animatable::constant(y);
        assert_eq!(current_zoom(&g, crop, &tr), Zoom::Fill);
        tr.position_x = Animatable::constant(x + 40.0);
        assert_eq!(current_zoom(&g, crop, &tr), Zoom::Keep);
    }
}
