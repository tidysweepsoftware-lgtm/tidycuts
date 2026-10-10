//! Video tab: Basic (transform, compositing, crop, background, shadow,
//! motion blur, stabilization, flicker removal, lens correction, auto
//! enhance, track matte), Mask and Enhance.

use super::kit::{self, Sec};
use super::{speed, Action, Kind, Panel};
use crate::theme;
use crate::widgets;
use egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, Shape, Stroke, StrokeKind, Ui};
use egui_phosphor::regular as ph;
use tidycuts_core::{
    Animatable, BackgroundFill, BlendMode, Clip, ClipBackground, Generator, Mask, MaskShape,
    Shadow, TrackKind, TrackMatte, Transform, Upscale,
};

/// Sub-tabs the Video tab has for a kind of clip.
pub fn subtabs(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Video | Kind::Image => &["Basic", "Mask", "Enhance"],
        Kind::Adjustment | Kind::Audio | Kind::Placeholder => &["Basic"],
        Kind::Title | Kind::Sticker | Kind::Solid | Kind::Visualizer => &["Basic", "Mask"],
    }
}

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    match p.st.video_sub {
        1 => mask_tab(p, ui, c),
        2 => enhance_tab(ui, c),
        _ => basic_tab(p, ui, c),
    }
}

fn basic_tab(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    if !p.fixed_length(c) {
        speed::duration_row(p, ui, c);
        ui.add_space(4.0);
    }
    split_cell(ui, c);
    if p.kind == Kind::Solid {
        fill(ui, c);
    }
    if p.kind != Kind::Adjustment {
        transform(p, ui, c);
    }
    compositing(p, ui, c);
    if p.kind == Kind::Adjustment {
        kit::hint(
            ui,
            "An adjustment layer applies its color, filters and effects to every track \
             below it. Opacity sets how strongly.",
        );
        return;
    }
    if p.kind.is_media() {
        crop(p, ui, c);
    }
    if p.kind != Kind::Solid {
        background(ui, c);
    }
    shadow(ui, c);
    motion_blur(ui, c);
    if p.kind == Kind::Video {
        stabilization(ui, c);
        flicker(ui, c);
    }
    if p.kind.is_media() {
        lens(ui, c);
        auto_enhance(ui, c);
    }
    if p.track == TrackKind::Video {
        track_matte(ui, c);
    }
}

/// The split-screen cell a clip sits in: its border, rounded corners and a
/// way out of the layout.
fn split_cell(ui: &mut Ui, c: &mut Clip) {
    let Some(cell) = &mut c.video.cell else {
        return;
    };
    let mut leave = false;
    let default = tidycuts_core::Cell::default();
    let dirty = cell.border != tidycuts_core::layout::DEFAULT_BORDER
        || cell.border_color != default.border_color
        || cell.radius != 0.0;
    let out = Sec::new("cell", "Split Screen Cell")
        .reset(dirty)
        .show(ui, |ui| {
            kit::slider_row(
                ui,
                "Border",
                &mut cell.border,
                0.0..=40.0,
                0,
                " px",
                tidycuts_core::layout::DEFAULT_BORDER,
            );
            widgets::color_row(ui, "Border color", &mut cell.border_color);
            kit::slider_row(ui, "Corners", &mut cell.radius, 0.0..=80.0, 0, " px", 0.0);
            kit::row_hint(
                ui,
                "The picture covers the cell. Position and Scale below move it inside the cell.",
            );
            if kit::text_button(ui, ph::X, "Take out of the layout").clicked() {
                leave = true;
            }
        });
    if out.reset {
        cell.border = tidycuts_core::layout::DEFAULT_BORDER;
        cell.border_color = default.border_color;
        cell.radius = 0.0;
    }
    if leave {
        c.video.cell = None;
    }
}

