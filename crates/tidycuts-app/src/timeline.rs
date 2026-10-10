//! Bottom panel: toolbar, ruler, up to 10 video and 5 audio tracks, clips
//! with trim handles, fades and volume lines, the playhead with its cut
//! button, and dropping from the library.

use crate::app::TidyCutsApp;
use crate::library::LibraryDrag;
use crate::theme;
use crate::thumbs::PEAKS_PER_SEC;
use crate::widgets;
use crate::widgets::short_duration;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use egui_phosphor::regular as ph;
use std::collections::HashMap;
use tidycuts_core::{
    Clip, ClipId, Edge, Generator, MarkerId, MediaKind, Project, Time, TrackId, TrackKind,
    MAX_AUDIO_TRACKS, MAX_VIDEO_TRACKS,
};

const TOOLBAR_H: f32 = 40.0;
const RULER_H: f32 = 30.0;
const HEADER_W: f32 = 140.0;
/// Lane heights: the main video track, other video tracks and audio tracks.
const MAIN_TRACK_H: f32 = 70.0;
const VIDEO_TRACK_H: f32 = 50.0;
const AUDIO_TRACK_H: f32 = 58.0;
/// The free lane kept above the top video track and below the last audio
/// track: dropping something there makes a new track.
const FREE_LANE_H: f32 = 40.0;
/// How small and how big a track can be dragged.
const MIN_TRACK_H: f32 = 24.0;
const MAX_TRACK_H: f32 = 320.0;
/// Half the height of the grab zone on the line between two track headers.
const RESIZE_GRAB: f32 = 4.0;
/// Lanes touch; a thin line tells them apart.
const ROW_GAP: f32 = 0.0;
/// Space above and below a clip inside its lane.
const CLIP_INSET: f32 = 4.0;
const SCROLLBAR_H: f32 = 12.0;
/// How close (in pixels) an edge must be to snap.
const SNAP_PX: f32 = 9.0;
/// Width of the trim zone at each end of a clip.
const EDGE_PX: f32 = 7.0;
const MIN_PPS: f32 = 0.5;
const MAX_PPS: f32 = 1200.0;

/// What a press on the lanes landed on.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Zone {
    Body,
    TrimStart,
    TrimEnd,
    Volume,
    FadeIn,
    FadeOut,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Empty {
        track: Option<TrackId>,
        t: Time,
    },
    Clip {
        clip: ClipId,
        track: TrackId,
        zone: Zone,
    },
    /// A transition badge: at the clip's start, or (`out`) at its end.
    Transition {
        clip: ClipId,
        out: bool,
    },
    Playhead,
}

#[derive(Debug, Clone)]
enum DragOp {
    Move {
        clip: ClipId,
        /// Pointer time minus clip start when the drag began.
        grab: Time,
        /// Every clip moving with it (multi-selection).
        group: Vec<ClipId>,
    },
    Trim {
        clip: ClipId,
        edge: Edge,
    },
    Volume {
        clip: ClipId,
        start: f64,
    },
    Fade {
        clip: ClipId,
        out: bool,
    },
    Scrub,
    Marquee,
}

struct Drag {
    op: DragOp,
    before: Project,
    was_dirty: bool,
    start: Pos2,
    /// Snap line to draw, if the drag snapped this frame.
    snapped: Option<Time>,
}

struct RulerDrag {
    start_x: f32,
    start_pps: f32,
    /// Time under the pointer when the drag began, kept in place.
    anchor: Time,
    moved: bool,
    scrub: bool,
}

pub struct TimelineState {
    pub playhead: Time,
    pub px_per_sec: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub snap: bool,
    /// The playhead is being dragged (the preview uses fast frames).
    pub scrubbing: bool,
    pub fit_requested: bool,
    /// Track height multiplier (small / normal / large).
    pub track_size: f32,
    /// Heights the user dragged tracks to (the rest follow `track_size`).
    pub track_heights: HashMap<TrackId, f32>,
    /// Scrollable height of the lanes last time, to keep the bottom in place
    /// when a track is added above.
    content_h: f32,
    drag: Option<Drag>,
    ruler: Option<RulerDrag>,
    /// The marker being dragged, with the project as it was and whether it
    /// has moved yet.
    marker_drag: Option<(MarkerId, Project, bool)>,
    /// The marker last clicked; Delete removes it when no clip is selected.
    pub selected_marker: Option<MarkerId>,
    menu: Option<Hit>,
    lanes_w: f32,
}

impl Default for TimelineState {
    fn default() -> Self {
        TimelineState {
            playhead: Time::ZERO,
            px_per_sec: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            snap: true,
            scrubbing: false,
            fit_requested: false,
            track_size: 1.0,
            track_heights: HashMap::new(),
            content_h: 0.0,
            drag: None,
            ruler: None,
            marker_drag: None,
            selected_marker: None,
            menu: None,
            lanes_w: 1000.0,
        }
    }
}

impl TimelineState {
    /// Zooms by `factor`, keeping the playhead where it is on screen.
    pub fn zoom_by(&mut self, factor: f32) {
        let px = self.playhead.as_secs_f64() as f32 * self.px_per_sec - self.scroll_x;
        let px = px.clamp(0.0, self.lanes_w);
        let t = (px + self.scroll_x) / self.px_per_sec;
        self.px_per_sec = (self.px_per_sec * factor).clamp(MIN_PPS, MAX_PPS);
        self.scroll_x = (t * self.px_per_sec - px).max(0.0);
    }
}

/// Maps between timeline time and screen x.
#[derive(Clone, Copy)]
struct View {
    left: f32,
    pps: f32,
    scroll: f32,
}

impl View {
    fn x(&self, t: Time) -> f32 {
        self.left + t.as_secs_f64() as f32 * self.pps - self.scroll
    }

    fn t(&self, x: f32) -> Time {
        Time::from_secs_f64(((x - self.left + self.scroll) / self.pps).max(0.0) as f64)
    }
}

#[derive(Clone, Copy)]
struct Row {
    id: TrackId,
    kind: TrackKind,
    rect: Rect,
    main: bool,
}

/// A track's height: what the user dragged it to, or the usual one for its
/// kind scaled by the size setting.
fn track_height(app: &TidyCutsApp, id: TrackId, kind: TrackKind, main: bool) -> f32 {
    if let Some(h) = app.timeline.track_heights.get(&id) {
        return h.clamp(MIN_TRACK_H, MAX_TRACK_H).round();
    }
    let base = match (kind, main) {
        (TrackKind::Video, true) => MAIN_TRACK_H,
        (TrackKind::Video, false) => VIDEO_TRACK_H,
        (TrackKind::Audio, _) => AUDIO_TRACK_H,
    };
    (base * app.timeline.track_size).round()
}

/// The free lanes at the top and bottom of the track stack (zero when no
/// more tracks of that kind can be added).
fn free_lane_heights(app: &TidyCutsApp) -> (f32, f32) {
    (
        if app.project.can_add_track(TrackKind::Video) {
            FREE_LANE_H
        } else {
            0.0
        },
        if app.project.can_add_track(TrackKind::Audio) {
            FREE_LANE_H
        } else {
            0.0
        },
    )
}

/// Height of everything in the lanes: the tracks and the free lanes.
fn content_height(app: &TidyCutsApp) -> f32 {
    let main = app.project.main_track();
    let (top, bottom) = free_lane_heights(app);
    top + bottom
        + app
            .project
            .tracks
            .iter()
            .map(|t| track_height(app, t.id, t.kind, Some(t.id) == main) + ROW_GAP)
            .sum::<f32>()
}

pub fn show(app: &mut TidyCutsApp, ui: &mut Ui) {
    // The panel's own top line takes the first pixel.
    let mut full = ui.max_rect();
    full.min.y += 1.0;
    let toolbar = Rect::from_min_size(full.min, egui::vec2(full.width(), TOOLBAR_H));
    toolbar_ui(app, ui, toolbar);
    ui.painter().hline(
        full.x_range(),
        toolbar.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER),
    );

    let body = Rect::from_min_max(egui::pos2(full.left(), toolbar.bottom()), full.max);
    let ruler = Rect::from_min_max(
        egui::pos2(body.left() + HEADER_W, body.top()),
        egui::pos2(body.right(), body.top() + RULER_H),
    );
    let lanes = Rect::from_min_max(
        egui::pos2(body.left() + HEADER_W, ruler.bottom()),
        egui::pos2(body.right(), body.bottom() - SCROLLBAR_H - 2.0),
    );
    let headers = Rect::from_min_max(
        egui::pos2(body.left(), ruler.bottom()),
        egui::pos2(body.left() + HEADER_W, lanes.bottom()),
    );
    app.timeline.lanes_w = lanes.width();

    if app.timeline.fit_requested {
        app.timeline.fit_requested = false;
        let len = app.project.duration().as_secs_f64().max(5.0) as f32;
        app.timeline.px_per_sec = (lanes.width() * 0.92 / len).clamp(MIN_PPS, MAX_PPS);
        app.timeline.scroll_x = 0.0;
    }
    handle_scroll_and_zoom(app, ui, body, lanes);
    let view = View {
        left: lanes.left(),
        pps: app.timeline.px_per_sec,
        scroll: app.timeline.scroll_x,
    };

    // The tracks sit at the bottom of the lanes, with a free lane above the
    // top video track and below the last audio track so there is always
    // somewhere to drop a clip that starts a new track. With many tracks the
    // lanes scroll.
    let main = app.project.main_track();
    let (free_top_h, free_bottom_h) = free_lane_heights(app);
    let heights: Vec<f32> = app
        .project
        .tracks
        .iter()
        .map(|t| track_height(app, t.id, t.kind, Some(t.id) == main) + ROW_GAP)
        .collect();
    let total_h = content_height(app);
    let spare = (lanes.height() - total_h).max(0.0);
    let max_scroll_y = (total_h - lanes.height()).max(0.0);
    // Staying at the bottom while the stack grows keeps the main track put.
    let was_bottom = app.timeline.content_h > 0.0
        && app.timeline.scroll_y >= (app.timeline.content_h - lanes.height()).max(0.0) - 1.0
        && app.timeline.scroll_y > 0.0;
    if was_bottom && total_h != app.timeline.content_h {
        app.timeline.scroll_y = max_scroll_y;
    }
    app.timeline.content_h = total_h;
    app.timeline.scroll_y = app.timeline.scroll_y.clamp(0.0, max_scroll_y);
    let origin = lanes.top() + spare - app.timeline.scroll_y;
    let free_top = (free_top_h > 0.0).then(|| {
        Rect::from_min_max(
            egui::pos2(lanes.left(), origin),
            egui::pos2(lanes.right(), origin + free_top_h),
        )
    });
    let mut y = origin + free_top_h;
    let mut rows: Vec<Row> = Vec::new();
    for (t, h) in app.project.tracks.iter().zip(&heights) {
        let h = h - ROW_GAP;
        rows.push(Row {
            id: t.id,
            kind: t.kind,
            rect: Rect::from_min_max(
                egui::pos2(lanes.left(), y),
                egui::pos2(lanes.right(), y + h),
            ),
            main: Some(t.id) == main,
        });
        y += h + ROW_GAP;
    }
    let free_bottom = (free_bottom_h > 0.0).then(|| {
        Rect::from_min_max(
            egui::pos2(lanes.left(), y),
            egui::pos2(lanes.right(), y + free_bottom_h),
        )
    });

    let painter = ui
        .painter()
        .with_clip_rect(Rect::from_min_max(headers.min, lanes.max));
    // The lanes are a shade darker than the panel; the headers match the
    // panel. A thin line runs along the top of every lane.
    painter.rect_filled(
        Rect::from_min_max(egui::pos2(lanes.left(), lanes.top()), lanes.max),
        CornerRadius::ZERO,
        theme::LANE_BG,
    );
    for r in &rows {
        painter.hline(
            lanes.left()..=lanes.right(),
            r.rect.top() + 0.5,
            Stroke::new(1.0, theme::LANE_LINE),
        );
    }
    if let Some(last) = rows.last() {
        painter.hline(
            lanes.left()..=lanes.right(),
            last.rect.bottom() + 0.5,
            Stroke::new(1.0, theme::LANE_LINE),
        );
    }

    free_lane_hints(
        app,
        ui,
        lanes,
        &rows,
        [
            (free_top, TrackKind::Video),
            (free_bottom, TrackKind::Audio),
        ],
    );
    let lanes_resp = ui.interact(lanes, Id::new("tl-lanes"), Sense::click_and_drag());
    track_headers(
        app,
        ui,
        headers,
        &rows,
        [
            (free_top, TrackKind::Video),
            (free_bottom, TrackKind::Audio),
        ],
    );
    draw_clips(app, ui, lanes, view, &rows);
    interact_lanes(app, ui, lanes, view, &rows, &lanes_resp);
    library_drop(app, ui, lanes, view, &rows, &lanes_resp);
    if app.project.tracks.iter().all(|t| t.clips.is_empty()) && app.timeline.drag.is_none() {
        empty_hint(ui, lanes, &rows);
    }
    ruler_ui(app, ui, ruler, view);
    playhead(app, ui, ruler, lanes, view, &rows);
    corner(
        app,
        ui,
        Rect::from_min_max(body.min, egui::pos2(headers.right(), ruler.bottom())),
    );
    scrollbar(
        app,
        ui,
        Rect::from_min_max(
            egui::pos2(lanes.left(), lanes.bottom() + 2.0),
            egui::pos2(lanes.right(), body.bottom()),
        ),
    );
}

// ----- toolbar ---------------------------------------------------------

fn tool(
    ui: &mut Ui,
    rect: Rect,
    icon: &str,
    tip: &str,
    enabled: bool,
    active: bool,
) -> egui::Response {
    let resp = ui.interact(
        rect,
        Id::new(("tl-tool", icon, tip)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    // A 30-pixel square; "on" is a darker fill with an accent icon.
    if active {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), theme::SEL);
    } else if enabled && resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), theme::HOVER);
    }
    let color = if !enabled {
        theme::TEXT_FAINT
    } else if active {
        theme::ACCENT
    } else if resp.hovered() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(18.0),
        color,
    );
    resp.on_hover_text(tip)
}

