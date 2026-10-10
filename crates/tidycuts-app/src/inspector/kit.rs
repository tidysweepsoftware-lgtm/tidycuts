//! Building blocks for the properties panel, layered on the shared
//! controls in `widgets`: sections with reset and extra header buttons,
//! keyframable rows shown in other units (dB, percent), chips, wide action
//! buttons and hints.

use crate::theme;
use crate::widgets::{self, Section};
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui,
};
use egui_phosphor::regular as ph;
use std::ops::RangeInclusive;
use tidycuts_core::{Animatable, Time};

/// Height of a section header (matches `widgets::Section`).
const HEADER_H: f32 = 31.0;
/// Where the title line of a section sits, from the top of the section.
const TITLE_CY: f32 = 19.5;

/// What happened in a section's header.
#[derive(Debug, Default, Clone, Copy)]
pub struct SecOut {
    /// The on/off switch was flipped.
    pub switched: bool,
    /// The reset button was clicked.
    pub reset: bool,
    /// Index of an extra header button that was clicked.
    pub button: Option<usize>,
}

/// A `widgets::Section` that can also show a reset button and a few icon
/// buttons in its header.
pub struct Sec<'a> {
    id: Id,
    title: &'a str,
    on: Option<&'a mut bool>,
    closed: bool,
    reset: Option<bool>,
    buttons: Vec<(&'static str, &'a str, bool)>,
}

impl<'a> Sec<'a> {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug, title: &'a str) -> Self {
        Sec {
            id: Id::new(("insp-sec", id_salt)),
            title,
            on: None,
            closed: false,
            reset: None,
            buttons: Vec::new(),
        }
    }

    /// Adds an on/off switch.
    pub fn switch(mut self, on: &'a mut bool) -> Self {
        self.on = Some(on);
        self
    }

    /// Starts collapsed.
    pub fn closed(mut self) -> Self {
        self.closed = true;
        self
    }

    /// Adds a reset button, shown when `can_reset`.
    pub fn reset(mut self, can_reset: bool) -> Self {
        self.reset = Some(can_reset);
        self
    }

    /// Adds an icon button (drawn right to left after the reset button).
    pub fn button(mut self, icon: &'static str, tip: &'a str, enabled: bool) -> Self {
        self.buttons.push((icon, tip, enabled));
        self
    }

    pub fn show(self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) -> SecOut {
        let top = ui.cursor().min;
        let width = ui.available_width();
        let has_switch = self.on.is_some();
        let mut s = Section::new(self.id, self.title);
        if let Some(on) = self.on {
            s = s.switch(on);
        }
        if self.closed {
            s = s.closed();
        }
        let mut out = SecOut {
            switched: s.show(ui, body),
            ..Default::default()
        };
        let header = Rect::from_min_size(top, egui::vec2(width, HEADER_H));
        let mut right = header.right() - if has_switch { 50.0 } else { 1.0 };
        if let Some(true) = self.reset {
            let r = Rect::from_min_max(
                egui::pos2(right - 24.0, header.top() + TITLE_CY - 12.0),
                egui::pos2(right, header.top() + TITLE_CY + 12.0),
            );
            right -= 26.0;
            if overlay_icon(
                ui,
                r,
                self.id.with("reset"),
                ph::ARROW_COUNTER_CLOCKWISE,
                "Reset this section",
                true,
            )
            .clicked()
            {
                out.reset = true;
            }
        }
        for (i, (icon, tip, enabled)) in self.buttons.into_iter().enumerate() {
            let r = Rect::from_min_max(
                egui::pos2(right - 24.0, header.top() + TITLE_CY - 12.0),
                egui::pos2(right, header.top() + TITLE_CY + 12.0),
            );
            right -= 26.0;
            if overlay_icon(ui, r, self.id.with(("btn", i)), icon, tip, enabled).clicked() {
                out.button = Some(i);
            }
        }
        out
    }
}

/// An icon button placed at a fixed spot without moving the layout cursor.
pub fn overlay_icon(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    icon: &str,
    tip: &str,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    let hovered = enabled && resp.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), theme::PANEL_BG_3);
    }
    let color = if !enabled {
        theme::TEXT_FAINT.gamma_multiply(0.6)
    } else if hovered {
        Color32::WHITE
    } else {
        theme::TEXT_DIM
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(15.0),
        color,
    );
    let resp = resp.on_hover_text(tip);
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Width `widgets::anim_row` lays out beyond the space it's given (its
/// slider leaves room for the box and buttons but not their spacing).
const ANIM_ROW_OVERFLOW: f32 = 0.0;

