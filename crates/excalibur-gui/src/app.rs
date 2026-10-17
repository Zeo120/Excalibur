use crate::theme::apply_cyber_theme;
use eframe::egui;

#[derive(Default)]
pub struct GuiApp {
    pub url_input: String,
    pub selected_tab: usize,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            url_input: "https://example.com".to_string(),
            selected_tab: 0,
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_cyber_theme(ctx);

        egui::TopBottomPanel::top("top_nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("⚔️ Excalibur");
                ui.separator();
                ui.selectable_value(&mut self.selected_tab, 0, "🌐 Browser");
                ui.selectable_value(&mut self.selected_tab, 1, "🔍 Inspector");
                ui.selectable_value(&mut self.selected_tab, 2, "🔁 Repeater");
                ui.selectable_value(&mut self.selected_tab, 3, "🛡️ Scanner");
            });

            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut self.url_input);
                if ui.button("Navigate").clicked() {
                    tracing::info!("Navigating to: {}", self.url_input);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.selected_tab {
            0 => {
                ui.heading("Multi-View Hybrid Canvas");
                ui.label("Visual Viewport / Live DOM / Hex Viewer");
            }
            1 => {
                ui.heading("Traffic Inspector");
                ui.label("HTTP/HTTPS Stream & Interceptor");
            }
            2 => {
                ui.heading("Request Repeater");
                ui.label("Manual Probe Workbench & Diff Engine");
            }
            3 => {
                ui.heading("Security & Vulnerability Scanner");
                ui.label("Active Audit Rules & Secret Hunter");
            }
            _ => {}
        });
    }
}
