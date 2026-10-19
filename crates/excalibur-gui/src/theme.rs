use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::Arc;

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

    // Stock Chrome Dark Mode & Neutral Black palette
    visuals.panel_fill = Color32::from_rgb(18, 18, 18); // #121212 Chrome top frame & tab strip
    visuals.window_fill = Color32::from_rgb(32, 33, 36); // #202124 Chrome toolbar, cards, menus
    visuals.extreme_bg_color = Color32::from_rgb(0, 0, 0); // #000000 Pure black webpage canvas

    // Refined subtle borders (1px) with crisp Chrome contrast
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 33, 36);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 51, 56));
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(232, 234, 237));
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 42, 45); // #282a2d
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(60, 64, 67));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(218, 220, 224));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 56, 61);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(95, 99, 104));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(66, 70, 76);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(138, 180, 248)); // Chrome Soft Blue
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = Color32::from_rgb(45, 48, 52);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(138, 180, 248));
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(37, 57, 88);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(138, 180, 248));

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
            FontId::new(13.5, FontFamily::Proportional),
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
