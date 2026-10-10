//! Controls shared by the properties panel and the dialogs: familiar
//! collapsible sections with on/off switches, labelled slider rows with a
//! value box, reset and keyframe buttons, tabs and segmented choices.

use crate::theme;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2,
};
use egui_phosphor::regular as ph;
use std::ops::RangeInclusive;
use tidycuts_core::{Animatable, Color, Time};

/// Width of the label column in property rows.
pub const LABEL_W: f32 = 88.0;

/// Width of the number at the end of slider rows.
pub const BOX_W: f32 = 44.0;

/// Space between the parts of a property row.
pub const ROW_GAP: f32 = 10.0;

/// An iOS-style on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = egui::vec2(34.0, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = if *on {
        theme::ACCENT
    } else if resp.hovered() {
        Color32::from_rgb(0x4a, 0x50, 0x59)
    } else {
        Color32::from_rgb(0x3a, 0x3f, 0x47)
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), bg);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t);
    p.circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A collapsible section with a bold title, a chevron and, optionally, an
/// on/off switch on the right (for example "Drop Shadow" or "Mask").
pub struct Section<'a> {
    id: Id,
    title: &'a str,
    enabled: Option<&'a mut bool>,
    default_open: bool,
}

impl<'a> Section<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, title: &'a str) -> Self {
        Section {
            id: Id::new(("section", id_salt)),
            title,
            enabled: None,
            default_open: true,
        }
    }

    /// Adds an on/off switch to the header.
    pub fn switch(mut self, on: &'a mut bool) -> Self {
        self.enabled = Some(on);
        self
    }

    pub fn closed(mut self) -> Self {
        self.default_open = false;
        self
    }

    /// Draws the section. Returns whether the switch changed.
    pub fn show(self, ui: &mut Ui, add: impl FnOnce(&mut Ui)) -> bool {
        let mut open: bool = ui
            .ctx()
            .data_mut(|d| *d.get_temp_mut_or(self.id, self.default_open));
        let mut switched = false;
        let width = ui.available_width();
        // 12 pixels above the title, the title line, 4 below.
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 31.0), Sense::click());
        let cy = rect.top() + 12.0 + 7.5;
        ui.painter().text(
            egui::pos2(rect.left() + 7.5, cy),
            Align2::CENTER_CENTER,
            section_icon(self.title),
            FontId::proportional(15.0),
            theme::ACCENT,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 23.0, cy),
            Align2::LEFT_CENTER,
            self.title.to_uppercase(),
            theme::strong_font(12.0),
            theme::TEXT,
        );
        if let Some(on) = self.enabled {
            let sw =
                Rect::from_center_size(egui::pos2(rect.right() - 24.0, cy), egui::vec2(34.0, 18.0));
            // A child Ui, so the switch doesn't move the parent's cursor.
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(sw));
            let r = toggle(&mut child, on);
            if r.changed() {
                switched = true;
                if *on {
                    open = true;
                }
            }
        } else {
            ui.painter().text(
                egui::pos2(rect.right() - 8.0, cy),
                Align2::CENTER_CENTER,
                if open {
                    ph::CARET_DOWN
                } else {
                    ph::CARET_RIGHT
                },
                FontId::proportional(11.0),
                theme::TEXT_FAINT,
            );
        }
        if resp.clicked() {
            open = !open;
        }
        ui.ctx().data_mut(|d| d.insert_temp(self.id, open));
        if open {
            ui.add_space(5.0);
            ui.scope(|ui| add(ui));
            ui.add_space(15.0);
        } else {
            ui.add_space(8.0);
        }
        let y = ui.cursor().top();
        ui.painter().hline(
            rect.left()..=rect.right(),
            y,
            Stroke::new(1.0, theme::BORDER),
        );
        switched
    }
}