/// The color or gradient of a solid clip.
fn fill(ui: &mut Ui, c: &mut Clip) {
    Sec::new("fill", "Fill").show(ui, |ui| {
        let mut mode = match c.generator {
            Some(Generator::Gradient { .. }) => 1,
            _ => 0,
        };
        let switched = widgets::row(ui, "Type", |ui| {
            widgets::segmented(ui, &mut mode, &["Solid", "Gradient"])
        });
        if switched {
            c.generator = Some(match c.generator.take() {
                Some(Generator::Solid(col)) => Generator::Gradient {
                    a: col,
                    b: [col[0] * 0.35, col[1] * 0.35, col[2] * 0.35, col[3]],
                    angle: 90.0,
                },
                Some(Generator::Gradient { a, .. }) => Generator::Solid(a),
                other => other.unwrap_or(Generator::Solid(tidycuts_core::clip::BLACK)),
            });
        }
        match &mut c.generator {
            Some(Generator::Solid(col)) => {
                widgets::color_row(ui, "Color", col);
            }
            Some(Generator::Gradient { a, b, angle }) => {
                widgets::color_row(ui, "Start Color", a);
                widgets::color_row(ui, "End Color", b);
                let mut deg = *angle as f64;
                if kit::slider_row(ui, "Angle", &mut deg, 0.0..=360.0, 0, "°", 90.0) {
                    *angle = deg as f32;
                }
            }
            _ => {}
        }
    });
}

fn transform(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let (w, h) = p.frame();
    let local = p.local;
    let def = Transform::default();
    let t = &mut c.transform;
    let v = &mut c.video;
    let is_default = t.scale_x == def.scale_x
        && t.scale_y == def.scale_y
        && t.position_x == def.position_x
        && t.position_y == def.position_y
        && t.rotation == def.rotation
        && !t.flip_horizontal
        && !t.flip_vertical
        && v.anchor == [0.0, 0.0]
        && v.corner_radius == 0.0;
    let st = &mut p.st;
    let out = Sec::new("transform", "Transform")
        .reset(!is_default)
        .show(ui, |ui| {
            let mut locked = st.scale_lock && t.scale_x == t.scale_y;
            if kit::toggle_row(ui, "Uniform Scale", &mut locked) {
                st.scale_lock = locked;
                if locked {
                    t.scale_y = t.scale_x.clone();
                }
            }
            if locked {
                if kit::anim_row(
                    ui,
                    "Scale",
                    &mut t.scale_x,
                    local,
                    0.0..=500.0,
                    1,
                    "%",
                    100.0,
                ) {
                    t.scale_y = t.scale_x.clone();
                }
            } else {
                kit::anim_row(
                    ui,
                    "Scale X",
                    &mut t.scale_x,
                    local,
                    0.0..=500.0,
                    1,
                    "%",
                    100.0,
                );
                kit::anim_row(
                    ui,
                    "Scale Y",
                    &mut t.scale_y,
                    local,
                    0.0..=500.0,
                    1,
                    "%",
                    100.0,
                );
            }
            kit::anim_row(
                ui,
                "Position X",
                &mut t.position_x,
                local,
                -w..=w,
                0,
                "",
                0.0,
            );
            kit::anim_row(
                ui,
                "Position Y",
                &mut t.position_y,
                local,
                -h..=h,
                0,
                "",
                0.0,
            );
            kit::anim_row(
                ui,
                "Rotate",
                &mut t.rotation,
                local,
                -360.0..=360.0,
                1,
                "°",
                0.0,
            );
            widgets::row(ui, "Flip & Turn", |ui| {
                let bw = kit::button_width(ui, 4);
                if kit::tool_button(ui, ph::FLIP_HORIZONTAL, "Horizontal", t.flip_horizontal, bw)
                    .on_hover_text("Flip horizontally (mirror)")
                    .clicked()
                {
                    t.flip_horizontal = !t.flip_horizontal;
                }
                if kit::tool_button(ui, ph::FLIP_VERTICAL, "Vertical", t.flip_vertical, bw)
                    .on_hover_text("Flip vertically (upside down)")
                    .clicked()
                {
                    t.flip_vertical = !t.flip_vertical;
                }
                if kit::tool_button(ui, ph::ARROW_COUNTER_CLOCKWISE, "90° Left", false, bw)
                    .on_hover_text("Turn a quarter turn counter-clockwise")
                    .clicked()
                {
                    turn(&mut t.rotation, -90.0);
                }
                if kit::tool_button(ui, ph::ARROW_CLOCKWISE, "90° Right", false, bw)
                    .on_hover_text("Turn a quarter turn clockwise")
                    .clicked()
                {
                    turn(&mut t.rotation, 90.0);
                }
            });
            kit::slider_row(
                ui,
                "Corner Radius",
                &mut v.corner_radius,
                0.0..=(h / 2.0).max(100.0),
                0,
                "",
                0.0,
            );
            kit::slider_row(
                ui,
                "Anchor X",
                &mut v.anchor[0],
                -w / 2.0..=w / 2.0,
                0,
                "",
                0.0,
            );
            kit::slider_row(
                ui,
                "Anchor Y",
                &mut v.anchor[1],
                -h / 2.0..=h / 2.0,
                0,
                "",
                0.0,
            );
            kit::row_hint(
                ui,
                "Position and anchor are in project pixels from the frame centre. \
                 ◇ adds a keyframe at the playhead.",
            );
        });
    if out.reset {
        let opacity = t.opacity.clone();
        *t = Transform {
            opacity,
            ..Transform::default()
        };
        v.anchor = [0.0, 0.0];
        v.corner_radius = 0.0;
    }
}

