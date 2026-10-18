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

    // Deep Obsidian Black & Rich Midnight Navy backgrounds
    visuals.panel_fill = Color32::from_rgb(6, 9, 15); // Deep Obsidian Black (Top chrome, sidebars, status bar)
    visuals.window_fill = Color32::from_rgb(10, 15, 26); // Midnight Navy Card Fill
    visuals.extreme_bg_color = Color32::from_rgb(3, 5, 8); // Absolute Pitch Black (Code & terminal viewports)

    // Widgets styling: Deep Black & Electric Cobalt / Cyan palette
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(10, 15, 26);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76));
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(201, 215, 235));
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(14, 22, 38);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(30, 52, 92));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(215, 228, 245));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(24, 46, 84);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, Color32::from_rgb(56, 189, 248));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(31, 111, 235);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = Color32::from_rgb(20, 36, 66);
    visuals.widgets.open.bg_stroke = Stroke::new(1.5_f32, Color32::from_rgb(56, 189, 248));
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(25, 75, 165);
    visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(56, 189, 248));

    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(10);

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