/// The small violet icon in front of a section's title.
fn section_icon(title: &str) -> &'static str {
    let t = title.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if any(&["transform"]) {
        ph::ARROWS_HORIZONTAL
    } else if any(&["crop"]) {
        ph::CROP
    } else if any(&["composit"]) {
        ph::STACK
    } else if any(&["shadow", "background", "fill", "cell", "matte", "mask"]) {
        ph::SELECTION_BACKGROUND
    } else if any(&["color", "hsl", "curve", "lut"]) {
        ph::PALETTE
    } else if any(&["speed", "ramp", "playback"]) {
        ph::GAUGE
    } else if any(&[
        "volume", "fade", "balance", "denoise", "ducking", "track", "stream",
    ]) {
        ph::SPEAKER_HIGH
    } else if any(&["equalizer", "voice"]) {
        ph::FADERS
    } else if any(&["font", "style", "decoration", "outline", "preset", "title"]) {
        ph::TEXT_T
    } else if any(&["motion"]) {
        ph::PERSON_SIMPLE_RUN
    } else if any(&["sticker", "visualizer"]) {
        ph::STICKER
    } else if any(&["effect", "lens", "enhance", "flicker", "transition"]) {
        ph::SPARKLE
    } else {
        ph::SLIDERS_HORIZONTAL
    }
}

/// A row with a label on the left and a control filling the rest: an
/// 88-pixel label column, then controls 10 pixels apart.
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        let (r, _) = ui.allocate_exact_size(egui::vec2(LABEL_W, 18.0), Sense::hover());
        ui.painter()
            .with_clip_rect(r.expand2(egui::vec2(6.0, 0.0)))
            .text(
                r.left_center(),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.5),
                theme::TEXT_DIM,
            );
        add(ui)
    })
    .inner
}

/// A small icon button without a frame.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme::PANEL_BG_3);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(14.0),
        if resp.hovered() {
            Color32::WHITE
        } else {
            theme::TEXT_DIM
        },
    );
    resp.on_hover_text(tip)
}