fn toolbar_ui(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect) {
    // The bar is 40 pixels with a line under it: buttons are 30-pixel squares
    // 3 apart, groups are told apart by a 1-pixel line with 8 on each side.
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::PANEL_BG);
    let cy = rect.center().y - 0.5;
    let mut x = rect.left() + 12.0;
    let seps = std::cell::RefCell::new(Vec::<f32>::new());
    let mut next = |gap: bool| {
        if gap {
            x += 8.0;
            seps.borrow_mut().push(x);
            // the line, 8 beside it, and the 3-pixel gap before the button
            x += 1.0 + 8.0 + 3.0;
        }
        let r = Rect::from_min_size(egui::pos2(x, cy - 15.0), egui::vec2(30.0, 30.0));
        x += 33.0;
        r
    };
    let has_sel = !app.selection.is_empty();
    let sel = app.selected_clip();
    let visual = sel.is_some_and(|c| {
        app.project
            .clip(c)
            .is_some_and(|(t, _)| t.kind == TrackKind::Video)
    });
    let cmd = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };

    if tool(
        ui,
        next(false),
        ph::ARROW_COUNTER_CLOCKWISE,
        &format!("Undo ({cmd}Z)"),
        app.can_undo(),
        false,
    )
    .clicked()
    {
        app.undo();
    }
    if tool(
        ui,
        next(false),
        ph::ARROW_CLOCKWISE,
        &format!("Redo ({cmd}Y)"),
        app.can_redo(),
        false,
    )
    .clicked()
    {
        app.redo();
    }
    if tool(
        ui,
        next(true),
        ph::TRASH,
        "Delete (Del) · Shift+Del closes the gap",
        has_sel,
        false,
    )
    .clicked()
    {
        app.delete_selection(false);
    }
    if tool(
        ui,
        next(false),
        ph::SCISSORS,
        &format!("Split at the playhead ({cmd}B)"),
        true,
        false,
    )
    .clicked()
    {
        app.split_at_playhead();
    }
    if tool(ui, next(false), ph::CROP, "Crop and zoom", visual, false).clicked() {
        if let Some(c) = sel {
            app.dialogs.open_crop(c);
        }
    }
    if tool(
        ui,
        next(false),
        ph::TEXT_T,
        &format!("Add a title at the playhead ({cmd}T)"),
        true,
        false,
    )
    .clicked()
    {
        app.add_title("", None);
    }
    let speed_r = next(false);
    let speed = tool(
        ui,
        speed_r,
        ph::GAUGE,
        &format!("Speed ({cmd}R)"),
        has_sel,
        false,
    );
    egui::Popup::menu(&speed).show(|ui| {
        ui.set_min_width(160.0);
        for s in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0, 8.0] {
            if ui.button(format!("{s}×")).clicked() {
                for id in app.selection.clone() {
                    app.set_speed(id, s);
                }
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Reverse").clicked() {
            app.toggle_reverse();
            ui.close();
        }
        if ui.button("Custom speed…").clicked() {
            app.inspector.request_tab("Speed");
            app.open_inspector();
            ui.close();
        }
    });
    if tool(
        ui,
        next(false),
        ph::DIAMOND,
        "Add keyframes at the playhead for position, scale, rotation and opacity",
        visual,
        false,
    )
    .clicked()
    {
        add_transform_keyframes(app);
    }
    if tool(
        ui,
        next(false),
        ph::PALETTE,
        "Color correction",
        visual,
        false,
    )
    .clicked()
    {
        app.inspector.request_tab("Color");
        app.open_inspector();
    }
    if tool(
        ui,
        next(true),
        ph::SPEAKER_HIGH,
        "Mute or unmute the selected clips",
        has_sel,
        false,
    )
    .clicked()
    {
        app.toggle_mute();
    }
    // Silence Detection: the quiet parts of the selected clip, found and removed.
    let silence_clip = sel.filter(|c| {
        crate::detection::unsuitable(
            app,
            *c,
            crate::detection::Detection::Silences {
                noise_db: -35.0,
                min_secs: 0.5,
            },
        )
        .is_none()
    });
    if tool(
        ui,
        next(false),
        ph::WAVEFORM,
        if silence_clip.is_some() {
            "Silence Detection: find the quiet parts of the selected clip and cut them out"
        } else {
            "Silence Detection: select a clip with sound first"
        },
        silence_clip.is_some(),
        false,
    )
    .clicked()
    {
        if let Some(c) = silence_clip {
            app.dialogs.open_silence(c);
        }
    }
    if tool(
        ui,
        next(false),
        ph::SLIDERS_HORIZONTAL,
        "Show or hide the properties panel",
        true,
        if app.prefs.layout_preset.has_dock() {
            app.inspector_open
        } else {
            app.left_shows_props
        },
    )
    .clicked()
    {
        if app.prefs.layout_preset.has_dock() {
            app.inspector_open = !app.inspector_open;
        } else {
            app.left_shows_props = !app.left_shows_props;
        }
    }

    if tool(
        ui,
        next(false),
        ph::FADERS,
        "Audio mixer: volume, mute and solo for every track",
        true,
        app.mixer.open,
    )
    .clicked()
    {
        app.mixer.open = !app.mixer.open;
    }

    if tool(
        ui,
        next(false),
        ph::LIGHTNING,
        &preview_tip(app),
        true,
        app.previews.rendering(),
    )
    .clicked()
    {
        if app.previews.rendering() {
            app.cancel_previews();
        } else {
            app.render_previews();
        }
    }

    // Right side, from the right edge (12 in): zoom in, a 110-pixel slider with
    // 6 pixels beside it, zoom out, a line, then fit, track height, snapping
    // and magnetic; everything 3 apart.
    let right = rect.right() - 12.0;
    let zoom_in = Rect::from_min_size(egui::pos2(right - 30.0, cy - 15.0), egui::vec2(30.0, 30.0));
    let slider = Rect::from_min_max(
        egui::pos2(zoom_in.left() - 3.0 - 6.0 - 110.0, cy - 9.0),
        egui::pos2(zoom_in.left() - 3.0 - 6.0, cy + 9.0),
    );
    let zoom_out = Rect::from_min_size(
        egui::pos2(slider.left() - 6.0 - 3.0 - 30.0, cy - 15.0),
        egui::vec2(30.0, 30.0),
    );
    let sep_right = zoom_out.left() - 3.0 - 8.0 - 1.0;
    seps.borrow_mut().push(sep_right);
    let fit = Rect::from_min_size(
        egui::pos2(sep_right - 8.0 - 3.0 - 30.0, cy - 15.0),
        egui::vec2(30.0, 30.0),
    );
    let size_r = fit.translate(egui::vec2(-33.0, 0.0));
    let snap_r = size_r.translate(egui::vec2(-33.0, 0.0));
    let magnet_r = snap_r.translate(egui::vec2(-33.0, 0.0));
    let magnetic = app.project.settings.magnetic;
    if tool(
        ui,
        magnet_r,
        ph::MAGNET,
        if magnetic {
            "Magnetic main track on: clips on Video 1 stay together with no gaps"
        } else {
            "Magnetic main track off"
        },
        true,
        magnetic,
    )
    .clicked()
    {
        app.set_magnetic(!magnetic);
    }
    let snap = app.timeline.snap;
    if tool(
        ui,
        snap_r,
        ph::ALIGN_CENTER_HORIZONTAL_SIMPLE,
        if snap {
            "Snapping on (N)"
        } else {
            "Snapping off (N)"
        },
        true,
        snap,
    )
    .clicked()
    {
        app.timeline.snap = !snap;
        app.prefs.snapping = !snap;
    }
    let size = tool(ui, size_r, ph::STACK, "Track height", true, false);
    egui::Popup::menu(&size).show(|ui| {
        for (label, v) in [("Small", 0.75), ("Normal", 1.0), ("Large", 1.4)] {
            if ui.radio(app.timeline.track_size == v, label).clicked() {
                app.timeline.track_size = v;
                ui.close();
            }
        }
    });
    if tool(
        ui,
        fit,
        ph::ARROWS_HORIZONTAL,
        "Zoom to fit the whole project",
        true,
        false,
    )
    .clicked()
    {
        app.timeline.fit_requested = true;
    }
    if tool(
        ui,
        zoom_out,
        ph::MAGNIFYING_GLASS_MINUS,
        "Zoom out",
        true,
        false,
    )
    .clicked()
    {
        app.timeline.zoom_by(1.0 / 1.5);
    }
    if tool(
        ui,
        zoom_in,
        ph::MAGNIFYING_GLASS_PLUS,
        "Zoom in",
        true,
        false,
    )
    .clicked()
    {
        app.timeline.zoom_by(1.5);
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(slider), |ui| {
        let mut z = f64::from(app.timeline.px_per_sec).ln();
        if widgets::slider_with(
            ui,
            widgets::LIBRARY_SLIDER,
            &mut z,
            f64::from(MIN_PPS).ln()..=f64::from(MAX_PPS).ln(),
            slider.width(),
            false,
        )
        .on_hover_text("Timeline zoom (or drag the ruler)")
        .changed()
        {
            let f = z.exp() as f32 / app.timeline.px_per_sec;
            app.timeline.zoom_by(f);
        }
    });
    for sx in seps.borrow().iter() {
        ui.painter().vline(
            *sx + 0.5,
            (cy - 9.0)..=(cy + 9.0),
            Stroke::new(1.0, theme::BORDER),
        );
    }
}

/// Toolbar keyframe button: keyframes the transform at the playhead.
fn add_transform_keyframes(app: &mut TidyCutsApp) {
    let t = app.playhead();
    let Some(id) = app.selected_clip() else {
        return;
    };
    let Some((_, clip)) = app.project.clip(id) else {
        return;
    };
    if !clip.contains(t) {
        app.notify("Move the playhead over the clip to add a keyframe.", false);
        return;
    }
    let lt = clip.local(t);
    app.edit(|p| match p.clip_mut(id) {
        Some(c) => {
            let tr = &mut c.transform;
            for a in [
                &mut tr.position_x,
                &mut tr.position_y,
                &mut tr.scale_x,
                &mut tr.scale_y,
                &mut tr.rotation,
                &mut tr.opacity,
            ] {
                let v = a.value_at(lt);
                a.set_keyframe(lt, v);
            }
            true
        }
        None => false,
    });
    app.inspector.request_tab("Video");
    app.open_inspector();
    app.notify(
        "Keyframes added. Change values in Properties to animate.",
        false,
    );
}

// ----- scrolling -------------------------------------------------------

fn handle_scroll_and_zoom(app: &mut TidyCutsApp, ui: &mut Ui, body: Rect, lanes: Rect) {
    let hovering = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| body.contains(p));
    if hovering {
        let (scroll, zoom, pointer, mods) = ui.ctx().input(|i| {
            (
                i.smooth_scroll_delta(),
                i.zoom_delta(),
                i.pointer.hover_pos(),
                i.modifiers,
            )
        });
        if zoom != 1.0 {
            // Zoom around the pointer so the time under it stays put.
            let px = pointer.map(|p| p.x).unwrap_or(lanes.left());
            let t = (px - lanes.left() + app.timeline.scroll_x) / app.timeline.px_per_sec;
            app.timeline.px_per_sec = (app.timeline.px_per_sec * zoom).clamp(MIN_PPS, MAX_PPS);
            app.timeline.scroll_x = t * app.timeline.px_per_sec - (px - lanes.left());
        } else if scroll != egui::Vec2::ZERO {
            let total_h = content_height(app);
            if scroll.x != 0.0 {
                app.timeline.scroll_x -= scroll.x;
            } else if mods.shift || total_h <= lanes.height() {
                app.timeline.scroll_x -= scroll.y;
            } else {
                app.timeline.scroll_y -= scroll.y;
            }
        }
    }
    // Follow the playhead during playback.
    if app.is_playing()
        && app.prefs.auto_scroll
        && app.preview.mode == crate::preview::PreviewMode::Timeline
    {
        let x = app.timeline.playhead.as_secs_f64() as f32 * app.timeline.px_per_sec
            - app.timeline.scroll_x;
        if x > lanes.width() - 30.0 || x < 0.0 {
            app.timeline.scroll_x =
                (app.timeline.playhead.as_secs_f64() as f32 * app.timeline.px_per_sec - 60.0)
                    .max(0.0);
        }
    }
    let content_w = content_width(app, lanes.width());
    app.timeline.scroll_x = app
        .timeline
        .scroll_x
        .clamp(0.0, (content_w - lanes.width()).max(0.0));
}

/// Scrollable width: the project plus room to keep working after it.
fn content_width(app: &TidyCutsApp, view_w: f32) -> f32 {
    app.project.duration().as_secs_f64() as f32 * app.timeline.px_per_sec + view_w * 0.75
}

fn scrollbar(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect) {
    let content = content_width(app, rect.width());
    if content <= rect.width() {
        return;
    }
    let frac = rect.width() / content;
    let thumb_w = (rect.width() * frac).max(40.0);
    let max_scroll = content - rect.width();
    let x = rect.left() + (app.timeline.scroll_x / max_scroll) * (rect.width() - thumb_w);
    let thumb = Rect::from_min_size(
        egui::pos2(x, rect.top() + 3.0),
        egui::vec2(thumb_w, rect.height() - 6.0),
    );
    let resp = ui.interact(rect, Id::new("tl-hscroll"), Sense::drag());
    ui.painter().rect_filled(
        thumb,
        CornerRadius::same(4),
        if resp.dragged() || resp.hovered() {
            theme::TEXT_FAINT
        } else {
            theme::PANEL_BG_3
        },
    );
    if resp.dragged() {
        let dx = resp.drag_delta().x;
        app.timeline.scroll_x = (app.timeline.scroll_x
            + dx / (rect.width() - thumb_w) * max_scroll)
            .clamp(0.0, max_scroll);
    }
}

// ----- ruler and playhead ----------------------------------------------

fn tick_step(pps: f32) -> f64 {
    const STEPS: [f64; 17] = [
        0.04, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0,
        1800.0, 3600.0,
    ];
    STEPS
        .iter()
        .copied()
        .find(|s| *s as f32 * pps >= 90.0)
        .unwrap_or(3600.0)
}

fn ruler_label(secs: f64, fps: f64) -> String {
    let whole = secs.floor() as i64;
    let frac = secs - whole as f64;
    let base = if whole >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            whole / 3600,
            (whole / 60) % 60,
            whole % 60
        )
    } else {
        format!("{:02}:{:02}", whole / 60, whole % 60)
    };
    if frac > 1e-6 {
        format!("{base}:{:02}", (frac * fps).round() as i64)
    } else {
        base
    }
}

