//! The "Graphite" theme: a near-black window, flat panels that meet in thin
//! lines, a violet accent and the Inter typeface.

use egui::{Color32, CornerRadius, FontData, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

pub const WINDOW_BG: Color32 = Color32::from_rgb(0x0f, 0x10, 0x12);
/// The title bar, a touch darker than the panels.
pub const BAR_BG: Color32 = Color32::from_rgb(0x12, 0x13, 0x15);
/// The strip of library tabs, a shade darker than the panel.
pub const TAB_BG: Color32 = Color32::from_rgb(0x14, 0x15, 0x17);
/// Selected list items.
pub const SEL: Color32 = Color32::from_rgb(0x25, 0x27, 0x2c);
/// Under the pointer.
pub const HOVER: Color32 = Color32::from_rgb(0x20, 0x22, 0x26);
/// Behind the timeline's lanes.
pub const LANE_BG: Color32 = Color32::from_rgb(0x12, 0x13, 0x15);
/// The thin lines between lanes.
pub const LANE_LINE: Color32 = Color32::from_rgb(0x1f, 0x21, 0x25);
pub const PANEL_BG: Color32 = Color32::from_rgb(0x17, 0x18, 0x1b);
pub const PANEL_BG_2: Color32 = Color32::from_rgb(0x1d, 0x1f, 0x23);
pub const PANEL_BG_3: Color32 = Color32::from_rgb(0x27, 0x29, 0x2e);
pub const FIELD_BG: Color32 = Color32::from_rgb(0x10, 0x11, 0x13);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x28, 0x2d);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe9, 0xec);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0x9e, 0xa7);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6a, 0x6e, 0x77);
/// The brand colour: violet.
pub const ACCENT: Color32 = Color32::from_rgb(0x8e, 0x7c, 0xff);
/// A dark violet for selected backgrounds and quiet highlights.
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x3b, 0x34, 0x74);
/// Behind icons that sit on the accent (dialog badges and the like).
pub const ACCENT_BG: Color32 = Color32::from_rgb(0x26, 0x23, 0x4d);
/// Text on top of the accent colour.
pub const ACCENT_TEXT: Color32 = Color32::WHITE;
/// Ready, done, on: green.
pub const GOOD: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x97);
pub const PLAYHEAD: Color32 = Color32::from_rgb(0xff, 0x5a, 0x5f);
pub const VIDEO_CLIP: Color32 = Color32::from_rgb(0x2d, 0x4a, 0x8a);
pub const IMAGE_CLIP: Color32 = Color32::from_rgb(0x2a, 0x5c, 0x86);
pub const TITLE_CLIP: Color32 = Color32::from_rgb(0x8a, 0x5a, 0x14);
pub const STICKER_CLIP: Color32 = Color32::from_rgb(0x86, 0x33, 0x6a);
pub const ADJUST_CLIP: Color32 = Color32::from_rgb(0x54, 0x39, 0x9a);
pub const SOLID_CLIP: Color32 = Color32::from_rgb(0x45, 0x4b, 0x55);
pub const AUDIO_CLIP: Color32 = Color32::from_rgb(0x14, 0x58, 0x4a);
pub const AUDIO_WAVE: Color32 = Color32::from_rgb(0x96, 0xff, 0xd7);
pub const DANGER: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);
pub const WARNING: Color32 = Color32::from_rgb(0xf2, 0xb1, 0x34);
pub const KEYFRAME: Color32 = Color32::from_rgb(0xff, 0xc8, 0x3d);

/// Color labels for clips (right-click > color mark), a familiar palette.
pub const MARKS: [Color32; 8] = [
    Color32::from_rgb(0xe5, 0x48, 0x4d),
    Color32::from_rgb(0xf2, 0x8c, 0x28),
    Color32::from_rgb(0xf2, 0xd0, 0x28),
    Color32::from_rgb(0x3c, 0xc4, 0x5a),
    Color32::from_rgb(0x2f, 0xd8, 0xb0),
    Color32::from_rgb(0x3d, 0x8b, 0xff),
    Color32::from_rgb(0x9b, 0x5c, 0xff),
    Color32::from_rgb(0xff, 0x6f, 0xc8),
];

/// Corners of buttons, fields and clips.
pub const SMALL_RADIUS: u8 = 5;

/// The medium-weight UI font family (menus, names).
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}

/// The semibold UI font family, for headings and labels that need weight.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Weight 550: tab labels.
pub fn tab_weight() -> FontFamily {
    FontFamily::Name("tab".into())
}