/// A keyframe diamond: filled when a keyframe sits at the playhead,
/// outlined in gold when the property is animated elsewhere.
pub fn diamond(ui: &mut Ui, at_playhead: bool, animated: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), Sense::click());
    let c = rect.center();
    let r = 5.0;
    let pts = vec![
        egui::pos2(c.x, c.y - r),
        egui::pos2(c.x + r, c.y),
        egui::pos2(c.x, c.y + r),
        egui::pos2(c.x - r, c.y),
    ];
    let color = if at_playhead || animated {
        theme::KEYFRAME
    } else if resp.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT_FAINT
    };
    let fill = if at_playhead {
        color
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .add(Shape::convex_polygon(pts, fill, Stroke::new(1.3, color)));
    let tip = match (at_playhead, animated) {
        (true, _) => {
            "Remove the keyframe at the playhead
Right-click to shape the motion"
        }
        (false, true) => {
            "Add a keyframe at the playhead
Right-click to shape the motion"
        }
        (false, false) => "Add a keyframe at the playhead",
    };
    resp.on_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// How a slider looks: the thickness of its rail, the size of its handle and
/// the colour of the part that isn't filled.
#[derive(Clone, Copy)]
pub struct SliderLook {
    rail: f32,
    handle_r: f32,
    track: Color32,
    shadow: bool,
    /// The rail runs the whole width and the handle starts a handle-radius in
    /// (the library's slider); otherwise the handle's centre runs the rail.
    shifted: bool,
}

/// In the properties panel and dialogs: a 3-pixel rail and an 11-pixel handle.
pub const PANEL_SLIDER: SliderLook = SliderLook {
    rail: 3.0,
    handle_r: 5.5,
    track: Color32::from_rgb(0x2d, 0x30, 0x36),
    shadow: false,
    shifted: false,
};

/// In the library and the timeline zoom: a 4-pixel rail and a 14-pixel handle.
pub const LIBRARY_SLIDER: SliderLook = SliderLook {
    rail: 4.0,
    handle_r: 7.0,
    track: Color32::from_rgb(0x26, 0x28, 0x2d),
    shadow: true,
    shifted: true,
};

/// A slim slider as in the design, filled up to a small round handle. `log`
/// spreads the range logarithmically (speeds).
pub fn slim_slider(
    ui: &mut Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    width: f32,
    log: bool,
) -> Response {
    slider_with(ui, PANEL_SLIDER, value, range, width, log)
}

/// A slider in either of the two looks.
pub fn slider_with(
    ui: &mut Ui,
    look: SliderLook,
    value: &mut f64,
    range: RangeInclusive<f64>,
    width: f32,
    log: bool,
) -> Response {
    let h = look.handle_r * 2.0 + 4.0;
    slim(
        ui,
        look,
        value,
        range,
        egui::vec2(width.max(24.0), h),
        false,
        log,
    )
}

/// The same, standing up (the mixer's faders).
pub fn slim_fader(
    ui: &mut Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    height: f32,
) -> Response {
    let w = PANEL_SLIDER.handle_r * 2.0 + 4.0;
    slim(
        ui,
        PANEL_SLIDER,
        value,
        range,
        egui::vec2(w, height.max(24.0)),
        true,
        false,
    )
}

fn slim(
    ui: &mut Ui,
    look: SliderLook,
    value: &mut f64,
    range: RangeInclusive<f64>,
    size: egui::Vec2,
    vertical: bool,
    log: bool,
) -> Response {
    let (lo, hi) = (*range.start(), *range.end());
    let r = look.handle_r;
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    // The handle's travel: the rail minus half a handle at each end.
    let (a, b) = if vertical {
        (rect.bottom() - r, rect.top() + r)
    } else if look.shifted {
        (rect.left() + r, rect.right() + r)
    } else {
        (rect.left(), rect.right())
    };
    let to_frac = |v: f64| -> f64 {
        let f = if log && lo > 0.0 {
            (v.max(lo).ln() - lo.ln()) / (hi.ln() - lo.ln())
        } else {
            (v - lo) / (hi - lo).max(f64::EPSILON)
        };
        f.clamp(0.0, 1.0)
    };
    if resp.is_pointer_button_down_on() {
        if let Some(p) = resp.interact_pointer_pos() {
            let at = if vertical { p.y } else { p.x };
            let f = (((at - a) / (b - a)) as f64).clamp(0.0, 1.0);
            let nv = if log && lo > 0.0 {
                (lo.ln() + f * (hi.ln() - lo.ln())).exp()
            } else {
                lo + f * (hi - lo)
            };
            if (nv - *value).abs() > f64::EPSILON {
                *value = nv.clamp(lo, hi);
                resp.mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let pos = a + (b - a) * to_frac(*value) as f32;
        let c = rect.center();
        let half = look.rail / 2.0;
        let (rail, filled, center) = if vertical {
            (
                Rect::from_min_max(egui::pos2(c.x - half, b), egui::pos2(c.x + half, a)),
                Rect::from_min_max(egui::pos2(c.x - half, pos), egui::pos2(c.x + half, a)),
                egui::pos2(c.x, pos),
            )
        } else {
            // the rail itself always spans the widget
            let l = rect.left();
            (
                Rect::from_min_max(
                    egui::pos2(l, c.y - half),
                    egui::pos2(rect.right(), c.y + half),
                ),
                Rect::from_min_max(
                    egui::pos2(l, c.y - half),
                    egui::pos2(pos.min(rect.right()), c.y + half),
                ),
                egui::pos2(pos, c.y),
            )
        };
        let p = ui.painter();
        let round = CornerRadius::same((half.ceil()) as u8);
        p.rect_filled(rail, round, look.track);
        p.rect_filled(filled, round, theme::ACCENT);
        if look.shadow {
            p.circle_filled(
                center + egui::vec2(0.0, 1.0),
                r + 1.5,
                Color32::from_black_alpha(0x50),
            );
        }
        p.circle_filled(center, r, Color32::WHITE);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A 32-pixel-high button with an optional icon, as in the design: filled
/// with the accent colour (`primary`) or just outlined.
pub fn pill_button(ui: &mut Ui, icon: Option<&str>, label: &str, primary: bool) -> Response {
    let font = theme::strong_font(13.5);
    let text = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::WHITE);
    let icon_w = if icon.is_some() { 13.5 + 7.0 } else { 0.0 };
    let w = (14.0 + icon_w + text.size().x + 14.0).ceil();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 32.0), Sense::click());
    let hot = resp.hovered();
    let (fill, fg) = if primary {
        (
            if hot {
                theme::ACCENT.lerp_to_gamma(Color32::WHITE, 0.1)
            } else {
                theme::ACCENT
            },
            Color32::WHITE,
        )
    } else {
        (
            if hot {
                theme::HOVER
            } else {
                Color32::TRANSPARENT
            },
            theme::TEXT,
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(6), fill);
    if !primary {
        p.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(icon) = icon {
        p.text(
            egui::pos2(rect.left() + 14.0 + 6.75, rect.center().y),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(13.5),
            fg,
        );
    }
    p.galley(
        egui::pos2(
            rect.left() + 14.0 + icon_w,
            rect.center().y - text.size().y / 2.0,
        ),
        text,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// The search box: 32 pixels high with a magnifier, on the darkest fill.
pub fn search_field(ui: &mut Ui, text: &mut String, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 32.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), theme::FIELD_BG);
    ui.painter().text(
        egui::pos2(rect.left() + 12.0 + 6.75, rect.center().y),
        Align2::CENTER_CENTER,
        ph::MAGNIFYING_GLASS,
        FontId::proportional(13.5),
        theme::TEXT_FAINT,
    );
    let field = Rect::from_min_max(
        egui::pos2(rect.left() + 12.0 + 13.5 + 8.0, rect.top()),
        egui::pos2(rect.right() - 10.0, rect.bottom()),
    );
    ui.put(
        field,
        egui::TextEdit::singleline(text)
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new("Search").color(theme::TEXT_FAINT))
            .vertical_align(egui::Align::Center)
            .desired_width(field.width()),
    )
}

/// A number you can drag or click to type in, shown as plain right-aligned
/// text (a box appears when the pointer is over it).
pub fn readout(
    ui: &mut Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
    width: f32,
) -> Response {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 18.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding = egui::vec2(3.0, 0.0);
            ui.style_mut().override_font_id = Some(FontId::proportional(12.0));
            let w = &mut ui.visuals_mut().widgets;
            for s in [&mut w.inactive, &mut w.noninteractive] {
                s.weak_bg_fill = Color32::TRANSPARENT;
                s.bg_fill = Color32::TRANSPARENT;
                s.bg_stroke = Stroke::NONE;
            }
            ui.add(
                egui::DragValue::new(value)
                    .range(range)
                    .speed(drag_speed(decimals))
                    .max_decimals(decimals)
                    .min_decimals(decimals.min(1))
                    .suffix(suffix),
            )
        },
    )
    .inner
}

/// A horizontal slider plus a value box. Returns the slider's response
/// merged with the box's.
pub fn slider(
    ui: &mut Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
) -> Response {
    let w = (ui.available_width() - BOX_W - ROW_GAP).max(60.0);
    let s = slim_slider(ui, value, range.clone(), w, false);
    let d = readout(ui, value, range, decimals, suffix, BOX_W);
    s.union(d)
}

fn drag_speed(decimals: usize) -> f64 {
    match decimals {
        0 => 1.0,
        1 => 0.1,
        _ => 0.01,
    }
}

/// A labelled slider row with a reset button. Returns true when the value
/// changed.
pub fn slider_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
    default: f64,
) -> bool {
    row(ui, label, |ui| {
        let w = (ui.available_width() - BOX_W - ROW_GAP).max(40.0);
        let s = slim_slider(ui, value, range.clone(), w, false);
        let d = readout(ui, value, range, decimals, suffix, BOX_W);
        let mut changed = s.changed() || d.changed();
        // Double-click the slider, or right-click the number, to reset.
        let mut reset = s.double_clicked();
        d.context_menu(|ui| {
            if ui.button("Reset").clicked() {
                reset = true;
                ui.close();
            }
        });
        if reset {
            *value = default;
            changed = true;
        }
        changed
    })
}

