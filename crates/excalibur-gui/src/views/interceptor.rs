use crate::icons::{paint_icon, Icon};
use crate::theme::colors;
use crate::widgets::{
    badge, code_block, empty_state, ghost_button, icon_text_button, list_row, method_color,
    primary_button, search_field, single_line, status_color, toggle,
};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Frame, Margin, RichText, ScrollArea, Ui};

#[derive(Clone, Debug)]
pub struct GuiTrafficItem {
    pub id: String,
    pub method: String,
    pub status: u16,
    pub url: String,
    pub size: usize,
    pub elapsed_ms: u64,
    pub request_raw: String,
    pub response_raw: String,
}

pub enum InterceptorAction {
    SendToRepeater(GuiTrafficItem),
}

#[derive(Default)]
pub struct InterceptorViewState {
    pub intercept_enabled: bool,
    pub filter_text: String,
    pub selected_idx: usize,
    pub traffic_items: Vec<GuiTrafficItem>,
    pub is_request_paused: bool,
    pub paused_req_buffer: String,
}

pub fn render_interceptor_view(
    ui: &mut Ui,
    state: &mut InterceptorViewState,
) -> Option<InterceptorAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
        toggle(ui, &mut state.intercept_enabled);
        let label = if state.intercept_enabled {
            RichText::new("Intercepting").color(colors::danger())
        } else {
            RichText::new("Passthrough").color(colors::text_dim())
        };
        ui.label(label.size(13.0));
        ui.add_space(8.0);
        search_field(
            ui,
            &mut state.filter_text,
            "Filter by URL",
            260.0,
            "interceptor_filter",
        );
        badge(
            ui,
            &format!("{} requests", state.traffic_items.len()),
            colors::text_dim(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_text_button(ui, Icon::Trash, "Clear").clicked() {
                state.traffic_items.clear();
                state.selected_idx = 0;
            }
        });
    });
    ui.add_space(8.0);

    if state.is_request_paused {
        Frame::NONE
            .fill(colors::danger_soft())
            .corner_radius(10)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                    paint_icon(ui, r, Icon::Warning, colors::warning());
                    ui.label(
                        RichText::new("Request paused on breakpoint")
                            .strong()
                            .color(colors::text()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ghost_button(ui, "Drop", true).clicked() {
                            state.is_request_paused = false;
                        }
                        if primary_button(ui, "Forward").clicked() {
                            state.is_request_paused = false;
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }

    egui::TopBottomPanel::bottom("inspector_details")
        .resizable(true)
        .min_height(140.0)
        .default_height(240.0)
        .frame(Frame::NONE.inner_margin(Margin {
            left: 0,
            right: 0,
            top: 8,
            bottom: 0,
        }))
        .show_inside(ui, |ui| {
            render_detail_panes(ui, state, &mut action);
        });

    egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            render_traffic_table(ui, state);
        });

    action
}

const COLS: [(&str, f32); 6] = [
    ("#", 40.0),
    ("Method", 74.0),
    ("Status", 60.0),
    ("URL", 0.0),
    ("Size", 80.0),
    ("Time", 64.0),
];

/// Column x-offsets for a row of `width`; the URL column takes the remainder.
fn column_xs(width: f32) -> [f32; 7] {
    let fixed: f32 = COLS.iter().map(|c| c.1).sum();
    let url_w = (width - fixed).max(80.0);
    let mut xs = [0.0; 7];
    for (i, (_, w)) in COLS.iter().enumerate() {
        let w = if *w == 0.0 { url_w } else { *w };
        xs[i + 1] = xs[i] + w;
    }
    xs
}

fn render_traffic_table(ui: &mut Ui, state: &mut InterceptorViewState) {
    let width = ui.available_width();
    let xs = column_xs(width);

    let (hdr, _) = ui.allocate_exact_size(vec2(width, 24.0), egui::Sense::hover());
    for (i, (name, _)) in COLS.iter().enumerate() {
        ui.painter().text(
            pos2(hdr.left() + xs[i] + 10.0, hdr.center().y),
            Align2::LEFT_CENTER,
            name.to_uppercase(),
            FontId::proportional(10.5),
            colors::text_faint(),
        );
    }

    let filter = state.filter_text.to_ascii_lowercase();
    let visible: Vec<usize> = state
        .traffic_items
        .iter()
        .enumerate()
        .filter(|(_, it)| filter.is_empty() || it.url.to_ascii_lowercase().contains(&filter))
        .map(|(i, _)| i)
        .collect();

    if visible.is_empty() {
        empty_state(
            ui,
            Icon::Network,
            "No traffic captured yet",
            "Pages you load are recorded here automatically.",
        );
        return;
    }

    ScrollArea::vertical()
        .id_salt("traffic_table_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
            for idx in visible {
                let item = &state.traffic_items[idx];
                let (rect, resp) = list_row(ui, 30.0, idx == state.selected_idx);
                let y = rect.center().y;
                let x = |i: usize| rect.left() + xs[i] + 10.0;
                let f = FontId::proportional(12.5);
                let p = ui.painter();
                p.text(
                    pos2(x(0), y),
                    Align2::LEFT_CENTER,
                    (idx + 1).to_string(),
                    f.clone(),
                    colors::text_faint(),
                );
                p.text(
                    pos2(x(1), y),
                    Align2::LEFT_CENTER,
                    &item.method,
                    FontId::monospace(12.0),
                    method_color(&item.method),
                );
                p.text(
                    pos2(x(2), y),
                    Align2::LEFT_CENTER,
                    item.status.to_string(),
                    FontId::monospace(12.0),
                    status_color(item.status),
                );
                let url = single_line(ui, &item.url, 12.5, colors::text(), xs[4] - xs[3] - 16.0);
                ui.painter()
                    .galley(pos2(x(3), y - url.size().y / 2.0), url, colors::text());
                let p = ui.painter();
                p.text(
                    pos2(x(4), y),
                    Align2::LEFT_CENTER,
                    format_size(item.size),
                    f.clone(),
                    colors::text_dim(),
                );
                p.text(
                    pos2(x(5), y),
                    Align2::LEFT_CENTER,
                    format!("{} ms", item.elapsed_ms),
                    f,
                    colors::text_dim(),
                );
                if resp.clicked() {
                    state.selected_idx = idx;
                }
            }
        });
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn render_detail_panes(
    ui: &mut Ui,
    state: &InterceptorViewState,
    action: &mut Option<InterceptorAction>,
) {
    let Some(item) = state.traffic_items.get(state.selected_idx) else {
        empty_state(
            ui,
            Icon::Code,
            "Select a request",
            "Raw request and response appear here.",
        );
        return;
    };

    ui.columns(2, |cols| {
        cols[0].horizontal(|ui| {
            ui.label(RichText::new("Request").strong().color(colors::text()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_text_button(ui, Icon::Send, "Send to Repeater").clicked() {
                    *action = Some(InterceptorAction::SendToRepeater(item.clone()));
                }
            });
        });
        ScrollArea::vertical()
            .id_salt("req_raw_scroll")
            .show(&mut cols[0], |ui| code_block(ui, &item.request_raw));

        cols[1].horizontal(|ui| {
            ui.label(RichText::new("Response").strong().color(colors::text()));
            badge(ui, &item.status.to_string(), status_color(item.status));
        });
        ScrollArea::vertical()
            .id_salt("res_raw_scroll")
            .show(&mut cols[1], |ui| code_block(ui, &item.response_raw));
    });
}
