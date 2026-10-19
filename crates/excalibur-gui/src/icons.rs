//! Vector icons painted with egui shapes on a 16-unit design grid, so the UI never
//! depends on glyph coverage of the loaded fonts.

use crate::theme::{colors, mix, palette};
use egui::{pos2, vec2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Back,
    Forward,
    Reload,
    Close,
    Home,
    Star,
    StarFilled,
    Menu,
    Plus,
    Minus,
    Lock,
    Warning,
    Sidebar,
    Search,
    Radar,
    Network,
    Maximize,
    SplitBottom,
    Gear,
    Palette,
    Shield,
    Pin,
    Clock,
    Trash,
    Copy,
    Send,
    Code,
    Globe,
    Check,
    ChevronRight,
    Info,
}

/// Paints `icon` centered in `rect` with the given color.
pub fn paint_icon(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let s = rect.width().min(rect.height()) / 16.0;
    let stroke = Stroke::new((1.5 * s).max(1.1), color);
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize| -> Vec<Pos2> {
        (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                at(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };

    match icon {
        Icon::Back | Icon::Forward => {
            let d = if icon == Icon::Back { -1.0 } else { 1.0 };
            p.line_segment([at(-5.0 * d, 0.0), at(5.0 * d, 0.0)], stroke);
            p.add(Shape::line(
                vec![at(0.5 * d, -4.5), at(5.0 * d, 0.0), at(0.5 * d, 4.5)],
                stroke,
            ));
        }
        Icon::Reload => {
            p.add(Shape::line(
                arc(0.0, 0.0, 5.0, -PI * 0.3, PI * 1.35, 22),
                stroke,
            ));
            let tip = at(5.0 * (-PI * 0.3).cos(), 5.0 * (-PI * 0.3).sin());
            p.add(Shape::convex_polygon(
                vec![
                    tip + vec2(-3.4, -1.0) * s,
                    tip + vec2(1.4, -2.8) * s,
                    tip + vec2(0.8, 2.4) * s,
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Close => {
            p.line_segment([at(-4.0, -4.0), at(4.0, 4.0)], stroke);
            p.line_segment([at(4.0, -4.0), at(-4.0, 4.0)], stroke);
        }
        Icon::Home => {
            p.add(Shape::line(
                vec![at(-6.0, -0.5), at(0.0, -6.0), at(6.0, -0.5)],
                stroke,
            ));
            p.add(Shape::line(
                vec![at(-4.0, -2.0), at(-4.0, 5.5), at(4.0, 5.5), at(4.0, -2.0)],
                stroke,
            ));
        }
        Icon::Star | Icon::StarFilled => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 6.2 } else { 2.7 };
                    let a = -PI / 2.0 + i as f32 * PI / 5.0;
                    at(r * a.cos(), r * a.sin() + 0.6)
                })
                .collect();
            if icon == Icon::StarFilled {
                for i in 0..10 {
                    p.add(Shape::convex_polygon(
                        vec![at(0.0, 0.6), pts[i], pts[(i + 1) % 10]],
                        color,
                        Stroke::NONE,
                    ));
                }
            }
            p.add(Shape::closed_line(pts, stroke));
        }
        Icon::Menu => {
            for x in [-5.0, 0.0, 5.0] {
                p.circle_filled(at(x, 0.0), 1.4 * s, color);
            }
        }
        Icon::Plus => {
            p.line_segment([at(-5.5, 0.0), at(5.5, 0.0)], stroke);
            p.line_segment([at(0.0, -5.5), at(0.0, 5.5)], stroke);
        }
        Icon::Minus => {
            p.line_segment([at(-5.5, 0.0), at(5.5, 0.0)], stroke);
        }
        Icon::Lock => {
            p.rect_filled(
                Rect::from_min_max(at(-4.5, -1.0), at(4.5, 5.5)),
                1.5 * s,
                color,
            );
            let mut shackle = vec![at(-3.0, -1.0)];
            shackle.extend(arc(0.0, -2.5, 3.0, PI, TAU, 12));
            shackle.push(at(3.0, -1.0));
            p.add(Shape::line(shackle, stroke));
        }
        Icon::Warning => {
            p.add(Shape::closed_line(
                vec![at(0.0, -6.0), at(6.5, 5.5), at(-6.5, 5.5)],
                stroke,
            ));
            p.line_segment([at(0.0, -2.0), at(0.0, 1.5)], stroke);
            p.circle_filled(at(0.0, 3.6), 0.9 * s, color);
        }
        Icon::Sidebar => {
            let r = Rect::from_min_max(at(-6.5, -5.5), at(6.5, 5.5));
            p.rect_stroke(r, 2.5 * s, stroke, egui::StrokeKind::Middle);
            p.line_segment([at(-2.0, -5.5), at(-2.0, 5.5)], stroke);
        }
        Icon::Search => {
            p.circle_stroke(at(-1.0, -1.0), 4.6 * s, stroke);
            p.line_segment([at(2.4, 2.4), at(6.0, 6.0)], stroke);
        }
        Icon::Radar => {
            p.circle_stroke(at(0.0, 0.0), 6.0 * s, stroke);
            p.circle_stroke(at(0.0, 0.0), 2.8 * s, stroke);
            p.line_segment([at(0.0, 0.0), at(4.3, -4.3)], stroke);
        }
        Icon::Network => {
            for (x, y) in [(0.0, -4.5), (-5.0, 4.5), (5.0, 4.5)] {
                p.circle_filled(at(x, y), 1.9 * s, color);
            }
            p.line_segment([at(0.0, -4.5), at(-5.0, 4.5)], stroke);
            p.line_segment([at(0.0, -4.5), at(5.0, 4.5)], stroke);
        }
        Icon::Maximize => {
            p.add(Shape::line(
                vec![at(1.5, -6.0), at(6.0, -6.0), at(6.0, -1.5)],
                stroke,
            ));
            p.add(Shape::line(
                vec![at(-1.5, 6.0), at(-6.0, 6.0), at(-6.0, 1.5)],
                stroke,
            ));
            p.line_segment([at(6.0, -6.0), at(1.5, -1.5)], stroke);
            p.line_segment([at(-6.0, 6.0), at(-1.5, 1.5)], stroke);
        }
        Icon::SplitBottom => {
            let r = Rect::from_min_max(at(-6.0, -5.0), at(6.0, 5.0));
            p.rect_stroke(r, 1.5 * s, stroke, egui::StrokeKind::Middle);
            p.rect_filled(
                Rect::from_min_max(at(-6.0, 1.0), at(6.0, 5.0)),
                1.5 * s,
                color,
            );
        }
        Icon::Gear => {
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                p.line_segment(
                    [
                        at(4.2 * a.cos(), 4.2 * a.sin()),
                        at(6.3 * a.cos(), 6.3 * a.sin()),
                    ],
                    Stroke::new(stroke.width * 1.5, color),
                );
            }
            p.circle_stroke(at(0.0, 0.0), 4.3 * s, stroke);
            p.circle_stroke(at(0.0, 0.0), 1.7 * s, stroke);
        }
        Icon::Palette => {
            p.add(Shape::line(
                arc(0.0, 0.0, 6.0, PI * 0.35, PI * 2.1, 26),
                stroke,
            ));
            p.add(Shape::line(
                vec![at(4.6, 3.9), at(2.2, 3.0), at(1.4, 5.0), at(2.0, 5.9)],
                stroke,
            ));
            for (x, y) in [(-3.0, -2.0), (0.5, -3.6), (3.4, -1.0), (-3.0, 2.0)] {
                p.circle_filled(at(x, y), 1.2 * s, color);
            }
        }
        Icon::Shield => {
            let pts = vec![
                at(0.0, -6.5),
                at(5.5, -4.2),
                at(5.0, 1.5),
                at(0.0, 6.5),
                at(-5.0, 1.5),
                at(-5.5, -4.2),
            ];
            p.add(Shape::closed_line(pts, stroke));
            p.add(Shape::line(
                vec![at(-2.4, 0.0), at(-0.4, 2.0), at(2.8, -1.8)],
                stroke,
            ));
        }
        Icon::Pin => {
            p.add(Shape::closed_line(
                vec![
                    at(-3.0, -6.0),
                    at(3.0, -6.0),
                    at(2.0, -1.0),
                    at(4.5, 1.5),
                    at(-4.5, 1.5),
                    at(-2.0, -1.0),
                ],
                stroke,
            ));
            p.line_segment([at(0.0, 1.5), at(0.0, 6.5)], stroke);
        }
        Icon::Clock => {
            p.circle_stroke(at(0.0, 0.0), 6.0 * s, stroke);
            p.add(Shape::line(
                vec![at(0.0, -3.5), at(0.0, 0.0), at(2.8, 1.8)],
                stroke,
            ));
        }
        Icon::Trash => {
            p.line_segment([at(-6.0, -4.0), at(6.0, -4.0)], stroke);
            p.add(Shape::line(
                vec![at(-2.0, -4.0), at(-2.0, -6.0), at(2.0, -6.0), at(2.0, -4.0)],
                stroke,
            ));
            p.add(Shape::line(
                vec![at(-4.5, -4.0), at(-3.8, 6.0), at(3.8, 6.0), at(4.5, -4.0)],
                stroke,
            ));
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(at(-2.5, -2.5), at(6.0, 6.0)),
                1.5 * s,
                stroke,
                egui::StrokeKind::Middle,
            );
            p.add(Shape::line(
                vec![
                    at(-2.5, 3.0),
                    at(-6.0, 3.0),
                    at(-6.0, -6.0),
                    at(3.0, -6.0),
                    at(3.0, -2.5),
                ],
                stroke,
            ));
        }
        Icon::Send => {
            p.add(Shape::closed_line(
                vec![at(-6.0, -5.5), at(6.5, 0.0), at(-6.0, 5.5), at(-3.5, 0.0)],
                stroke,
            ));
            p.line_segment([at(-3.5, 0.0), at(2.5, 0.0)], stroke);
        }
        Icon::Code => {
            p.add(Shape::line(
                vec![at(-2.5, -5.0), at(-7.0, 0.0), at(-2.5, 5.0)],
                stroke,
            ));
            p.add(Shape::line(
                vec![at(2.5, -5.0), at(7.0, 0.0), at(2.5, 5.0)],
                stroke,
            ));
        }
        Icon::Globe => {
            p.circle_stroke(at(0.0, 0.0), 6.0 * s, stroke);
            p.line_segment([at(-6.0, 0.0), at(6.0, 0.0)], stroke);
            p.add(Shape::line(arc(-4.2, 0.0, 7.3, -0.95, 0.95, 14), stroke));
            p.add(Shape::line(
                arc(4.2, 0.0, 7.3, PI - 0.95, PI + 0.95, 14),
                stroke,
            ));
        }
        Icon::Check => {
            p.add(Shape::line(
                vec![at(-5.0, 0.5), at(-1.5, 4.0), at(5.5, -4.0)],
                stroke,
            ));
        }
        Icon::ChevronRight => {
            p.add(Shape::line(
                vec![at(-2.0, -5.0), at(3.0, 0.0), at(-2.0, 5.0)],
                stroke,
            ));
        }
        Icon::Info => {
            p.circle_stroke(at(0.0, 0.0), 6.0 * s, stroke);
            p.line_segment([at(0.0, -0.5), at(0.0, 3.8)], stroke);
            p.circle_filled(at(0.0, -3.0), 0.9 * s, color);
        }
    }
}