/// `widgets::anim_row` kept inside the panel's width: the row is laid out
/// in a scope narrowed by its overflow, so rows don't widen the panel one
/// after another and every row's controls line up on the right.
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
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width() - ANIM_ROW_OVERFLOW);
        widgets::anim_row(ui, label, anim, local, range, decimals, suffix, default)
    })
    .inner
}

/// A labelled slider row with a value box and reset button whose columns
/// line up with [`anim_row`]'s (a gap stands where the keyframe button is).
pub fn slider_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
    default: f64,
) -> bool {
    widgets::slider_row(ui, label, value, range, decimals, suffix, default)
}

/// Width of the slider in [`anim_row`] and [`slider_row`] for a row whose
/// controls start at the left of `ui` (after the label).
pub fn slider_width(controls_width: f32) -> f32 {
    (controls_width - widgets::BOX_W - widgets::ROW_GAP - ANIM_ROW_OVERFLOW).max(40.0)
}

/// A copy of an animatable value with every value passed through `f`.
pub fn map_anim(a: &Animatable, f: impl Fn(f64) -> f64) -> Animatable {
    let mut out = a.clone();
    out.value = f(a.value);
    for k in &mut out.keyframes {
        k.value = f(k.value);
    }
    out
}

/// A keyframable row shown in display units: `to` turns the stored value
/// into what the row shows, `from` turns it back.
#[allow(clippy::too_many_arguments)]
pub fn anim_row_mapped(
    ui: &mut Ui,
    label: &str,
    anim: &mut Animatable,
    local: Time,
    to: impl Fn(f64) -> f64,
    from: impl Fn(f64) -> f64,
    range: RangeInclusive<f64>,
    decimals: usize,
    suffix: &str,
    default: f64,
) -> bool {
    let mut shown = map_anim(anim, to);
    if anim_row(
        ui, label, &mut shown, local, range, decimals, suffix, default,
    ) {
        *anim = map_anim(&shown, from);
        true
    } else {
        false
    }
}

/// A keyframable fraction (0..1 stored) shown as a percentage.
#[allow(clippy::too_many_arguments)]
pub fn anim_row_percent(
    ui: &mut Ui,
    label: &str,
    anim: &mut Animatable,
    local: Time,
    range: RangeInclusive<f64>,
    default: f64,
) -> bool {
    anim_row_mapped(
        ui,
        label,
        anim,
        local,
        |v| v * 100.0,
        |v| v / 100.0,
        range,
        0,
        "%",
        default,
    )
}

/// A labelled on/off switch.
pub fn toggle_row(ui: &mut Ui, label: &str, on: &mut bool) -> bool {
    widgets::row(ui, label, |ui| widgets::toggle(ui, on).changed())
}

/// A slider row for a span of time, in seconds.
pub fn time_row(ui: &mut Ui, label: &str, t: &mut Time, max_secs: f64) -> bool {
    let mut s = t.as_secs_f64();
    let max = max_secs.max(0.1);
    if slider_row(ui, label, &mut s, 0.0..=max, 2, " s", 0.0) {
        *t = Time::from_secs_f64(s.clamp(0.0, max));
        true
    } else {
        false
    }
}

/// Small explanatory text under a control.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(theme::TEXT_FAINT)).wrap());
}

/// Hint text indented to line up with the controls of `widgets::row`.
pub fn row_hint(ui: &mut Ui, text: &str) {
    let left = (widgets::LABEL_W + ui.spacing().item_spacing.x).min(127.0) as i8;
    ui.add_space(-3.0);
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left,
            right: 8,
            ..Default::default()
        })
        .show(ui, |ui| hint(ui, text));
}