/// Adds a quarter turn, keeping a constant rotation within ±180°.
fn turn(rot: &mut Animatable, by: f64) {
    rot.offset(by);
    if !rot.is_animated() {
        let mut r = rot.value.rem_euclid(360.0);
        if r > 180.0 {
            r -= 360.0;
        }
        rot.value = r;
    }
}

fn compositing(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    let adjustment = p.kind == Kind::Adjustment;
    let is_default =
        c.blend == BlendMode::Normal && c.transform.opacity == Animatable::constant(100.0);
    let out = Sec::new("compositing", "Compositing")
        .reset(!is_default)
        .show(ui, |ui| {
            if !adjustment {
                let options: Vec<(BlendMode, &str)> =
                    BlendMode::ALL.iter().map(|b| (*b, b.label())).collect();
                widgets::choice_row(ui, "Blend Mode", &mut c.blend, &options);
            }
            kit::anim_row(
                ui,
                "Opacity",
                &mut c.transform.opacity,
                local,
                0.0..=100.0,
                0,
                "%",
                100.0,
            );
        });
    if out.reset {
        c.blend = BlendMode::Normal;
        c.transform.opacity = Animatable::constant(100.0);
    }
}

fn crop(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let actions = &mut p.actions;
    let cropped = !c.crop.is_none();
    let out = Sec::new("crop", "Crop")
        .reset(cropped)
        .closed()
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if kit::text_button(ui, ph::CROP, "Crop & Zoom…").clicked() {
                    actions.push(Action::OpenCrop);
                }
            });
            ui.add_space(2.0);
            let cr = &mut c.crop;
            let mut changed = false;
            for (label, v) in [
                ("Left", &mut cr.left),
                ("Right", &mut cr.right),
                ("Top", &mut cr.top),
                ("Bottom", &mut cr.bottom),
            ] {
                let mut pct = *v * 100.0;
                if kit::slider_row(ui, label, &mut pct, 0.0..=95.0, 1, "%", 0.0) {
                    *v = pct / 100.0;
                    changed = true;
                }
            }
            if changed {
                *cr = cr.clamped();
            }
            kit::row_hint(ui, "How much is cut off each side of the picture.");
        });
    if out.reset {
        c.crop = Default::default();
    }
}

fn background(ui: &mut Ui, c: &mut Clip) {
    let bg = &mut c.video.background;
    let mut on = bg.fill != BackgroundFill::None;
    let can_reset = *bg != ClipBackground::default();
    let out = Sec::new("background", "Background")
        .switch(&mut on)
        .reset(can_reset)
        .closed()
        .show(ui, |ui| {
            let mut mode = usize::from(bg.fill == BackgroundFill::Color);
            if widgets::row(ui, "Fill", |ui| {
                widgets::segmented(ui, &mut mode, &["Blur", "Color"])
            }) {
                bg.fill = if mode == 1 {
                    BackgroundFill::Color
                } else {
                    BackgroundFill::Blur
                };
            }
            if mode == 1 {
                widgets::color_row(ui, "Color", &mut bg.color);
            } else {
                kit::slider_row(ui, "Blur", &mut bg.blur, 0.0..=100.0, 0, "", 60.0);
            }
            kit::row_hint(
                ui,
                "Fills the frame around a clip that doesn't cover it, like a vertical \
                 phone video in a widescreen project.",
            );
        });
    if out.switched {
        bg.fill = if on {
            BackgroundFill::Blur
        } else {
            BackgroundFill::None
        };
    }
    if out.reset {
        *bg = ClipBackground::default();
    }
}

