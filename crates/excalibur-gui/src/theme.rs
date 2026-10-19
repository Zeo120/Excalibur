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

    // Elegant Minimalist Obsidian & Slate palette
    visuals.panel_fill = Color32::from_rgb(11, 15, 23); // Primary panels, toolbars, sidebars
    visuals.window_fill = Color32::from_rgb(15, 20, 31); // Elevated cards and modal surfaces
    visuals.extreme_bg_color = Color32::from_rgb(7, 10, 16); // Deep viewport / code canvas

    // Refined subtle borders (1px) with crisp contrast
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 20, 31);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59));
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(226, 232, 240));
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 27, 42);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(37, 51, 74));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(28, 38, 59);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = Color32::from_rgb(24, 33, 51);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(30, 58, 138);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));

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