/// Square borderless button with hover fill. `on_chrome` uses translucent fills
/// that suit the gradient window background rather than the page canvas.
pub fn icon_button_ex(
    ui: &mut Ui,
    icon: Icon,
    enabled: bool,
    tooltip: &str,
    size: f32,
    tint: Option<Color32>,
    on_chrome: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), sense);
    if ui.is_rect_visible(rect) {
        if enabled && resp.hovered() {
            let fill = if on_chrome {
                colors::chrome_hover()
            } else {
                colors::hover()
            };
            ui.painter().rect_filled(rect, size * 0.28, fill);
        }
        let color = match (enabled, tint) {
            (false, _) => colors::text_faint().gamma_multiply(0.55),
            (true, Some(t)) => t,
            (true, None) if resp.hovered() => colors::text(),
            (true, None) => colors::text_dim(),
        };
        let r = Rect::from_center_size(rect.center(), vec2(size * 0.54, size * 0.54));
        paint_icon(ui, r, icon, color);
    }
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

pub fn icon_button(ui: &mut Ui, icon: Icon, enabled: bool, tooltip: &str) -> Response {
    icon_button_ex(ui, icon, enabled, tooltip, 28.0, None, false)
}

pub fn chrome_button(ui: &mut Ui, icon: Icon, enabled: bool, tooltip: &str) -> Response {
    icon_button_ex(ui, icon, enabled, tooltip, 30.0, None, true)
}