fn shadow(ui: &mut Ui, c: &mut Clip) {
    let sh = &mut c.video.shadow;
    let mut on = sh.enabled;
    let can_reset = *sh
        != Shadow {
            enabled: sh.enabled,
            ..Shadow::default()
        };
    let out = Sec::new("shadow", "Drop Shadow")
        .switch(&mut on)
        .reset(can_reset)
        .closed()
        .show(ui, |ui| {
            widgets::color_row(ui, "Color", &mut sh.color);
            kit::slider_row(ui, "Opacity", &mut sh.opacity, 0.0..=100.0, 0, "%", 60.0);
            kit::slider_row(ui, "Distance", &mut sh.distance, 0.0..=200.0, 0, "", 20.0);
            kit::slider_row(ui, "Angle", &mut sh.angle, 0.0..=360.0, 0, "°", 135.0);
            kit::slider_row(ui, "Blur", &mut sh.blur, 0.0..=100.0, 0, "", 40.0);
        });
    if out.switched {
        sh.enabled = on;
    }
    if out.reset {
        *sh = Shadow {
            enabled: sh.enabled,
            ..Shadow::default()
        };
    }
}

/// A section whose switch turns a 0..100 strength on (at `default`) or off.
fn strength_section(
    ui: &mut Ui,
    id: &str,
    title: &str,
    label: &str,
    value: &mut f64,
    default: f64,
    hint: &str,
) {
    let mut on = *value > 0.0;
    let out = Sec::new(id, title).switch(&mut on).closed().show(ui, |ui| {
        let mut v = *value;
        if kit::slider_row(ui, label, &mut v, 0.0..=100.0, 0, "", default) {
            *value = v;
        }
        kit::row_hint(ui, hint);
    });
    if out.switched {
        *value = if on { default } else { 0.0 };
    }
}

fn motion_blur(ui: &mut Ui, c: &mut Clip) {
    strength_section(
        ui,
        "motion-blur",
        "Motion Blur",
        "Strength",
        &mut c.video.motion_blur,
        50.0,
        "Blurs the clip along its own movement (keyframes and animations).",
    );
}

fn stabilization(ui: &mut Ui, c: &mut Clip) {
    strength_section(
        ui,
        "stabilize",
        "Stabilization",
        "Smoothness",
        &mut c.video.stabilize,
        50.0,
        "Smooths out camera shake. The clip is analyzed once in the background and \
         zoomed in slightly so moving edges stay hidden.",
    );
}

fn flicker(ui: &mut Ui, c: &mut Clip) {
    let mut on = c.video.deflicker;
    let out = Sec::new("deflicker", "Flicker Removal")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            kit::hint(
                ui,
                "Evens out brightness flicker from artificial lights and time-lapses.",
            );
        });
    if out.switched {
        c.video.deflicker = on;
    }
}

fn lens(ui: &mut Ui, c: &mut Clip) {
    let v = &mut c.video.lens_correction;
    let mut on = *v != 0.0;
    let out = Sec::new("lens", "Lens Correction")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            kit::slider_row(ui, "Amount", v, -100.0..=100.0, 0, "", 40.0);
            kit::row_hint(
                ui,
                "Straightens the bending of wide-angle and action cameras. Negative \
                 values bend the other way.",
            );
        });
    if out.switched {
        *v = if on { 40.0 } else { 0.0 };
    }
}

fn auto_enhance(ui: &mut Ui, c: &mut Clip) {
    strength_section(
        ui,
        "auto-enhance",
        "Auto Enhance",
        "Amount",
        &mut c.video.auto_enhance,
        60.0,
        "Sets levels and white balance automatically from the clip's own colors.",
    );
}

