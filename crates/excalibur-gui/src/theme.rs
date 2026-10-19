//! Runtime theme engine.
//!
//! A [`ThemePreset`] describes a handful of anchor colors (window gradient, page
//! canvas, accent). [`Palette::from_prefs`] derives every semantic UI color from
//! those anchors, and [`apply`] installs the palette globally so that all views
//! read it through the [`colors`] accessor functions.

use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::{Arc, RwLock};

// ───────────────────────────────── Presets ─────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct ThemePreset {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub chrome_top: Color32,
    pub chrome_bottom: Color32,
    pub canvas: Color32,
    pub accent: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const PRESETS: [ThemePreset; 9] = [
    ThemePreset {
        id: "obsidian",
        name: "Obsidian",
        dark: true,
        chrome_top: hex(0x1C1B20),
        chrome_bottom: hex(0x141317),
        canvas: hex(0x202024),
        accent: hex(0xF4A87C),
    },
    ThemePreset {
        id: "porcelain",
        name: "Porcelain",
        dark: false,
        chrome_top: hex(0xEBE8EE),
        chrome_bottom: hex(0xDDDAE3),
        canvas: hex(0xFFFFFF),
        accent: hex(0xDD6B36),
    },
    ThemePreset {
        id: "fjord",
        name: "Fjord",
        dark: true,
        chrome_top: hex(0x212A38),
        chrome_bottom: hex(0x171E29),
        canvas: hex(0x252E3B),
        accent: hex(0x88C0D0),
    },
    ThemePreset {
        id: "moss",
        name: "Moss",
        dark: true,
        chrome_top: hex(0x1B241E),
        chrome_bottom: hex(0x121914),
        canvas: hex(0x1E2822),
        accent: hex(0x8FD3A7),
    },
    ThemePreset {
        id: "dusk",
        name: "Dusk Rose",
        dark: true,
        chrome_top: hex(0x281D26),
        chrome_bottom: hex(0x1A1419),
        canvas: hex(0x2A2029),
        accent: hex(0xF2A1BD),
    },
    ThemePreset {
        id: "dune",
        name: "Dune",
        dark: false,
        chrome_top: hex(0xEFE7D9),
        chrome_bottom: hex(0xE2D6C2),
        canvas: hex(0xFFFCF6),
        accent: hex(0xB0702F),
    },
    ThemePreset {
        id: "glacier",
        name: "Glacier",
        dark: false,
        chrome_top: hex(0xDFE7F1),
        chrome_bottom: hex(0xCCD8E7),
        canvas: hex(0xFFFFFF),
        accent: hex(0x2F74E0),
    },
    ThemePreset {
        id: "excalibur",
        name: "Excalibur",
        dark: true,
        chrome_top: hex(0x16171A),
        chrome_bottom: hex(0x0E0F11),
        canvas: hex(0x191A1D),
        accent: hex(0xD9AE5B),
    },
    ThemePreset {
        id: "void",
        name: "Void",
        dark: true,
        chrome_top: hex(0x000000),
        chrome_bottom: hex(0x000000),
        canvas: hex(0x0B0B0C),
        accent: hex(0xA78BFA),
    },
];

/// Accent overrides offered in Appearance settings.
pub const ACCENT_SWATCHES: [(&str, Color32); 8] = [
    ("Peach", hex(0xF4A87C)),
    ("Coral", hex(0xF07167)),
    ("Gold", hex(0xD9AE5B)),
    ("Mint", hex(0x7FD1A5)),
    ("Teal", hex(0x5CC8C8)),
    ("Sky", hex(0x6AA8FF)),
    ("Violet", hex(0xA78BFA)),
    ("Pink", hex(0xF29BD0)),
];

pub fn preset_by_id(id: &str) -> &'static ThemePreset {
    PRESETS.iter().find(|p| p.id == id).unwrap_or(&PRESETS[0])
}

// ─────────────────────────────── Preferences ───────────────────────────────

/// User-facing appearance preferences, persisted as browser settings.
#[derive(Clone, Debug, PartialEq)]
pub struct UiPrefs {
    pub theme_id: String,
    pub accent: Option<Color32>,
    pub gradient: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme_id: PRESETS[0].id.to_string(),
            accent: None,
            gradient: true,
        }
    }
}

