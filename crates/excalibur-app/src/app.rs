use std::{rc::Rc, sync::Arc, time::Duration};

use excalibur_cef::{CefEvent, CefRuntime};
use excalibur_core::{normalize_url_or_search, BrowserConfig, TabManager};
use raw_window_handle::HasWindowHandle;

pub struct ExcaliburBrowserApp {
    config: BrowserConfig,
    runtime: Rc<CefRuntime>,
    tabs: TabManager,
    active_tab_id: u64,
    browser_spawned: bool,
    omnibox_id: egui::Id,
    omnibox_text: String,
    last_bounds: Option<(i32, i32, i32, i32)>,
    last_window_title: String,
}

impl ExcaliburBrowserApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: BrowserConfig,
        runtime: Rc<CefRuntime>,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        runtime.set_wake_callback(Arc::new(move |delay_ms| {
            if delay_ms <= 0 {
                ctx.request_repaint();
            } else {
                let capped = u64::try_from(delay_ms).unwrap_or(16).min(33);
                ctx.request_repaint_after(Duration::from_millis(capped));
            }
        }));

        configure_theme(&cc.egui_ctx);

        let mut tabs = TabManager::new();
        let active_tab_id = tabs.open_tab(config.home_url.clone());
        let omnibox_text = config.home_url.clone();

        Self {
            config,
            runtime,
            tabs,
            active_tab_id,
            browser_spawned: false,
            omnibox_id: egui::Id::unique("excalibur_omnibox"),
            omnibox_text,
            last_bounds: None,
            last_window_title: "Excalibur".to_owned(),
        }
    }

    fn process_cef_events(&mut self, ctx: &egui::Context, omnibox_focused: bool) {
        for event in self.runtime.poll_events() {
            match event {
                CefEvent::ContextInitialized => {
                    ctx.request_repaint();
                }
                CefEvent::BrowserCreated { tab_id } => {
                    tracing::info!(tab_id, "CEF child browser created");
                }
                CefEvent::TitleChanged { tab_id, title } => {
                    tracing::info!(tab_id, %title, "Title changed");
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.title = title.clone();
                    }
                    if tab_id == self.active_tab_id && !title.is_empty() {
                        let full = format!("{title} - Excalibur");
                        if full != self.last_window_title {
                            self.last_window_title = full.clone();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(full));
                        }
                    }
                }
                CefEvent::AddressChanged { tab_id, url } => {
                    tracing::info!(tab_id, %url, "Address changed");
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.url = url.clone();
                    }
                    if tab_id == self.active_tab_id && !omnibox_focused {
                        self.omnibox_text = url;
                    }
                }
                CefEvent::LoadingStateChanged {
                    tab_id,
                    is_loading,
                    can_go_back,
                    can_go_forward,
                } => {
                    tracing::info!(
                        tab_id,
                        is_loading,
                        can_go_back,
                        can_go_forward,
                        "Loading state changed"
                    );
                    if let Some(tab) = self.tabs.tab_mut(tab_id) {
                        tab.is_loading = is_loading;
                        tab.can_go_back = can_go_back;
                        tab.can_go_forward = can_go_forward;
                    }
                }
                CefEvent::BrowserClosed { tab_id } => {
                    tracing::info!(tab_id, "CEF child browser closed");
                }
            }
        }
    }

    fn navigate_omnibox(&mut self) {
        let target = normalize_url_or_search(&self.omnibox_text, &self.config.search_template);
        self.omnibox_text = target.clone();
        if let Some(tab) = self.tabs.tab_mut(self.active_tab_id) {
            tab.url = target.clone();
        }
        self.runtime.navigate(self.active_tab_id, &target);
    }
}

impl eframe::App for ExcaliburBrowserApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.runtime.pump_work();
        ctx.request_repaint_after(Duration::from_millis(33));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let (can_back, can_forward, is_loading) = self
            .tabs
            .active_tab()
            .map(|t| (t.can_go_back, t.can_go_forward, t.is_loading))
            .unwrap_or((false, false, false));

        let mut omnibox_focused = false;
        let mut submit_navigation = false;

        egui::Panel::top("excalibur_toolbar")
            .exact_size(38.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    if ui.add_enabled(can_back, egui::Button::new("<")).clicked() {
                        self.runtime.go_back(self.active_tab_id);
                    }
                    if ui
                        .add_enabled(can_forward, egui::Button::new(">"))
                        .clicked()
                    {
                        self.runtime.go_forward(self.active_tab_id);
                    }
                    let reload_label = if is_loading { "X" } else { "R" };
                    if ui.button(reload_label).clicked() {
                        if is_loading {
                            self.runtime.stop_load(self.active_tab_id);
                        } else {
                            self.runtime.reload(self.active_tab_id);
                        }
                    }

                    let avail = ui.available_width().max(120.0);
                    let response = ui.add_sized(
                        [avail, 24.0],
                        egui::TextEdit::singleline(&mut self.omnibox_text)
                            .id(self.omnibox_id)
                            .hint_text("Enter URL or search"),
                    );
                    omnibox_focused = response.has_focus();
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit_navigation = true;
                    }
                    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L)) {
                        response.request_focus();
                    }
                });
            });

        if submit_navigation {
            self.navigate_omnibox();
        }

        self.process_cef_events(&ctx, omnibox_focused);

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(12, 12, 16)))
            .show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                let ppp = ctx.pixels_per_point();
                let bounds = (
                    (rect.min.x * ppp).round() as i32,
                    (rect.min.y * ppp).round() as i32,
                    (rect.width() * ppp).round() as i32,
                    (rect.height() * ppp).round() as i32,
                );

                if !self.browser_spawned {
                    if self.runtime.is_context_ready() && bounds.2 > 0 && bounds.3 > 0 {
                        if let Ok(window_handle) = frame.window_handle() {
                            let url = self
                                .tabs
                                .active_tab()
                                .map(|t| t.url.clone())
                                .unwrap_or_else(|| self.config.home_url.clone());
                            match self.runtime.create_child_browser(
                                self.active_tab_id,
                                window_handle.as_raw(),
                                bounds,
                                &url,
                            ) {
                                Ok(_) => {
                                    self.browser_spawned = true;
                                    self.last_bounds = Some(bounds);
                                }
                                Err(err) => {
                                    tracing::error!("Failed to spawn CEF child browser: {err}");
                                }
                            }
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Initializing Chromium...");
                        });
                    }
                } else if self.last_bounds != Some(bounds) && bounds.2 > 0 && bounds.3 > 0 {
                    self.runtime.resize_browser(
                        self.active_tab_id,
                        bounds.0,
                        bounds.1,
                        bounds.2,
                        bounds.3,
                    );
                    self.last_bounds = Some(bounds);
                }
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        tracing::info!("UI on_exit invoked, shutting down CEF");
        self.runtime.shutdown_cleanly();
        tracing::info!("CEF shutdown complete");
    }
}

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(14, 14, 18);
    visuals.extreme_bg_color = egui::Color32::from_rgb(8, 8, 11);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(24, 24, 30);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(36, 36, 44);
    ctx.set_visuals(visuals);
}