/// A labelled slider row for a keyframable property, edited at `local`
/// (time from the clip's start). Returns true when anything changed.
#[allow(clippy::too_many_arguments)]
pub fn anim_row(
    ui: &mut Ui,
    label: &str,
    anim: &mut Animatable,
    local: Time,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
    default: f64,
) -> bool {
    let tol = Time::from_millis(20);
    row(ui, label, |ui| {
        let mut v = anim.value_at(local);
        let mut changed = false;
        // slider, number and keyframe diamond, 10 pixels apart
        let w = (ui.available_width() - BOX_W - 16.0 - 2.0 * ROW_GAP).max(40.0);
        let s = slim_slider(ui, &mut v, range.clone(), w, false);
        let d = readout(ui, &mut v, range, decimals, suffix, BOX_W);
        if s.changed() || d.changed() {
            anim.set_value_at(local, v, tol);
            changed = true;
        }
        // Double-click the slider, or right-click the number, to reset
        // (keyframes go too).
        let mut reset = s.double_clicked();
        d.context_menu(|ui| {
            if ui.button("Reset").clicked() {
                reset = true;
                ui.close();
            }
        });
        if reset {
            *anim = Animatable::constant(default);
            changed = true;
        }
        let here = anim.keyframe_near(local, tol).is_some();
        let dm = diamond(ui, here, anim.is_animated());
        if dm.clicked() {
            anim.toggle_keyframe(local, tol);
            changed = true;
        }
        if anim.is_animated() {
            dm.context_menu(|ui| {
                changed |= crate::easing::menu(ui, anim, local);
            });
        }
        changed
    })
}