// ───────────────────────────── Site identity ─────────────────────────────

/// Host portion of a URL, without a leading `www.`.
pub fn url_host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

pub fn is_internal_url(url: &str) -> bool {
    url.starts_with("excalibur://") || url.starts_with("about:")
}

/// Stable per-site hue so every site gets a recognisable monogram tile.
pub fn site_color(host: &str) -> Color32 {
    let hash = host
        .bytes()
        .fold(2166136261_u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (hash % 360) as f32 / 360.0;
    Color32::from(egui::ecolor::Hsva::new(hue, 0.5, 0.85, 1.0))
}

/// Rounded monogram tile standing in for a favicon.
pub fn paint_site_badge(ui: &Ui, rect: Rect, url: &str) {
    let p = ui.painter();
    let radius = rect.height() * 0.28;
    if is_internal_url(url) {
        p.rect_filled(rect, radius, colors::accent_soft());
        paint_brand_mark(ui, rect.shrink(rect.width() * 0.2), colors::accent());
        return;
    }
    let host = url_host(url);
    let letter = host
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('•');
    let base = site_color(host);
    let (bg, fg) = if palette().dark {
        (mix(colors::canvas(), base, 0.3), base)
    } else {
        (
            mix(colors::canvas(), base, 0.22),
            mix(base, Color32::BLACK, 0.4),
        )
    };
    p.rect_filled(rect, radius, bg);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(rect.height() * 0.6),
        fg,
    );
}

