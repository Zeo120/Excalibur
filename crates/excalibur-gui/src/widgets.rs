//! Shared, theme-aware building blocks for pages and tools.

use crate::icons::{paint_icon, Icon};
use crate::theme::{colors, mix};
use egui::{
    pos2, text::LayoutJob, vec2, Align2, Color32, CornerRadius, FontId, Frame, Galley, Margin,
    Rect, Response, RichText, Sense, Stroke, TextFormat, Ui,
};
use std::sync::Arc;

/// Lays out single-line text truncated with an ellipsis to fit `max_width`.
pub fn single_line(ui: &Ui, text: &str, size: f32, color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = max_width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.painter().layout_job(job)
}

/// Centered, width-capped, vertically scrolling page column.
pub fn page(ui: &mut Ui, id: &str, max_width: f32, add: impl FnOnce(&mut Ui, f32)) {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let avail = ui.available_width();
            let width = (avail - 48.0).clamp(240.0, max_width);
            let side = ((avail - width) / 2.0).max(16.0);
            ui.add_space(36.0);
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    add(ui, width);
                });
            });
            ui.add_space(48.0);
        });
}

/// Large page title with an optional muted subtitle.
pub fn page_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(title)
            .size(28.0)
            .strong()
            .color(colors::text()),
    );
    if !subtitle.is_empty() {
        ui.add_space(2.0);
        ui.label(RichText::new(subtitle).size(13.5).color(colors::text_dim()));
    }
}

/// Small uppercase label used above groups.
pub fn overline(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .strong()
            .color(colors::text_faint()),
    );
}

pub fn card_frame() -> Frame {
    Frame::NONE
        .fill(colors::card())
        .stroke(Stroke::new(1.0_f32, colors::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Titled settings-style card: heading, description, then contents.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    description: &str,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    card(ui, |ui| {
        ui.label(
            RichText::new(title)
                .size(15.0)
                .strong()
                .color(colors::text()),
        );
        if !description.is_empty() {
            ui.label(
                RichText::new(description)
                    .size(12.5)
                    .color(colors::text_dim()),
            );
        }
        ui.add_space(10.0);
        add(ui)
    })
}

/// Filled accent button.
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(13.0),
        colors::on_accent(),
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 28.0, 32.0), Sense::click());
    let fill = if resp.hovered() {
        colors::accent_hover()
    } else {
        colors::accent()
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        colors::on_accent(),
    );
    resp
}

/// Quiet outlined button; `danger` tints it red.
pub fn ghost_button(ui: &mut Ui, label: &str, danger: bool) -> Response {
    let fg = if danger {
        colors::danger()
    } else {
        colors::text()
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, 30.0), Sense::click());
    let fill = match (resp.hovered(), danger) {
        (true, true) => colors::danger_soft(),
        (true, false) => colors::hover(),
        _ => Color32::TRANSPARENT,
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0_f32, colors::border_strong()),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// Small button with a leading icon.
pub fn icon_text_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(12.5),
        colors::text_dim(),
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, colors::hover());
    }
    let col = if resp.hovered() {
        colors::text()
    } else {
        colors::text_dim()
    };
    paint_icon(
        ui,
        Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), vec2(13.0, 13.0)),
        icon,
        col,
    );
    ui.painter().galley(
        pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0),
        galley,
        col,
    );
    resp
}

/// Segmented control. Returns `true` when the selection changed.
pub fn segmented<T: PartialEq + Clone>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::NONE
        .fill(colors::field())
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                for (opt, label) in options {
                    let selected = value == opt;
                    let col = if selected {
                        colors::text()
                    } else {
                        colors::text_dim()
                    };
                    let galley = ui.painter().layout_no_wrap(
                        label.to_string(),
                        FontId::proportional(12.5),
                        col,
                    );
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(galley.size().x + 22.0, 26.0), Sense::click());
                    if selected {
                        ui.painter().rect_filled(rect, 7.0, colors::card());
                        ui.painter().rect_stroke(
                            rect,
                            7.0,
                            Stroke::new(1.0_f32, colors::border()),
                            egui::StrokeKind::Inside,
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 7.0, colors::hover());
                    }
                    ui.painter()
                        .galley(rect.center() - galley.size() / 2.0, galley, col);
                    if resp.clicked() && !selected {
                        *value = opt.clone();
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// iOS-style switch bound to `on`. Returns the response; `changed()` is set on toggle.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let track = mix(colors::border_strong(), colors::accent(), t);
    ui.painter().rect_filled(rect, 10.0, track);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    ui.painter()
        .circle_filled(pos2(x, rect.center().y), 7.5, Color32::WHITE);
    resp
}

/// Labelled row with a trailing control (for settings lists).
pub fn setting_row(ui: &mut Ui, title: &str, hint: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(13.5).color(colors::text()));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(11.5).color(colors::text_faint()));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
}

