use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};

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

        if ui.button("Clear Log").clicked() {
            state.traffic_items.clear();
            state.selected_idx = 0;
        }

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
        .min_height(140.0)
        .default_height(240.0)
        .show_inside(ui, |ui| {
            render_detail_panes(ui, state, &mut action);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_traffic_table(ui, state);
    });

    action
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

fn render_detail_panes(
    ui: &mut Ui,
    state: &InterceptorViewState,
    action: &mut Option<InterceptorAction>,
) {
    let (req_raw, res_raw, selected_item) =
        if let Some(item) = state.traffic_items.get(state.selected_idx) {
            (&item.request_raw, &item.response_raw, Some(item))
        } else {
            (
                &"No request selected.".to_string(),
                &"No response selected.".to_string(),
                None,
            )
        };

    ui.columns(2, |cols| {
        cols[0].group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("HTTP Request Inspector");
                if let Some(item) = selected_item {
                    if ui.button("Send to Repeater").clicked() {
                        *action = Some(InterceptorAction::SendToRepeater(item.clone()));
                    }
                }
            });
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