fn track_matte(ui: &mut Ui, c: &mut Clip) {
    let m = &mut c.video.track_matte;
    let mut on = *m != TrackMatte::None;
    let out = Sec::new("track-matte", "Track Matte")
        .switch(&mut on)
        .closed()
        .show(ui, |ui| {
            let options: Vec<(TrackMatte, &str)> = TrackMatte::ALL[1..]
                .iter()
                .map(|t| (*t, t.label()))
                .collect();
            let mut v = if *m == TrackMatte::None {
                TrackMatte::Alpha
            } else {
                *m
            };
            if widgets::choice_row(ui, "Matte", &mut v, &options) {
                *m = v;
            }
            kit::row_hint(
                ui,
                "Uses the clip on the track above as a stencil for this one; the matte \
                 clip itself is hidden. Luma uses brightness, Alpha uses transparency.",
            );
        });
    if out.switched {
        *m = if on {
            TrackMatte::Alpha
        } else {
            TrackMatte::None
        };
    }
}

// ---- Mask --------------------------------------------------------------

fn mask_tab(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let local = p.local;
    kit::caption(ui, "Shape");
    let selected = c
        .video
        .mask
        .as_ref()
        .map(|m| m.shape.index() as usize + 1)
        .unwrap_or(0);
    let clicked = widgets::tile_grid(
        ui,
        "mask-shapes",
        MaskShape::ALL.len() + 1,
        egui::vec2(78.0, 58.0),
        Some(selected),
        |i| {
            if i == 0 {
                "None".into()
            } else {
                MaskShape::ALL[i - 1].label().into()
            }
        },
        draw_mask_tile,
    );
    if let Some(i) = clicked {
        if i == 0 {
            c.video.mask = None;
        } else {
            let shape = MaskShape::ALL[i - 1];
            let mut m = Mask::new(shape);
            if let Some(old) = c.video.mask.take() {
                if old.shape == shape {
                    m = old;
                } else {
                    m.x = old.x;
                    m.y = old.y;
                    m.rotation = old.rotation;
                    m.feather = old.feather;
                    m.invert = old.invert;
                }
            }
            c.video.mask = Some(m);
        }
    }
    ui.add_space(6.0);
    let Some(m) = &mut c.video.mask else {
        kit::hint(
            ui,
            "Pick a shape to show only part of the clip. Everything outside the \
             shape becomes transparent, so the tracks below show through.",
        );
        return;
    };
    let shape = m.shape;
    let fresh = Mask::new(shape);
    let can_reset = *m
        != Mask {
            invert: m.invert,
            ..fresh.clone()
        };
    let out = Sec::new("mask-params", shape.label())
        .reset(can_reset)
        .show(ui, |ui| {
            kit::anim_row_percent(ui, "Position X", &mut m.x, local, -50.0..=150.0, 50.0);
            kit::anim_row_percent(ui, "Position Y", &mut m.y, local, -50.0..=150.0, 50.0);
            let (dw, dh) = (fresh.width.value * 100.0, fresh.height.value * 100.0);
            match shape {
                MaskShape::Line => {}
                MaskShape::DoubleLine => {
                    kit::anim_row_percent(ui, "Band Width", &mut m.height, local, 0.0..=200.0, dh);
                }
                _ => {
                    kit::anim_row_percent(ui, "Width", &mut m.width, local, 0.0..=200.0, dw);
                    kit::anim_row_percent(ui, "Height", &mut m.height, local, 0.0..=200.0, dh);
                }
            }
            kit::anim_row(
                ui,
                "Rotate",
                &mut m.rotation,
                local,
                -180.0..=180.0,
                1,
                "°",
                0.0,
            );
            if shape == MaskShape::Rectangle {
                kit::anim_row(
                    ui,
                    "Roundness",
                    &mut m.roundness,
                    local,
                    0.0..=100.0,
                    0,
                    "",
                    0.0,
                );
            }
            kit::anim_row(
                ui,
                "Feather",
                &mut m.feather,
                local,
                0.0..=100.0,
                0,
                "",
                2.0,
            );
            kit::toggle_row(ui, "Invert", &mut m.invert);
            kit::row_hint(
                ui,
                "Position and size are percentages of the clip's picture; 50% is the \
                 middle.",
            );
        });
    if out.reset {
        *m = Mask {
            invert: m.invert,
            ..fresh
        };
    }
}