/// A labelled color swatch that opens a picker.
pub fn color_row(ui: &mut Ui, label: &str, color: &mut Color) -> bool {
    row(ui, label, |ui| color_button(ui, color))
}

pub fn color_button(ui: &mut Ui, color: &mut Color) -> bool {
    let mut c = egui::Rgba::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
    let r = egui::color_picker::color_edit_button_rgba(
        ui,
        &mut c,
        egui::color_picker::Alpha::OnlyBlend,
    );
    if r.changed() {
        let [r0, g, b, a] = c.to_rgba_unmultiplied();
        *color = [r0, g, b, a];
        true
    } else {
        false
    }
}

/// A drop-down list of choices. Returns true when the choice changed.
pub fn choice<T: PartialEq + Clone>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| *l)
        .unwrap_or("—");
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(ui.available_width().min(220.0))
        .show_ui(ui, |ui| {
            for (v, l) in options {
                if ui.selectable_label(v == value, *l).clicked() && v != value {
                    *value = v.clone();
                    changed = true;
                }
            }
        });
    changed
}

pub fn choice_row<T: PartialEq + Clone>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    row(ui, label, |ui| choice(ui, label, value, options))
}

/// A row of buttons where exactly one is selected.
pub fn segmented(ui: &mut Ui, selected: &mut usize, labels: &[&str]) -> bool {
    let mut changed = false;
    let h = 26.0;
    let w = ui.available_width();
    let each = (w / labels.len().max(1) as f32).min(140.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(each * labels.len() as f32, h), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), theme::FIELD_BG);
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            rect.min + egui::vec2(i as f32 * each, 0.0),
            egui::vec2(each, h),
        )
        .shrink(2.0);
        let resp = ui.interact(r, ui.id().with(("seg", i, *l)), Sense::click());
        let on = *selected == i;
        if on {
            ui.painter()
                .rect_filled(r, CornerRadius::same(5), theme::PANEL_BG_3);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r, CornerRadius::same(5), theme::PANEL_BG_2);
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *l,
            FontId::proportional(12.5),
            if on { Color32::WHITE } else { theme::TEXT_DIM },
        );
        if resp.clicked() && !on {
            *selected = i;
            changed = true;
        }
    }
    changed
}

/// Underlined tabs across the top of a panel. Returns true on a change.
pub fn tabs(ui: &mut Ui, selected: &mut usize, labels: &[&str]) -> bool {
    let mut changed = false;
    let h = 36.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::hover());
    let mut x = rect.left() + 4.0;
    for (i, l) in labels.iter().enumerate() {
        let galley =
            ui.painter()
                .layout_no_wrap(l.to_string(), theme::heading_font(13.5), Color32::WHITE);
        let w = galley.size().x + 22.0;
        let r = Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, h));
        let resp = ui.interact(r, ui.id().with(("tab", i, *l)), Sense::click());
        let on = *selected == i;
        let color = if on {
            Color32::WHITE
        } else if resp.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        ui.painter().galley_with_override_text_color(
            egui::pos2(
                r.center().x - galley.size().x / 2.0,
                r.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
        if on {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(r.left() + 8.0, r.bottom() - 3.0),
                    egui::pos2(r.right() - 8.0, r.bottom()),
                ),
                CornerRadius::same(2),
                theme::ACCENT,
            );
        }
        if resp.clicked() && !on {
            *selected = i;
            changed = true;
        }
        x += w;
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, theme::BORDER),
    );
    changed
}

