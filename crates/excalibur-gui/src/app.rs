use crate::theme::apply_cyber_theme;
use crate::views::{
    browser::{BrowserAction, BrowserRenderer},
    dom_inspector::{render_dom_inspector, DomInspectorState},
    interceptor::{
        render_interceptor_view, GuiTrafficItem, InterceptorAction, InterceptorViewState,
    },
    proxy_manager::{render_proxy_manager, ProxyManagerState},
    repeater::{render_repeater_view, RepeaterViewState},
    scanner::{render_scanner_view, GuiFindingItem, ScannerViewState},
};
use eframe::egui::{self, Color32, Frame, Margin, RichText, Stroke, Ui};
use excalibur_audit::ResponseAuditor;
use excalibur_core::{
    client::{ClientOptions, HttpClient},
    network::HttpRequest,
};
use excalibur_dom::{DomNode, HtmlParser};
use excalibur_script::JsRuntime;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone)]
pub struct BrowserTab {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub raw_html: String,
    pub parsed_dom: Option<DomNode>,
    pub is_loading: bool,
    pub status_code: Option<u16>,
    pub elapsed_ms: Option<u64>,
    pub error_msg: Option<String>,
}

impl BrowserTab {
    pub fn new(id: usize, url: &str) -> Self {
        let initial_url = if url.is_empty() {
            "about:blank".to_string()
        } else {
            url.to_string()
        };

        Self {
            id,
            title: "New Tab".to_string(),
            url: initial_url.clone(),
            history: vec![initial_url],
            history_idx: 0,
            raw_html: String::new(),
            parsed_dom: None,
            is_loading: false,
            status_code: None,
            elapsed_ms: None,
            error_msg: None,
        }
    }
}

pub struct PageLoadedEvent {
    pub tab_id: usize,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub raw_html: String,
    pub parsed_dom: DomNode,
    pub title: String,
    pub findings: Vec<GuiFindingItem>,
    pub traffic_item: GuiTrafficItem,
}

