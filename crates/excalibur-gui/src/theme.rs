use egui::{Color32, Visuals};

pub fn apply_cyber_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(18, 20, 26);
    visuals.window_fill = Color32::from_rgb(24, 27, 34);
    visuals.extreme_bg_color = Color32::from_rgb(12, 14, 18);
    ctx.set_visuals(visuals);
}
