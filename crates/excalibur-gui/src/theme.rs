use egui::{Color32, Stroke, Visuals};

pub fn apply_cyber_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Dark obsidian / slate modern backgrounds
    visuals.panel_fill = Color32::from_rgb(13, 17, 23); // #0d1117 (Chrome bar & top tab strip)
    visuals.window_fill = Color32::from_rgb(22, 27, 34); // #161b22 (Card / panel container)
    visuals.extreme_bg_color = Color32::from_rgb(1, 4, 9); // #010409 (Code / hex inspector views)

    // Widgets styling with smooth corner radii and responsive contrast
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 27, 34);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61));
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(201, 209, 217));
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 33, 40);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(201, 209, 217));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 61);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.active.bg_fill = Color32::from_rgb(31, 111, 235);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    visuals.widgets.open.bg_fill = Color32::from_rgb(33, 38, 45);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgb(31, 111, 235);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));

    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(10);
    ctx.set_style(style);
}
