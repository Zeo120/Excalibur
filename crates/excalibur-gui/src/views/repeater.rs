use crate::theme::colors;
use crate::widgets::{badge, card, code_block, method_color, primary_button, status_color, toggle};
use eframe::egui::{self, vec2, Frame, Margin, RichText, ScrollArea, Stroke, Ui};

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

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

fn mono_editor(ui: &mut Ui, text: &mut String, rows: usize) {
    Frame::NONE
        .fill(colors::field())
        .stroke(Stroke::new(1.0_f32, colors::border()))
        .corner_radius(8)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(text)
                    .frame(false)
                    .code_editor()
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY),
            );
        });
}

pub fn render_repeater_view(ui: &mut Ui, state: &mut RepeaterViewState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
        egui::ComboBox::from_id_salt("repeater_method")
            .width(96.0)
            .selected_text(
                RichText::new(&state.method)
                    .monospace()
                    .strong()
                    .color(method_color(&state.method)),
            )
            .show_ui(ui, |ui| {
                for m in METHODS {
                    ui.selectable_value(
                        &mut state.method,
                        m.to_string(),
                        RichText::new(m).monospace().color(method_color(m)),
                    );
                }
            });

        let url_w = (ui.available_width() - 210.0).max(160.0);
        Frame::NONE
            .fill(colors::field())
            .stroke(Stroke::new(1.0_f32, colors::border()))
            .corner_radius(8)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.url)
                        .frame(false)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(url_w),
                );
            });

        if primary_button(ui, "Send").clicked() {
            state.response_status = 200;
            state.response_elapsed_ms = 19;
            state.response_raw = format!(
                "HTTP/1.1 200 OK\r\nServer: Excalibur-Engine/0.1\r\nDate: {}\r\nContent-Type: application/json\r\n\r\n{{\"status\":\"dispatched\",\"timestamp\":{}}}",
                chrono::Utc::now().to_rfc2822(),
                chrono::Utc::now().timestamp()
            );
        }
        toggle(ui, &mut state.show_diff);
        ui.label(RichText::new("Diff").size(12.5).color(colors::text_dim()));
    });

    ui.add_space(10.0);

    ui.columns(2, |cols| {
        card(&mut cols[0], |ui| {
            ui.label(RichText::new("Request").strong().color(colors::text()));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Headers")
                    .size(12.0)
                    .color(colors::text_faint()),
            );
            ScrollArea::vertical()
                .id_salt("repeater_headers")
                .max_height(150.0)
                .show(ui, |ui| mono_editor(ui, &mut state.headers, 5));
            ui.add_space(8.0);
            ui.label(RichText::new("Body").size(12.0).color(colors::text_faint()));
            ScrollArea::vertical()
                .id_salt("repeater_body")
                .max_height(200.0)
                .show(ui, |ui| mono_editor(ui, &mut state.body, 6));
        });

        card(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Response").strong().color(colors::text()));
                badge(
                    ui,
                    &state.response_status.to_string(),
                    status_color(state.response_status),
                );
                ui.label(
                    RichText::new(format!("{} ms", state.response_elapsed_ms))
                        .size(12.0)
                        .color(colors::text_faint()),
                );
            });
            ui.add_space(6.0);
            ScrollArea::vertical()
                .id_salt("repeater_response")
                .show(ui, |ui| {
                    if state.show_diff {
                        ui.label(
                            RichText::new("− Request body")
                                .size(12.0)
                                .color(colors::danger()),
                        );
                        code_block(ui, &state.body);
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("+ Response")
                                .size(12.0)
                                .color(colors::success()),
                        );
                    }
                    code_block(ui, &state.response_raw);
                });
        });
    });
}
