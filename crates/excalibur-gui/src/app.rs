use crate::theme::apply_cyber_theme;
use crate::views::{
    browser::{render_browser_view, BrowserViewState},
    interceptor::{render_interceptor_view, InterceptorViewState},
    repeater::{render_repeater_view, RepeaterViewState},
    scanner::{render_scanner_view, ScannerViewState},
};
use eframe::egui::{self, Color32, RichText};

pub struct GuiApp {
    pub url_input: String,
    pub selected_tab: usize,
    pub browser_state: BrowserViewState,
    pub interceptor_state: InterceptorViewState,
    pub repeater_state: RepeaterViewState,
    pub scanner_state: ScannerViewState,
}

impl GuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            url_input: "https://example.com".to_string(),
            selected_tab: 0,
            browser_state: BrowserViewState::default(),
            interceptor_state: InterceptorViewState::default(),
            repeater_state: RepeaterViewState::default(),
            scanner_state: ScannerViewState::default(),
        }
    }

    pub fn render_panels(&mut self, ctx: &egui::Context) {
        apply_cyber_theme(ctx);

        // Top Navigation & Action Panel
        egui::TopBottomPanel::top("top_nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("[Excalibur Security Workbench]")
                        .size(16.0)
                        .strong()
                        .color(Color32::from_rgb(88, 166, 255)),
                );
                ui.separator();
                ui.selectable_value(&mut self.selected_tab, 0, "[1] Browser Viewport");
                ui.selectable_value(&mut self.selected_tab, 1, "[2] Traffic Inspector");
                ui.selectable_value(&mut self.selected_tab, 2, "[3] Request Repeater");
                ui.selectable_value(&mut self.selected_tab, 3, "[4] Security Scanner");
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Target URL:");
                ui.text_edit_singleline(&mut self.url_input);
                if ui.button("Navigate").clicked() {
                    tracing::info!("Navigating browser to: {}", self.url_input);
                    let sample = format!(
                        "<!DOCTYPE html><html><head><title>{}</title></head><body><h1>Loaded {}</h1><p>Active document rendered.</p></body></html>",
                        self.url_input, self.url_input
                    );
                    self.browser_state.raw_html = sample.clone();
                    self.browser_state.parsed_dom =
                        Some(excalibur_dom::HtmlParser::parse(&sample));
                }
            });
        });

        // Bottom Status / Telemetry Panel
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Proxy: 127.0.0.1:8080 (Active)")
                        .color(Color32::from_rgb(63, 185, 80)),
                );
                ui.separator();
                ui.label(format!("Target: {}", self.url_input));
                ui.separator();
                ui.label(format!(
                    "Traffic Captured: {}",
                    self.interceptor_state.traffic_items.len()
                ));
                ui.separator();
                ui.label(format!("Findings: {}", self.scanner_state.findings.len()));
            });
        });

        // Central Workbench Viewport
        egui::CentralPanel::default().show(ctx, |ui| match self.selected_tab {
            0 => render_browser_view(ui, &mut self.browser_state, &self.url_input),
            1 => render_interceptor_view(ui, &mut self.interceptor_state),
            2 => render_repeater_view(ui, &mut self.repeater_state),
            3 => render_scanner_view(ui, &mut self.scanner_state),
            _ => {}
        });
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_panels(ctx);
    }
}