impl UiPrefs {
    pub fn load(storage: &excalibur_core::WorkspaceStorage) -> Self {
        let get = |k: &str| storage.get_browser_setting(k).ok().flatten();
        let mut prefs = Self::default();
        if let Some(id) = get("theme_preset") {
            prefs.theme_id = preset_by_id(&id).id.to_string();
        }
        prefs.accent = get("theme_accent").and_then(|s| parse_hex(&s));
        prefs.gradient = get("theme_gradient").as_deref() != Some("off");
        prefs
    }

    pub fn save(&self, storage: &excalibur_core::WorkspaceStorage) {
        let _ = storage.set_browser_setting("theme_preset", &self.theme_id);
        let accent = self.accent.map(to_hex).unwrap_or_else(|| "default".into());
        let _ = storage.set_browser_setting("theme_accent", &accent);
        let _ =
            storage.set_browser_setting("theme_gradient", if self.gradient { "on" } else { "off" });
    }
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    u32::from_str_radix(s, 16).ok().map(hex)
}

// ───────────────────────────────── Palette ─────────────────────────────────

/// Every semantic color the UI uses, derived from a preset + preferences.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub chrome_top: Color32,
    pub chrome_bottom: Color32,
    pub chrome_hover: Color32,
    pub chrome_active: Color32,
    pub canvas: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub link: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub shadow: Color32,
}

/// Linear blend of two colors in gamma space; `t = 0` gives `a`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

impl Palette {
    pub fn from_prefs(prefs: &UiPrefs) -> Self {
        let p = preset_by_id(&prefs.theme_id);
        let dark = p.dark;
        let accent = prefs.accent.unwrap_or(p.accent);
        let canvas = p.canvas;
        let text = if dark { hex(0xECEBEF) } else { hex(0x1D1C21) };
        let chrome_bottom = if prefs.gradient {
            p.chrome_bottom
        } else {
            p.chrome_top
        };

        // Light themes need a deeper accent to keep text contrast on white
        let accent_ink = if dark {
            accent
        } else {
            mix(accent, hex(0x000000), 0.12)
        };

        let (success, warning, danger) = if dark {
            (hex(0x5CCB94), hex(0xE7B04A), hex(0xF26468))
        } else {
            (hex(0x1E9A5F), hex(0xB7791F), hex(0xD23B41))
        };

        Self {
            dark,
            chrome_top: p.chrome_top,
            chrome_bottom,
            chrome_hover: if dark {
                Color32::from_white_alpha(14)
            } else {
                Color32::from_black_alpha(14)
            },
            chrome_active: if dark {
                Color32::from_white_alpha(24)
            } else {
                Color32::from_white_alpha(190)
            },
            canvas,
            panel: mix(canvas, text, if dark { 0.03 } else { 0.025 }),
            card: mix(canvas, text, if dark { 0.06 } else { 0.035 }),
            field: if dark {
                mix(canvas, hex(0x000000), 0.28)
            } else {
                mix(canvas, text, 0.045)
            },
            hover: mix(canvas, text, if dark { 0.10 } else { 0.07 }),
            border: mix(canvas, text, if dark { 0.11 } else { 0.12 }),
            border_strong: mix(canvas, text, if dark { 0.20 } else { 0.22 }),
            text,
            text_dim: mix(text, canvas, 0.33),
            text_faint: mix(text, canvas, 0.52),
            accent: accent_ink,
            accent_hover: mix(
                accent_ink,
                if dark { hex(0xFFFFFF) } else { hex(0x000000) },
                0.15,
            ),
            accent_soft: mix(canvas, accent, if dark { 0.18 } else { 0.14 }),
            on_accent: if luminance(accent_ink) > 0.55 {
                hex(0x1A1A1A)
            } else {
                hex(0xFFFFFF)
            },
            link: if dark { hex(0x8AB4F8) } else { hex(0x1A5FD0) },
            success,
            success_soft: mix(canvas, success, 0.16),
            warning,
            danger,
            danger_soft: mix(canvas, danger, 0.16),
            shadow: Color32::from_black_alpha(if dark { 120 } else { 40 }),
        }
    }
}

static PALETTE: RwLock<Option<Palette>> = RwLock::new(None);