/// A grid of selectable tiles with a label under each (animation presets,
/// mask shapes). `draw` paints the tile's picture. Returns the clicked index.
pub fn tile_grid(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    count: usize,
    tile: Vec2,
    selected: Option<usize>,
    mut label: impl FnMut(usize) -> String,
    mut draw: impl FnMut(&egui::Painter, Rect, usize, bool),
) -> Option<usize> {
    let gap = 8.0;
    let per_row = ((ui.available_width() + gap) / (tile.x + gap))
        .floor()
        .max(1.0) as usize;
    let mut clicked = None;
    for row_start in (0..count).step_by(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in row_start..(row_start + per_row).min(count) {
                let (rect, resp) =
                    ui.allocate_exact_size(tile + egui::vec2(0.0, 18.0), Sense::click());
                let pic = Rect::from_min_size(rect.min, tile);
                let on = selected == Some(i);
                ui.painter()
                    .rect_filled(pic, CornerRadius::same(6), theme::FIELD_BG);
                draw(ui.painter(), pic, i, resp.hovered());
                let stroke = if on {
                    Stroke::new(2.0, theme::ACCENT)
                } else if resp.hovered() {
                    Stroke::new(1.0, theme::TEXT_DIM)
                } else {
                    Stroke::new(1.0, theme::BORDER)
                };
                ui.painter()
                    .rect_stroke(pic, CornerRadius::same(6), stroke, StrokeKind::Inside);
                let text = label(i);
                ui.painter().with_clip_rect(rect).text(
                    egui::pos2(rect.center().x, rect.bottom() - 8.0),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(11.5),
                    if on { theme::ACCENT } else { theme::TEXT_DIM },
                );
                if resp.clicked() {
                    clicked = Some(i);
                }
                let _ = id;
            }
        });
    }
    clicked
}

/// Volume percent (100 = unchanged) shown as decibels.
pub fn percent_to_db(p: f64) -> f64 {
    if p <= 0.0 {
        -60.0
    } else {
        (20.0 * (p / 100.0).log10()).max(-60.0)
    }
}

pub fn db_to_percent(db: f64) -> f64 {
    if db <= -60.0 {
        0.0
    } else {
        100.0 * 10f64.powf(db / 20.0)
    }
}

/// A short, friendly duration like "2.5s" or "1:05".
pub fn short_duration(t: Time) -> String {
    let s = t.as_secs_f64();
    if s < 60.0 {
        format!("{s:.1}s")
    } else {
        format!("{}:{:02}", (s / 60.0) as i64, (s % 60.0) as i64)
    }
}

/// Like [`row`], with a label column of the given width (dialogs use a
/// wider one than the properties panel).
pub fn labeled_row<R>(ui: &mut Ui, label: &str, label_w: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(label_w, 28.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
        add(ui)
    })
    .inner
}

/// A small upper-case heading over a group of controls.
pub fn group_title(ui: &mut Ui, title: &str) {
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .font(theme::heading_font(11.0))
            .color(theme::TEXT_FAINT),
    );
}

/// Dim helper text under a control, wrapped to the available width.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).color(theme::TEXT_DIM)).wrap());
}

