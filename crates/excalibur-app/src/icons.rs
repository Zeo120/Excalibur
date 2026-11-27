use egui::{pos2, vec2, Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui};

use crate::theme;

#[derive(Debug, Clone, Copy)]
pub enum IconKind {
    Back,
    Forward,
    Reload,
    Stop,
    Plus,
    Close,
    StarOutline,
    StarFilled,
    Shield,
    Sleep,
    Bookmarks,
    History,
    Settings,
}

/// Draws a consistent 26x26px vector icon button with clear hover, active, and disabled states.
pub fn icon_button(
    ui: &mut Ui,
    kind: IconKind,
    enabled: bool,
    selected: bool,
    tint_override: Option<Color32>,
) -> Response {
    let size = vec2(26.0, 26.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let bg = if !enabled {
            Color32::TRANSPARENT
        } else if response.is_pointer_button_down_on() || selected {
            theme::ACCENT_SOFT
        } else if response.hovered() {
            theme::BG_HOVER
        } else {
            Color32::TRANSPARENT
        };

        let border = if selected && enabled {
            Stroke::new(1.0, theme::ACCENT)
        } else if response.hovered() && enabled {
            Stroke::new(1.0, theme::BORDER_STRONG)
        } else {
            Stroke::NONE
        };

        ui.painter().rect(
            rect,
            CornerRadius::same(4),
            bg,
            border,
            egui::StrokeKind::Inside,
        );

        let color = if !enabled {
            theme::TEXT_MUTED
        } else if let Some(c) = tint_override {
            c
        } else if selected {
            theme::ACCENT
        } else if response.hovered() {
            theme::TEXT_PRIMARY
        } else {
            theme::TEXT_SECONDARY
        };

        paint_icon(ui.painter(), rect.shrink(6.0), kind, color);
    }

    response
}

/// Paints a small vector icon inside `rect` (used inside tabs and inline chips).
pub fn paint_icon(painter: &egui::Painter, rect: Rect, kind: IconKind, color: Color32) {
    let c = rect.center();
    let w = rect.width();
    let h = rect.height();
    let r = w.min(h) * 0.45;
    let stroke = Stroke::new(1.5, color);

    match kind {
        IconKind::Back => {
            painter.line_segment([pos2(c.x + r * 0.7, c.y), pos2(c.x - r * 0.7, c.y)], stroke);
            painter.line_segment(
                [pos2(c.x - r * 0.1, c.y - r * 0.6), pos2(c.x - r * 0.7, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - r * 0.1, c.y + r * 0.6), pos2(c.x - r * 0.7, c.y)],
                stroke,
            );
        }
        IconKind::Forward => {
            painter.line_segment([pos2(c.x - r * 0.7, c.y), pos2(c.x + r * 0.7, c.y)], stroke);
            painter.line_segment(
                [pos2(c.x + r * 0.1, c.y - r * 0.6), pos2(c.x + r * 0.7, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.1, c.y + r * 0.6), pos2(c.x + r * 0.7, c.y)],
                stroke,
            );
        }
        IconKind::Reload => {
            let pts: Vec<_> = (0..=14)
                .map(|i| {
                    let t = (i as f32 / 14.0) * std::f32::consts::PI * 1.55 - 0.3;
                    pos2(c.x + r * 0.85 * t.cos(), c.y + r * 0.85 * t.sin())
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            let tip = pos2(
                c.x + r * 0.85 * (-0.3_f32).cos(),
                c.y + r * 0.85 * (-0.3_f32).sin(),
            );
            painter.line_segment([tip, pos2(tip.x - 3.5, tip.y - 1.5)], stroke);
            painter.line_segment([tip, pos2(tip.x - 0.5, tip.y - 4.0)], stroke);
        }
        IconKind::Stop => {
            let d = r * 0.65;
            painter.line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], stroke);
            painter.line_segment([pos2(c.x + d, c.y - d), pos2(c.x - d, c.y + d)], stroke);
        }
        IconKind::Plus => {
            let d = r * 0.75;
            painter.line_segment([pos2(c.x - d, c.y), pos2(c.x + d, c.y)], stroke);
            painter.line_segment([pos2(c.x, c.y - d), pos2(c.x, c.y + d)], stroke);
        }
        IconKind::Close => {
            let d = r * 0.55;
            painter.line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], stroke);
            painter.line_segment([pos2(c.x + d, c.y - d), pos2(c.x - d, c.y + d)], stroke);
        }
        IconKind::StarOutline | IconKind::StarFilled => {
            let mut pts = Vec::with_capacity(10);
            for i in 0..10 {
                let rad = if i % 2 == 0 { r * 0.95 } else { r * 0.42 };
                let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::PI / 5.0;
                pts.push(pos2(c.x + rad * angle.cos(), c.y + rad * angle.sin()));
            }
            if matches!(kind, IconKind::StarFilled) {
                painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
            } else {
                pts.push(pts[0]);
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        IconKind::Shield => {
            let top_y = c.y - r * 0.9;
            let mid_y = c.y + r * 0.1;
            let bot_y = c.y + r * 0.95;
            let side = r * 0.78;
            let pts = vec![
                pos2(c.x, top_y),
                pos2(c.x + side, top_y + r * 0.35),
                pos2(c.x + side * 0.85, mid_y + r * 0.3),
                pos2(c.x, bot_y),
                pos2(c.x - side * 0.85, mid_y + r * 0.3),
                pos2(c.x - side, top_y + r * 0.35),
                pos2(c.x, top_y),
            ];
            painter.add(egui::Shape::line(pts, stroke));
        }
        IconKind::Sleep => {
            // Crescent moon profile drawn via offset arcs
            let pts: Vec<_> = (0..=12)
                .map(|i| {
                    let a = -0.9 + (i as f32 / 12.0) * 3.6;
                    pos2(c.x + r * 0.78 * a.cos(), c.y + r * 0.78 * a.sin())
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            let inner: Vec<_> = (0..=10)
                .map(|i| {
                    let a = -0.5 + (i as f32 / 10.0) * 2.7;
                    pos2(
                        c.x + r * 0.35 + r * 0.55 * a.cos(),
                        c.y - r * 0.15 + r * 0.55 * a.sin(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(inner, stroke));
        }
        IconKind::Bookmarks => {
            let left = c.x - r * 0.6;
            let right = c.x + r * 0.6;
            let top = c.y - r * 0.85;
            let bot = c.y + r * 0.85;
            let pts = vec![
                pos2(left, bot),
                pos2(left, top),
                pos2(right, top),
                pos2(right, bot),
                pos2(c.x, c.y + r * 0.3),
                pos2(left, bot),
            ];
            painter.add(egui::Shape::line(pts, stroke));
        }
        IconKind::History => {
            painter.circle_stroke(c, r * 0.8, stroke);
            painter.line_segment([c, pos2(c.x, c.y - r * 0.45)], stroke);
            painter.line_segment([c, pos2(c.x + r * 0.4, c.y + r * 0.2)], stroke);
        }
        IconKind::Settings => {
            painter.circle_stroke(c, r * 0.42, stroke);
            for i in 0..6 {
                let a = (i as f32) * std::f32::consts::PI / 3.0;
                let p1 = pos2(c.x + r * 0.62 * a.cos(), c.y + r * 0.62 * a.sin());
                let p2 = pos2(c.x + r * 0.92 * a.cos(), c.y + r * 0.92 * a.sin());
                painter.line_segment([p1, p2], stroke);
            }
        }
    }
}
