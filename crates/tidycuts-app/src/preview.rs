//! The player: the GPU-rendered picture, on-canvas moving and scaling of the
//! selected clip, the transport bar and full-screen mode.

use crate::app::TidyCutsApp;
use crate::engine;
use crate::library::LibraryDrag;
use crate::prefs::PreviewQuality;
use crate::theme;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind,
    Vec2,
};
use egui_phosphor::regular as ph;
use std::time::{Duration, Instant};
use tidycuts_core::{Clip, MediaId, MediaKind, Project, Time, TrackKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Timeline,
    Source,
}

pub struct PreviewState {
    pub mode: PreviewMode,
    pub source_media: Option<MediaId>,
    pub source_time: Time,
    /// Pixel size the picture is rendered at, worked out from the space the
    /// player had last frame.
    pub render_size: (u32, u32),
    pub fullscreen: bool,
    /// Dragging the source preview's seek bar.
    pub scrubbing: bool,
    pub show_safe_zones: bool,
    pub show_grid: bool,
    drag: Option<CanvasDrag>,
    last_pointer_move: Instant,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState {
            mode: PreviewMode::Timeline,
            source_media: None,
            source_time: Time::ZERO,
            render_size: (960, 540),
            fullscreen: false,
            scrubbing: false,
            show_safe_zones: false,
            show_grid: false,
            drag: None,
            last_pointer_move: Instant::now(),
        }
    }
}

impl PreviewState {
    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }
}

#[derive(Debug, Clone, Copy)]
enum DragKind {
    Move,
    /// Scaling from a corner; the value is the starting distance from the
    /// clip's centre.
    Scale(f32),
    Rotate(f32),
}

#[derive(Debug, Clone, Copy)]
struct CanvasDrag {
    kind: DragKind,
    start: Pos2,
    /// Position, scale and rotation when the drag began.
    pos: (f64, f64),
    scale: (f64, f64),
    rotation: f64,
}

/// Where a visual clip sits on the canvas, in project pixels.
struct ClipBox {
    center: [f32; 2],
    size: [f32; 2],
    rotation: f32,
}

/// The clip's outline for the on-canvas handles. Titles have no fixed size
/// here, so only moving works for them.
fn clip_box(project: &Project, clip: &Clip, lt: Time) -> Option<ClipBox> {
    let (cw, ch) = (
        project.settings.width as f32,
        project.settings.height as f32,
    );
    let base = match (
        &clip.generator,
        clip.media_id.and_then(|m| project.media(m)),
    ) {
        (None, Some(m)) if m.kind != MediaKind::Audio && m.width > 0 => {
            let s = (cw / m.width as f32).min(ch / m.height.max(1) as f32);
            [m.width as f32 * s, m.height as f32 * s]
        }
        (Some(tidycuts_core::Generator::Sticker(s)), _) => {
            let k = ch / 1080.0;
            let h = tidycuts_render::catalog::stickers::STICKER_HEIGHT * k;
            let aspect = tidycuts_render::catalog::stickers::get(&s.kind)
                .map(|d| d.aspect)
                .unwrap_or(1.0);
            [h * aspect, h]
        }
        (Some(tidycuts_core::Generator::Solid(_)), _)
        | (Some(tidycuts_core::Generator::Gradient { .. }), _) => [cw, ch],
        _ => return None,
    };
    let crop = clip.crop.clamped();
    let tr = &clip.transform;
    let sx = tr.scale_x.value_at(lt) as f32 / 100.0;
    let sy = tr.scale_y.value_at(lt) as f32 / 100.0;
    let rotation = tr.rotation.value_at(lt) as f32;
    // Same placement as the renderer: a crop keeps the rest of the picture
    // where it was, and scaling and rotation turn around the anchor.
    let (w0, h0) = (
        (1.0 - crop.left - crop.right) as f32,
        (1.0 - crop.top - crop.bottom) as f32,
    );
    let offset = [
        (crop.left as f32 + w0 * 0.5 - 0.5) * base[0],
        (crop.top as f32 + h0 * 0.5 - 0.5) * base[1],
    ];
    let anchor = [clip.video.anchor[0] as f32, clip.video.anchor[1] as f32];
    let rel = [(offset[0] - anchor[0]) * sx, (offset[1] - anchor[1]) * sy];
    let (sin, cos) = rotation.to_radians().sin_cos();
    let turned = [rel[0] * cos - rel[1] * sin, rel[0] * sin + rel[1] * cos];
    Some(ClipBox {
        center: [
            cw * 0.5 + tr.position_x.value_at(lt) as f32 + anchor[0] + turned[0],
            ch * 0.5 + tr.position_y.value_at(lt) as f32 + anchor[1] + turned[1],
        ],
        size: [base[0] * w0 * sx, base[1] * h0 * sy],
        rotation,
    })
}

