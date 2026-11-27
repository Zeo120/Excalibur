use egui::{Color32, CornerRadius, Stroke};

pub const BG_CANVAS: Color32 = Color32::from_rgb(9, 9, 13);
pub const BG_TAB_BAR: Color32 = Color32::from_rgb(12, 12, 17);
pub const BG_TOOLBAR: Color32 = Color32::from_rgb(17, 17, 24);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(26, 27, 36);
pub const BG_HOVER: Color32 = Color32::from_rgb(36, 38, 50);
pub const BG_FIELD: Color32 = Color32::from_rgb(7, 7, 10);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 20);
pub const BORDER_STRONG: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 42);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 236, 241);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(148, 148, 163);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(94, 94, 110);

pub const ACCENT: Color32 = Color32::from_rgb(56, 189, 248);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(14, 48, 66, 180);
pub const STATUS_SUCCESS: Color32 = Color32::from_rgb(61, 214, 140);
pub const STATUS_WARNING: Color32 = Color32::from_rgb(229, 169, 60);
pub const STATUS_DANGER: Color32 = Color32::from_rgb(240, 82, 82);

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.visuals = egui::Visuals::dark();

    let v = &mut style.visuals;
    v.panel_fill = BG_TOOLBAR;
    v.window_fill = BG_ELEVATED;
    v.extreme_bg_color = BG_FIELD;
    v.faint_bg_color = BG_TAB_BAR;
    v.override_text_color = Some(TEXT_PRIMARY);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let radius = CornerRadius::same(4);
    v.widgets.noninteractive.bg_fill = BG_TOOLBAR;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = BG_ELEVATED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.hovered.corner_radius = radius;

    v.widgets.active.bg_fill = ACCENT_SOFT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.active.corner_radius = radius;

    ctx.set_global_style(style);
}