pub enum PageNavigationEvent {
    Success(Box<PageLoadedEvent>),
    Failure {
        tab_id: usize,
        url: String,
        error: String,
    },
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TabNavLayout {
    Top,
    Sidebar,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum XModeTab {
    Interceptor,
    Repeater,
    Scanner,
    DomAndScripts,
    ProxyManager,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum XModeLayout {
    SplitDrawer,
    FullscreenTools,
}

pub struct GuiApp {
    // Multi-tab browser state
    pub tabs: Vec<BrowserTab>,
    pub active_tab_id: usize,
    pub next_tab_id: usize,
    pub omnibox_input: String,
    pub browser_renderer: BrowserRenderer,

    // Layout configuration
    pub tab_nav_layout: TabNavLayout,
    pub show_favorites_bar: bool,

    // The user's requested X-Mode Toggle
    pub x_mode_enabled: bool,
    pub x_mode_tab: XModeTab,
    pub x_mode_layout: XModeLayout,

    // Penetration testing tool states
    pub interceptor_state: InterceptorViewState,
    pub repeater_state: RepeaterViewState,
    pub scanner_state: ScannerViewState,
    pub dom_inspector_state: DomInspectorState,
    pub proxy_state: ProxyManagerState,

    // Async page loading channels
    pub page_tx: Sender<PageNavigationEvent>,
    pub page_rx: Receiver<PageNavigationEvent>,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Install loaders for web image rendering (PNG, JPEG, SVG, GIF, WEBP)
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (tx, rx) = channel();

        // Start background MITM proxy listener for external tools & Full Web Engine
        tokio::spawn(async move {
            if let Ok(ca) = excalibur_proxy::CertificateAuthority::new_ephemeral() {
                let ca = std::sync::Arc::new(ca);
                let (event_tx, _event_rx) = tokio::sync::mpsc::channel(256);
                let interceptor =
                    std::sync::Arc::new(excalibur_proxy::TrafficInterceptor::new(event_tx));
                if let Ok(bind_addr) = "127.0.0.1:8080".parse() {
                    if let Ok(server) =
                        excalibur_proxy::ProxyServer::new(bind_addr, ca, interceptor, None, None)
                    {
                        let _ = server.run().await;
                    }
                }
            }
        });

        let mut app = Self {
            tabs: Vec::new(),
            active_tab_id: 1,
            next_tab_id: 2,
            omnibox_input: "https://example.com".to_string(),
            browser_renderer: BrowserRenderer::new(),

            tab_nav_layout: TabNavLayout::Top,
            show_favorites_bar: true,

            x_mode_enabled: false,
            x_mode_tab: XModeTab::Interceptor,
            x_mode_layout: XModeLayout::SplitDrawer,

            interceptor_state: InterceptorViewState::default(),
            repeater_state: RepeaterViewState::default(),
            scanner_state: ScannerViewState::default(),
            dom_inspector_state: DomInspectorState::default(),
            proxy_state: ProxyManagerState::default(),

            page_tx: tx,
            page_rx: rx,
        };

        // Create initial tab and load default page
        let initial_tab = BrowserTab::new(1, "https://example.com");
        app.tabs.push(initial_tab);
        app.navigate(1, "https://example.com");

        app
    }

    pub fn active_tab(&self) -> Option<&BrowserTab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut BrowserTab> {
        self.tabs.iter_mut().find(|t| t.id == self.active_tab_id)
    }

    /// Navigates a tab to a specified URL asynchronously.
    pub fn navigate(&mut self, tab_id: usize, url: &str) {
        let normalized = if url == "about:blank" || url == "excalibur://newtab" {
            url.to_string()
        } else if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url.to_string()
        };

        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.url = normalized.clone();
            tab.is_loading = true;
            tab.error_msg = None;

            if tab.history.is_empty() || tab.history[tab.history_idx] != normalized {
                tab.history.truncate(tab.history_idx + 1);
                tab.history.push(normalized.clone());
                tab.history_idx = tab.history.len() - 1;
            }

            self.omnibox_input = normalized.clone();

            if normalized == "about:blank" || normalized == "excalibur://newtab" {
                tab.is_loading = false;
                tab.title = "New Tab".to_string();
                tab.parsed_dom = None;
                tab.raw_html.clear();
                return;
            }

            let tx = self.page_tx.clone();
            let target_url = normalized.clone();

            tokio::spawn(async move {
                let client = match HttpClient::new(ClientOptions::default()) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to initialize HTTP client: {}", e);
                        return;
                    }
                };

                let mut req = HttpRequest::new("GET", &target_url);
                req.headers.append(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Excalibur/1.0 (Security Browser)",
                );
                req.headers.append(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                );

                match client.dispatch(&req).await {
                    Ok(res) => {
                        let body_str = String::from_utf8_lossy(&res.body).to_string();
                        let dom = HtmlParser::parse(&body_str);

                        // Extract title
                        let title = dom
                            .find_by_tag("title")
                            .first()
                            .map(|t| t.text_content().trim().to_string())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| target_url.clone());

                        // Audit response
                        let is_https = target_url.starts_with("https://");
                        let auditor = ResponseAuditor::new();
                        let report = auditor.audit_response(&res, is_https);

                        let mut findings = Vec::new();
                        for h in report.header_findings {
                            findings.push(GuiFindingItem {
                                rule_id: h.rule_id,
                                severity: h.severity.as_str().to_string(),
                                title: h.title,
                                description: h.description,
                                recommendation: h.recommendation,
                                evidence: h.header_value.unwrap_or_default(),
                                curl_command: format!("curl -I '{}'", target_url),
                            });
                        }
                        for s in report.secret_findings {
                            findings.push(GuiFindingItem {
                                rule_id: s.rule_id,
                                severity: s.severity.as_str().to_string(),
                                title: format!("Disclosed Secret: {}", s.rule_name),
                                description: format!(
                                    "Sensitive credential pattern matched at offset {}",
                                    s.offset
                                ),
                                recommendation: "Rotate secret and suppress client exposure."
                                    .into(),
                                evidence: s.redacted_text,
                                curl_command: format!("curl -X GET '{}'", target_url),
                            });
                        }

                        // DOM XSS Sink tracking via QuickJS
                        if let Ok(js) = JsRuntime::new() {
                            let _ = js.set_url(&target_url);
                            for script in dom.find_by_tag("script") {
                                let code = script.text_content();
                                if !code.trim().is_empty() {
                                    let _ = js.eval(code.trim());
                                }
                            }
                            for sink in js.get_sink_findings() {
                                findings.push(GuiFindingItem {
                                    rule_id: "SEC-DOM-XSS-SINK".into(),
                                    severity: "HIGH".into(),
                                    title: format!("DOM XSS Sink Called: {}", sink.sink_name),
                                    description: format!(
                                        "A dangerous sink function was called with client payload: {}",
                                        sink.argument
                                    ),
                                    recommendation: "Sanitize user inputs before passing to eval(), innerHTML, or document.write().".into(),
                                    evidence: sink.argument,
                                    curl_command: format!("curl -i '{}'", target_url),
                                });
                            }
                        }

                        let traffic_item = GuiTrafficItem {
                            id: format!("req-{}", res.request_id),
                            method: "GET".into(),
                            status: res.status,
                            url: target_url.clone(),
                            size: res.body.len(),
                            elapsed_ms: res.elapsed_ms,
                            request_raw: format!(
                                "GET {} HTTP/1.1\r\n{}",
                                target_url,
                                req.headers.to_raw_string()
                            ),
                            response_raw: format!(
                                "HTTP/1.1 {} {}\r\n{}\r\n{}",
                                res.status,
                                res.reason,
                                res.headers.to_raw_string(),
                                body_str
                            ),
                        };

                        let final_url = res
                            .headers
                            .get("x-final-url")
                            .unwrap_or(&target_url)
                            .to_string();

                        let _ = tx.send(PageNavigationEvent::Success(Box::new(PageLoadedEvent {
                            tab_id,
                            url: final_url,
                            status: res.status,
                            elapsed_ms: res.elapsed_ms,
                            raw_html: body_str,
                            parsed_dom: dom,
                            title,
                            findings,
                            traffic_item,
                        })));
                    }
                    Err(e) => {
                        tracing::error!("Navigation error for {}: {}", target_url, e);
                        let _ = tx.send(PageNavigationEvent::Failure {
                            tab_id,
                            url: target_url,
                            error: e.to_string(),
                        });
                    }
                }
            });
        }
    }

    pub fn poll_network_events(&mut self) {
        while let Ok(event) = self.page_rx.try_recv() {
            match event {
                PageNavigationEvent::Success(loaded) => {
                    let tab_id = loaded.tab_id;
                    let loaded_url = loaded.url.clone();
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.is_loading = false;
                        tab.error_msg = None;
                        tab.url = loaded_url.clone();
                        tab.status_code = Some(loaded.status);
                        tab.elapsed_ms = Some(loaded.elapsed_ms);
                        tab.raw_html = loaded.raw_html;
                        tab.parsed_dom = Some(loaded.parsed_dom);
                        tab.title = loaded.title;
                    }

                    if self.active_tab_id == tab_id {
                        self.omnibox_input = loaded_url;
                    }

                    // Populate X-Mode security data automatically
                    self.interceptor_state
                        .traffic_items
                        .push(loaded.traffic_item);
                    for f in loaded.findings {
                        if !self.scanner_state.findings.iter().any(|existing| {
                            existing.rule_id == f.rule_id && existing.evidence == f.evidence
                        }) {
                            self.scanner_state.findings.push(f);
                        }
                    }
                }
                PageNavigationEvent::Failure {
                    tab_id,
                    url: _,
                    error,
                } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.is_loading = false;
                        tab.error_msg = Some(error);
                    }
                }
            }
        }
    }

    pub fn render_panels(&mut self, ctx: &egui::Context) {
        apply_cyber_theme(ctx);
        self.poll_network_events();

        // 1. Top Browser Navigation Header
        egui::TopBottomPanel::top("browser_top_header").show(ctx, |ui| {
            if self.tab_nav_layout == TabNavLayout::Top {
                self.render_tab_strip(ui);
                ui.add_space(2.0);
            }
            self.render_omnibox_bar(ui);
            if self.show_favorites_bar {
                ui.add_space(2.0);
                self.render_favorites_bar(ui);
            }
            if self.x_mode_enabled {
                ui.add_space(3.0);
                self.render_x_mode_toolbar(ui);
            }
        });

        // 2. Left Vertical Tab Sidebar (Zen / Arc Modern Mode)
        if self.tab_nav_layout == TabNavLayout::Sidebar {
            egui::SidePanel::left("zen_vertical_tab_sidebar")
                .exact_width(220.0)
                .resizable(false)
                .show(ctx, |ui| {
                    self.render_vertical_tab_sidebar(ui);
                });
        }

        // 3. Bottom Status Bar
        egui::TopBottomPanel::bottom("browser_status_bar").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // 4. Central Webpage / X-Mode Layout
        if !self.x_mode_enabled {
            // Pure Modern Browser Mode (100% Screen Space)
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_browser_content(ui);
            });
        } else {
            // X-Mode is Active!
            match self.x_mode_layout {
                XModeLayout::SplitDrawer => {
                    // Split: Browser View on Top, Security Tools on Bottom
                    egui::TopBottomPanel::bottom("x_mode_drawer")
                        .resizable(true)
                        .min_height(160.0)
                        .default_height(280.0)
                        .show(ctx, |ui| {
                            self.render_x_mode_workbench(ui);
                        });

                    egui::CentralPanel::default().show(ctx, |ui| {
                        self.render_browser_content(ui);
                    });
                }
                XModeLayout::FullscreenTools => {
                    // Fullscreen Security Workbench View
                    egui::CentralPanel::default().show(ctx, |ui| {
                        self.render_x_mode_workbench(ui);
                    });
                }
            }
        }
    }

    fn render_tab_strip(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let mut close_tab_id = None;

            for tab in &self.tabs {
                let is_active = tab.id == self.active_tab_id;
                let bg_color = if is_active {
                    Color32::from_rgb(33, 38, 45)
                } else {
                    Color32::from_rgb(18, 21, 26)
                };

                Frame::NONE
                    .fill(bg_color)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active {
                            Color32::from_rgb(88, 166, 255)
                        } else {
                            Color32::from_rgb(48, 54, 61)
                        },
                    ))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let tag = site_pill_tag(&tab.url);
                            ui.label(
                                RichText::new(format!("[{}]", tag))
                                    .size(10.0)
                                    .color(Color32::from_rgb(88, 166, 255))
                                    .strong(),
                            );

                            let label_text = if tab.title.len() > 18 {
                                format!("{}...", &tab.title[..16])
                            } else {
                                tab.title.clone()
                            };

                            let title_btn =
                                ui.selectable_label(is_active, RichText::new(label_text).strong());
                            if title_btn.clicked() {
                                self.active_tab_id = tab.id;
                                self.omnibox_input = tab.url.clone();
                            }

                            if tab.is_loading {
                                ui.label(
                                    RichText::new("..")
                                        .color(Color32::from_rgb(88, 166, 255))
                                        .strong(),
                                );
                            }

                            if self.tabs.len() > 1 && ui.small_button("x").clicked() {
                                close_tab_id = Some(tab.id);
                            }
                        });
                    });
                ui.add_space(2.0);
            }

            if let Some(id) = close_tab_id {
                self.tabs.retain(|t| t.id != id);
                if self.active_tab_id == id {
                    if let Some(first) = self.tabs.first() {
                        self.active_tab_id = first.id;
                        self.omnibox_input = first.url.clone();
                    }
                }
            }

            // New Tab '+' button
            if ui.button("+").clicked() {
                let new_id = self.next_tab_id;
                self.next_tab_id += 1;
                self.tabs
                    .push(BrowserTab::new(new_id, "excalibur://newtab"));
                self.active_tab_id = new_id;
                self.omnibox_input = "excalibur://newtab".to_string();
            }

            // Right-aligned X-Mode Toggle & Menu
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // THE X-MODE TOGGLE SWITCH
                let toggle_text = if self.x_mode_enabled {
                    RichText::new("[ X-MODE: ACTIVE ]")
                        .color(Color32::from_rgb(248, 81, 73))
                        .strong()
                } else {
                    RichText::new("[ X-MODE: OFF ]").color(Color32::from_rgb(139, 148, 158))
                };

                let x_btn = ui.button(toggle_text);
                if x_btn.clicked() {
                    self.x_mode_enabled = !self.x_mode_enabled;
                }

                ui.separator();
                let zen_toggle = match self.tab_nav_layout {
                    TabNavLayout::Top => "[Zen Sidebar]",
                    TabNavLayout::Sidebar => "[Top Tabs]",
                };
                if ui.small_button(zen_toggle).clicked() {
                    self.tab_nav_layout = match self.tab_nav_layout {
                        TabNavLayout::Top => TabNavLayout::Sidebar,
                        TabNavLayout::Sidebar => TabNavLayout::Top,
                    };
                }

                ui.label(
                    RichText::new("Excalibur")
                        .strong()
                        .color(Color32::from_rgb(88, 166, 255)),
                );
            });
        });
    }

    fn render_vertical_tab_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("EXCALIBUR")
                    .size(16.0)
                    .strong()
                    .color(Color32::from_rgb(88, 166, 255)),
            );
            ui.label(
                RichText::new("[ ZEN ]")
                    .size(11.0)
                    .color(Color32::from_rgb(63, 185, 80))
                    .strong(),
            );
        });
        ui.add_space(4.0);

        if ui
            .button(RichText::new("+ New Tab").strong().color(Color32::WHITE))
            .clicked()
        {
            let new_id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs
                .push(BrowserTab::new(new_id, "excalibur://newtab"));
            self.active_tab_id = new_id;
            self.omnibox_input = "excalibur://newtab".to_string();
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("OPEN TABS ({})", self.tabs.len()))
                .size(11.0)
                .color(Color32::from_rgb(139, 148, 158))
                .strong(),
        );
        ui.add_space(4.0);

        let mut close_tab_id = None;
        let mut select_tab_id = None;
        let mut new_omnibox = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for tab in &self.tabs {
                    let is_active = tab.id == self.active_tab_id;
                    let bg_color = if is_active {
                        Color32::from_rgb(33, 38, 45)
                    } else {
                        Color32::from_rgb(18, 21, 26)
                    };

                    Frame::NONE
                        .fill(bg_color)
                        .stroke(Stroke::new(
                            1.0_f32,
                            if is_active {
                                Color32::from_rgb(88, 166, 255)
                            } else {
                                Color32::from_rgb(48, 54, 61)
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let tag = site_pill_tag(&tab.url);
                                ui.label(
                                    RichText::new(format!("[{}]", tag))
                                        .size(10.0)
                                        .color(Color32::from_rgb(88, 166, 255))
                                        .strong(),
                                );

                                let label_text = if tab.title.len() > 14 {
                                    format!("{}...", &tab.title[..12])
                                } else {
                                    tab.title.clone()
                                };

                                if ui
                                    .selectable_label(is_active, RichText::new(label_text).strong())
                                    .clicked()
                                {
                                    select_tab_id = Some(tab.id);
                                    new_omnibox = Some(tab.url.clone());
                                }

                                if self.tabs.len() > 1 && ui.small_button("x").clicked() {
                                    close_tab_id = Some(tab.id);
                                }
                            });
                        });
                    ui.add_space(4.0);
                }
            });

        if let Some(id) = select_tab_id {
            self.active_tab_id = id;
            if let Some(url) = new_omnibox {
                self.omnibox_input = url;
            }
        }

        if let Some(id) = close_tab_id {
            self.tabs.retain(|t| t.id != id);
            if self.active_tab_id == id {
                if let Some(first) = self.tabs.first() {
                    self.active_tab_id = first.id;
                    self.omnibox_input = first.url.clone();
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        if ui.button("Switch to Top Tabs").clicked() {
            self.tab_nav_layout = TabNavLayout::Top;
        }
    }

    fn normalize_target(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            "about:blank".to_string()
        } else if trimmed == "about:blank"
            || trimmed == "excalibur://newtab"
            || trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
        {
            trimmed.to_string()
        } else if trimmed.contains('.') && !trimmed.contains(' ') {
            format!("https://{}", trimmed)
        } else {
            format!(
                "https://html.duckduckgo.com/html/?q={}",
                trimmed.replace(' ', "+")
            )
        }
    }

    fn render_omnibox_bar(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        let (can_back, can_fwd, is_https, is_internal, is_loading) =
            if let Some(tab) = self.active_tab() {
                (
                    tab.history_idx > 0,
                    tab.history_idx + 1 < tab.history.len(),
                    tab.url.starts_with("https://"),
                    tab.url.starts_with("about:") || tab.url.starts_with("excalibur:"),
                    tab.is_loading,
                )
            } else {
                (false, false, false, false, false)
            };

        Frame::NONE
            .fill(Color32::from_rgb(22, 27, 34))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                    // Back button
                    if ui.add_enabled(can_back, egui::Button::new("<")).clicked() {
                        if let Some(tab) = self.active_tab_mut() {
                            if tab.history_idx > 0 {
                                tab.history_idx -= 1;
                                let prev_url = tab.history[tab.history_idx].clone();
                                self.navigate(active_id, &prev_url);
                            }
                        }
                    }

                    // Forward button
                    if ui.add_enabled(can_fwd, egui::Button::new(">")).clicked() {
                        if let Some(tab) = self.active_tab_mut() {
                            if tab.history_idx + 1 < tab.history.len() {
                                tab.history_idx += 1;
                                let next_url = tab.history[tab.history_idx].clone();
                                self.navigate(active_id, &next_url);
                            }
                        }
                    }

                    // Reload / Cancel button
                    let reload_label = if is_loading { "X" } else { "R" };
                    if ui.button(RichText::new(reload_label).strong()).clicked() {
                        if is_loading {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.is_loading = false;
                            }
                        } else {
                            let current_url = self.omnibox_input.clone();
                            self.navigate(active_id, &current_url);
                        }
                    }

                    // Home button
                    if ui.button(RichText::new("H").strong()).clicked() {
                        self.navigate(active_id, "excalibur://newtab");
                    }

                    ui.add_space(4.0);

                    // Central Omnibox Pill
                    let omnibox_width = (ui.available_width() - 170.0).max(180.0);
                    let mut navigate_target = None;

                    Frame::NONE
                        .fill(Color32::from_rgb(11, 14, 20))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.set_width(omnibox_width);
                            ui.horizontal(|ui| {
                                // Security Pill
                                if is_internal {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(19, 35, 56))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(31, 111, 235),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("[INTERNAL]")
                                                    .size(10.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(88, 166, 255)),
                                            );
                                        });
                                } else if is_https {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(22, 42, 28))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(35, 134, 54),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("[HTTPS]")
                                                    .size(10.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(63, 185, 80)),
                                            );
                                        });
                                } else {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(40, 30, 16))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(158, 106, 3),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("[INSECURE]")
                                                    .size(10.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(210, 153, 34)),
                                            );
                                        });
                                }

                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut self.omnibox_input)
                                        .hint_text("Search or enter web address...")
                                        .desired_width(ui.available_width() - 85.0),
                                );

                                if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    navigate_target = Some(self.omnibox_input.clone());
                                }

                                if is_loading {
                                    ui.spinner();
                                }

                                if ui.small_button("Copy").clicked() {
                                    ui.ctx().copy_text(self.omnibox_input.clone());
                                }

                                if ui
                                    .button(RichText::new("Go").strong().color(Color32::WHITE))
                                    .clicked()
                                {
                                    navigate_target = Some(self.omnibox_input.clone());
                                }
                            });
                        });

                    if let Some(target) = navigate_target {
                        let normalized = Self::normalize_target(&target);
                        self.navigate(active_id, &normalized);
                    }

                    // Right Toolbar Cluster
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let fav_text = if self.show_favorites_bar {
                            "[Favs: On]"
                        } else {
                            "[Favs: Off]"
                        };
                        if ui.small_button(fav_text).clicked() {
                            self.show_favorites_bar = !self.show_favorites_bar;
                        }

                        if ui.small_button("DOM").clicked() {
                            self.x_mode_enabled = true;
                            self.x_mode_tab = XModeTab::DomAndScripts;
                        }

                        if self.tab_nav_layout == TabNavLayout::Sidebar {
                            let toggle_text = if self.x_mode_enabled {
                                RichText::new("[ X-MODE: ACTIVE ]")
                                    .color(Color32::from_rgb(248, 81, 73))
                                    .strong()
                            } else {
                                RichText::new("[ X-MODE: OFF ]")
                                    .color(Color32::from_rgb(139, 148, 158))
                            };
                            if ui.button(toggle_text).clicked() {
                                self.x_mode_enabled = !self.x_mode_enabled;
                            }
                        }
                    });
                });
            });
    }

    fn render_favorites_bar(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        Frame::NONE
            .fill(Color32::from_rgb(16, 20, 26))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(33, 38, 45)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("FAVORITES:")
                            .size(10.0)
                            .strong()
                            .color(Color32::from_rgb(139, 148, 158)),
                    );

                    let shortcuts = [
                        ("OWASP", "https://owasp.org"),
                        ("PortSwigger", "https://portswigger.net/web-security"),
                        ("HackerNews", "https://news.ycombinator.com"),
                        ("GitHub", "https://github.com"),
                        ("Example", "https://example.com"),
                        ("Cloudflare Trace", "https://cloudflare.com/cdn-cgi/trace"),
                        ("HTTPBin", "https://httpbin.org/html"),
                        ("Local Proxy", "http://127.0.0.1:8080"),
                    ];

                    for (name, target) in shortcuts {
                        if ui.small_button(name).clicked() {
                            self.navigate(active_id, target);
                        }
                    }
                });
            });
    }

    fn render_x_mode_toolbar(&mut self, ui: &mut Ui) {
        Frame::NONE
            .fill(Color32::from_rgb(22, 27, 34))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("X-MODE WORKBENCH:")
                            .color(Color32::from_rgb(248, 81, 73))
                            .strong(),
                    );

                    let tools = [
                        (XModeTab::Interceptor, "[1] Live Interceptor"),
                        (XModeTab::Repeater, "[2] Request Repeater"),
                        (XModeTab::Scanner, "[3] Vulnerability Radar"),
                        (XModeTab::DomAndScripts, "[4] DOM & Scripts"),
                        (XModeTab::ProxyManager, "[5] MITM Proxy"),
                    ];

                    for (tab_val, label) in tools {
                        let is_sel = self.x_mode_tab == tab_val;
                        let text = if is_sel {
                            RichText::new(label)
                                .color(Color32::from_rgb(88, 166, 255))
                                .strong()
                        } else {
                            RichText::new(label).color(Color32::from_rgb(201, 209, 217))
                        };
                        if ui.selectable_label(is_sel, text).clicked() {
                            self.x_mode_tab = tab_val;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("x Close").clicked() {
                            self.x_mode_enabled = false;
                        }
                        ui.selectable_value(
                            &mut self.x_mode_layout,
                            XModeLayout::FullscreenTools,
                            "Fullscreen",
                        );
                        ui.selectable_value(
                            &mut self.x_mode_layout,
                            XModeLayout::SplitDrawer,
                            "Split Drawer",
                        );
                    });
                });
            });
    }

    fn render_browser_content(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        let (url, dom, is_loading, error_msg) = if let Some(tab) = self.active_tab() {
            (
                tab.url.clone(),
                tab.parsed_dom.clone(),
                tab.is_loading,
                tab.error_msg.clone(),
            )
        } else {
            ("about:blank".to_string(), None, false, None)
        };

        if let Some(action) =
            self.browser_renderer
                .render(ui, &url, dom.as_ref(), is_loading, error_msg.as_deref())
        {
            match action {
                BrowserAction::Navigate(target) => {
                    self.navigate(active_id, &target);
                }
            }
        }
    }

    fn render_x_mode_workbench(&mut self, ui: &mut Ui) {
        match self.x_mode_tab {
            XModeTab::Interceptor => {
                if let Some(action) = render_interceptor_view(ui, &mut self.interceptor_state) {
                    match action {
                        InterceptorAction::SendToRepeater(item) => {
                            self.repeater_state.method = item.method;
                            self.repeater_state.url = item.url;
                            self.x_mode_tab = XModeTab::Repeater;
                        }
                    }
                }
            }
            XModeTab::Repeater => render_repeater_view(ui, &mut self.repeater_state),
            XModeTab::Scanner => render_scanner_view(ui, &mut self.scanner_state),
            XModeTab::DomAndScripts => {
                let (dom, raw_html) = if let Some(tab) = self.active_tab() {
                    (tab.parsed_dom.clone(), tab.raw_html.clone())
                } else {
                    (None, String::new())
                };
                render_dom_inspector(ui, &mut self.dom_inspector_state, dom.as_ref(), &raw_html);
            }
            XModeTab::ProxyManager => render_proxy_manager(ui, &mut self.proxy_state),
        }
    }

    fn render_status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(tab) = self.active_tab() {
                if tab.is_loading {
                    ui.label(
                        RichText::new("[..] CONNECTING...")
                            .color(Color32::from_rgb(88, 166, 255))
                            .strong(),
                    );
                } else if let Some(status) = tab.status_code {
                    let status_color = if status < 300 {
                        Color32::from_rgb(63, 185, 80)
                    } else if status < 400 {
                        Color32::from_rgb(210, 153, 34)
                    } else {
                        Color32::from_rgb(248, 81, 73)
                    };
                    ui.label(
                        RichText::new(format!("[OK] HTTP {}", status))
                            .color(status_color)
                            .strong(),
                    );
                    if let Some(ms) = tab.elapsed_ms {
                        ui.label(format!("({} ms)", ms));
                    }
                } else {
                    ui.label(
                        RichText::new("[READY]")
                            .color(Color32::from_rgb(63, 185, 80))
                            .strong(),
                    );
                }
                ui.separator();
                let display_url = if tab.url.len() > 50 {
                    format!("{}...", &tab.url[..47])
                } else {
                    tab.url.clone()
                };
                ui.label(format!("URL: {}", display_url));
                if let Some(ref dom) = tab.parsed_dom {
                    ui.separator();
                    let nodes_count = count_dom_nodes(dom);
                    ui.label(format!("DOM: {} nodes", nodes_count));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let issues = self.scanner_state.findings.len();
                let issue_text = if issues > 0 {
                    RichText::new(format!("[ Radar: {} Flaws ]", issues))
                        .color(Color32::from_rgb(248, 81, 73))
                        .strong()
                } else {
                    RichText::new("[ Radar: 0 Flaws ]").color(Color32::from_rgb(63, 185, 80))
                };

                if ui.button(issue_text).clicked() {
                    self.x_mode_enabled = true;
                    self.x_mode_tab = XModeTab::Scanner;
                }

                ui.separator();
                let captured = self.interceptor_state.traffic_items.len();
                let proxy_btn =
                    ui.button(format!("Proxy: 127.0.0.1:8080 (Captured: {})", captured));
                if proxy_btn.clicked() {
                    self.x_mode_enabled = true;
                    self.x_mode_tab = XModeTab::Interceptor;
                }

                ui.separator();
                let layout_btn = match self.tab_nav_layout {
                    TabNavLayout::Top => "[Zen Sidebar]",
                    TabNavLayout::Sidebar => "[Top Tabs]",
                };
                if ui.small_button(layout_btn).clicked() {
                    self.tab_nav_layout = match self.tab_nav_layout {
                        TabNavLayout::Top => TabNavLayout::Sidebar,
                        TabNavLayout::Sidebar => TabNavLayout::Top,
                    };
                }
            });
        });
    }
}

fn site_pill_tag(url: &str) -> &'static str {
    if url.contains("owasp") {
        "SEC"
    } else if url.contains("portswigger") {
        "LAB"
    } else if url.contains("github") {
        "GIT"
    } else if url.contains("ycombinator") {
        "HN"
    } else if url.contains("cloudflare") {
        "CDN"
    } else if url.contains("httpbin") {
        "BIN"
    } else if url.contains("cve") {
        "CVE"
    } else if url.contains("about:blank") || url.contains("newtab") {
        "TAB"
    } else if url.contains("127.0.0.1") || url.contains("localhost") {
        "MITM"
    } else {
        "WEB"
    }
}

fn count_dom_nodes(node: &DomNode) -> usize {
    1 + node.children.iter().map(count_dom_nodes).sum::<usize>()
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_panels(ctx);
    }
}