pub fn show(app: &mut TidyCutsApp, ui: &mut egui::Ui) {
    let full = ui.max_rect();
    // 40 pixels of tabs and a line; a 59-pixel transport bar with its line.
    let header = Rect::from_min_size(full.min, egui::vec2(full.width(), 40.0));
    let transport = Rect::from_min_max(egui::pos2(full.left(), full.bottom() - 59.0), full.max);
    let area = Rect::from_min_max(
        egui::pos2(full.left(), header.bottom() + 1.0),
        egui::pos2(full.right(), transport.top()),
    );
    header_bar(app, ui, header);
    // The stage is darker than the panel; the picture sits 14 pixels in from
    // the sides and 6 from the top and bottom.
    ui.painter().rect_filled(
        area,
        CornerRadius::ZERO,
        Color32::from_rgb(0x0a, 0x0a, 0x0b),
    );
    picture(app, ui, area.shrink2(egui::vec2(14.0, 6.0)), false);
    transport_panel(app, ui, transport);
}

/// The player filling the whole screen; the controls appear when the
/// mouse moves.
pub fn show_fullscreen(app: &mut TidyCutsApp, ui: &mut egui::Ui, full: Rect) {
    ui.painter()
        .rect_filled(full, CornerRadius::ZERO, Color32::BLACK);
    picture(app, ui, full, true);
    let moved = ui.ctx().input(|i| i.pointer.delta() != Vec2::ZERO);
    if moved {
        app.preview.last_pointer_move = Instant::now();
    }
    let idle = app.preview.last_pointer_move.elapsed();
    let show = idle < Duration::from_secs(2) || !app.is_playing();
    if show {
        let bar = Rect::from_min_max(egui::pos2(full.left(), full.bottom() - 64.0), full.max);
        ui.painter()
            .rect_filled(bar, CornerRadius::ZERO, Color32::from_black_alpha(170));
        transport_bar(app, ui, bar.shrink2(egui::vec2(24.0, 6.0)), true);
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    } else {
        ui.ctx().set_cursor_icon(CursorIcon::None);
    }
}

fn header_bar(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect) {
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme::BORDER),
    );
    // Timeline / Source: 12 pixels in, each tab padded 6 and 10 apart, the
    // selected one underlined in the accent colour.
    let mut x = rect.left() + 12.0;
    let modes = [
        (PreviewMode::Timeline, "Timeline"),
        (PreviewMode::Source, "Source"),
    ];
    for (mode, label) in modes {
        let enabled = mode == PreviewMode::Timeline
            || app.preview.source_media.or(app.selected_media).is_some();
        let galley =
            ui.painter()
                .layout_no_wrap(label.into(), theme::heading_font(13.5), Color32::WHITE);
        let r = Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(galley.size().x + 12.0, rect.height()),
        );
        let resp = ui.interact(
            r,
            Id::new(("pv-mode", label)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let on = app.preview.mode == mode;
        if on {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(r.left(), rect.bottom() - 2.0),
                    egui::pos2(r.right(), rect.bottom()),
                ),
                CornerRadius::ZERO,
                theme::ACCENT,
            );
        }
        let color = if on {
            Color32::WHITE
        } else if enabled && resp.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_FAINT
        };
        ui.painter().galley_with_override_text_color(
            egui::pos2(r.left() + 6.0, r.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if resp.clicked() {
            match mode {
                PreviewMode::Timeline => app.show_timeline(),
                PreviewMode::Source => {
                    if let Some(m) = app.preview.source_media.or(app.selected_media) {
                        app.show_source(m);
                    }
                }
            }
        }
        x = r.right() + 10.0;
    }
    if app.preview.mode == PreviewMode::Source {
        if let Some(m) = app.preview.source_media.and_then(|id| app.media_item(id)) {
            let name = m.name.clone();
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(12.5),
                theme::TEXT_DIM,
            );
        }
    }

    // Right: full screen (a 30-pixel button, 12 in) and the quality chip.
    let full_r = Rect::from_center_size(
        egui::pos2(rect.right() - 12.0 - 15.0, rect.center().y),
        egui::vec2(30.0, 30.0),
    );
    let resp = ui.interact(full_r, Id::new("pv-head-full"), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(full_r, CornerRadius::same(9), theme::HOVER);
    }
    ui.painter().text(
        full_r.center(),
        Align2::CENTER_CENTER,
        ph::CORNERS_OUT,
        FontId::proportional(17.0),
        if resp.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        },
    );
    if resp.on_hover_text("Full screen (F)").clicked() {
        app.preview.toggle_fullscreen(ui.ctx());
    }
    let quality = app.prefs.preview_quality;
    let text = ui.painter().layout_no_wrap(
        quality.label().into(),
        FontId::proportional(12.0),
        theme::TEXT_DIM,
    );
    // padding 6 and 10, a 6-pixel gap, then a small arrow
    let chip_w = 10.0 + text.size().x + 6.0 + 12.0 + 10.0;
    let chip = Rect::from_center_size(
        egui::pos2(full_r.left() - 6.0 - chip_w / 2.0, rect.center().y),
        egui::vec2(chip_w, 27.0),
    );
    let chip_resp = ui.interact(chip, Id::new("pv-quality-chip"), Sense::click());
    ui.painter()
        .rect_filled(chip, CornerRadius::same(9), theme::FIELD_BG);
    ui.painter().galley(
        egui::pos2(chip.left() + 10.0, chip.center().y - text.size().y / 2.0),
        text,
        theme::TEXT_DIM,
    );
    ui.painter().text(
        egui::pos2(chip.right() - 10.0 - 6.0, chip.center().y),
        Align2::CENTER_CENTER,
        ph::CARET_RIGHT,
        FontId::proportional(12.0),
        theme::TEXT_DIM,
    );
    let chip_resp = chip_resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Preview resolution: lower is smoother on slow computers");
    egui::Popup::menu(&chip_resp).show(|ui| {
        for o in PreviewQuality::ALL {
            if ui.selectable_label(quality == o, o.label()).clicked() {
                app.prefs.preview_quality = o;
                app.prefs.save();
            }
        }
    });
}