fn ruler_ui(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect, view: View) {
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, theme::PANEL_BG);
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    let fps = app.project.settings.frame_rate.as_f64();
    let major = tick_step(view.pps);
    let minor = major / 5.0;
    let start = view.t(rect.left()).as_secs_f64();
    let end = view.t(rect.right()).as_secs_f64();
    let mut i = (start / minor).floor() as i64;
    loop {
        let secs = i as f64 * minor;
        if secs > end {
            break;
        }
        let x = view.x(Time::from_secs_f64(secs));
        let is_major = i.rem_euclid(5) == 0;
        let h = if is_major { 10.0 } else { 5.0 };
        painter.vline(
            x,
            (rect.bottom() - h)..=rect.bottom(),
            Stroke::new(1.0, theme::TEXT_FAINT),
        );
        if is_major {
            painter.text(
                egui::pos2(x + 4.0, rect.top() + 5.0),
                Align2::LEFT_TOP,
                ruler_label(secs, fps),
                FontId::proportional(11.0),
                theme::TEXT_DIM,
            );
        }
        i += 1;
    }
    previews_bar(app, &painter, rect, view);
    // The project's end.
    let end_x = view.x(app.project.duration());
    if end_x > rect.left() && end_x < rect.right() && app.project.duration() > Time::ZERO {
        painter.vline(end_x, rect.y_range(), Stroke::new(1.0, theme::ACCENT_DIM));
    }

    // Click seeks; dragging zooms around where the drag started; the
    // playhead handle scrubs.
    let resp = ui.interact(rect, Id::new("tl-ruler"), Sense::click_and_drag());
    let pointer = resp.interact_pointer_pos();
    let handle_x = view.x(app.timeline.playhead);
    if resp.hovered() {
        let near = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| (p.x - handle_x).abs() < 8.0);
        // Windows has no magnifier cursors, so a double arrow says "drag
        // sideways"; the hand is for the playhead's handle.
        ui.ctx().set_cursor_icon(if near {
            CursorIcon::Grab
        } else {
            CursorIcon::ResizeHorizontal
        });
        if !near && !resp.dragged() {
            resp.clone().on_hover_text(
                "Drag left or right to zoom\nClick to move the playhead here\nCtrl + scroll also zooms",
            );
        }
    }
    if resp.drag_started() {
        let origin = ui.ctx().input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(pointer) {
            let scrub = (p.x - handle_x).abs() < 8.0;
            app.timeline.ruler = Some(RulerDrag {
                start_x: p.x,
                start_pps: app.timeline.px_per_sec,
                anchor: view.t(p.x),
                moved: false,
                scrub,
            });
            if scrub {
                app.pause();
            }
        }
    }
    if let (true, Some(p), Some(d)) = (resp.dragged(), pointer, app.timeline.ruler.as_mut()) {
        if d.scrub {
            app.timeline.scrubbing = true;
            let t = app.project.settings.frame_rate.snap(view.t(p.x));
            app.seek(t);
        } else {
            let dx = p.x - d.start_x;
            if dx.abs() > 3.0 {
                d.moved = true;
            }
            if d.moved {
                let pps = (d.start_pps * (dx / 160.0).exp()).clamp(MIN_PPS, MAX_PPS);
                let anchor_px =
                    d.anchor.as_secs_f64() as f32 * d.start_pps - (d.start_x - view.left);
                let screen = d.start_x - view.left;
                let _ = anchor_px;
                app.timeline.px_per_sec = pps;
                app.timeline.scroll_x = (d.anchor.as_secs_f64() as f32 * pps - screen).max(0.0);
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                // How much of the project fits across the window now.
                let across = Time::from_secs_f64((rect.width() / pps) as f64);
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    Id::new("zoom-tip"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.add(
                        egui::Label::new(format!(
                            "{}  {} across",
                            if dx > 0.0 {
                                ph::MAGNIFYING_GLASS_PLUS
                            } else {
                                ph::MAGNIFYING_GLASS_MINUS
                            },
                            short_duration(across)
                        ))
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            }
        }
    }
    if resp.drag_stopped() {
        if let Some(d) = app.timeline.ruler.take() {
            if !d.moved && !d.scrub {
                app.seek(app.project.settings.frame_rate.snap(d.anchor));
            }
        }
        app.timeline.scrubbing = false;
    }
    if resp.clicked() {
        if let Some(p) = pointer {
            let t = app.project.settings.frame_rate.snap(view.t(p.x));
            app.seek(t);
        }
    }
    markers_ui(app, ui, rect, view);
}

/// The tooltip of the Render Preview button.
fn preview_tip(app: &TidyCutsApp) -> String {
    if let Some(p) = app.previews.progress() {
        return format!("Rendering previews, {:.0}% done. Click to stop.", p * 100.0);
    }
    match app.previews.missing() {
        0 => "Render preview: nothing needs it right now. Parts that are hard to play get an amber mark on the ruler.".into(),
        n => format!("Render preview: get {n} hard-to-play part{} ready so they play smoothly", if n == 1 { "" } else { "s" }),
    }
}

/// The thin bar along the bottom of the ruler: green where a saved preview
/// will play, amber where the picture is hard to play and has none yet.
fn previews_bar(app: &TidyCutsApp, painter: &egui::Painter, rect: Rect, view: View) {
    let amber = Color32::from_rgb(0xff, 0xb3, 0x47);
    let y = (rect.bottom() - 4.0)..=(rect.bottom() - 1.0);
    let drawing = app.previews.drawing();
    for p in &app.previews.pieces {
        let (x0, x1) = (view.x(p.chunk.start), view.x(p.chunk.end));
        if x1 < rect.left() || x0 > rect.right() {
            continue;
        }
        let bar = |x0: f32, x1: f32, color: Color32| {
            painter.rect_filled(
                Rect::from_x_y_ranges(x0..=x1, y.clone()),
                CornerRadius::ZERO,
                color,
            );
        };
        if p.ready {
            bar(x0, x1, theme::GOOD);
        } else if p.chunk.heavy {
            // Being drawn right now: fills in as it goes.
            match drawing.filter(|(key, _)| *key == p.chunk.key) {
                Some((_, done)) => {
                    bar(x0, x1, amber.gamma_multiply(0.45));
                    bar(x0, x0 + (x1 - x0) * done, theme::GOOD);
                }
                None => bar(x0, x1, amber),
            }
        }
    }
}

/// The colours a marker can have; new markers use the mint accent.
pub const MARKER_COLORS: [(&str, Color32); 8] = [
    ("Red", Color32::from_rgb(0xff, 0x5d, 0x6a)),
    ("Orange", Color32::from_rgb(0xff, 0x9f, 0x43)),
    ("Yellow", Color32::from_rgb(0xff, 0xd9, 0x4a)),
    ("Green", Color32::from_rgb(0x7a, 0xd9, 0x5b)),
    ("Mint", theme::ACCENT),
    ("Blue", Color32::from_rgb(0x4a, 0x9e, 0xff)),
    ("Purple", Color32::from_rgb(0xa7, 0x7b, 0xff)),
    ("Pink", Color32::from_rgb(0xff, 0x7b, 0xc8)),
];

pub fn marker_color(index: u8) -> Color32 {
    MARKER_COLORS[(index as usize).min(MARKER_COLORS.len() - 1)].1
}

/// Draws the markers as flags on the ruler's lower edge. Click jumps to one,
/// dragging moves it (snapping like clips do), right-click renames,
/// recolours or deletes it.
fn markers_ui(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect, view: View) {
    let painter = ui.painter().with_clip_rect(rect);
    let flag_h = 15.0;
    let ids: Vec<MarkerId> = app.project.markers.iter().map(|m| m.id).collect();
    for (n, id) in ids.iter().enumerate() {
        let Some(m) = app.project.marker(*id).cloned() else {
            continue;
        };
        let x = view.x(m.time);
        if x < rect.left() - 120.0 || x > rect.right() + 8.0 {
            continue;
        }
        let color = marker_color(m.color);
        let selected = app.timeline.selected_marker == Some(m.id);
        let top = rect.bottom() - flag_h - 1.0;
        let hit = Rect::from_min_max(
            egui::pos2(x - 7.0, top - 2.0),
            egui::pos2(x + 7.0, rect.bottom()),
        );
        let resp = ui
            .interact(hit, Id::new(("tl-marker", m.id)), Sense::click_and_drag())
            .on_hover_text(if m.note.is_empty() {
                m.name.clone()
            } else {
                format!("{}\n{}", m.name, m.note)
            });
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        // A flag: a pole with a pennant to the right, so the point is exact.
        painter.vline(x, top..=rect.bottom(), Stroke::new(1.5, color));
        let pennant = vec![
            egui::pos2(x, top),
            egui::pos2(x + 11.0, top + 4.5),
            egui::pos2(x, top + 9.0),
        ];
        painter.add(Shape::convex_polygon(
            pennant,
            color,
            if selected {
                Stroke::new(1.5, Color32::WHITE)
            } else {
                Stroke::NONE
            },
        ));
        // Its name, when there's room before the next marker.
        let next_x = ids
            .get(n + 1)
            .and_then(|i| app.project.marker(*i))
            .map_or(f32::MAX, |m| view.x(m.time));
        if !m.name.is_empty() && next_x - x > 70.0 {
            painter.text(
                egui::pos2(x + 15.0, top + 4.5),
                Align2::LEFT_CENTER,
                &m.name,
                FontId::proportional(10.5),
                theme::TEXT_DIM,
            );
        }

        if resp.drag_started() {
            app.timeline.selected_marker = Some(m.id);
            app.timeline.marker_drag = Some((m.id, app.project.clone(), false));
            app.pause();
        }
        if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
            if app
                .timeline
                .marker_drag
                .as_ref()
                .is_some_and(|d| d.0 == m.id)
            {
                // Snap to everything except itself.
                let mut others = app.project.clone();
                others.markers.retain(|o| o.id != m.id);
                let (t, _) = snap_time(app, view, &others, view.t(p.x), &[]);
                if app.project.move_marker(m.id, t) {
                    if let Some(d) = app.timeline.marker_drag.as_mut() {
                        d.2 |= t != m.time;
                    }
                    app.touch();
                }
            }
        }
        if resp.drag_stopped() {
            if let Some((_, before, moved)) = app.timeline.marker_drag.take() {
                if moved {
                    app.record(before);
                }
            }
        }
        if resp.clicked() {
            app.timeline.selected_marker = Some(m.id);
            app.pause();
            app.seek(m.time);
        }
        resp.context_menu(|ui| marker_menu(app, ui, m.id));
    }
}

fn marker_menu(app: &mut TidyCutsApp, ui: &mut Ui, id: MarkerId) {
    let Some(m) = app.project.marker(id).cloned() else {
        return;
    };
    ui.set_min_width(190.0);
    ui.menu_button("Rename", |ui| {
        let key = Id::new(("rename-marker", id));
        let mut name: String = ui
            .data_mut(|d| d.get_temp(key))
            .unwrap_or_else(|| m.name.clone());
        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(200.0));
        ui.data_mut(|d| d.insert_temp(key, name.clone()));
        if r.lost_focus() || ui.button("Rename").clicked() {
            let n = name.trim().to_string();
            app.edit(|p| match p.marker_mut(id) {
                Some(mk) if mk.name != n => {
                    mk.name = n;
                    true
                }
                _ => false,
            });
            ui.data_mut(|d| d.remove::<String>(key));
            ui.close();
        }
    });
    ui.menu_button("Note", |ui| {
        let key = Id::new(("note-marker", id));
        let mut note: String = ui
            .data_mut(|d| d.get_temp(key))
            .unwrap_or_else(|| m.note.clone());
        let r = ui.add(
            egui::TextEdit::multiline(&mut note)
                .desired_width(220.0)
                .desired_rows(3),
        );
        ui.data_mut(|d| d.insert_temp(key, note.clone()));
        if r.lost_focus() || ui.button("Save note").clicked() {
            app.edit(|p| match p.marker_mut(id) {
                Some(mk) if mk.note != note => {
                    mk.note = note;
                    true
                }
                _ => false,
            });
            ui.data_mut(|d| d.remove::<String>(key));
            ui.close();
        }
    });
    ui.menu_button("Color", |ui| {
        for (i, (name, col)) in MARKER_COLORS.iter().enumerate() {
            let label = egui::RichText::new(format!("●  {name}")).color(*col);
            if ui.radio(m.color as usize == i, label).clicked() {
                app.edit(|p| match p.marker_mut(id) {
                    Some(mk) => {
                        mk.color = i as u8;
                        true
                    }
                    None => false,
                });
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button("Delete marker").clicked() {
        app.delete_marker(id);
        ui.close();
    }
}

fn clips_at_playhead(app: &TidyCutsApp) -> bool {
    let t = app.timeline.playhead;
    app.project
        .tracks
        .iter()
        .any(|tr| !tr.locked && tr.clip_at(t).is_some())
}

fn playhead(
    app: &mut TidyCutsApp,
    ui: &mut Ui,
    ruler: Rect,
    lanes: Rect,
    view: View,
    rows: &[Row],
) {
    let x = view.x(app.timeline.playhead);
    if x < ruler.left() - 1.0 || x > ruler.right() + 1.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(
        egui::pos2(ruler.left() - 10.0, ruler.top()),
        lanes.max,
    ));
    // A 2-pixel line from the top of the ruler, and a 14 by 18 head with a
    // rounded point.
    painter.vline(
        x,
        ruler.top()..=lanes.bottom(),
        Stroke::new(2.0, theme::PLAYHEAD),
    );
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(x - 7.0, ruler.top() + 2.0),
            egui::vec2(14.0, 18.0),
        ),
        CornerRadius {
            nw: 2,
            ne: 2,
            sw: 9,
            se: 9,
        },
        theme::PLAYHEAD,
    );

    // The cut button on the playhead, like a pair of scissors. It shows when
    // the pointer is near the playhead.
    let near = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| (p.x - x).abs() < 18.0 && p.y > ruler.top() && p.y < lanes.bottom());
    if !near || app.is_playing() || !clips_at_playhead(app) {
        return;
    }
    let pill = Rect::from_center_size(egui::pos2(x, lanes.top() + 13.0), egui::vec2(30.0, 22.0));
    let resp = ui.interact(pill, Id::new("tl-cut"), Sense::click());
    let targets = app.targets_at_playhead();
    if resp.hovered() {
        // Show which clips the cut goes through.
        for id in &targets {
            if let Some((tr, _)) = app.project.clip(*id) {
                if let Some(row) = rows.iter().find(|r| r.id == tr.id) {
                    let seg = [
                        egui::pos2(x, row.rect.top() - 2.0),
                        egui::pos2(x, row.rect.bottom() + 2.0),
                    ];
                    painter.extend(Shape::dashed_line(
                        &seg,
                        Stroke::new(3.0, Color32::WHITE),
                        5.0,
                        3.0,
                    ));
                }
            }
        }
    }
    let fill = if resp.hovered() {
        Color32::from_rgb(0xff, 0x6b, 0x76)
    } else {
        theme::PLAYHEAD
    };
    painter.rect_filled(
        pill.translate(egui::vec2(0.0, 1.5)),
        CornerRadius::same(11),
        Color32::from_black_alpha(80),
    );
    painter.rect_filled(pill, CornerRadius::same(11), fill);
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        ph::SCISSORS,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    let cmd = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };
    if resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if targets.len() == 1 && !app.selection.is_empty() {
            format!("Split the selected clip here ({cmd}B)")
        } else if targets.len() == 1 {
            format!("Split the clip here ({cmd}B)")
        } else {
            format!("Split {} clips here ({cmd}B)", targets.len())
        })
        .clicked()
    {
        app.split_at_playhead();
    }
}

/// The top-left corner: add-track menu and the playhead time.
fn corner(app: &mut TidyCutsApp, ui: &mut Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::PANEL_BG);
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    ui.painter().vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, theme::BORDER),
    );
    // The playhead time and the add-track button show when the pointer is here.
    let hot = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| rect.contains(p));
    if hot {
        let fps = app.project.settings.frame_rate;
        ui.painter().text(
            rect.right_center() - egui::vec2(10.0, 0.0),
            Align2::RIGHT_CENTER,
            app.timeline.playhead.timecode(fps),
            FontId::monospace(11.5),
            theme::TEXT_DIM,
        );
    }
    let b = Rect::from_center_size(
        rect.left_center() + egui::vec2(20.0, 0.0),
        egui::vec2(28.0, 24.0),
    );
    let resp = ui.interact(b, Id::new("tl-add-track"), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(b, CornerRadius::same(6), theme::HOVER);
    }
    if hot {
        ui.painter().text(
            b.center(),
            Align2::CENTER_CENTER,
            ph::PLUS_SQUARE,
            FontId::proportional(17.0),
            theme::TEXT,
        );
    }
    let resp = resp.on_hover_text("Add a track");
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        let v = app.project.track_count(TrackKind::Video);
        let a = app.project.track_count(TrackKind::Audio);
        if ui
            .add_enabled(
                v < MAX_VIDEO_TRACKS,
                egui::Button::new(format!(
                    "{}  Add video track  ({v}/{MAX_VIDEO_TRACKS})",
                    ph::FILM_STRIP
                )),
            )
            .clicked()
        {
            app.add_track(TrackKind::Video, None);
            ui.close();
        }
        if ui
            .add_enabled(
                a < MAX_AUDIO_TRACKS,
                egui::Button::new(format!(
                    "{}  Add audio track  ({a}/{MAX_AUDIO_TRACKS})",
                    ph::MUSIC_NOTES
                )),
            )
            .clicked()
        {
            app.add_track(TrackKind::Audio, None);
            ui.close();
        }
    });
}