/// Picture for mask tile `i` (0 = none).
fn draw_mask_tile(painter: &Painter, rect: Rect, i: usize, hovered: bool) {
    let color = if hovered {
        Color32::WHITE
    } else {
        theme::TEXT_DIM
    };
    let fill = theme::ACCENT.gamma_multiply(0.35);
    let stroke = Stroke::new(1.6, color);
    let r = rect.shrink2(egui::vec2(18.0, 12.0));
    let c = r.center();
    let clip = painter.with_clip_rect(rect.shrink(3.0));
    if i == 0 {
        clip.text(
            c,
            Align2::CENTER_CENTER,
            ph::PROHIBIT,
            FontId::proportional(24.0),
            color,
        );
        return;
    }
    match MaskShape::ALL[i - 1] {
        MaskShape::Line => {
            let inner = rect.shrink(3.0);
            let a = egui::pos2(inner.left(), c.y + 8.0);
            let b = egui::pos2(inner.right(), c.y - 8.0);
            clip.add(Shape::convex_polygon(
                vec![a, b, inner.right_bottom(), inner.left_bottom()],
                fill,
                Stroke::NONE,
            ));
            clip.line_segment([a, b], stroke);
        }
        MaskShape::DoubleLine => {
            let inner = rect.shrink(3.0);
            let off = egui::vec2(0.0, 9.0);
            let a = egui::pos2(inner.left(), c.y + 6.0);
            let b = egui::pos2(inner.right(), c.y - 6.0);
            clip.add(Shape::convex_polygon(
                vec![a - off, b - off, b + off, a + off],
                fill,
                Stroke::NONE,
            ));
            clip.line_segment([a - off, b - off], stroke);
            clip.line_segment([a + off, b + off], stroke);
        }
        MaskShape::Rectangle => {
            let r = Rect::from_center_size(c, egui::vec2(36.0, 26.0));
            clip.rect(r, CornerRadius::same(3), fill, stroke, StrokeKind::Middle);
        }
        MaskShape::Ellipse => {
            clip.circle(c, 15.0, fill, stroke);
        }
        MaskShape::Star => {
            let pts: Vec<egui::Pos2> = (0..10)
                .map(|k| {
                    let a = std::f32::consts::PI * (k as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
                    let rad = if k % 2 == 0 { 17.0 } else { 7.0 };
                    c + egui::vec2(a.cos(), a.sin()) * rad
                })
                .collect();
            clip.add(Shape::closed_line(pts, stroke));
        }
        MaskShape::Heart => {
            let pts: Vec<egui::Pos2> = (0..48)
                .map(|k| {
                    let t = k as f32 / 48.0 * std::f32::consts::TAU;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos()
                        - 5.0 * (2.0 * t).cos()
                        - 2.0 * (3.0 * t).cos()
                        - (4.0 * t).cos();
                    c + egui::vec2(x, -y) * 1.0 + egui::vec2(0.0, -1.0)
                })
                .collect();
            clip.add(Shape::closed_line(pts, stroke));
        }
    }
}

// ---- Enhance -----------------------------------------------------------

fn enhance_tab(ui: &mut Ui, c: &mut Clip) {
    let e = &mut c.enhance;
    let out = Sec::new("enhance", "Video Enhancer")
        .reset(!e.is_off())
        .show(ui, |ui| {
            kit::slider_row(ui, "Denoise", &mut e.denoise, 0.0..=100.0, 0, "", 0.0);
            kit::row_hint(ui, "Smooths grain and sensor noise from low light.");
            kit::slider_row(ui, "Deblock", &mut e.deblock, 0.0..=100.0, 0, "", 0.0);
            kit::row_hint(ui, "Hides the blocky squares of heavily compressed video.");
            kit::slider_row(ui, "Sharpen", &mut e.sharpen, 0.0..=100.0, 0, "", 0.0);
            kit::row_hint(ui, "Brings out fine detail and edges.");
            let mut i = Upscale::ALL.iter().position(|u| *u == e.upscale).unwrap_or(0);
            if widgets::row(ui, "Upscaling", |ui| {
                widgets::segmented(ui, &mut i, &["Standard", "Sharp", "Enhanced"])
            }) {
                e.upscale = Upscale::ALL[i];
            }
            kit::row_hint(
                ui,
                match e.upscale {
                    Upscale::Standard => "Smooth filtering when the clip is shown bigger than its own size.",
                    Upscale::Sharp => "Crisper filtering for low-resolution clips.",
                    Upscale::Enhanced => {
                        "Edge-aware upscaling with adaptive sharpening, for small clips in a big project."
                    }
                },
            );
        });
    if out.reset {
        *e = Default::default();
    }
}