fn picture(app: &mut TidyCutsApp, ui: &mut egui::Ui, area: Rect, fullscreen: bool) {
    let settings = match app.preview.mode {
        PreviewMode::Timeline => app.project.settings.clone(),
        PreviewMode::Source => match app.preview.source_media.and_then(|id| app.media_item(id)) {
            Some(m) => engine::source_project(m, &app.project.settings).settings,
            None => app.project.settings.clone(),
        },
    };
    let aspect = settings.width.max(1) as f32 / settings.height.max(1) as f32;
    let mut size = area.size();
    if size.x / size.y.max(1.0) > aspect {
        size.x = size.y * aspect;
    } else {
        size.y = size.x / aspect;
    }
    let rect = Rect::from_center_size(area.center(), size);
    let ppp = ui.ctx().pixels_per_point();
    let playing = app.is_playing();
    let factor = app
        .prefs
        .preview_quality
        .factor(playing, settings.width * settings.height);
    app.preview.render_size =
        engine::fit_size(&settings, (rect.width() * ppp, rect.height() * ppp), factor);

    let painter = ui.painter().with_clip_rect(area);
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    match app.engine.as_ref().and_then(|e| e.texture) {
        Some(tex) => {
            painter.image(
                tex,
                rect,
                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Preview unavailable",
                FontId::proportional(14.0),
                theme::TEXT_DIM,
            );
        }
    }
    // Audio has no picture: show what's playing instead of black.
    if app.preview.mode == PreviewMode::Source {
        if let Some(m) = app
            .preview
            .source_media
            .and_then(|id| app.media_item(id))
            .filter(|m| m.kind == MediaKind::Audio)
        {
            let c = rect.center();
            painter.text(
                c - egui::vec2(0.0, 18.0),
                Align2::CENTER_CENTER,
                ph::WAVEFORM,
                FontId::proportional((rect.height() * 0.2).clamp(28.0, 72.0)),
                theme::AUDIO_WAVE,
            );
            painter.text(
                c + egui::vec2(0.0, (rect.height() * 0.1).clamp(14.0, 40.0) + 4.0),
                Align2::CENTER_CENTER,
                &m.name,
                theme::heading_font(14.0),
                theme::TEXT_DIM,
            );
        }
    }
    if app.preview.show_grid {
        for i in 1..3 {
            let fx = rect.left() + rect.width() * i as f32 / 3.0;
            let fy = rect.top() + rect.height() * i as f32 / 3.0;
            painter.vline(
                fx,
                rect.y_range(),
                Stroke::new(1.0, Color32::from_white_alpha(70)),
            );
            painter.hline(
                rect.x_range(),
                fy,
                Stroke::new(1.0, Color32::from_white_alpha(70)),
            );
        }
    }
    if app.preview.show_safe_zones {
        for (f, a) in [(0.9, 80), (0.8, 110)] {
            painter.rect_stroke(
                Rect::from_center_size(rect.center(), rect.size() * f),
                CornerRadius::ZERO,
                Stroke::new(1.0, Color32::from_white_alpha(a)),
                StrokeKind::Inside,
            );
        }
    }
    let empty = app.preview.mode == PreviewMode::Timeline && app.project.duration() == Time::ZERO;
    if empty && !fullscreen {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Import media and drag it to the timeline to start editing",
            FontId::proportional(13.5),
            theme::TEXT_FAINT,
        );
    }

    let resp = ui.interact(rect, Id::new("pv-canvas"), Sense::click_and_drag());
    if resp.double_clicked() {
        // On a title, sticker or effect, open its settings; otherwise full screen.
        if let Some(tab) = app
            .selected_clip()
            .and_then(|id| crate::inspector::main_tab_of(app, id))
        {
            app.inspector.request_tab(tab);
            app.open_inspector();
            return;
        }
        app.preview.toggle_fullscreen(ui.ctx());
        return;
    }
    if fullscreen {
        if resp.clicked() {
            app.toggle_play();
        }
        return;
    }
    if app.preview.mode == PreviewMode::Timeline {
        drop_on_cell(app, ui, rect, &resp);
    }
    if app.preview.mode == PreviewMode::Timeline && !playing {
        canvas_handles(app, ui, rect, &resp);
    } else if resp.clicked() {
        app.toggle_play();
    }
}