/// The palette currently in effect (falls back to the default preset).
pub fn palette() -> Palette {
    if let Ok(guard) = PALETTE.read() {
        if let Some(p) = *guard {
            return p;
        }
    }
    Palette::from_prefs(&UiPrefs::default())
}

/// Semantic color accessors used by every view.
pub mod colors {
    use super::palette;
    use egui::Color32;

    macro_rules! accessor {
        ($($name:ident),* $(,)?) => {
            $(#[inline] pub fn $name() -> Color32 { palette().$name })*
        };
    }

    accessor!(
        chrome_top,
        chrome_bottom,
        chrome_hover,
        chrome_active,
        canvas,
        panel,
        card,
        field,
        hover,
        border,
        border_strong,
        text,
        text_dim,
        text_faint,
        accent,
        accent_hover,
        accent_soft,
        on_accent,
        link,
        success,
        success_soft,
        warning,
        danger,
        danger_soft,
        shadow,
    );

    pub fn youtube_red() -> Color32 {
        Color32::from_rgb(255, 0, 0)
    }
}

// ───────────────────────────── egui integration ─────────────────────────────

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let candidates: &[(&str, &str, FontFamily)] = &[
        (
            "SegoeUI",
            "C:\\Windows\\Fonts\\segoeui.ttf",
            FontFamily::Proportional,
        ),
        (
            "Consolas",
            "C:\\Windows\\Fonts\\consola.ttf",
            FontFamily::Monospace,
        ),
        (
            "Inter",
            "/usr/share/fonts/TTF/Inter-Regular.ttf",
            FontFamily::Proportional,
        ),
        (
            "NotoSans",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            FontFamily::Proportional,
        ),
        (
            "SFNS",
            "/System/Library/Fonts/SFNS.ttf",
            FontFamily::Proportional,
        ),
    ];
    let mut installed = std::collections::HashSet::new();
    for (name, path, family) in candidates {
        if installed.contains(family) {
            continue;
        }
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_string(), Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .entry(family.clone())
                .or_default()
                .insert(0, name.to_string());
            installed.insert(family.clone());
        }
    }

    ctx.set_fonts(fonts);
}

/// Installs `prefs` as the active palette and restyles egui to match.
pub fn apply(ctx: &egui::Context, prefs: &UiPrefs) {
    let pal = Palette::from_prefs(prefs);
    if let Ok(mut guard) = PALETTE.write() {
        *guard = Some(pal);
    }

    let mut v = if pal.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = pal.canvas;
    v.window_fill = pal.card;
    v.window_stroke = Stroke::new(1.0_f32, pal.border);
    v.extreme_bg_color = pal.field;
    v.faint_bg_color = pal.panel;
    v.code_bg_color = pal.field;
    v.hyperlink_color = pal.link;
    v.warn_fg_color = pal.warning;
    v.error_fg_color = pal.danger;
    v.override_text_color = None;
    v.window_shadow = egui::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: pal.shadow,
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: pal.shadow,
    };

    let r = egui::CornerRadius::same(7);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = pal.panel;
    w.noninteractive.weak_bg_fill = pal.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.noninteractive.corner_radius = r;

    w.inactive.bg_fill = pal.card;
    w.inactive.weak_bg_fill = pal.card;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text_dim);
    w.inactive.corner_radius = r;

    w.hovered.bg_fill = pal.hover;
    w.hovered.weak_bg_fill = pal.hover;
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = pal.accent_soft;
    w.active.weak_bg_fill = pal.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0_f32, pal.accent);
    w.active.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;

    w.open.bg_fill = pal.card;
    w.open.weak_bg_fill = pal.card;
    w.open.bg_stroke = Stroke::new(1.0_f32, pal.border_strong);
    w.open.fg_stroke = Stroke::new(1.0_f32, pal.text);
    w.open.corner_radius = r;

    v.selection.bg_fill = pal.accent_soft;
    v.selection.stroke = Stroke::new(1.0_f32, pal.accent);
    v.text_cursor.stroke = Stroke::new(1.5_f32, pal.accent);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.slider_trailing_fill = true;

    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.interact_size.y = 26.0;
    style.spacing.scroll = egui::style::ScrollStyle::floating();
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
            FontId::new(21.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style(style);
}