/// Rounded pill buttons that wrap onto several lines. Returns the clicked
/// index.
pub fn chips(ui: &mut Ui, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (i, l) in labels.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(
                l.to_string(),
                FontId::proportional(12.5),
                Color32::WHITE,
            );
            let size = egui::vec2(galley.size().x + 22.0, 26.0);
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let on = selected == Some(i);
            let (fill, stroke, text) = if on {
                (
                    theme::ACCENT_DIM,
                    Stroke::new(1.0, theme::ACCENT),
                    Color32::WHITE,
                )
            } else if resp.hovered() {
                (
                    theme::PANEL_BG_3,
                    Stroke::new(1.0, theme::BORDER),
                    Color32::WHITE,
                )
            } else {
                (
                    theme::FIELD_BG,
                    Stroke::new(1.0, theme::BORDER),
                    theme::TEXT_DIM,
                )
            };
            ui.painter().rect(
                rect,
                CornerRadius::same(13),
                fill,
                stroke,
                StrokeKind::Inside,
            );
            ui.painter().galley_with_override_text_color(
                rect.center() - galley.size() / 2.0,
                galley,
                text,
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// A labelled row of chips.
pub fn chips_row(
    ui: &mut Ui,
    label: &str,
    labels: &[&str],
    selected: Option<usize>,
) -> Option<usize> {
    widgets::row(ui, label, |ui| chips(ui, labels, selected))
}

/// A tall full-width button with an icon, a title and a line of detail,
/// for the panel's main actions (Detach Audio).
pub fn wide_button(ui: &mut Ui, icon: &str, title: &str, detail: &str, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), sense);
    let hovered = enabled && resp.hovered();
    let (fill, fg, sub) = if !enabled {
        (theme::PANEL_BG_2, theme::TEXT_FAINT, theme::TEXT_FAINT)
    } else if hovered {
        (
            theme::ACCENT.lerp_to_gamma(Color32::WHITE, 0.12),
            theme::ACCENT_TEXT,
            theme::ACCENT_TEXT.gamma_multiply(0.8),
        )
    } else {
        (
            theme::ACCENT,
            theme::ACCENT_TEXT,
            theme::ACCENT_TEXT.gamma_multiply(0.8),
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), fill);
    if !enabled {
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0, theme::BORDER),
            StrokeKind::Inside,
        );
    }
    let icon_c = egui::pos2(rect.left() + 30.0, rect.center().y);
    p.circle_filled(
        icon_c,
        17.0,
        if enabled {
            Color32::from_black_alpha(28)
        } else {
            theme::PANEL_BG_3
        },
    );
    p.text(
        icon_c,
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(19.0),
        fg,
    );
    let text_x = rect.left() + 58.0;
    let clip = p.with_clip_rect(rect.shrink(4.0));
    clip.text(
        egui::pos2(text_x, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        title,
        theme::heading_font(14.5),
        fg,
    );
    clip.text(
        egui::pos2(text_x, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        detail,
        FontId::proportional(12.0),
        sub,
    );
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A framed button with an icon and a label that can show a pressed state
/// (flip, bold, alignment).
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, on: bool, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 30.0), Sense::click());
    let (fill, stroke, fg) = if on {
        (
            theme::ACCENT_DIM,
            Stroke::new(1.0, theme::ACCENT),
            Color32::WHITE,
        )
    } else if resp.hovered() {
        (
            theme::PANEL_BG_3,
            Stroke::new(1.0, theme::BORDER),
            Color32::WHITE,
        )
    } else {
        (
            theme::FIELD_BG,
            Stroke::new(1.0, theme::BORDER),
            theme::TEXT,
        )
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(6),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    // Without room for the words, show the icon alone (hover tells what it is).
    let words_w = p
        .layout_no_wrap(
            label.to_string(),
            FontId::proportional(12.5),
            Color32::WHITE,
        )
        .size()
        .x;
    if label.is_empty() || 22.0 + words_w + 8.0 > rect.width() {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(16.0),
            fg,
        );
    } else {
        let galley = p.layout_no_wrap(
            label.to_string(),
            FontId::proportional(12.5),
            Color32::WHITE,
        );
        let total = 16.0 + 6.0 + galley.size().x;
        let x0 = rect.center().x - total / 2.0;
        let clip = p.with_clip_rect(rect);
        clip.text(
            egui::pos2(x0 + 8.0, rect.center().y),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(15.0),
            fg,
        );
        clip.galley_with_override_text_color(
            egui::pos2(x0 + 22.0, rect.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
    }
    let resp = if label.is_empty() {
        resp
    } else {
        resp.on_hover_text(label)
    };
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Lays out `n` equal tool buttons across a row's control area. Returns the
/// width each one gets.
pub fn button_width(ui: &Ui, n: usize) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    ((ui.available_width() - 58.0 - gap * (n as f32 - 1.0)) / n as f32).clamp(36.0, 150.0)
}

/// A secondary text button with an icon.
pub fn text_button(ui: &mut Ui, icon: &str, text: &str) -> Response {
    ui.add(theme::outline_button(
        RichText::new(format!("{icon}  {text}")).size(13.0),
    ))
    .on_hover_cursor(CursorIcon::PointingHand)
}

/// Uppercase caption above a group of controls.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .color(theme::TEXT_FAINT)
            .strong(),
    );
}

/// An empty-state block: a large faint icon with a title and a line of text.
pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, text: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(40.0).color(theme::TEXT_FAINT));
        ui.add_space(4.0);
        ui.label(RichText::new(title).font(theme::heading_font(15.0)));
        ui.add(egui::Label::new(RichText::new(text).size(12.5).color(theme::TEXT_DIM)).wrap());
    });
    ui.add_space(16.0);
}

/// Decimals that suit a slider spanning `span` units.
pub fn decimals_for(span: f64) -> usize {
    if span >= 100.0 {
        0
    } else if span >= 10.0 {
        1
    } else {
        2
    }
}