/// The free lanes above and below the tracks: quiet until something is
/// being dragged, then outlined with a hint, lit while the pointer is over
/// the part of the lanes that would make a new track.
fn free_lane_hints(
    app: &TidyCutsApp,
    ui: &Ui,
    lanes: Rect,
    rows: &[Row],
    free: [(Option<Rect>, TrackKind); 2],
) {
    let moving_clip = matches!(
        app.timeline.drag.as_ref().map(|d| &d.op),
        Some(DragOp::Move { .. })
    );
    let dragging = moving_clip || egui::DragAndDrop::has_payload_of_type::<LibraryDrag>(ui.ctx());
    if !dragging {
        return;
    }
    let pointer = ui.ctx().pointer_hover_pos();
    let painter = ui.painter().with_clip_rect(lanes);
    for (rect, kind) in free {
        let Some(rect) = rect else { continue };
        // Anything past the last track on that side counts as the free lane.
        let target = match kind {
            TrackKind::Video => Rect::from_min_max(
                lanes.min,
                egui::pos2(
                    lanes.right(),
                    rows.first().map_or(rect.bottom(), |r| r.rect.top()),
                ),
            ),
            TrackKind::Audio => Rect::from_min_max(
                egui::pos2(
                    lanes.left(),
                    rows.last().map_or(rect.top(), |r| r.rect.bottom()),
                ),
                lanes.max,
            ),
        };
        let lit = pointer.is_some_and(|p| target.contains(p));
        let inner = rect.shrink2(egui::vec2(6.0, 4.0));
        if lit {
            painter.rect_filled(
                inner,
                CornerRadius::same(5),
                theme::ACCENT.gamma_multiply(0.14),
            );
        }
        let color = if lit {
            theme::ACCENT
        } else {
            theme::TEXT_FAINT
        };
        let pts = [
            inner.left_top(),
            inner.right_top(),
            inner.right_bottom(),
            inner.left_bottom(),
            inner.left_top(),
        ];
        painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, color), 6.0, 4.0));
        painter.text(
            inner.center(),
            Align2::CENTER_CENTER,
            match kind {
                TrackKind::Video => format!("{}  Drop here for a new track", ph::PLUS),
                TrackKind::Audio => format!("{}  Drop here for a new audio track", ph::PLUS),
            },
            FontId::proportional(12.0),
            color,
        );
    }
}

// ----- tracks ----------------------------------------------------------

fn track_headers(
    app: &mut TidyCutsApp,
    ui: &mut Ui,
    rect: Rect,
    rows: &[Row],
    free: [(Option<Rect>, TrackKind); 2],
) {
    let painter = ui.painter().with_clip_rect(rect);
    // The free lanes get a small button that adds an empty track there.
    for (lane, kind) in free {
        let Some(lane) = lane else { continue };
        let r = Rect::from_min_max(
            egui::pos2(rect.left(), lane.top()),
            egui::pos2(rect.right(), lane.bottom()),
        );
        if r.bottom() < rect.top() || r.top() > rect.bottom() {
            continue;
        }
        painter.rect_filled(r, CornerRadius::ZERO, theme::PANEL_BG);
        painter.vline(
            r.right() - 0.5,
            r.y_range(),
            Stroke::new(1.0, theme::BORDER),
        );
        let b = Rect::from_center_size(
            egui::pos2(r.left() + 24.0, r.center().y),
            egui::vec2(28.0, 24.0),
        );
        let resp = ui.interact(b, Id::new(("tl-free-add", kind as u8)), Sense::click());
        if resp.hovered() {
            painter.rect_filled(b, CornerRadius::same(6), theme::HOVER);
        }
        painter.text(
            b.center(),
            Align2::CENTER_CENTER,
            ph::PLUS,
            FontId::proportional(15.0),
            if resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_FAINT
            },
        );
        let tip = match kind {
            TrackKind::Video => "Add a video track",
            TrackKind::Audio => "Add an audio track",
        };
        if resp.on_hover_text(tip).clicked() {
            app.add_track(kind, None);
        }
    }
    let mut video_n = app.project.track_count(TrackKind::Video);
    let mut audio_n = 0;
    for row in rows {
        let r = Rect::from_min_max(
            egui::pos2(rect.left(), row.rect.top()),
            egui::pos2(rect.right(), row.rect.bottom()),
        );
        // Numbering as is usual: the bottom video track is 1.
        let number = match row.kind {
            TrackKind::Video => {
                let n = video_n;
                video_n -= 1;
                n
            }
            TrackKind::Audio => {
                audio_n += 1;
                audio_n
            }
        };
        if r.bottom() < rect.top() || r.top() > rect.bottom() {
            continue;
        }
        painter.rect_filled(r, CornerRadius::ZERO, theme::PANEL_BG);
        painter.hline(
            r.x_range(),
            r.top() + 0.5,
            Stroke::new(1.0, theme::LANE_LINE),
        );
        painter.vline(
            r.right() - 0.5,
            r.y_range(),
            Stroke::new(1.0, theme::BORDER),
        );
        let Some(track) = app.project.track(row.id) else {
            continue;
        };
        let (locked, muted, hidden) = (track.locked, track.muted, track.hidden);
        let cy = r.center().y + 0.5;
        // The number (bold, 16 wide), then what kind of track it is.
        painter.text(
            egui::pos2(r.left() + 10.0, cy),
            Align2::LEFT_CENTER,
            number.to_string(),
            theme::bold_font(12.0),
            theme::TEXT,
        );
        painter.text(
            egui::pos2(r.left() + 10.0 + 16.0 + 8.0 + 7.5, cy),
            Align2::CENTER_CENTER,
            match row.kind {
                TrackKind::Video => ph::VIDEO,
                TrackKind::Audio => ph::MUSIC_NOTES,
            },
            FontId::proportional(15.0),
            theme::TEXT_DIM,
        );
        // On the right, 10 in and 8 apart: eye (video only), lock, sound.
        let mut toggles: Vec<(&str, String, bool, u8)> = Vec::new();
        if row.kind == TrackKind::Video {
            toggles.push((
                if hidden { ph::EYE_SLASH } else { ph::EYE },
                if hidden {
                    "Show track".into()
                } else {
                    "Hide track".into()
                },
                hidden,
                2,
            ));
        }
        toggles.push((
            if locked {
                ph::LOCK_SIMPLE
            } else {
                ph::LOCK_SIMPLE_OPEN
            },
            if locked {
                "Unlock track".into()
            } else {
                "Lock track".into()
            },
            locked,
            0,
        ));
        toggles.push((
            if muted {
                ph::SPEAKER_SLASH
            } else {
                ph::SPEAKER_HIGH
            },
            if muted {
                "Unmute track".into()
            } else {
                "Mute track".into()
            },
            muted,
            1,
        ));
        let n = toggles.len();
        for (i, (icon, tip, on, which)) in toggles.into_iter().enumerate() {
            let centre = egui::pos2(r.right() - 10.0 - 7.5 - (n - 1 - i) as f32 * 23.0, cy);
            let b = Rect::from_center_size(centre, egui::vec2(22.0, 22.0));
            let resp = ui.interact(b, Id::new(("track-toggle", row.id, which)), Sense::click());
            let color = if on {
                if which == 0 {
                    theme::WARNING
                } else {
                    theme::DANGER
                }
            } else if resp.hovered() {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            painter.text(
                centre,
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(15.0),
                color,
            );
            if resp.on_hover_text(tip).clicked() {
                let id = row.id;
                app.edit(|p| match p.track_mut(id) {
                    Some(t) => {
                        match which {
                            0 => t.locked = !t.locked,
                            1 => t.muted = !t.muted,
                            _ => t.hidden = !t.hidden,
                        }
                        true
                    }
                    None => false,
                });
            }
        }
        // Right-click anywhere on the header for the track's menu.
        let header_resp = ui.interact(
            Rect::from_min_max(r.min, egui::pos2(r.right() - 80.0, r.bottom())),
            Id::new(("track-head", row.id)),
            Sense::click(),
        );
        let row = *row;
        header_resp.context_menu(|ui| track_menu(app, ui, row));
        resize_grab(app, ui, rect, row);
    }
    // The line under the last header.
    if let Some(last) = rows.last() {
        painter.hline(
            rect.x_range(),
            last.rect.bottom() + 0.5,
            Stroke::new(1.0, theme::LANE_LINE),
        );
    }
}

/// The line under a track header can be dragged to make the track taller
/// or shorter (double-click puts it back).
fn resize_grab(app: &mut TidyCutsApp, ui: &mut Ui, headers: Rect, row: Row) {
    let line = row.rect.bottom();
    let grab = Rect::from_min_max(
        egui::pos2(headers.left(), line - RESIZE_GRAB),
        egui::pos2(headers.right(), line + RESIZE_GRAB),
    );
    if grab.bottom() < headers.top() || grab.top() > headers.bottom() {
        return;
    }
    let resp = ui.interact(
        grab,
        Id::new(("track-resize", row.id)),
        Sense::click_and_drag(),
    );
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        ui.painter().with_clip_rect(headers).hline(
            headers.x_range(),
            line,
            Stroke::new(2.0, theme::ACCENT),
        );
    }
    if resp.double_clicked() {
        app.timeline.track_heights.remove(&row.id);
    } else if resp.dragged() {
        let now = track_height(app, row.id, row.kind, row.main);
        let h = (now + resp.drag_delta().y).clamp(MIN_TRACK_H, MAX_TRACK_H);
        app.timeline.track_heights.insert(row.id, h);
    }
}

fn track_menu(app: &mut TidyCutsApp, ui: &mut Ui, row: Row) {
    ui.set_min_width(220.0);
    let index = app
        .project
        .tracks
        .iter()
        .position(|t| t.id == row.id)
        .unwrap_or(0);
    let can_v = app.project.can_add_track(TrackKind::Video);
    let can_a = app.project.can_add_track(TrackKind::Audio);
    match row.kind {
        TrackKind::Video => {
            if ui
                .add_enabled(can_v, egui::Button::new("Add video track above"))
                .clicked()
            {
                app.add_track(TrackKind::Video, Some(index));
                ui.close();
            }
            if ui
                .add_enabled(can_v, egui::Button::new("Add video track below"))
                .clicked()
            {
                app.add_track(TrackKind::Video, Some(index + 1));
                ui.close();
            }
        }
        TrackKind::Audio => {
            if ui
                .add_enabled(can_a, egui::Button::new("Add audio track above"))
                .clicked()
            {
                app.add_track(TrackKind::Audio, Some(index));
                ui.close();
            }
            if ui
                .add_enabled(can_a, egui::Button::new("Add audio track below"))
                .clicked()
            {
                app.add_track(TrackKind::Audio, Some(index + 1));
                ui.close();
            }
        }
    }
    ui.separator();
    let has_clips = app
        .project
        .track(row.id)
        .is_some_and(|t| !t.clips.is_empty());
    if ui
        .add_enabled(
            has_clips,
            egui::Button::new("Select all clips on this track"),
        )
        .clicked()
    {
        if let Some(t) = app.project.track(row.id) {
            app.selection = t.clips.iter().map(|c| c.id).collect();
        }
        ui.close();
    }
    ui.separator();
    let can_delete = !(row.main && app.project.track_count(TrackKind::Video) <= 1);
    if ui
        .add_enabled(
            can_delete,
            egui::Button::new(egui::RichText::new("Delete track").color(theme::DANGER)),
        )
        .clicked()
    {
        app.remove_track(row.id);
        ui.close();
    }
}

fn row_at(rows: &[Row], y: f32) -> Option<Row> {
    rows.iter()
        .find(|r| y >= r.rect.top() - ROW_GAP / 2.0 && y < r.rect.bottom() + ROW_GAP / 2.0)
        .copied()
}

// ----- snapping --------------------------------------------------------

/// Candidate snap points: zero, the playhead, markers and every clip edge.
fn snap_points(project: &Project, playhead: Time, ignore: &[ClipId]) -> Vec<Time> {
    let mut pts = vec![Time::ZERO, playhead];
    pts.extend(project.markers.iter().map(|m| m.time));
    for t in &project.tracks {
        for c in t.clips.iter().filter(|c| !ignore.contains(&c.id)) {
            pts.push(c.start);
            pts.push(c.end());
        }
    }
    pts
}

/// Snaps a time to the nearest snap point within reach. Returns the time
/// and the point it snapped to.
fn snap_time(
    app: &TidyCutsApp,
    view: View,
    project: &Project,
    t: Time,
    ignore: &[ClipId],
) -> (Time, Option<Time>) {
    let t = app.project.settings.frame_rate.snap(t.max(Time::ZERO));
    if !app.timeline.snap {
        return (t, None);
    }
    let tol = Time::from_secs_f64((SNAP_PX / view.pps) as f64);
    snap_points(project, app.timeline.playhead, ignore)
        .into_iter()
        .filter(|p| (*p - t).abs() <= tol)
        .min_by_key(|p| (*p - t).abs())
        .map(|p| (p, Some(p)))
        .unwrap_or((t, None))
}

/// Snaps a clip's start so either of its edges lands on a snap point.
fn snap_span(
    app: &TidyCutsApp,
    view: View,
    project: &Project,
    start: Time,
    len: Time,
    ignore: &[ClipId],
) -> (Time, Option<Time>) {
    let start = app.project.settings.frame_rate.snap(start.max(Time::ZERO));
    if !app.timeline.snap {
        return (start, None);
    }
    let tol = Time::from_secs_f64((SNAP_PX / view.pps) as f64);
    let mut best: Option<(Time, Time, Time)> = None;
    for p in snap_points(project, app.timeline.playhead, ignore) {
        for (candidate, line) in [(p, p), (p - len, p)] {
            let d = (candidate - start).abs();
            if d <= tol && candidate >= Time::ZERO && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, candidate, line));
            }
        }
    }
    best.map(|(_, c, l)| (c, Some(l))).unwrap_or((start, None))
}

// ----- clips -----------------------------------------------------------

fn clip_rect(view: View, row: &Row, c: &Clip) -> Rect {
    let x0 = view.x(c.start);
    let x1 = view.x(c.end()).max(x0 + 3.0);
    // The lane's line, then a margin above and below the clip.
    Rect::from_min_max(
        egui::pos2(x0, row.rect.top() + 1.0 + CLIP_INSET),
        egui::pos2(x1, row.rect.bottom() - CLIP_INSET),
    )
}