/// Weight 650: buttons and group titles.
pub fn strong() -> FontFamily {
    FontFamily::Name("strong".into())
}

pub fn tab_font(size: f32) -> FontId {
    FontId::new(size, tab_weight())
}

pub fn strong_font(size: f32) -> FontId {
    FontId::new(size, strong())
}

/// The bold UI font family (the app name, buttons).
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn heading_font(size: f32) -> FontId {
    FontId::new(size, semibold())
}

pub fn medium_font(size: f32) -> FontId {
    FontId::new(size, medium())
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}

pub fn apply(ctx: &egui::Context) {
    // Windows lets people double-click up to half a second apart (and a
    // trackpad finger wobbles); egui's own limits are tighter, so a relaxed
    // double-click on a clip never opened its properties.
    ctx.options_mut(|o| {
        o.input_options.max_double_click_delay = 0.55;
        o.input_options.max_click_dist = 9.0;
    });
    let mut fonts = egui::FontDefinitions::default();
    // One variable Inter file, used at several weights.
    let inter = FontData::from_static(tidycuts_render::fonts::inter());
    for (name, weight) in [
        ("inter", 400.0),
        ("inter-medium", 500.0),
        ("inter-tab", 550.0),
        ("inter-semibold", 600.0),
        ("inter-strong", 650.0),
        ("inter-bold", 700.0),
    ] {
        let tweak = egui::FontTweak {
            coords: egui::epaint::text::VariationCoords::new([(b"wght", weight)]),
            ..Default::default()
        };
        fonts
            .font_data
            .insert(name.into(), Arc::new(inter.clone().tweak(tweak)));
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".into());
    let rest: Vec<String> = fonts.families[&FontFamily::Proportional]
        .iter()
        .skip(1)
        .cloned()
        .collect();
    for (family, name) in [
        (medium(), "inter-medium"),
        (tab_weight(), "inter-tab"),
        (semibold(), "inter-semibold"),
        (strong(), "inter-strong"),
        (bold(), "inter-bold"),
    ] {
        let mut list = vec![name.to_string()];
        list.extend(rest.iter().cloned());
        fonts.families.insert(family, list);
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // Icons in every weight too.
    if let Some(icons) = fonts.families[&FontFamily::Proportional]
        .iter()
        .find(|f| f.contains("phosphor"))
        .cloned()
    {
        for family in [medium(), tab_weight(), semibold(), strong(), bold()] {
            fonts.families.get_mut(&family).unwrap().push(icons.clone());
        }
    }
    ctx.set_fonts(fonts);

    let mut v = Visuals::dark();
    v.panel_fill = PANEL_BG;
    v.window_fill = PANEL_BG_2;
    v.extreme_bg_color = FIELD_BG;
    v.faint_bg_color = PANEL_BG_2;
    v.code_bg_color = FIELD_BG;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = ACCENT_DIM;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.override_text_color = Some(TEXT);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL_BG;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    w.inactive.bg_fill = PANEL_BG_3;
    w.inactive.weak_bg_fill = PANEL_BG_3;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.bg_fill = Color32::from_rgb(0x32, 0x34, 0x3a);
    w.hovered.weak_bg_fill = Color32::from_rgb(0x32, 0x34, 0x3a);
    w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x46, 0x49, 0x51));
    w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.bg_fill = ACCENT_DIM;
    w.active.weak_bg_fill = ACCENT_DIM;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.open.bg_fill = PANEL_BG_3;
    w.open.weak_bg_fill = PANEL_BG_3;
    for s in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        s.corner_radius = CornerRadius::same(SMALL_RADIUS);
    }
    ctx.set_visuals(v);
    ctx.global_style_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
            (TextStyle::Heading, FontId::new(17.0, semibold())),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        // Also sets the size of slider handles (a fifth of this high, so 18 gives a slim one).
        style.spacing.interact_size = egui::vec2(40.0, 18.0);
        style.spacing.slider_width = 160.0;
        style.spacing.combo_width = 160.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.window_margin = egui::Margin::same(16);
        style.spacing.slider_rail_height = 3.0;
        style.spacing.icon_width = 16.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.interaction.tooltip_delay = 0.35;
    });
}

/// Primary call-to-action button, for Import and Export.
pub fn accent_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.to_string())
            .color(ACCENT_TEXT)
            .font(heading_font(13.5)),
    )
    .fill(ACCENT)
    .corner_radius(CornerRadius::same(SMALL_RADIUS))
}

/// A quiet secondary button with an outline.
pub fn outline_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(SMALL_RADIUS))
}