/// Rounded search/text field with a leading icon.
pub fn search_field(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    width: f32,
    id_salt: &str,
) -> Response {
    Frame::NONE
        .fill(colors::field())
        .stroke(Stroke::new(1.0_f32, colors::border()))
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_width(width - 16.0);
            ui.set_height(32.0);
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                paint_icon(ui, r.shrink(1.0), Icon::Search, colors::text_faint());
                ui.add(
                    egui::TextEdit::singleline(text)
                        .id_salt(id_salt)
                        .frame(false)
                        .hint_text(hint)
                        .desired_width(ui.available_width()),
                )
            })
            .inner
        })
        .inner
}

/// Plain framed single-line text input.
pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    Frame::NONE
        .fill(colors::field())
        .stroke(Stroke::new(1.0_f32, colors::border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .frame(false)
                    .hint_text(hint)
                    .desired_width(width - 16.0),
            )
        })
        .inner
}

/// Monospace block on a recessed background, selectable.
pub fn code_block(ui: &mut Ui, text: &str) {
    Frame::NONE
        .fill(colors::field())
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .family(egui::FontFamily::Monospace)
                        .size(12.5)
                        .color(colors::text()),
                )
                .selectable(true)
                .wrap(),
            );
        });
}

/// Small rounded label with tinted background.
pub fn badge(ui: &mut Ui, text: &str, fg: Color32) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(11.0), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 14.0, 20.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 10.0, mix(colors::card(), fg, 0.16));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// Centered empty-state illustration with title and hint.
pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
        ui.painter()
            .circle_filled(r.center(), 22.0, colors::hover());
        paint_icon(ui, r.shrink(12.0), icon, colors::text_faint());
        ui.add_space(10.0);
        ui.label(RichText::new(title).size(15.0).color(colors::text()));
        ui.label(RichText::new(hint).size(12.5).color(colors::text_faint()));
        ui.add_space(28.0);
    });
}

/// Hairline divider spanning the available width.
pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0_f32, colors::border()),
    );
}

/// Draws a label + right-aligned shortcut menu entry. Returns `true` when clicked.
pub fn menu_item(ui: &mut Ui, icon: Option<Icon>, label: &str, shortcut: &str) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 7.0, colors::hover());
    }
    let col = if resp.hovered() {
        colors::text()
    } else {
        colors::text_dim()
    };
    let mut x = rect.left() + 10.0;
    if let Some(icon) = icon {
        paint_icon(
            ui,
            Rect::from_center_size(pos2(x + 7.0, rect.center().y), vec2(14.0, 14.0)),
            icon,
            col,
        );
        x += 24.0;
    }
    ui.painter().text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        col,
    );
    if !shortcut.is_empty() {
        ui.painter().text(
            pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(11.5),
            colors::text_faint(),
        );
    }
    resp.clicked()
}

pub fn popup_frame() -> Frame {
    Frame::NONE
        .fill(colors::card())
        .stroke(Stroke::new(1.0_f32, colors::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(6))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: colors::shadow(),
        })
}

// ─────────────────────────── Security semantics ───────────────────────────

pub fn severity_color(severity: &str) -> Color32 {
    match severity {
        "CRITICAL" => mix(colors::danger(), Color32::from_rgb(160, 40, 200), 0.35),
        "HIGH" => colors::danger(),
        "MEDIUM" => colors::warning(),
        "LOW" => colors::link(),
        _ => colors::text_faint(),
    }
}

pub fn status_color(status: u16) -> Color32 {
    match status {
        0..=299 => colors::success(),
        300..=399 => colors::warning(),
        _ => colors::danger(),
    }
}

pub fn method_color(method: &str) -> Color32 {
    match method {
        "GET" => colors::link(),
        "POST" => colors::success(),
        "PUT" | "PATCH" => colors::warning(),
        "DELETE" => colors::danger(),
        _ => colors::text_dim(),
    }
}

/// Paints a selectable full-width list row background and returns its response.
pub fn list_row(ui: &mut Ui, height: f32, selected: bool) -> (Rect, Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 8.0, colors::accent_soft());
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, colors::hover());
    }
    (rect, resp)
}

/// Fills a rounded rectangle with a vertical gradient from `top` to `bottom`.
pub fn paint_gradient(
    painter: &egui::Painter,
    rect: Rect,
    top: Color32,
    bottom: Color32,
    radius: f32,
) {
    use std::f32::consts::FRAC_PI_2;
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let color_at = |y: f32| {
        mix(
            top,
            bottom,
            ((y - rect.top()) / rect.height()).clamp(0.0, 1.0),
        )
    };
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -FRAC_PI_2),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
        (pos2(rect.left() + r, rect.top() + r), 2.0 * FRAC_PI_2),
    ];
    let mut mesh = egui::Mesh::default();
    let center = rect.center();
    mesh.colored_vertex(center, color_at(center.y));
    for (c, a0) in corners {
        for i in 0..=6 {
            let a = a0 + FRAC_PI_2 * i as f32 / 6.0;
            let pt = pos2(c.x + r * a.cos(), c.y + r * a.sin());
            mesh.colored_vertex(pt, color_at(pt.y));
        }
    }
    let n = mesh.vertices.len() as u32 - 1;
    for i in 1..=n {
        mesh.add_triangle(0, i, if i == n { 1 } else { i + 1 });
    }
    painter.add(egui::Shape::mesh(mesh));
}