/// Whether `p` is inside the four-cornered shape `q` (corners in order).
fn inside_quad(q: &[[f64; 2]; 4], p: [f64; 2]) -> bool {
    let mut sign = 0.0f64;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        // A triangle repeats a corner: that side has no length and says nothing.
        if (b[0] - a[0]).abs() + (b[1] - a[1]).abs() < 1e-9 {
            continue;
        }
        if cross.abs() < 1e-9 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    true
}

/// Media dragged from the library over an empty split-screen cell in the
/// picture: the cell lights up, and dropping fills it.
fn drop_on_cell(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) {
    let Some(payload) = resp.dnd_hover_payload::<LibraryDrag>() else {
        return;
    };
    let LibraryDrag::Media(media) = *payload else {
        return;
    };
    let Some(pos) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let (w, h) = (
        f64::from(app.project.settings.width.max(1)),
        f64::from(app.project.settings.height.max(1)),
    );
    let k = f64::from(rect.width()) / w;
    let point = [
        f64::from(pos.x - rect.left()) / k,
        f64::from(pos.y - rect.top()) / k,
    ];
    let t = app.playhead();
    let hit = app
        .project
        .tracks
        .iter()
        .filter(|tr| tr.kind == TrackKind::Video && !tr.locked)
        .flat_map(|tr| tr.clips.iter())
        .filter(|c| c.is_placeholder() && c.contains(t))
        .filter_map(|c| Some((c.id, c.video.cell.as_ref()?.shape(w, h).outer)))
        .find(|(_, outer)| inside_quad(outer, point));
    let Some((clip, outer)) = hit else {
        return;
    };
    let to_screen = |p: [f64; 2]| {
        egui::pos2(
            rect.left() + (p[0] * k) as f32,
            rect.top() + (p[1] * k) as f32,
        )
    };
    let corners: Vec<Pos2> = outer.iter().map(|p| to_screen(*p)).collect();
    ui.painter().add(egui::Shape::convex_polygon(
        corners,
        Color32::from_rgba_unmultiplied(45, 212, 170, 50),
        Stroke::new(2.5, theme::ACCENT),
    ));
    if resp.dnd_release_payload::<LibraryDrag>().is_some() {
        app.fill_cell(clip, media);
    }
}

