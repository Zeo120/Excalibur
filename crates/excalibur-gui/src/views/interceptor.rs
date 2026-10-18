use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};

#[derive(Clone)]
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

pub struct InterceptorViewState {
    pub intercept_enabled: bool,
    pub filter_text: String,
    pub selected_idx: usize,
    pub traffic_items: Vec<GuiTrafficItem>,
    pub is_request_paused: bool,
    pub paused_req_buffer: String,
}

impl Default for InterceptorViewState {
    fn default() -> Self {
        let items = vec![
            GuiTrafficItem {
                id: "req-1".into(),
                method: "GET".into(),
                status: 200,
                url: "https://example.com/".into(),
                size: 1256,
                elapsed_ms: 34,
                request_raw: "GET / HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Excalibur/0.1.0\r\n\r\n".into(),
                response_raw: "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<!doctype html><html>...</html>".into(),
            },
            GuiTrafficItem {
                id: "req-2".into(),
                method: "POST".into(),
                status: 403,
                url: "https://example.com/api/v1/auth".into(),
                size: 218,
                elapsed_ms: 112,
                request_raw: "POST /api/v1/auth HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\n\r\n{\"user\":\"admin\"}".into(),
                response_raw: "HTTP/1.1 403 Forbidden\r\nServer: nginx/1.18.0\r\n\r\n{\"error\":\"Invalid CSRF token\"}".into(),
            },
        ];

        Self {
            intercept_enabled: false,
            filter_text: String::new(),
            selected_idx: 0,
            traffic_items: items,
            is_request_paused: false,
            paused_req_buffer: String::new(),
        }
    }
}

pub fn render_interceptor_view(ui: &mut Ui, state: &mut InterceptorViewState) {
    ui.horizontal(|ui| {
        let toggle_text = if state.intercept_enabled {
            RichText::new("[ Intercept: ON ]")
                .color(Color32::from_rgb(248, 81, 73))
                .strong()
        } else {
            RichText::new("[ Intercept: OFF ]").color(Color32::from_rgb(63, 185, 80))
        };

        if ui.button(toggle_text).clicked() {
            state.intercept_enabled = !state.intercept_enabled;
        }

        ui.separator();
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.filter_text);

        if state.is_request_paused {
            ui.separator();
            ui.label(
                RichText::new("PAUSED ON BREAKPOINT")
                    .color(Color32::from_rgb(210, 153, 34))
                    .strong(),
            );
            if ui.button("Forward").clicked() {
                state.is_request_paused = false;
            }
            if ui.button("Drop").clicked() {
                state.is_request_paused = false;
            }
        }
    });

    ui.separator();

    // Split top table and bottom inspector
    egui::TopBottomPanel::bottom("inspector_details")
        .resizable(true)
        .min_height(150.0)
        .default_height(250.0)
        .show_inside(ui, |ui| {
            render_detail_panes(ui, state);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_traffic_table(ui, state);
    });
}

fn render_traffic_table(ui: &mut Ui, state: &mut InterceptorViewState) {
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("traffic_grid")
            .striped(true)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.label(RichText::new("#").strong());
                ui.label(RichText::new("Method").strong());
                ui.label(RichText::new("Status").strong());
                ui.label(RichText::new("URL").strong());
                ui.label(RichText::new("Size").strong());
                ui.label(RichText::new("Time").strong());
                ui.end_row();

                let filter = state.filter_text.to_ascii_lowercase();

                for (idx, item) in state.traffic_items.iter().enumerate() {
                    if !filter.is_empty() && !item.url.to_ascii_lowercase().contains(&filter) {
                        continue;
                    }

                    let is_selected = idx == state.selected_idx;

                    let status_color = if item.status < 300 {
                        Color32::from_rgb(63, 185, 80)
                    } else if item.status < 400 {
                        Color32::from_rgb(210, 153, 34)
                    } else {
                        Color32::from_rgb(248, 81, 73)
                    };

                    let row_label = format!("{}", idx + 1);
                    if ui.selectable_label(is_selected, row_label).clicked() {
                        state.selected_idx = idx;
                    }

                    ui.label(RichText::new(&item.method).color(Color32::from_rgb(88, 166, 255)));
                    ui.label(RichText::new(format!("{}", item.status)).color(status_color));
                    ui.label(&item.url);
                    ui.label(format!("{} B", item.size));
                    ui.label(format!("{} ms", item.elapsed_ms));
                    ui.end_row();
                }
            });
    });
}

fn render_detail_panes(ui: &mut Ui, state: &mut InterceptorViewState) {
    let (req_raw, res_raw) = if let Some(item) = state.traffic_items.get(state.selected_idx) {
        (&item.request_raw, &item.response_raw)
    } else {
        (
            &"No request selected.".to_string(),
            &"No response selected.".to_string(),
        )
    };

    ui.columns(2, |cols| {
        cols[0].group(|ui| {
            ui.heading("HTTP Request Inspector");
            ScrollArea::vertical().show(ui, |ui| {
                ui.code(req_raw);
            });
        });

        cols[1].group(|ui| {
            ui.heading("HTTP Response Inspector");
            ScrollArea::vertical().show(ui, |ui| {
                ui.code(res_raw);
            });
        });
    });
}