/// The Excalibur mark: an upright blade with a crossguard.
pub fn paint_brand_mark(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let s = rect.height() / 16.0;
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    p.add(Shape::convex_polygon(
        vec![
            at(0.0, -8.0),
            at(1.8, -5.5),
            at(1.8, 3.0),
            at(-1.8, 3.0),
            at(-1.8, -5.5),
        ],
        color,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_min_max(at(-5.5, 3.0), at(5.5, 4.8)),
        0.8 * s,
        color,
    );
    p.rect_filled(Rect::from_min_max(at(-0.9, 4.8), at(0.9, 7.0)), 0.0, color);
    p.circle_filled(at(0.0, 7.6), 1.2 * s, color);
}

/// Rotating arc used while something loads.
pub fn paint_spinner(ui: &Ui, rect: Rect, color: Color32) {
    let t = ui.input(|i| i.time) as f32;
    let c = rect.center();
    let r = rect.width() * 0.4;
    let pts: Vec<Pos2> = (0..=16)
        .map(|i| {
            let a = t * 5.0 + (i as f32 / 16.0) * PI * 1.4;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    ui.painter()
        .add(Shape::line(pts, Stroke::new(1.8_f32, color)));
}

/// Truncates to at most `max_chars` characters, appending an ellipsis.
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