/// Whether a clip makes sound the timeline should draw (audio clips, and
/// video clips that still carry their own sound).
fn shows_audio(app: &TidyCutsApp, kind: TrackKind, c: &Clip) -> bool {
    let Some(m) = c.media_id.and_then(|m| app.project.media(m)) else {
        return false;
    };
    match kind {
        TrackKind::Audio => true,
        TrackKind::Video => m.kind == MediaKind::Video && m.has_audio && !c.audio.muted,
    }
}

/// Screen x of a clip's fade-in and fade-out handles (kept just inside the
/// clip's corners when there is no fade, so they can still be grabbed).
fn fade_dots(view: View, r: Rect, c: &Clip) -> (f32, f32) {
    let fi = view.x(c.start + c.audio.fade_in).max(r.left() + 6.0);
    let fo = view.x(c.end() - c.audio.fade_out).min(r.right() - 6.0);
    (fi, fo)
}

/// Height of the waveform strip at the bottom of a video clip.
fn video_wave_h(r: Rect) -> f32 {
    (r.height() * 0.3).round()
}

/// Y of an audio clip's volume line (0..200%).
fn volume_y(r: Rect, volume: f64) -> f32 {
    let body = Rect::from_min_max(egui::pos2(r.left(), r.top() + 18.0), r.max);
    body.bottom() - (volume / 200.0).clamp(0.0, 1.0) as f32 * body.height()
}

fn hit_test(app: &TidyCutsApp, view: View, rows: &[Row], p: Pos2) -> Hit {
    let px = view.x(app.timeline.playhead);
    let Some(row) = row_at(rows, p.y) else {
        return Hit::Empty {
            track: None,
            t: view.t(p.x),
        };
    };
    let Some(track) = app.project.track(row.id) else {
        return Hit::Empty {
            track: None,
            t: view.t(p.x),
        };
    };
    for c in &track.clips {
        for out in [false, true] {
            if transition_rect(view, &row, c, out).is_some_and(|r| r.contains(p)) {
                return Hit::Transition { clip: c.id, out };
            }
        }
    }
    for c in &track.clips {
        let r = clip_rect(view, &row, c);
        if !r.expand2(egui::vec2(2.0, 0.0)).contains(p) {
            continue;
        }
        let audio = shows_audio(app, row.kind, c);
        if row.kind == TrackKind::Audio {
            let vy = volume_y(r, c.volume.value);
            if !c.volume.is_animated() && (p.y - vy).abs() < 4.0 && r.width() > 30.0 {
                return Hit::Clip {
                    clip: c.id,
                    track: row.id,
                    zone: Zone::Volume,
                };
            }
        }
        if audio && p.y < r.top() + 18.0 && r.width() > 40.0 {
            let (fi, fo) = fade_dots(view, r, c);
            if (p.x - fi).abs() < 6.0 {
                return Hit::Clip {
                    clip: c.id,
                    track: row.id,
                    zone: Zone::FadeIn,
                };
            }
            if (p.x - fo).abs() < 6.0 {
                return Hit::Clip {
                    clip: c.id,
                    track: row.id,
                    zone: Zone::FadeOut,
                };
            }
        }
        let zone = if p.x - r.left() < EDGE_PX && r.width() > EDGE_PX * 3.0 {
            Zone::TrimStart
        } else if r.right() - p.x < EDGE_PX && r.width() > EDGE_PX * 3.0 {
            Zone::TrimEnd
        } else {
            Zone::Body
        };
        // The playhead line is grabbable over clips too, but clips' edges win.
        if zone == Zone::Body && (p.x - px).abs() < 3.0 {
            return Hit::Playhead;
        }
        return Hit::Clip {
            clip: c.id,
            track: row.id,
            zone,
        };
    }
    if (p.x - px).abs() < 4.0 {
        return Hit::Playhead;
    }
    Hit::Empty {
        track: Some(row.id),
        t: view.t(p.x),
    }
}

fn draw_clips(app: &mut TidyCutsApp, ui: &mut Ui, lanes: Rect, view: View, rows: &[Row]) {
    let painter = ui.painter().with_clip_rect(lanes);
    for row in rows {
        if row.rect.bottom() < lanes.top() || row.rect.top() > lanes.bottom() {
            continue;
        }
        let Some(track) = app.project.track(row.id) else {
            continue;
        };
        let dim = track.hidden || track.muted;
        let locked = track.locked;
        let clips: Vec<Clip> = track.clips.clone();
        for c in &clips {
            let r = clip_rect(view, row, c);
            if r.right() < lanes.left() || r.left() > lanes.right() {
                continue;
            }
            draw_clip(
                app,
                &painter,
                row,
                c,
                r,
                view,
                if dim || c.disabled { 0.45 } else { 1.0 },
            );
        }
        for c in &clips {
            for (out, tr) in [(false, &c.transition_in), (true, &c.transition_out)] {
                if let (Some(tr), Some(r)) = (tr, transition_rect(view, row, c, out)) {
                    draw_transition_badge(&painter, r, &tr.kind);
                }
            }
        }
        if locked {
            // Diagonal hatching over locked tracks.
            let mut x = row.rect.left() - row.rect.height();
            let mut shapes = Vec::new();
            while x < row.rect.right() {
                shapes.push(Shape::line_segment(
                    [
                        egui::pos2(x, row.rect.bottom()),
                        egui::pos2(x + row.rect.height(), row.rect.top()),
                    ],
                    Stroke::new(1.0, Color32::from_white_alpha(14)),
                ));
                x += 12.0;
            }
            painter
                .with_clip_rect(row.rect.intersect(lanes))
                .extend(shapes);
        }
    }
    // Snap guide while dragging.
    if let Some(t) = app.timeline.drag.as_ref().and_then(|d| d.snapped) {
        let x = view.x(t);
        painter.vline(x, lanes.y_range(), Stroke::new(1.0, theme::ACCENT));
    }
}

/// A clip's fill and the brighter edge down its left side.
fn clip_colors(app: &TidyCutsApp, kind: TrackKind, c: &Clip) -> (Color32, Color32) {
    let rgb = |r, g, b| Color32::from_rgb(r, g, b);
    match (
        &c.generator,
        c.media_id
            .and_then(|m| app.project.media(m))
            .map(|m| m.kind),
    ) {
        (Some(Generator::Title(_)), _) => (theme::TITLE_CLIP, rgb(0xf0, 0xb0, 0x4a)),
        (Some(Generator::Sticker(_)), _) | (Some(Generator::Visualizer(_)), _) => {
            (theme::STICKER_CLIP, rgb(0xe0, 0x6f, 0xb8))
        }
        (Some(Generator::Adjustment), _) => (theme::ADJUST_CLIP, rgb(0x9a, 0x7c, 0xf0)),
        (Some(_), _) => (theme::SOLID_CLIP, rgb(0x9a, 0xa3, 0xb0)),
        (None, Some(MediaKind::Image)) => (theme::IMAGE_CLIP, rgb(0x6f, 0xb4, 0xe6)),
        _ => match kind {
            TrackKind::Video => (theme::VIDEO_CLIP, rgb(0x6f, 0x93, 0xe6)),
            TrackKind::Audio => (theme::AUDIO_CLIP, theme::GOOD),
        },
    }
}

