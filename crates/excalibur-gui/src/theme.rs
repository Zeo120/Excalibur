use egui::{Color32, Stroke, Visuals};

pub fn apply_cyber_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    // Dark obsidian / slate backgrounds
    visuals.panel_fill = Color32::from_rgb(13, 17, 23); // #0d1117
    visuals.window_fill = Color32::from_rgb(22, 27, 34); // #161b22
    visuals.extreme_bg_color = Color32::from_rgb(1, 4, 9); // #010409

    // Widgets styling
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 27, 34);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61));
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(201, 209, 217));

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(33, 38, 45);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(201, 209, 217));

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 61);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals.widgets.active.bg_fill = Color32::from_rgb(31, 111, 235);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    visuals.selection.bg_fill = Color32::from_rgb(31, 111, 235);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(88, 166, 255));

    ctx.set_visuals(visuals);
}
