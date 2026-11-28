use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

pub const BG_SHELL: Color32 = Color32::from_rgb(20, 20, 23);
pub const BG_SURFACE: Color32 = Color32::from_rgb(28, 28, 33);
pub const BG_PILL: Color32 = Color32::from_rgb(35, 35, 41);
pub const BG_HOVER: Color32 = Color32::from_rgb(46, 46, 54);
pub const BG_FIELD: Color32 = Color32::from_rgb(15, 15, 18);

pub const BORDER_HAIRLINE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 16);
pub const BORDER_STRONG: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 38);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(242, 242, 245);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(161, 161, 174);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(113, 113, 126);

pub const ACCENT: Color32 = Color32::from_rgb(235, 123, 84);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(52, 29, 22, 210);
pub const STATUS_SUCCESS: Color32 = Color32::from_rgb(74, 212, 146);
pub const STATUS_WARNING: Color32 = Color32::from_rgb(235, 179, 71);
pub const STATUS_DANGER: Color32 = Color32::from_rgb(239, 95, 95);

pub fn apply_theme(ctx: &egui::Context) {
    install_system_fonts(ctx);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    style.visuals = egui::Visuals::dark();

    let v = &mut style.visuals;
    v.panel_fill = BG_SHELL;
    v.window_fill = BG_SURFACE;
    v.extreme_bg_color = BG_FIELD;
    v.faint_bg_color = BG_SURFACE;
    v.override_text_color = Some(TEXT_PRIMARY);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let radius = CornerRadius::same(7);
    v.widgets.noninteractive.bg_fill = BG_SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_HAIRLINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.hovered.corner_radius = radius;

    v.widgets.active.bg_fill = ACCENT_SOFT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.active.corner_radius = radius;

    ctx.set_global_style(style);
}

fn install_system_fonts(ctx: &egui::Context) {
    #[cfg(target_os = "windows")]
    {
        let mut fonts = FontDefinitions::default();
        let mut loaded_any = false;

        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe_ui".to_owned(), Arc::new(FontData::from_owned(bytes)));
            if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
                list.insert(0, "segoe_ui".to_owned());
            }
            loaded_any = true;
        }
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
            fonts
                .font_data
                .insert("consolas".to_owned(), Arc::new(FontData::from_owned(bytes)));
            if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
                list.insert(0, "consolas".to_owned());
            }
            loaded_any = true;
        }

        if loaded_any {
            ctx.set_fonts(fonts);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = ctx;
}