/// The selected clip's outline with move, scale and rotate handles.
fn canvas_handles(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect, resp: &egui::Response) {
    let t = app.playhead();
    let Some(id) = app.selected_clip() else {
        return;
    };
    let Some((track, clip)) = app.project.clip(id) else {
        return;
    };
    if track.kind != TrackKind::Video || !clip.contains(t) || clip.is_adjustment() || track.locked {
        return;
    }
    let clip = clip.clone();
    let lt = clip.local(t);
    let k = rect.width() / app.project.settings.width.max(1) as f32;
    let to_screen = |p: [f32; 2]| rect.min + egui::vec2(p[0] * k, p[1] * k);
    let bx = clip_box(&app.project, &clip, lt);

    let mut corners = Vec::new();
    let mut rot_handle = None;
    if let Some(b) = &bx {
        let (s, c) = b.rotation.to_radians().sin_cos();
        let pt = |dx: f32, dy: f32| {
            let (x, y) = (dx * b.size[0] * 0.5, dy * b.size[1] * 0.5);
            to_screen([b.center[0] + x * c - y * s, b.center[1] + x * s + y * c])
        };
        corners = vec![pt(-1.0, -1.0), pt(1.0, -1.0), pt(1.0, 1.0), pt(-1.0, 1.0)];
        let p = ui.painter().with_clip_rect(rect.expand(12.0));
        let mut outline = corners.clone();
        outline.push(corners[0]);
        p.add(egui::Shape::line(outline, Stroke::new(1.5, theme::ACCENT)));
        for q in &corners {
            p.rect_filled(
                Rect::from_center_size(*q, egui::vec2(9.0, 9.0)),
                CornerRadius::same(2),
                Color32::WHITE,
            );
            p.rect_stroke(
                Rect::from_center_size(*q, egui::vec2(9.0, 9.0)),
                CornerRadius::same(2),
                Stroke::new(1.0, theme::ACCENT),
                StrokeKind::Outside,
            );
        }
        let top_mid = pt(0.0, -1.0);
        let dir = (top_mid - to_screen(b.center)).normalized();
        let rh = top_mid + dir * 22.0;
        p.line_segment([top_mid, rh], Stroke::new(1.0, theme::ACCENT));
        p.circle_filled(rh, 5.0, Color32::WHITE);
        p.circle_stroke(rh, 5.0, Stroke::new(1.0, theme::ACCENT));
        rot_handle = Some(rh);
    } else {
        // Titles: a small crosshair at their position.
        let c = to_screen([
            app.project.settings.width as f32 * 0.5 + clip.transform.position_x.value_at(lt) as f32,
            app.project.settings.height as f32 * 0.5
                + clip.transform.position_y.value_at(lt) as f32,
        ]);
        let p = ui.painter();
        p.line_segment(
            [c - egui::vec2(8.0, 0.0), c + egui::vec2(8.0, 0.0)],
            Stroke::new(1.5, theme::ACCENT),
        );
        p.line_segment(
            [c - egui::vec2(0.0, 8.0), c + egui::vec2(0.0, 8.0)],
            Stroke::new(1.5, theme::ACCENT),
        );
    }

    let pointer = ui.ctx().pointer_interact_pos();
    let center_screen = bx.as_ref().map(|b| to_screen(b.center));
    let near = |q: Pos2| pointer.is_some_and(|p| p.distance(q) < 10.0);
    if resp.hovered() || resp.dragged() {
        if corners.iter().any(|q| near(*q)) {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
        } else if rot_handle.is_some_and(near) {
            ui.ctx().set_cursor_icon(CursorIcon::Alias);
        } else {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
    }
    if resp.drag_started() {
        // Judge the grab by where the button went down, not where the
        // drag was noticed a few pixels later.
        let origin = ui.ctx().input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(pointer) {
            let tr = &clip.transform;
            let near = |q: Pos2| p.distance(q) < 10.0;
            let kind = if let (Some(c), true) = (center_screen, corners.iter().any(|q| near(*q))) {
                DragKind::Scale(p.distance(c).max(1.0))
            } else if let (Some(c), true) = (center_screen, rot_handle.is_some_and(near)) {
                DragKind::Rotate((p - c).angle())
            } else {
                DragKind::Move
            };
            app.preview.drag = Some(CanvasDrag {
                kind,
                start: p,
                pos: (tr.position_x.value_at(lt), tr.position_y.value_at(lt)),
                scale: (tr.scale_x.value_at(lt), tr.scale_y.value_at(lt)),
                rotation: tr.rotation.value_at(lt),
            });
        }
    }
    if let (Some(d), Some(p)) = (app.preview.drag, pointer) {
        if resp.dragged() {
            let tol = Time::from_millis(20);
            let mut c = clip.clone();
            let tr = &mut c.transform;
            match d.kind {
                DragKind::Move => {
                    let delta = (p - d.start) / k;
                    let mut nx = d.pos.0 + delta.x as f64;
                    let mut ny = d.pos.1 + delta.y as f64;
                    // Snap to the centre lines.
                    if nx.abs() < 8.0 / k as f64 {
                        nx = 0.0;
                    }
                    if ny.abs() < 8.0 / k as f64 {
                        ny = 0.0;
                    }
                    tr.position_x.set_value_at(lt, nx.round(), tol);
                    tr.position_y.set_value_at(lt, ny.round(), tol);
                }
                DragKind::Scale(d0) => {
                    if let Some(cs) = center_screen {
                        let f = (p.distance(cs) / d0) as f64;
                        tr.scale_x.set_value_at(
                            lt,
                            (d.scale.0 * f).clamp(1.0, 2000.0).round(),
                            tol,
                        );
                        tr.scale_y.set_value_at(
                            lt,
                            (d.scale.1 * f).clamp(1.0, 2000.0).round(),
                            tol,
                        );
                    }
                }
                DragKind::Rotate(a0) => {
                    if let Some(cs) = center_screen {
                        let a = (p - cs).angle();
                        let mut r = d.rotation + (a - a0).to_degrees() as f64;
                        // Snap to quarter turns.
                        let q = (r / 90.0).round() * 90.0;
                        if (r - q).abs() < 3.0 {
                            r = q;
                        }
                        tr.rotation.set_value_at(lt, r.round(), tol);
                    }
                }
            }
            let ctx = ui.ctx().clone();
            app.commit_clip(&ctx, c);
        }
    }
    if resp.drag_stopped() {
        app.preview.drag = None;
    }
}

/// The transport bar under the player in the window: timecode on the left,
/// the buttons in the middle, size and tools on the right.
fn transport_panel(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect) {
    let fps = app.project.settings.frame_rate;
    let (pos, len) = match app.preview.mode {
        PreviewMode::Timeline => (app.playhead(), app.project.duration()),
        PreviewMode::Source => {
            let len = app
                .preview
                .source_media
                .and_then(|id| app.media_item(id))
                .map(|m| m.duration)
                .unwrap_or(Time::ZERO);
            (app.preview.source_time, len)
        }
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::TAB_BG);
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    // 10 pixels of padding above the 34-pixel play button.
    let cy = rect.top() + 1.0 + 10.0 + 17.0;

    // Left: where we are and how long it is.
    let mono = FontId::monospace(13.0);
    let now = ui
        .painter()
        .layout_no_wrap(pos.timecode(fps), mono.clone(), theme::TEXT);
    let left = rect.left() + 18.0;
    ui.painter().galley(
        egui::pos2(left, cy - now.size().y / 2.0),
        now.clone(),
        theme::TEXT,
    );
    let total =
        ui.painter()
            .layout_no_wrap(format!("/ {}", len.timecode(fps)), mono, theme::TEXT_FAINT);
    // In a narrow player the less important things go: first the size, then
    // the length, then the thumbnail button, then the snapshot button.
    let group_w = 32.0 * 4.0 + 38.0 + 10.0 * 4.0;
    let base = 18.0 + now.size().x + 14.0 + group_w + 14.0 + 32.0 + 18.0;
    let with_snap = base + 14.0 + 32.0;
    let with_thumb = with_snap + 14.0 + 32.0;
    let with_total = with_thumb + 14.0 + total.size().x;
    let show_snap = rect.width() >= with_snap;
    let show_thumb = rect.width() >= with_thumb;
    let show_total = rect.width() >= with_total;
    let total_x = left + now.size().x + 14.0;
    if show_total {
        ui.painter().galley(
            egui::pos2(total_x, cy - total.size().y / 2.0),
            total.clone(),
            theme::TEXT_FAINT,
        );
    }
    let left_end = if show_total {
        total_x + total.size().x
    } else {
        left + now.size().x
    };

    // Right: the size, a snapshot button and full screen, 14 apart.
    let icon_button = |ui: &mut egui::Ui, centre: Pos2, icon: &str, tip: &str, id: &str| -> bool {
        let r = Rect::from_center_size(centre, egui::vec2(32.0, 32.0));
        let resp = ui.interact(r, Id::new(("pv-tp", id)), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(9), theme::HOVER);
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(17.0),
            if resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            },
        );
        resp.on_hover_text(tip).clicked()
    };
    let mut right = rect.right() - 18.0;
    let full_c = egui::pos2(right - 16.0, cy);
    if icon_button(ui, full_c, ph::CORNERS_OUT, "Full screen (F)", "full") {
        app.preview.toggle_fullscreen(ui.ctx());
    }
    right = full_c.x - 16.0 - 14.0;
    if show_snap {
        let snap_c = egui::pos2(right - 16.0, cy);
        if icon_button(
            ui,
            snap_c,
            ph::CAMERA,
            "Take a snapshot: saves this frame as a picture in the library",
            "snap",
        ) {
            app.take_snapshot();
        }
        right = snap_c.x - 16.0 - 14.0;
    }
    if show_thumb {
        let thumb_c = egui::pos2(right - 16.0, cy);
        if icon_button(
            ui,
            thumb_c,
            ph::IMAGE_SQUARE,
            "Make a thumbnail: a picture with big words, for your video's cover",
            "thumb",
        ) {
            crate::dialogs::open_thumbnail(app);
        }
        right = thumb_c.x - 16.0 - 14.0;
    }
    let size = app.preview.render_size;
    let info = ui.painter().layout_no_wrap(
        format!(
            "{} \u{b7} {}\u{d7}{}",
            app.project.settings.aspect_label(),
            size.0,
            size.1
        ),
        FontId::proportional(12.0),
        theme::TEXT_FAINT,
    );
    // The size only when it fits clear of everything else.
    let show_info = right - info.size().x - 14.0 > left_end + group_w + 28.0;
    if show_info {
        ui.painter().galley(
            egui::pos2(right - info.size().x, cy - info.size().y / 2.0),
            info.clone(),
            theme::TEXT_FAINT,
        );
    }
    let right_start = if show_info {
        right - info.size().x
    } else {
        right + 14.0
    };

    // Middle: the five buttons, centred in the room between.
    let playing = app.is_playing();
    let lo = rect.left() + group_w / 2.0 + 8.0;
    let hi = (rect.right() - group_w / 2.0 - 8.0).max(lo);
    let mid = ((left_end + right_start) / 2.0).clamp(lo, hi);
    let mut x = mid - group_w / 2.0;
    for i in 0..5 {
        let main = i == 2;
        let w = if main { 38.0 } else { 32.0 };
        let r = Rect::from_min_size(
            egui::pos2(x, cy - if main { 17.0 } else { 16.0 }),
            egui::vec2(w, if main { 34.0 } else { 32.0 }),
        );
        x += w + 10.0;
        let tip = [
            "Go to start (Home)",
            "Previous frame (\u{2190})",
            "Play / pause (Space)",
            "Next frame (\u{2192})",
            "Stop",
        ][i];
        let resp = ui.interact(r, Id::new(("pv-btn", i)), Sense::click());
        if main {
            ui.painter().rect_filled(
                r,
                CornerRadius::same(6),
                if resp.hovered() {
                    Color32::from_rgb(0x35, 0x37, 0x3d)
                } else {
                    Color32::from_rgb(0x2a, 0x2c, 0x31)
                },
            );
            // A filled play triangle, or two bars for pause.
            let c = r.center();
            if playing {
                for dx in [-3.6, 3.6] {
                    ui.painter().rect_filled(
                        Rect::from_center_size(egui::pos2(c.x + dx, c.y), egui::vec2(4.0, 12.0)),
                        CornerRadius::same(1),
                        Color32::WHITE,
                    );
                }
            } else {
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(c.x - 4.5, c.y - 7.0),
                        egui::pos2(c.x - 4.5, c.y + 7.0),
                        egui::pos2(c.x + 7.0, c.y),
                    ],
                    Color32::WHITE,
                    Stroke::NONE,
                ));
            }
        } else {
            if resp.hovered() {
                ui.painter()
                    .rect_filled(r, CornerRadius::same(9), theme::HOVER);
            }
            let icon = [ph::SKIP_BACK, ph::CARET_LEFT, "", ph::CARET_RIGHT, ph::STOP][i];
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(17.0),
                if resp.hovered() {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                },
            );
        }
        if resp.on_hover_text(tip).clicked() {
            match i {
                0 | 4 => {
                    app.pause();
                    match app.preview.mode {
                        PreviewMode::Timeline => app.seek(Time::ZERO),
                        PreviewMode::Source => app.seek_source(Time::ZERO),
                    }
                }
                1 => app.step_frames(-1),
                2 => app.toggle_play(),
                _ => app.step_frames(1),
            }
        }
    }
}

