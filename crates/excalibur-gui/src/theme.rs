use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

pub mod colors {
    use egui::Color32;

    pub const CANVAS_BG: Color32 = Color32::from_rgb(0, 0, 0); // #000000 Pure Black Canvas
    pub const SIDEBAR_BG: Color32 = Color32::from_rgb(17, 19, 24); // #111318 Deep Charcoal Frame
    pub const TOOLBAR_BG: Color32 = Color32::from_rgb(25, 28, 35); // #191C23 Elevated Toolbar Surfaces
    pub const CARD_BG: Color32 = Color32::from_rgb(32, 37, 48); // #202530 Elevated Cards & Panels
    pub const INPUT_BG: Color32 = Color32::from_rgb(22, 25, 32); // #161920 Omnibox & Input Fill
    pub const BORDER: Color32 = Color32::from_rgb(38, 42, 51); // #262A33 Low-contrast subtle borders
    pub const BORDER_LIGHT: Color32 = Color32::from_rgb(46, 51, 61); // #2E333D Hover & Elevated borders
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 237, 243); // #E6EDF3 Soft White Primary
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(139, 148, 158); // #8B949E Muted Slate Secondary
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(110, 118, 129); // #6E7681 Micro Muted Text
    pub const ACCENT: Color32 = Color32::from_rgb(88, 166, 255); // #58A6FF Restrained Electric Blue
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(121, 192, 255); // #79C0FF Bright Blue Hover
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

    // Premium Dark Edition Palette: Pure Black & Deep Charcoal
    visuals.panel_fill = colors::SIDEBAR_BG;
    visuals.window_fill = colors::TOOLBAR_BG;
    visuals.extreme_bg_color = colors::CANVAS_BG;

    // Refined subtle 1px borders with low-contrast clarity
    visuals.widgets.noninteractive.bg_fill = colors::TOOLBAR_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_PRIMARY);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = colors::INPUT_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_SECONDARY);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(37, 43, 54);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors::BORDER_LIGHT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(45, 52, 66);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, colors::ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = Color32::from_rgb(32, 37, 48);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, colors::ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(30, 48, 77);
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