/// A large selectable choice with a radio dot, a title, a detail line and
/// an optional note: for questions with a few clear answers.
pub fn option_card(
    ui: &mut Ui,
    selected: bool,
    icon: &str,
    title: &str,
    detail: &str,
    note: &str,
) -> Response {
    let h = if note.is_empty() { 56.0 } else { 70.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    let fill = if selected {
        Color32::from_rgb(0x1b, 0x2e, 0x2a)
    } else if resp.hovered() {
        theme::PANEL_BG_3
    } else {
        theme::PANEL_BG_2
    };
    p.rect_filled(rect, CornerRadius::same(10), fill);
    let stroke = if selected {
        Stroke::new(1.5, theme::ACCENT)
    } else {
        Stroke::new(1.0, theme::BORDER)
    };
    p.rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Inside);
    // Icon badge.
    let badge = Rect::from_center_size(
        rect.left_center() + egui::vec2(32.0, 0.0),
        egui::vec2(36.0, 36.0),
    );
    p.rect_filled(badge, CornerRadius::same(9), theme::FIELD_BG);
    p.text(
        badge.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(19.0),
        if selected {
            theme::ACCENT
        } else {
            theme::TEXT_DIM
        },
    );
    let x = badge.right() + 14.0;
    let top = rect.top() + if note.is_empty() { 11.0 } else { 10.0 };
    p.text(
        egui::pos2(x, top),
        Align2::LEFT_TOP,
        title,
        theme::heading_font(14.0),
        Color32::WHITE,
    );
    p.text(
        egui::pos2(x, top + 20.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(12.5),
        if selected { theme::ACCENT } else { theme::TEXT },
    );
    if !note.is_empty() {
        p.text(
            egui::pos2(x, top + 37.0),
            Align2::LEFT_TOP,
            note,
            FontId::proportional(12.0),
            theme::TEXT_DIM,
        );
    }
    // Radio dot.
    let c = rect.right_center() - egui::vec2(24.0, 0.0);
    p.circle_stroke(
        c,
        8.0,
        Stroke::new(
            1.5,
            if selected {
                theme::ACCENT
            } else {
                theme::TEXT_FAINT
            },
        ),
    );
    if selected {
        p.circle_filled(c, 4.5, theme::ACCENT);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A row in a sidebar list: an icon and a label, highlighted when
/// selected.
pub fn list_item(ui: &mut Ui, selected: bool, icon: &str, label: &str, detail: &str) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(7), theme::PANEL_BG_3);
        p.rect_filled(
            Rect::from_min_size(
                rect.left_top() + egui::vec2(0.0, 8.0),
                egui::vec2(3.0, rect.height() - 16.0),
            ),
            CornerRadius::same(2),
            theme::ACCENT,
        );
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), theme::PANEL_BG_2);
    }
    let color = if selected {
        Color32::WHITE
    } else if resp.hovered() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    p.text(
        rect.left_center() + egui::vec2(14.0, 0.0),
        Align2::LEFT_CENTER,
        icon,
        FontId::proportional(15.0),
        if selected { theme::ACCENT } else { color },
    );
    p.text(
        rect.left_center() + egui::vec2(38.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        if selected {
            theme::heading_font(13.0)
        } else {
            FontId::proportional(13.0)
        },
        color,
    );
    if !detail.is_empty() {
        p.text(
            rect.right_center() - egui::vec2(10.0, 0.0),
            Align2::RIGHT_CENTER,
            detail,
            FontId::proportional(11.0),
            theme::TEXT_FAINT,
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A rounded progress bar in the accent color.
pub fn progress_bar(ui: &mut Ui, fraction: f32, height: f32, color: Color32) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let r = CornerRadius::same((height / 2.0).min(255.0) as u8);
    ui.painter().rect_filled(rect, r, theme::FIELD_BG);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let w = (rect.width() * f).max(height);
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, egui::vec2(w, height)),
            r,
            color,
        );
    }
    resp
}

/// A read-only box showing a file or folder path, shortened from the left
/// when it doesn't fit. The full path shows on hover.
pub fn path_box(ui: &mut Ui, path: &str, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 28.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), theme::FIELD_BG);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, theme::BORDER),
        StrokeKind::Inside,
    );
    let font = FontId::proportional(12.5);
    let room = rect.width() - 20.0;
    let mut shown = path.to_string();
    let fits = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), theme::TEXT)
            .size()
            .x
            <= room
    };
    if !fits(&shown) {
        let chars: Vec<char> = path.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            start += 1;
            shown = format!("…{}", chars[start..].iter().collect::<String>());
            if fits(&shown) {
                break;
            }
        }
    }
    ui.painter().with_clip_rect(rect).text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        Align2::LEFT_CENTER,
        shown,
        font,
        theme::TEXT,
    );
    resp.on_hover_text(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decibels_round_trip() {
        assert!((percent_to_db(100.0)).abs() < 1e-9);
        assert!((db_to_percent(6.0) - 199.526).abs() < 0.01);
        assert_eq!(db_to_percent(-60.0), 0.0);
        assert_eq!(percent_to_db(0.0), -60.0);
    }
}