fn transport_bar(app: &mut TidyCutsApp, ui: &mut egui::Ui, rect: Rect, fullscreen: bool) {
    let fps = app.project.settings.frame_rate;
    let (pos, len) = match app.preview.mode {
        PreviewMode::Timeline => (app.playhead(), app.project.duration()),
        PreviewMode::Source => {
            let len = app
                .preview
                .source_media
                .and_then(|id| app.media_item(id))
                .map(|m| m.duration)
                .unwrap_or(Time::ZERO);
            (app.preview.source_time, len)
        }
    };
    if !fullscreen {
        ui.painter()
            .hline(rect.x_range(), rect.top(), Stroke::new(1.0, theme::BORDER));
    }

    // Seek bar along the top of the transport.
    let bar = Rect::from_min_max(
        egui::pos2(rect.left() + 12.0, rect.top() + 6.0),
        egui::pos2(rect.right() - 12.0, rect.top() + 14.0),
    );
    let seek = ui.interact(
        bar.expand2(egui::vec2(0.0, 4.0)),
        Id::new(("pv-seek", fullscreen)),
        Sense::click_and_drag(),
    );
    let frac = if len > Time::ZERO {
        (pos.as_secs_f64() / len.as_secs_f64()).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let track = Rect::from_center_size(bar.center(), egui::vec2(bar.width(), 4.0));
    ui.painter()
        .rect_filled(track, CornerRadius::same(2), Color32::from_white_alpha(40));
    ui.painter().rect_filled(
        Rect::from_min_max(
            track.min,
            egui::pos2(track.left() + track.width() * frac, track.bottom()),
        ),
        CornerRadius::same(2),
        theme::ACCENT,
    );
    if seek.hovered() || seek.dragged() {
        ui.painter().circle_filled(
            egui::pos2(track.left() + track.width() * frac, track.center().y),
            6.0,
            Color32::WHITE,
        );
    }
    if let (true, Some(p)) = (
        seek.clicked() || seek.dragged(),
        seek.interact_pointer_pos(),
    ) {
        let f = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
        let t = fps.snap(Time::from_secs_f64(len.as_secs_f64() * f));
        match app.preview.mode {
            PreviewMode::Timeline => {
                app.timeline.scrubbing = seek.dragged();
                app.seek(t)
            }
            PreviewMode::Source => {
                app.preview.scrubbing = seek.dragged();
                app.seek_source(t)
            }
        }
    }
    if seek.drag_stopped() {
        app.timeline.scrubbing = false;
        app.preview.scrubbing = false;
    }

    let cy = rect.top() + 34.0;
    let text_color = if fullscreen {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    ui.painter().text(
        egui::pos2(rect.left() + 14.0, cy),
        Align2::LEFT_CENTER,
        pos.timecode(fps),
        FontId::monospace(13.0),
        text_color,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 118.0, cy),
        Align2::LEFT_CENTER,
        format!("/ {}", len.timecode(fps)),
        FontId::monospace(13.0),
        theme::TEXT_DIM,
    );

    let playing = app.is_playing();
    let buttons: [(&str, &str); 5] = [
        (ph::SKIP_BACK, "Go to start (Home)"),
        (ph::CARET_LEFT, "Previous frame (←)"),
        (
            if playing { ph::PAUSE } else { ph::PLAY },
            "Play / pause (Space)",
        ),
        (ph::CARET_RIGHT, "Next frame (→)"),
        (ph::STOP, "Stop"),
    ];
    let w = 34.0;
    let x0 = rect.center().x - w * 2.5;
    for (i, (icon, tip)) in buttons.iter().enumerate() {
        let r = Rect::from_center_size(
            egui::pos2(x0 + w * (i as f32 + 0.5), cy),
            egui::vec2(30.0, 30.0),
        );
        let resp = ui.interact(r, Id::new(("pv-btn", i, fullscreen)), Sense::click());
        let main = i == 2;
        if main {
            ui.painter().circle_filled(
                r.center(),
                15.0,
                if resp.hovered() {
                    Color32::WHITE
                } else {
                    theme::TEXT
                },
            );
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(6), Color32::from_white_alpha(20));
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *icon,
            FontId::proportional(if main { 16.0 } else { 15.0 }),
            if main { Color32::BLACK } else { text_color },
        );
        if resp.on_hover_text(*tip).clicked() {
            match i {
                0 => {
                    app.pause();
                    match app.preview.mode {
                        PreviewMode::Timeline => app.seek(Time::ZERO),
                        PreviewMode::Source => app.seek_source(Time::ZERO),
                    }
                }
                1 => app.step_frames(-1),
                2 => app.toggle_play(),
                3 => app.step_frames(1),
                _ => {
                    app.pause();
                    if app.preview.mode == PreviewMode::Timeline {
                        app.seek(Time::ZERO);
                    } else {
                        app.seek_source(Time::ZERO);
                    }
                }
            }
        }
    }

    // Right: add the source clip to the timeline, and full screen.
    let right = rect.right() - 16.0;
    if app.preview.mode == PreviewMode::Source && !fullscreen {
        let r = Rect::from_min_max(
            egui::pos2(right - 150.0, cy - 13.0),
            egui::pos2(right, cy + 13.0),
        );
        if ui
            .put(
                r,
                theme::outline_button(format!("{}  Add to timeline", ph::PLUS)),
            )
            .clicked()
        {
            if let Some(m) = app.preview.source_media {
                app.add_media_to_timeline(m, None);
            }
        }
    } else {
        let r = Rect::from_center_size(egui::pos2(right - 12.0, cy), egui::vec2(28.0, 28.0));
        let resp = ui.interact(r, Id::new(("pv-full", fullscreen)), Sense::click());
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            if fullscreen {
                ph::CORNERS_IN
            } else {
                ph::CORNERS_OUT
            },
            FontId::proportional(16.0),
            if resp.hovered() {
                Color32::WHITE
            } else {
                theme::TEXT_DIM
            },
        );
        if resp
            .on_hover_text(if fullscreen {
                "Exit full screen (Esc)"
            } else {
                "Full screen (F)"
            })
            .clicked()
        {
            app.preview.toggle_fullscreen(ui.ctx());
        }
        let snap_r = Rect::from_center_size(egui::pos2(right - 44.0, cy), egui::vec2(28.0, 28.0));
        let snap = ui.interact(snap_r, Id::new(("pv-snapshot", fullscreen)), Sense::click());
        ui.painter().text(
            snap_r.center(),
            Align2::CENTER_CENTER,
            ph::CAMERA,
            FontId::proportional(16.0),
            if snap.hovered() {
                Color32::WHITE
            } else {
                theme::TEXT_DIM
            },
        );
        if snap
            .on_hover_text("Take a snapshot: saves this frame as a picture in the library")
            .clicked()
        {
            app.take_snapshot();
        }
        let size = app.preview.render_size;
        ui.painter().text(
            egui::pos2(right - 66.0, cy),
            Align2::RIGHT_CENTER,
            format!(
                "{}  ·  {}×{}",
                app.project.settings.aspect_label(),
                size.0,
                size.1
            ),
            FontId::proportional(11.5),
            theme::TEXT_FAINT,
        );
    }
}

#[cfg(test)]
mod cell_tests {
    use super::inside_quad;

    #[test]
    fn a_point_is_found_inside_a_cell_only() {
        let left = [[0.0, 0.0], [960.0, 0.0], [960.0, 1080.0], [0.0, 1080.0]];
        assert!(inside_quad(&left, [480.0, 540.0]));
        assert!(!inside_quad(&left, [1200.0, 540.0]));
        // A triangle repeats its last corner.
        let tri = [[0.0, 0.0], [100.0, 0.0], [0.0, 100.0], [0.0, 100.0]];
        assert!(inside_quad(&tri, [20.0, 20.0]));
        assert!(!inside_quad(&tri, [80.0, 80.0]));
    }
}
