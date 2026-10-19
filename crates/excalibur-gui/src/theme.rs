use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

pub mod colors {
    use egui::Color32;

    pub const CANVAS_BG: Color32 = Color32::from_rgb(17, 19, 24); // #111318 Main Dark Background
    pub const SIDEBAR_BG: Color32 = Color32::from_rgb(17, 19, 24); // #111318 Deep Charcoal Frame
    pub const TOOLBAR_BG: Color32 = Color32::from_rgb(25, 28, 35); // #191C23 Elevated Toolbar Surfaces
    pub const CARD_BG: Color32 = Color32::from_rgb(32, 37, 48); // #202530 Elevated Surfaces & Cards
    pub const INPUT_BG: Color32 = Color32::from_rgb(22, 25, 32); // #161920 Omnibox & Input Fill
    pub const BORDER: Color32 = Color32::from_rgb(48, 54, 66); // #303642 Subtle Low-contrast Borders
    pub const BORDER_LIGHT: Color32 = Color32::from_rgb(61, 68, 82); // #3D4452 Hover & Elevated Borders
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 234, 240); // #E6EAF0 Primary Soft White
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(154, 164, 178); // #9AA4B2 Secondary Slate
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(107, 118, 133); // #6B7685 Muted Caption Text
    pub const ACCENT: Color32 = Color32::from_rgb(101, 169, 255); // #65A9FF Restrained Accent Blue
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(133, 189, 255); // #85BDFF Accent Hover Bright
    pub const SUCCESS: Color32 = Color32::from_rgb(63, 185, 80); // #3FB950 Secure Green
    pub const DANGER: Color32 = Color32::from_rgb(248, 81, 73); // #F85149 Insecure / Danger Red
    pub const YOUTUBE_RED: Color32 = Color32::from_rgb(255, 0, 0); // #FF0000 Player Red
}

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        // 1. Proportional modern UI font: Segoe UI
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts.font_data.insert(
                "SegoeUI".to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "SegoeUI".to_owned());
        }

        // 2. Monospace developer font: Consolas
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\consola.ttf") {
            fonts.font_data.insert(
                "Consolas".to_owned(),
                Arc::new(egui::FontData::from_owned(data)),
            );
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "Consolas".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

pub fn apply_cyber_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Coordinated Dark Theme Surfaces
    visuals.panel_fill = colors::CANVAS_BG;
    visuals.window_fill = colors::TOOLBAR_BG;
    visuals.extreme_bg_color = colors::INPUT_BG;

    // Standardized widget states and 6px corner radii
    visuals.widgets.noninteractive.bg_fill = colors::TOOLBAR_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_PRIMARY);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = colors::INPUT_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_SECONDARY);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = colors::CARD_BG;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors::BORDER_LIGHT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(40, 48, 64);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, colors::ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = colors::CARD_BG;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, colors::ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(26, 42, 68);
    visuals.selection.stroke = Stroke::new(1.0_f32, colors::ACCENT);

    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(8);

    // Modern typography hierarchy with crisp font metrics
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style(style);
}
