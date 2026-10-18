use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};

pub struct RepeaterViewState {
    pub method: String,
    pub url: String,
    pub headers: String,
    pub body: String,
    pub response_status: u16,
    pub response_elapsed_ms: u64,
    pub response_raw: String,
    pub show_diff: bool,
}

impl Default for RepeaterViewState {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            url: "https://example.com/api/v1/test".to_string(),
            headers: "Host: example.com\r\nUser-Agent: Excalibur-Repeater/1.0\r\nAccept: application/json".to_string(),
            body: String::new(),
            response_status: 200,
            response_elapsed_ms: 28,
            response_raw: "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 32\r\n\r\n{\"message\": \"Repeater response\"}".to_string(),
            show_diff: false,
        }
    }
}

pub fn render_repeater_view(ui: &mut Ui, state: &mut RepeaterViewState) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("repeater_method")
            .selected_text(&state.method)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.method, "GET".into(), "GET");
                ui.selectable_value(&mut state.method, "POST".into(), "POST");
                ui.selectable_value(&mut state.method, "PUT".into(), "PUT");
                ui.selectable_value(&mut state.method, "DELETE".into(), "DELETE");
                ui.selectable_value(&mut state.method, "PATCH".into(), "PATCH");
                ui.selectable_value(&mut state.method, "HEAD".into(), "HEAD");
                ui.selectable_value(&mut state.method, "OPTIONS".into(), "OPTIONS");
            });

        ui.label("URL:");
        ui.text_edit_singleline(&mut state.url);

        let send_btn = RichText::new("Send Request")
            .color(Color32::WHITE)
            .strong();
        if ui.button(send_btn).clicked() {
            state.response_status = 200;
            state.response_elapsed_ms = 19;
            state.response_raw = format!(
                "HTTP/1.1 200 OK\r\nServer: Excalibur-Engine/0.1\r\nDate: {}\r\nContent-Type: application/json\r\n\r\n{{\"status\":\"dispatched\",\"timestamp\":{}}}",
                chrono::Utc::now().to_rfc2822(),
                chrono::Utc::now().timestamp()
            );
        }

        ui.separator();
        ui.checkbox(&mut state.show_diff, "Show Side-by-Side Diff");
    });

    ui.separator();

    ui.columns(2, |cols| {
        // Left Column: Request Builder
        cols[0].group(|ui| {
            ui.heading("Request Composition");
            ui.label("Headers (Key: Value):");
            ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                ui.text_edit_multiline(&mut state.headers);
            });

            ui.add_space(8.0);
            ui.label("Body Payload:");
            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                ui.text_edit_multiline(&mut state.body);
            });
        });

        // Right Column: Response / Diff Viewer
        cols[1].group(|ui| {
            let status_color = if state.response_status < 300 {
                Color32::from_rgb(63, 185, 80)
            } else {
                Color32::from_rgb(248, 81, 73)
            };

            ui.horizontal(|ui| {
                ui.heading("Response Viewer");
                ui.label(
                    RichText::new(format!("Status: {}", state.response_status))
                        .color(status_color)
                        .strong(),
                );
                ui.label(format!("({} ms)", state.response_elapsed_ms));
            });

            if state.show_diff {
                ui.label(RichText::new("Diff Comparison").italics());
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label("--- Request Payload ---");
                    ui.code(&state.body);
                    ui.label("+++ Response Body +++");
                    ui.code(&state.response_raw);
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.code(&state.response_raw);
                });
            }
        });
    });
}