pub fn clip_name(app: &TidyCutsApp, c: &Clip) -> String {
    if let Some(n) = &c.name {
        return n.clone();
    }
    match &c.generator {
        Some(Generator::Adjustment) => {
            let names: Vec<&str> = c
                .effects
                .iter()
                .filter_map(|e| {
                    tidycuts_render::catalog::effects::get(&e.kind)
                        .map(|d| d.name)
                        .or_else(|| tidycuts_render::catalog::filters::get(&e.kind).map(|d| d.name))
                })
                .collect();
            if names.is_empty() {
                "Adjustment".into()
            } else {
                names.join(", ")
            }
        }
        Some(Generator::Sticker(s)) => tidycuts_render::catalog::stickers::get(&s.kind)
            .map(|d| d.name.to_string())
            .unwrap_or_else(|| s.kind.clone()),
        Some(g) => g.label(),
        None => c
            .media_id
            .and_then(|m| app.project.media(m))
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "Missing media".into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_clip(
    app: &mut TidyCutsApp,
    painter: &egui::Painter,
    row: &Row,
    c: &Clip,
    r: Rect,
    view: View,
    alpha: f32,
) {
    let (body_col, edge_col) = clip_colors(app, row.kind, c);
    let a = |col: Color32| col.gamma_multiply(alpha);
    let radius = CornerRadius::same(4);
    painter.rect_filled(r, radius, a(body_col));
    // The name sits on the picture, in the top 18 pixels.
    let head = Rect::from_min_size(r.min, egui::vec2(r.width(), 18.0));
    let body = r;
    let visible = body.intersect(painter.clip_rect());
    let media = c.media_id.and_then(|m| app.project.media(m)).cloned();
    let audio = shows_audio(app, row.kind, c);

    match (row.kind, &media, &c.generator) {
        (TrackKind::Video, Some(m), None) => {
            let strip = if audio {
                Rect::from_min_max(
                    body.min,
                    egui::pos2(body.right(), body.bottom() - video_wave_h(r)),
                )
            } else {
                body
            };
            filmstrip(
                app,
                painter,
                c,
                m,
                strip,
                strip.intersect(visible),
                view,
                alpha,
            );
            if audio {
                let wave = Rect::from_min_max(egui::pos2(body.left(), strip.bottom()), body.max);
                painter.with_clip_rect(wave.intersect(visible)).rect_filled(
                    wave,
                    CornerRadius::ZERO,
                    a(Color32::from_rgb(0x12, 0x3d, 0x35)),
                );
                waveform(
                    app,
                    painter,
                    c,
                    m,
                    wave,
                    wave.intersect(visible),
                    view,
                    alpha,
                    theme::AUDIO_WAVE,
                );
            }
        }
        (TrackKind::Audio, Some(m), _) => {
            // The wave fills the clip below the name (20 pixels down, 2 up).
            let wave = Rect::from_min_max(
                egui::pos2(body.left(), body.top() + 20.0),
                egui::pos2(body.right(), body.bottom() - 2.0),
            );
            waveform(
                app,
                painter,
                c,
                m,
                wave,
                wave.intersect(visible),
                view,
                alpha,
                theme::AUDIO_WAVE,
            );
        }
        (_, _, Some(Generator::Solid(col))) => {
            let fill = Color32::from_rgba_unmultiplied(
                (col[0] * 255.0) as u8,
                (col[1] * 255.0) as u8,
                (col[2] * 255.0) as u8,
                255,
            );
            painter.with_clip_rect(visible).rect_filled(
                body.shrink(3.0),
                CornerRadius::same(3),
                a(fill),
            );
        }
        (_, _, Some(Generator::Gradient { a: ca, b: cb, .. })) => {
            let c1 = Color32::from_rgb(
                (ca[0] * 255.0) as u8,
                (ca[1] * 255.0) as u8,
                (ca[2] * 255.0) as u8,
            );
            let c2 = Color32::from_rgb(
                (cb[0] * 255.0) as u8,
                (cb[1] * 255.0) as u8,
                (cb[2] * 255.0) as u8,
            );
            let inner = body.shrink(3.0);
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(inner.left_top(), a(c1));
            mesh.colored_vertex(inner.right_top(), a(c2));
            mesh.colored_vertex(inner.right_bottom(), a(c2));
            mesh.colored_vertex(inner.left_bottom(), a(c1));
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.with_clip_rect(visible).add(Shape::mesh(mesh));
        }
        // Titles, stickers and effects show only their name, with an icon.
        _ => {}
    }

    // A bright edge on the left tells the kinds of clip apart at a glance.
    painter
        .with_clip_rect(r.intersect(painter.clip_rect()))
        .rect_filled(
            Rect::from_min_size(r.min, egui::vec2(3.0, r.height())),
            CornerRadius {
                nw: 4,
                ne: 0,
                sw: 4,
                se: 0,
            },
            a(edge_col),
        );

    // Header: color mark, name and badges.
    let label_clip = Rect::from_min_max(head.min, egui::pos2(head.right() - 4.0, head.bottom()))
        .intersect(painter.clip_rect());
    let p = painter.with_clip_rect(label_clip);
    let mut tx = head.left().max(painter.clip_rect().left()) + 8.0;
    if let Some(m) = c.mark {
        let col = theme::MARKS[m as usize % theme::MARKS.len()];
        p.rect_filled(
            Rect::from_min_size(
                egui::pos2(tx - 2.0, head.top() + 4.0),
                egui::vec2(4.0, 10.0),
            ),
            CornerRadius::same(1),
            col,
        );
        tx += 7.0;
    }
    let mut badges = String::new();
    if c.link.is_some() {
        badges.push_str(ph::LINK_SIMPLE);
        badges.push(' ');
    }
    if !c.effects.is_empty() && !c.is_adjustment() {
        badges.push_str(ph::SPARKLE);
        badges.push(' ');
    }
    if c.ramp.is_some() {
        badges.push_str(ph::CHART_LINE);
        badges.push(' ');
    } else if (c.speed - 1.0).abs() > 1e-6 {
        badges.push_str(&format!("{:.2}× ", c.speed).replace(".00", ""));
    }
    if c.reverse {
        badges.push_str(ph::ARROW_U_UP_LEFT);
        badges.push(' ');
    }
    if c.audio.muted
        && media
            .as_ref()
            .is_some_and(|m| m.has_audio || m.kind == MediaKind::Audio)
    {
        badges.push_str(ph::SPEAKER_SLASH);
        badges.push(' ');
    }
    if c.disabled {
        badges.push_str(ph::EYE_SLASH);
        badges.push(' ');
    }
    if !badges.is_empty() {
        let g = p.layout_no_wrap(badges, FontId::proportional(11.0), a(Color32::WHITE));
        let w = g.size().x;
        p.galley(
            egui::pos2(tx, head.center().y - g.size().y / 2.0),
            g,
            Color32::WHITE,
        );
        tx += w + 2.0;
    }
    // An icon for clips that are not a picture or video file.
    let icon = match (&c.generator, row.kind) {
        (Some(Generator::Title(_)), _) => Some(ph::TEXT_T),
        (Some(Generator::Sticker(_)), _) => Some(ph::STICKER),
        (Some(Generator::Adjustment), _) => Some(ph::SPARKLE),
        (None, TrackKind::Audio) => Some(ph::SPEAKER_HIGH),
        _ => None,
    };
    let name = clip_name(app, c);
    let name_cy = r.top() + 12.5;
    let mut nx = tx;
    if let Some(icon) = icon {
        p.text(
            egui::pos2(nx + 1.0, name_cy + 1.0),
            Align2::LEFT_CENTER,
            icon,
            FontId::proportional(11.0),
            Color32::from_black_alpha(0x90),
        );
        p.text(
            egui::pos2(nx, name_cy),
            Align2::LEFT_CENTER,
            icon,
            FontId::proportional(11.0),
            a(Color32::WHITE),
        );
        nx += 16.0;
    }
    // White, with a soft shadow so it reads on any picture.
    p.text(
        egui::pos2(nx, name_cy + 1.0),
        Align2::LEFT_CENTER,
        &name,
        theme::heading_font(11.5),
        Color32::from_black_alpha(0x90),
    );
    p.text(
        egui::pos2(nx, name_cy),
        Align2::LEFT_CENTER,
        &name,
        theme::heading_font(11.5),
        a(Color32::WHITE),
    );

    // Keyframe diamonds along the bottom.
    let mut times: Vec<Time> = Vec::new();
    let mut cc = c.clone();
    cc.for_each_animatable(|an| times.extend(an.keyframes.iter().map(|k| k.at)));
    times.sort();
    times.dedup();
    for t in times {
        let x = view.x(c.start + t);
        let y = r.bottom() - 6.0;
        if x >= r.left() && x <= r.right() {
            painter.add(Shape::convex_polygon(
                vec![
                    egui::pos2(x, y - 4.0),
                    egui::pos2(x + 4.0, y),
                    egui::pos2(x, y + 4.0),
                    egui::pos2(x - 4.0, y),
                ],
                theme::KEYFRAME,
                Stroke::new(1.0, Color32::BLACK),
            ));
        }
    }

    // Fades and the volume line for clips with sound.
    if audio && r.width() > 40.0 {
        let fi = view.x(c.start + c.audio.fade_in);
        let fo = view.x(c.end() - c.audio.fade_out);
        let line = Stroke::new(1.0, a(Color32::from_white_alpha(170)));
        if c.audio.fade_in > Time::ZERO {
            painter.line_segment(
                [
                    egui::pos2(r.left(), r.bottom()),
                    egui::pos2(fi, head.bottom()),
                ],
                line,
            );
        }
        if c.audio.fade_out > Time::ZERO {
            painter.line_segment(
                [
                    egui::pos2(fo, head.bottom()),
                    egui::pos2(r.right(), r.bottom()),
                ],
                line,
            );
        }
        let hovered = painter
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| r.contains(p));
        if hovered || c.audio.fade_in > Time::ZERO {
            painter.circle_filled(
                egui::pos2(fade_dots(view, r, c).0, head.center().y),
                4.0,
                Color32::WHITE,
            );
        }
        if hovered || c.audio.fade_out > Time::ZERO {
            painter.circle_filled(
                egui::pos2(fade_dots(view, r, c).1, head.center().y),
                4.0,
                Color32::WHITE,
            );
        }
    }
    if row.kind == TrackKind::Audio && r.width() > 30.0 {
        let vy = volume_y(r, c.volume.value);
        painter.hline(
            r.left() + 2.0..=r.right() - 2.0,
            vy,
            Stroke::new(1.0, a(Color32::from_rgb(0xff, 0xd0, 0x5a))),
        );
    }

    if c.disabled {
        painter.rect_filled(r, radius, Color32::from_black_alpha(110));
    }
    if app.is_selected(c.id) {
        painter.rect_stroke(
            r,
            radius,
            Stroke::new(2.0, Color32::WHITE),
            StrokeKind::Inside,
        );
    }
    // Trim handles when hovered.
    if let Some(pp) = painter.ctx().pointer_hover_pos() {
        if r.contains(pp) && r.width() > EDGE_PX * 3.0 {
            for (x, near) in [
                (r.left(), pp.x - r.left() < EDGE_PX),
                (r.right(), r.right() - pp.x < EDGE_PX),
            ] {
                if near {
                    let h = Rect::from_center_size(
                        egui::pos2(x, r.center().y),
                        egui::vec2(5.0, r.height() - 8.0),
                    );
                    painter.rect_filled(h, CornerRadius::same(2), theme::WARNING);
                }
            }
        }
    }
}

/// Where a clip's start (or end) transition badge is drawn: centred on the
/// cut for a start transition, inside the clip's end for an end one.
fn transition_rect(view: View, row: &Row, c: &Clip, out: bool) -> Option<Rect> {
    let (a, b) = if out {
        let d = c.transition_out.as_ref()?.duration;
        (view.x(c.end() - d), view.x(c.end()))
    } else {
        let half = c.transition_in.as_ref()?.duration.scale(0.5);
        (view.x(c.start - half), view.x(c.start + half))
    };
    let (a, b) = if b - a < 14.0 {
        let m = (a + b) * 0.5;
        (m - 7.0, m + 7.0)
    } else {
        (a, b)
    };
    Some(Rect::from_min_max(
        egui::pos2(a, row.rect.top() + 1.0 + CLIP_INSET + 18.0),
        egui::pos2(b, row.rect.top() + 1.0 + CLIP_INSET + 34.0),
    ))
}

fn transition_slot(c: &mut Clip, out: bool) -> &mut Option<tidycuts_core::Transition> {
    if out {
        &mut c.transition_out
    } else {
        &mut c.transition_in
    }
}

fn draw_transition_badge(painter: &egui::Painter, r: Rect, kind: &str) {
    painter.rect_filled(
        r,
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(0x10, 0x12, 0x16, 220),
    );
    painter.rect_stroke(
        r,
        CornerRadius::same(4),
        Stroke::new(1.0, theme::ACCENT),
        StrokeKind::Inside,
    );
    let name = tidycuts_render::catalog::transitions::get(kind)
        .map(|d| d.name)
        .unwrap_or("Transition");
    painter
        .with_clip_rect(r.intersect(painter.clip_rect()))
        .text(
            r.center(),
            Align2::CENTER_CENTER,
            if r.width() > 70.0 {
                format!("{} {name}", ph::INTERSECT)
            } else {
                ph::INTERSECT.to_string()
            },
            FontId::proportional(10.5),
            theme::ACCENT,
        );
}

#[allow(clippy::too_many_arguments)]
fn filmstrip(
    app: &mut TidyCutsApp,
    painter: &egui::Painter,
    clip: &Clip,
    media: &tidycuts_core::MediaItem,
    body: Rect,
    visible: Rect,
    view: View,
    alpha: f32,
) {
    if visible.width() <= 0.0 || body.height() < 8.0 {
        return;
    }
    let aspect = if media.width > 0 && media.height > 0 {
        media.width as f32 / media.height as f32
    } else {
        16.0 / 9.0
    };
    let tile_w = (body.height() * aspect).max(8.0);
    let p = painter.with_clip_rect(visible);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    if media.kind == MediaKind::Image {
        let Some(tex) = app.thumbs.poster(media, None) else {
            return;
        };
        let first = ((visible.left() - body.left()) / tile_w).floor().max(0.0) as i32;
        let last = ((visible.right() - body.left()) / tile_w).ceil() as i32;
        for i in first..last {
            let tile = Rect::from_min_size(
                egui::pos2(body.left() + i as f32 * tile_w, body.top()),
                egui::vec2(tile_w, body.height()),
            );
            p.image(tex.id(), tile, uv, Color32::WHITE.gamma_multiply(alpha));
        }
        return;
    }
    let Some(cache) = app.engine.as_ref().and_then(|e| e.scrub.get(media)) else {
        return;
    };
    let first = ((visible.left() - body.left()) / tile_w).floor().max(0.0) as i32;
    let last = ((visible.right() - body.left()) / tile_w).ceil() as i32;
    for i in first..last {
        let x = body.left() + i as f32 * tile_w;
        let t = clip
            .source_time(view.t(x + tile_w * 0.5))
            .as_secs_f64()
            .clamp(0.0, media.duration.as_secs_f64());
        if let Some(tex) = app.thumbs.strip_frame(media, &cache, t) {
            let tile =
                Rect::from_min_size(egui::pos2(x, body.top()), egui::vec2(tile_w, body.height()));
            p.image(tex.id(), tile, uv, Color32::WHITE.gamma_multiply(alpha));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn waveform(
    app: &mut TidyCutsApp,
    painter: &egui::Painter,
    clip: &Clip,
    media: &tidycuts_core::MediaItem,
    body: Rect,
    visible: Rect,
    view: View,
    alpha: f32,
    color: Color32,
) {
    let Some(peaks) = app.thumbs.waveform(media, clip.audio.stream) else {
        return;
    };
    if peaks.is_empty() || visible.width() <= 0.0 {
        return;
    }
    let gain = (clip.volume.value / 100.0).clamp(0.0, 2.0) as f32;
    let color = color.gamma_multiply(alpha * 0.95);
    crate::wave::draw(
        painter,
        &peaks,
        PEAKS_PER_SEC as f64,
        Rect::from_min_max(
            egui::pos2(visible.left(), body.top()),
            egui::pos2(visible.right(), body.bottom()),
        ),
        |x| clip.source_time(view.t(x)).as_secs_f64(),
        gain,
        color,
    );
}

// ----- pointer handling ------------------------------------------------

fn interact_lanes(
    app: &mut TidyCutsApp,
    ui: &mut Ui,
    lanes: Rect,
    view: View,
    rows: &[Row],
    resp: &egui::Response,
) {
    let pointer = ui.ctx().pointer_interact_pos();
    let mods = ui.ctx().input(|i| i.modifiers);

    // Cursor feedback.
    if app.timeline.drag.is_none() {
        if let Some(p) = ui.ctx().pointer_hover_pos().filter(|p| lanes.contains(*p)) {
            let cursor = match hit_test(app, view, rows, p) {
                Hit::Clip {
                    zone: Zone::TrimStart | Zone::TrimEnd,
                    ..
                } => CursorIcon::ResizeHorizontal,
                Hit::Clip {
                    zone: Zone::Volume, ..
                } => CursorIcon::ResizeVertical,
                Hit::Clip {
                    zone: Zone::FadeIn | Zone::FadeOut,
                    ..
                } => CursorIcon::ResizeColumn,
                Hit::Playhead => CursorIcon::ResizeHorizontal,
                Hit::Transition { .. } => CursorIcon::PointingHand,
                _ => CursorIcon::Default,
            };
            if cursor != CursorIcon::Default {
                ui.ctx().set_cursor_icon(cursor);
            }
        }
    }

    // Clicks.
    if resp.clicked() {
        if let Some(p) = pointer {
            match hit_test(app, view, rows, p) {
                Hit::Clip { clip, .. } => {
                    app.select(clip, mods.command || mods.shift);
                }
                Hit::Transition { clip, .. } => {
                    app.select(clip, false);
                }
                Hit::Empty { t, .. } => {
                    app.clear_selection();
                    app.seek(app.project.settings.frame_rate.snap(t));
                }
                Hit::Playhead => {}
            }
        }
    }
    if resp.double_clicked() {
        if let Some(p) = pointer {
            if let Hit::Clip { clip, .. } | Hit::Transition { clip, .. } =
                hit_test(app, view, rows, p)
            {
                app.select(clip, false);
                // Text goes to the text tab, not the video settings.
                if let Some(tab) = crate::inspector::main_tab_of(app, clip) {
                    app.inspector.request_tab(tab);
                }
                app.open_inspector();
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(p) = pointer {
            let hit = hit_test(app, view, rows, p);
            if let Hit::Clip { clip, .. } | Hit::Transition { clip, .. } = hit {
                if !app.is_selected(clip) {
                    app.select(clip, false);
                }
            }
            app.timeline.menu = Some(hit);
        }
    }
    resp.context_menu(|ui| {
        if let Some(hit) = app.timeline.menu {
            context_menu(app, ui, hit);
        }
    });

    // Drags.
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            // Where the button went down, not where the drag was noticed.
            let start = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .unwrap_or(p - resp.drag_delta());
            let hit = hit_test(app, view, rows, start);
            let op = match hit {
                Hit::Clip { clip, track, zone } => {
                    let locked = app.project.track(track).is_some_and(|t| t.locked);
                    if !app.is_selected(clip) {
                        app.select(clip, mods.command || mods.shift);
                    }
                    if locked {
                        None
                    } else {
                        let c = app.project.clip(clip).map(|(_, c)| c.clone());
                        c.map(|c| match zone {
                            Zone::Body => {
                                let group: Vec<ClipId> =
                                    if app.selection.len() > 1 && app.is_selected(clip) {
                                        app.selection.clone()
                                    } else {
                                        vec![clip]
                                    };
                                DragOp::Move {
                                    clip,
                                    grab: view.t(start.x) - c.start,
                                    group,
                                }
                            }
                            Zone::TrimStart => DragOp::Trim {
                                clip,
                                edge: Edge::Start,
                            },
                            Zone::TrimEnd => DragOp::Trim {
                                clip,
                                edge: Edge::End,
                            },
                            Zone::Volume => DragOp::Volume {
                                clip,
                                start: c.volume.value,
                            },
                            Zone::FadeIn => DragOp::Fade { clip, out: false },
                            Zone::FadeOut => DragOp::Fade { clip, out: true },
                        })
                    }
                }
                Hit::Playhead => {
                    app.pause();
                    Some(DragOp::Scrub)
                }
                Hit::Transition { .. } => None,
                Hit::Empty { .. } => {
                    if !(mods.command || mods.shift) {
                        app.clear_selection();
                    }
                    Some(DragOp::Marquee)
                }
            };
            if let Some(op) = op {
                app.timeline.drag = Some(Drag {
                    op,
                    before: app.project.clone(),
                    was_dirty: app.dirty,
                    start,
                    snapped: None,
                });
            }
        }
    }
    if resp.dragged() {
        if let Some(p) = pointer {
            drag_update(app, ui, lanes, view, rows, p);
            // Scroll when dragging near the sides.
            let edge = 40.0;
            if p.x > lanes.right() - edge {
                app.timeline.scroll_x += ((p.x - (lanes.right() - edge)) / 4.0).min(20.0);
            } else if p.x < lanes.left() + edge && app.timeline.scroll_x > 0.0 {
                app.timeline.scroll_x -= (((lanes.left() + edge) - p.x) / 4.0).min(20.0);
            }
            ui.ctx().request_repaint();
        }
    }
    if resp.drag_stopped() {
        finish_drag(app);
    }
}

fn drag_update(app: &mut TidyCutsApp, ui: &mut Ui, lanes: Rect, view: View, rows: &[Row], p: Pos2) {
    let Some(drag) = app.timeline.drag.as_mut() else {
        return;
    };
    let op = drag.op.clone();
    let before = drag.before.clone();
    let start = drag.start;
    let mut snapped = None;
    match op {
        DragOp::Scrub => {
            app.timeline.scrubbing = true;
            let (t, _) = snap_time(app, view, &before, view.t(p.x), &[]);
            app.timeline.drag.as_mut().unwrap().snapped = None;
            app.seek(t);
            return;
        }
        DragOp::Marquee => {
            let r = Rect::from_two_pos(start, p);
            ui.painter().with_clip_rect(lanes).rect(
                r,
                CornerRadius::same(2),
                theme::ACCENT.gamma_multiply(0.12),
                Stroke::new(1.0, theme::ACCENT),
                StrokeKind::Inside,
            );
            let mut hits = Vec::new();
            for row in rows {
                if let Some(t) = app.project.track(row.id) {
                    for c in &t.clips {
                        if clip_rect(view, row, c).intersects(r) {
                            hits.push(c.id);
                        }
                    }
                }
            }
            app.selection = hits;
            return;
        }
        _ => {}
    }
    let mut project = before.clone();
    match op {
        DragOp::Move { clip, grab, group } => {
            let Some((from, c)) = before.clip(clip) else {
                return;
            };
            let (from_id, kind, len, orig) = (from.id, from.kind, c.duration(), c.start);
            let wanted = view.t(p.x) - grab;
            let (start_t, s) = snap_span(app, view, &before, wanted, len, &group);
            snapped = s;
            let delta = start_t - orig;
            if group.len() <= 1 {
                // A single clip can change track (same kind), or make a new
                // track when dragged above the top video track or below the
                // last audio track.
                // Titles, stickers and effects stay off the main track, where
                // they would push the video around.
                let overlay = matches!(
                    c.generator,
                    Some(
                        Generator::Title(_)
                            | Generator::Sticker(_)
                            | Generator::Adjustment
                            | Generator::Visualizer(_)
                    )
                );
                let main = before.main_track();
                let mut target = row_at(rows, p.y)
                    .filter(|r| r.kind == kind)
                    .filter(|r| !(overlay && Some(r.id) == main))
                    .map(|r| r.id)
                    .unwrap_or(from_id);
                let above = rows.first().is_some_and(|r| p.y < r.rect.top() - 6.0);
                let below = rows.last().is_some_and(|r| p.y > r.rect.bottom() + 6.0);
                if (kind == TrackKind::Video && above) || (kind == TrackKind::Audio && below) {
                    if let Some(new) = project.add_track(kind) {
                        target = new;
                    }
                }
                project.move_clip(clip, target, start_t);
            } else {
                // Move the group together, keeping each clip on its track.
                let mut items: Vec<(ClipId, TrackId, Time)> = group
                    .iter()
                    .filter_map(|id| before.clip(*id).map(|(t, c)| (*id, t.id, c.start)))
                    .collect();
                items.sort_by_key(|(_, _, s)| *s);
                if delta > Time::ZERO {
                    items.reverse();
                }
                for (id, track, s) in items {
                    project.move_clip(id, track, (s + delta).max(Time::ZERO));
                }
            }
        }
        DragOp::Trim { clip, edge } => {
            let (t, s) = snap_time(app, view, &before, view.t(p.x), &[clip]);
            snapped = s;
            project.trim_clip(clip, edge, t);
        }
        DragOp::Volume { clip, start: v0 } => {
            if let Some((_, c)) = before.clip(clip) {
                let r = rows
                    .iter()
                    .find(|r| before.clip(clip).is_some_and(|(t, _)| t.id == r.id))
                    .map(|r| clip_rect(view, r, c));
                if let Some(r) = r {
                    let h = r.height() - 18.0;
                    let dv = -((p.y - start.y) / h.max(1.0)) as f64 * 200.0;
                    let v = (v0 + dv).clamp(0.0, 200.0).round();
                    if let Some(c) = project.clip_mut(clip) {
                        c.volume.value = v;
                    }
                    let db = crate::widgets::percent_to_db(v);
                    egui::Tooltip::always_open(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        Id::new("vol-tip"),
                        egui::PopupAnchor::Pointer,
                    )
                    .show(|ui| {
                        ui.label(format!("Volume {:+.1} dB", db));
                    });
                }
            }
        }
        DragOp::Fade { clip, out } => {
            if let Some(c) = project.clip_mut(clip) {
                let t = view.t(p.x);
                let len = c.duration();
                let max = len.scale(0.5);
                if out {
                    c.audio.fade_out = (c.end() - t).max(Time::ZERO).min(max);
                } else {
                    c.audio.fade_in = (t - c.start).max(Time::ZERO).min(max);
                }
                let shown = if out {
                    c.audio.fade_out
                } else {
                    c.audio.fade_in
                };
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    Id::new("fade-tip"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.label(format!(
                        "Fade {} {}",
                        if out { "out" } else { "in" },
                        short_duration(shown)
                    ));
                });
            }
        }
        DragOp::Scrub | DragOp::Marquee => {}
    }
    if project != app.project {
        app.project = project;
        app.touch();
    }
    if let Some(d) = app.timeline.drag.as_mut() {
        d.snapped = snapped;
    }
}

fn finish_drag(app: &mut TidyCutsApp) {
    app.timeline.scrubbing = false;
    let Some(drag) = app.timeline.drag.take() else {
        return;
    };
    match drag.op {
        DragOp::Scrub | DragOp::Marquee => {}
        _ => {
            if app.project != drag.before {
                app.record(drag.before);
            } else {
                app.dirty = drag.was_dirty;
            }
        }
    }
}

fn context_menu(app: &mut TidyCutsApp, ui: &mut Ui, hit: Hit) {
    ui.set_min_width(240.0);
    let cmd = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };
    let item = |ui: &mut Ui, label: &str, shortcut: &str, enabled: bool| -> bool {
        let c = ui
            .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
            .clicked();
        if c {
            ui.close();
        }
        c
    };
    match hit {
        Hit::Empty { track, t } => {
            if item(ui, "Paste", &format!("{cmd}V"), !app.clipboard.is_empty()) {
                app.seek(t);
                app.paste();
            }
            if let Some(track) = track {
                let gap = app
                    .project
                    .track(track)
                    .and_then(|tr| tr.gap_at(t))
                    .is_some();
                if item(ui, "Delete Gap", "", gap) {
                    app.delete_gap(track, t);
                }
            }
            ui.separator();
            if item(ui, "Add Title Here", "", true) {
                app.seek(t);
                app.add_title("", None);
            }
            if item(ui, "Select All", &format!("{cmd}A"), true) {
                app.select_all();
            }
        }
        Hit::Transition { clip, out } => {
            ui.label(egui::RichText::new("Transition length").color(theme::TEXT_DIM));
            for secs in [0.5, 1.0, 1.5, 2.0, 3.0] {
                if ui.button(format!("{secs} s")).clicked() {
                    app.edit(|p| {
                        match p
                            .clip_mut(clip)
                            .and_then(|c| transition_slot(c, out).as_mut())
                        {
                            Some(tr) => {
                                tr.duration = Time::from_secs_f64(secs);
                                true
                            }
                            None => false,
                        }
                    });
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .button(egui::RichText::new("Remove Transition").color(theme::DANGER))
                .clicked()
            {
                app.edit(|p| match p.clip_mut(clip) {
                    Some(c) => transition_slot(c, out).take().is_some(),
                    None => false,
                });
                ui.close();
            }
        }
        Hit::Clip { clip, .. } => clip_menu(app, ui, clip),
        Hit::Playhead => {
            if item(ui, "Split", &format!("{cmd}B"), true) {
                app.split_at_playhead();
            }
        }
    }
}

/// The right-click menu on a clip, following the usual one.
fn clip_menu(app: &mut TidyCutsApp, ui: &mut Ui, clip: ClipId) {
    let cmd = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    };
    let Some((track, c)) = app.project.clip(clip) else {
        return;
    };
    let kind = track.kind;
    let c = c.clone();
    let media = c.media_id.and_then(|m| app.project.media(m)).cloned();
    let is_media = media.is_some();
    let visual = kind == TrackKind::Video;
    let has_sound = media
        .as_ref()
        .is_some_and(|m| m.has_audio || m.kind == MediaKind::Audio);
    let item = |ui: &mut Ui, label: &str, shortcut: &str, enabled: bool| -> bool {
        let r = ui
            .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
            .clicked();
        if r {
            ui.close();
        }
        r
    };
    if c.is_placeholder() {
        let can_fill = app.selected_media.is_some_and(|m| {
            app.project
                .media(m)
                .is_some_and(|m| m.kind != MediaKind::Audio)
        });
        if item(ui, "Fill with Selected Media", "", can_fill) {
            if let Some(m) = app.selected_media {
                app.fill_cell(clip, m);
            }
        }
        ui.separator();
    }
    if item(ui, "Cut", &format!("{cmd}X"), true) {
        app.cut_selection();
    }
    if item(ui, "Copy", &format!("{cmd}C"), true) {
        app.copy_selection();
    }
    if item(ui, "Paste", &format!("{cmd}V"), !app.clipboard.is_empty()) {
        app.paste();
    }
    if item(ui, "Duplicate", &format!("{cmd}D"), true) {
        app.duplicate_selection();
    }
    ui.separator();
    let under = c.contains(app.playhead());
    if item(ui, "Split", &format!("{cmd}B"), under) {
        app.split_at_playhead();
    }
    if item(ui, "Trim Start to Playhead", "Alt+[", under) {
        app.trim_to_playhead(Edge::Start);
    }
    if item(ui, "Trim End to Playhead", "Alt+]", under) {
        app.trim_to_playhead(Edge::End);
    }
    if item(ui, "Ripple Delete", "Shift+Del", true) {
        app.delete_selection(true);
    }
    if item(ui, "Delete", "Del", true) {
        app.delete_selection(false);
    }
    ui.separator();
    if visual {
        if item(ui, "Crop and Zoom…", "Alt+C", true) {
            app.dialogs.open_crop(clip);
        }
        if item(ui, "Crop to Fit", "", is_media) {
            app.crop_selection_to_fit();
        }
        if item(ui, "Rotate 90° Clockwise", "", true) {
            app.rotate_selection_90();
        }
    }
    ui.menu_button("Speed", |ui| {
        for s in [0.25, 0.5, 1.0, 1.5, 2.0, 4.0] {
            if ui
                .radio((c.speed - s).abs() < 1e-6, format!("{s}×"))
                .clicked()
            {
                for id in app.selection.clone() {
                    app.set_speed(id, s);
                }
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Custom…").clicked() {
            app.inspector.request_tab("Speed");
            app.open_inspector();
            ui.close();
        }
    });
    if item(
        ui,
        if c.reverse {
            "Play Forwards"
        } else {
            "Reverse"
        },
        "",
        is_media,
    ) {
        app.toggle_reverse();
    }
    if visual {
        let can_freeze = media.as_ref().is_some_and(|m| m.kind == MediaKind::Video)
            && c.contains(app.playhead());
        if item(ui, "Freeze Frame", &format!("{cmd}Alt+F"), can_freeze) {
            app.freeze_frame();
        }
    }
    ui.separator();
    if item(
        ui,
        "Detach Audio",
        &format!("{cmd}Alt+D"),
        app.can_detach(clip),
    ) {
        app.detach_audio();
    }
    let linked = app
        .project
        .clip(clip)
        .is_some_and(|(_, c)| c.link.is_some());
    if linked && item(ui, "Unlink from its Audio / Video", "", true) {
        app.unlink_selection();
    }
    if is_media {
        ui.menu_button("Smart Cuts", |ui| {
            use crate::detection::Detection;
            if item(ui, "Silence Detection…", "", has_sound) {
                app.dialogs.open_silence(clip);
                ui.close();
            }
            let video = media.as_ref().is_some_and(|m| m.kind == MediaKind::Video);
            if item(ui, "Split at Scene Changes", "", video) {
                app.start_detection(clip, Detection::Scenes { threshold: 0.3 });
                ui.close();
            }
            if item(ui, "Add Beat Markers", "", has_sound) {
                app.start_detection(clip, Detection::Beats);
                ui.close();
            }
        });
    }
    if has_sound {
        ui.menu_button("Audio Gain", |ui| {
            for db in [6.0, 3.0, 0.0, -3.0, -6.0, -12.0] {
                if ui.button(format!("{db:+} dB")).clicked() {
                    let pct = crate::widgets::db_to_percent(db);
                    app.for_selected(|c| {
                        c.volume = tidycuts_core::Animatable::constant(pct);
                    });
                    ui.close();
                }
            }
        });
        if item(ui, if c.audio.muted { "Unmute" } else { "Mute" }, "", true) {
            app.toggle_mute();
        }
        if let Some(m) = &media {
            if m.audio_streams > 1 {
                ui.menu_button("Audio Track", |ui| {
                    for s in 0..m.audio_streams {
                        if ui
                            .radio(c.audio.stream == s, format!("Track {}", s + 1))
                            .clicked()
                        {
                            app.for_selected(|c| c.audio.stream = s);
                            ui.close();
                        }
                    }
                });
            }
        }
    }
    ui.separator();
    if is_media {
        let others: Vec<(tidycuts_core::MediaId, String)> = app
            .project
            .media
            .iter()
            .filter(|m| Some(m.id) != c.media_id)
            .filter(|m| match kind {
                TrackKind::Video => m.kind != MediaKind::Audio,
                TrackKind::Audio => m.kind == MediaKind::Audio || m.has_audio,
            })
            .map(|m| (m.id, m.name.clone()))
            .collect();
        ui.add_enabled_ui(!others.is_empty(), |ui| {
            ui.menu_button("Replace Clip", |ui| {
                for (id, name) in others {
                    if ui.button(name).clicked() {
                        app.replace_clip_media(clip, id);
                        ui.close();
                    }
                }
            });
        });
    }
    if item(
        ui,
        if c.disabled {
            "Enable Clip"
        } else {
            "Disable Clip"
        },
        "",
        true,
    ) {
        app.toggle_disable();
    }
    if item(ui, "Export Selected Clips…", "", true) {
        let sel: Vec<ClipId> = app.selection.clone();
        let range = sel
            .iter()
            .filter_map(|id| app.project.clip(*id).map(|(_, c)| (c.start, c.end())))
            .fold(None, |acc: Option<(Time, Time)>, (a, b)| {
                Some(acc.map_or((a, b), |(x, y)| (x.min(a), y.max(b))))
            });
        app.dialogs
            .open_export(&app.project, &app.prefs, app.project_path.as_deref());
        if let Some(r) = range {
            app.dialogs.set_export_range(r);
        }
    }
    ui.separator();
    if item(ui, "Select Clips From Here", "", true) {
        app.select_from(clip);
    }
    ui.menu_button("Rename", |ui| {
        let id = Id::new(("rename", clip));
        let mut name: String = ui
            .data_mut(|d| d.get_temp(id))
            .unwrap_or_else(|| clip_name(app, &c));
        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(200.0));
        ui.data_mut(|d| d.insert_temp(id, name.clone()));
        if r.lost_focus() || ui.button("Rename").clicked() {
            let n = name.trim().to_string();
            app.edit(|p| match p.clip_mut(clip) {
                Some(cl) => {
                    cl.name = if n.is_empty() { None } else { Some(n.clone()) };
                    true
                }
                None => false,
            });
            ui.data_mut(|d| d.remove::<String>(id));
            ui.close();
        }
    });
    if item(ui, "Properties", "Double-click", true) {
        app.open_inspector();
    }
    if let Some(m) = &media {
        if item(ui, "Locate in Media Library", "", true) {
            app.library.tab = crate::library::LibraryTab::Media;
            app.selected_media = Some(m.id);
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Color mark").color(theme::TEXT_DIM));
        for (i, col) in theme::MARKS.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::click());
            ui.painter().circle_filled(r.center(), 6.5, *col);
            if c.mark == Some(i as u8) {
                ui.painter()
                    .circle_stroke(r.center(), 7.5, Stroke::new(1.5, Color32::WHITE));
            }
            if resp.clicked() {
                app.set_mark(if c.mark == Some(i as u8) {
                    None
                } else {
                    Some(i as u8)
                });
                ui.close();
            }
        }
    });
}

// ----- dropping from the library ---------------------------------------

/// What a library drop would do at the pointer, for the preview ghost.
enum DropPlan {
    /// A new clip: on `track` (or a new track) at `start`, `len` long.
    Place {
        track: Option<TrackId>,
        kind: TrackKind,
        start: Time,
        len: Time,
    },
    /// Onto an existing clip (effects, transitions).
    OnClip { clip: ClipId, at_end: bool },
}

fn drop_plan(
    app: &TidyCutsApp,
    view: View,
    rows: &[Row],
    p: Pos2,
    payload: &LibraryDrag,
) -> Option<DropPlan> {
    let hit = hit_test(app, view, rows, p);
    let empty = app.project.duration() == Time::ZERO;
    let row = row_at(rows, p.y);
    // Past the top track (into the free lane) starts a new video track; past
    // the last one, a new audio track, as long as the limits allow.
    let above = rows.first().is_some_and(|r| p.y < r.rect.top() - 4.0)
        && app.project.can_add_track(TrackKind::Video);
    let below = rows.last().is_some_and(|r| p.y > r.rect.bottom() + 4.0)
        && app.project.can_add_track(TrackKind::Audio);
    let t = view.t(p.x);
    let place = |kind: TrackKind,
                 len: Time,
                 fits: &dyn Fn(TrackKind) -> bool|
     -> Option<DropPlan> {
        let new_track = (kind == TrackKind::Video && above) || (kind == TrackKind::Audio && below);
        let track = if new_track {
            None
        } else {
            match row.filter(|r| fits(r.kind) && app.project.track(r.id).is_some_and(|t| !t.locked))
            {
                Some(r) => Some(r.id),
                None => Some(app.project.default_track_for(if kind == TrackKind::Audio {
                    MediaKind::Audio
                } else {
                    MediaKind::Video
                })?),
            }
        };
        let (start, _) = snap_span(
            app,
            view,
            &app.project,
            if empty { Time::ZERO } else { t },
            len,
            &[],
        );
        Some(DropPlan::Place {
            track,
            kind,
            start,
            len,
        })
    };
    // Titles, stickers, effects and visualizers get a track of their own: a
    // free overlay track under the pointer, or else a new one on top. They
    // never go into the main track, where they would push the video around.
    let overlay = |len: Time, at: Option<Time>| -> Option<DropPlan> {
        let (start, _) = snap_span(app, view, &app.project, at.unwrap_or(t), len, &[]);
        let reuse = row
            .filter(|r| r.kind == TrackKind::Video && !r.main && !above && !below)
            .filter(|r| {
                app.project
                    .track(r.id)
                    .is_some_and(|tr| !tr.locked && tr.is_free(start, len, None))
            })
            .map(|r| r.id);
        Some(DropPlan::Place {
            track: reuse,
            kind: TrackKind::Video,
            start,
            len,
        })
    };
    match payload {
        LibraryDrag::Media(id) => {
            // Onto an empty split-screen cell: fills it.
            if let Hit::Clip { clip, .. } = hit {
                if app
                    .project
                    .clip(clip)
                    .is_some_and(|(_, c)| c.is_placeholder())
                    && app
                        .project
                        .media(*id)
                        .is_some_and(|m| m.kind != MediaKind::Audio)
                {
                    return Some(DropPlan::OnClip {
                        clip,
                        at_end: false,
                    });
                }
            }
            let m = app.project.media(*id)?;
            let kind = if m.kind == MediaKind::Audio {
                TrackKind::Audio
            } else {
                TrackKind::Video
            };
            let fits = |k: TrackKind| match k {
                TrackKind::Video => m.kind != MediaKind::Audio,
                TrackKind::Audio => m.kind == MediaKind::Audio || m.has_audio,
            };
            let kind = match row {
                Some(r) if fits(r.kind) && !above && !below => r.kind,
                _ => kind,
            };
            place(kind, m.duration, &fits)
        }
        LibraryDrag::Sound(id) => {
            let def = tidycuts_sounds::get(id)?;
            let fits = |k: TrackKind| k == TrackKind::Audio;
            place(
                TrackKind::Audio,
                Time::from_secs_f64(def.secs as f64),
                &fits,
            )
        }
        LibraryDrag::Title(_)
        | LibraryDrag::Sticker(_)
        | LibraryDrag::Template(_)
        | LibraryDrag::Layout(_)
        | LibraryDrag::Visualizer(_) => {
            let secs = match payload {
                LibraryDrag::Title(id) => tidycuts_render::catalog::titles::get(id)
                    .map(|t| t.duration as f64)
                    .unwrap_or(app.prefs.title_duration),
                LibraryDrag::Sticker(_) => app.prefs.sticker_duration,
                LibraryDrag::Template(i) => app.template_length(*i).as_secs_f64(),
                LibraryDrag::Visualizer(_) => 10.0,
                _ => app.prefs.image_duration,
            };
            let len = Time::from_secs_f64(secs);
            match payload {
                LibraryDrag::Title(_) | LibraryDrag::Sticker(_) | LibraryDrag::Visualizer(_) => {
                    overlay(len, None)
                }
                _ => place(TrackKind::Video, len, &|k| k == TrackKind::Video),
            }
        }
        // An effect dropped on a clip becomes its own clip over that clip, on
        // a new track; anywhere else it starts where it is dropped.
        LibraryDrag::Effect(_) => match hit {
            Hit::Clip { clip, track, .. }
                if app
                    .project
                    .track(track)
                    .is_some_and(|t| t.kind == TrackKind::Video) =>
            {
                let (_, c) = app.project.clip(clip)?;
                Some(DropPlan::Place {
                    track: None,
                    kind: TrackKind::Video,
                    start: c.start,
                    len: c.duration(),
                })
            }
            _ => overlay(Time::from_secs_f64(app.prefs.image_duration), None),
        },
        LibraryDrag::Transition(_) => {
            match hit {
                Hit::Clip { clip, track, .. } => {
                    let tr = app.project.track(track)?;
                    if tr.kind != TrackKind::Video {
                        return None;
                    }
                    let c = tr.clips.iter().find(|c| c.id == clip)?;
                    let mid = c.start + c.duration().scale(0.5);
                    if t < mid {
                        Some(DropPlan::OnClip {
                            clip,
                            at_end: false,
                        })
                    } else {
                        // Onto the next clip's start when they touch.
                        match tr.clips.iter().find(|n| {
                            (n.start - c.end()).abs() < Time::from_millis(2) && n.id != c.id
                        }) {
                            Some(n) => Some(DropPlan::OnClip {
                                clip: n.id,
                                at_end: false,
                            }),
                            None => Some(DropPlan::OnClip { clip, at_end: true }),
                        }
                    }
                }
                Hit::Transition { clip, out } => Some(DropPlan::OnClip { clip, at_end: out }),
                _ => None,
            }
        }
    }
}

fn library_drop(
    app: &mut TidyCutsApp,
    ui: &mut Ui,
    lanes: Rect,
    view: View,
    rows: &[Row],
    resp: &egui::Response,
) {
    let Some(payload) = resp.dnd_hover_payload::<LibraryDrag>() else {
        return;
    };
    let Some(p) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let plan = drop_plan(app, view, rows, p, &payload);
    let painter = ui.painter().with_clip_rect(lanes);
    match &plan {
        Some(DropPlan::Place {
            track,
            kind,
            start,
            len,
        }) => {
            let r = match track.and_then(|t| rows.iter().find(|r| r.id == t)) {
                Some(row) => Rect::from_min_max(
                    egui::pos2(view.x(*start), row.rect.top()),
                    egui::pos2(view.x(*start + *len), row.rect.bottom()),
                ),
                None => {
                    // A new track: above the top row or below the last.
                    let h = match kind {
                        TrackKind::Video => VIDEO_TRACK_H,
                        TrackKind::Audio => AUDIO_TRACK_H,
                    } * 0.6;
                    let y = match kind {
                        TrackKind::Video => rows
                            .first()
                            .map(|r| r.rect.top() - h - 4.0)
                            .unwrap_or(lanes.top()),
                        TrackKind::Audio => rows
                            .last()
                            .map(|r| r.rect.bottom() + 4.0)
                            .unwrap_or(lanes.bottom() - h),
                    };
                    painter.hline(
                        lanes.x_range(),
                        if *kind == TrackKind::Video {
                            y + h + 2.0
                        } else {
                            y - 2.0
                        },
                        Stroke::new(1.5, theme::ACCENT),
                    );
                    Rect::from_min_max(
                        egui::pos2(view.x(*start), y),
                        egui::pos2(view.x(*start + *len), y + h),
                    )
                }
            };
            painter.rect_filled(r, CornerRadius::same(5), theme::ACCENT.gamma_multiply(0.25));
            painter.rect_stroke(
                r,
                CornerRadius::same(5),
                Stroke::new(1.5, theme::ACCENT),
                StrokeKind::Inside,
            );
        }
        Some(DropPlan::OnClip { clip, at_end }) => {
            if let Some((tr, c)) = app.project.clip(*clip) {
                if let Some(row) = rows.iter().find(|r| r.id == tr.id) {
                    let r = clip_rect(view, row, c);
                    let mark = match payload.as_ref() {
                        LibraryDrag::Transition(_) => {
                            let x = if *at_end { r.right() } else { r.left() };
                            Rect::from_center_size(
                                egui::pos2(x, r.center().y),
                                egui::vec2(24.0, r.height()),
                            )
                        }
                        _ => r,
                    };
                    painter.rect_filled(
                        mark,
                        CornerRadius::same(5),
                        theme::ACCENT.gamma_multiply(0.3),
                    );
                    painter.rect_stroke(
                        mark,
                        CornerRadius::same(5),
                        Stroke::new(2.0, theme::ACCENT),
                        StrokeKind::Inside,
                    );
                }
            }
        }
        None => {}
    }
    if resp.dnd_release_payload::<LibraryDrag>().is_none() {
        return;
    }
    let Some(plan) = plan else { return };
    match (payload.as_ref(), plan) {
        (
            LibraryDrag::Media(m),
            DropPlan::Place {
                track, kind, start, ..
            },
        ) => {
            let track = match track {
                Some(t) => Some(t),
                None => app.add_track(kind, None),
            };
            if let Some(t) = track {
                app.add_media_to_timeline(*m, Some((t, start)));
            }
        }
        (
            LibraryDrag::Sound(id),
            DropPlan::Place {
                track, kind, start, ..
            },
        ) => {
            let track = match track {
                Some(t) => Some(t),
                None => app.add_track(kind, None),
            };
            if let Some(t) = track {
                app.add_sound(id, Some((t, start)));
            }
        }
        (
            LibraryDrag::Title(id),
            DropPlan::Place {
                track, start, len, ..
            },
        ) => {
            if let Some(at) = new_track_at(app, track, start, len) {
                app.add_title(id, Some(at));
            }
        }
        (
            LibraryDrag::Sticker(id),
            DropPlan::Place {
                track, start, len, ..
            },
        ) => {
            if let Some(at) = new_track_at(app, track, start, len) {
                app.add_sticker(id, Some(at));
            }
        }
        (LibraryDrag::Template(i), DropPlan::Place { track, start, .. }) => {
            app.add_template(*i, Some((track, start)));
        }
        (LibraryDrag::Layout(id), DropPlan::Place { start, .. }) => {
            app.add_layout(id, Some(start));
        }
        (
            LibraryDrag::Visualizer(id),
            DropPlan::Place {
                track, start, len, ..
            },
        ) => {
            if let Some(at) = new_track_at(app, track, start, len) {
                app.add_visualizer(id, Some(at));
            }
        }
        (LibraryDrag::Media(m), DropPlan::OnClip { clip, .. }) => app.fill_cell(clip, *m),
        (
            LibraryDrag::Effect(id),
            DropPlan::Place {
                track, start, len, ..
            },
        ) => {
            if let Some(at) = new_track_at(app, track, start, len) {
                app.add_effect_clip(id, Some(at), len);
            }
        }
        (LibraryDrag::Transition(id), DropPlan::OnClip { clip, at_end }) => {
            if at_end {
                app.set_transition_out(clip, id);
            } else {
                app.set_transition(clip, id);
            }
            app.selection = vec![clip];
        }
        _ => {}
    }
}

/// A drop spot on an existing track, or on a new top video track (or, when
/// all the video tracks are in use, any overlay track with room).
fn new_track_at(
    app: &mut TidyCutsApp,
    track: Option<TrackId>,
    start: Time,
    len: Time,
) -> Option<(TrackId, Time)> {
    match track {
        Some(t) => Some((t, start)),
        None if app.project.can_add_track(TrackKind::Video) => {
            app.add_track(TrackKind::Video, None).map(|t| (t, start))
        }
        None => {
            let free = app.project.overlay_track_for(start, len);
            if free.is_none() {
                app.notify("All the video tracks are busy there.", true);
            }
            free.map(|t| (t, start))
        }
    }
}

fn empty_hint(ui: &Ui, lanes: Rect, rows: &[Row]) {
    let anchor = rows
        .iter()
        .find(|r| r.main)
        .map(|r| r.rect)
        .unwrap_or(lanes);
    let box_r = Rect::from_center_size(
        egui::pos2(lanes.center().x, anchor.center().y),
        egui::vec2(380.0, anchor.height() - 10.0),
    );
    let painter = ui.painter().with_clip_rect(lanes);
    let pts = [
        box_r.left_top(),
        box_r.right_top(),
        box_r.right_bottom(),
        box_r.left_bottom(),
        box_r.left_top(),
    ];
    painter.extend(Shape::dashed_line(
        &pts,
        Stroke::new(1.0, theme::TEXT_FAINT),
        6.0,
        4.0,
    ));
    painter.text(
        box_r.center(),
        Align2::CENTER_CENTER,
        format!(
            "{}  Drag and drop media here to start your video",
            ph::IMAGE
        ),
        FontId::proportional(13.0),
        theme::TEXT_DIM,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruler_steps_keep_labels_apart() {
        assert_eq!(tick_step(24.0), 5.0);
        assert_eq!(tick_step(100.0), 1.0);
        assert_eq!(tick_step(2.0), 60.0);
        assert_eq!(ruler_label(65.0, 25.0), "01:05");
        assert_eq!(ruler_label(3725.0, 25.0), "1:02:05");
        assert_eq!(ruler_label(1.2, 25.0), "00:01:05");
    }

    #[test]
    fn view_maps_time_both_ways() {
        let v = View {
            left: 100.0,
            pps: 20.0,
            scroll: 40.0,
        };
        assert_eq!(v.x(Time::from_secs_f64(3.0)), 120.0);
        assert_eq!(v.t(120.0), Time::from_secs_f64(3.0));
        assert_eq!(v.t(0.0), Time::ZERO);
    }

    #[test]
    fn zoom_keeps_the_playhead_in_place() {
        let mut s = TimelineState {
            playhead: Time::from_secs_f64(10.0),
            px_per_sec: 40.0,
            scroll_x: 100.0,
            ..Default::default()
        };
        let before = s.playhead.as_secs_f64() as f32 * s.px_per_sec - s.scroll_x;
        s.zoom_by(2.0);
        let after = s.playhead.as_secs_f64() as f32 * s.px_per_sec - s.scroll_x;
        assert!((before - after).abs() < 0.01);
    }
}
