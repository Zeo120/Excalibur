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
pub enum XModeTab {
    DevWebView,
    Interceptor,
    Repeater,
    Scanner,
    DomAndScripts,
    ProxyManager,
}

#[derive(PartialEq, Eq, Clone, Copy)]
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

    /// Launches the Full Web Engine (dedicated chromeless browser window routed via Excalibur proxy).
    pub fn launch_full_web_engine(&self, url: &str) {
        if cfg!(test) || std::env::var("EXCALIBUR_HEADLESS").is_ok() {
            return;
        }

        let edge_paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];

        let mut launched = false;
        let temp_profile = std::env::temp_dir().join("excalibur_full_web_profile");

        for path in edge_paths {
            if std::path::Path::new(path).exists() {
                let _ = std::process::Command::new(path)
                    .arg(format!("--app={}", url))
                    .arg("--proxy-server=http://127.0.0.1:8080")
                    .arg("--ignore-certificate-errors")
                    .arg(format!("--user-data-dir={}", temp_profile.display()))
                    .spawn();
                launched = true;
                break;
            }
        }

        if !launched {
            let _ = webbrowser::open(url);
        }
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

            // Launch Full Web View (Real-world chromeless browser engine via proxy) by default
            self.launch_full_web_engine(&normalized);

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
            self.render_tab_strip(ui);
            ui.add_space(4.0);
            self.render_omnibox_bar(ui);
            if self.x_mode_enabled {
                ui.add_space(4.0);
                self.render_x_mode_toolbar(ui);
            }
        });

        // 2. Bottom Status Bar
        egui::TopBottomPanel::bottom("browser_status_bar").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // 3. Central Webpage / X-Mode Layout
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
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let label_text = if tab.title.len() > 20 {
                                format!("{}...", &tab.title[..18])
                            } else {
                                tab.title.clone()
                            };

                            let title_btn =
                                ui.selectable_label(is_active, RichText::new(label_text).strong());
                            if title_btn.clicked() {
                                self.active_tab_id = tab.id;
                                self.omnibox_input = tab.url.clone();
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
                self.tabs.push(BrowserTab::new(new_id, "about:blank"));
                self.active_tab_id = new_id;
                self.omnibox_input.clear();
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
                ui.label(
                    RichText::new("Excalibur Browser")
                        .strong()
                        .color(Color32::from_rgb(88, 166, 255)),
                );
            });
        });
    }

    fn render_omnibox_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let active_id = self.active_tab_id;
            let (can_back, can_fwd, is_https) = if let Some(tab) = self.active_tab() {
                (
                    tab.history_idx > 0,
                    tab.history_idx + 1 < tab.history.len(),
                    tab.url.starts_with("https://"),
                )
            } else {
                (false, false, false)
            };

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

            // Reload button
            if ui.button("Reload").clicked() {
                let current_url = self.omnibox_input.clone();
                self.navigate(active_id, &current_url);
            }

            // Home button
            if ui.button("Home").clicked() {
                self.navigate(active_id, "about:blank");
            }

            // HTTPS security lock badge
            let lock_badge = if is_https {
                RichText::new("[HTTPS]").color(Color32::from_rgb(63, 185, 80))
            } else {
                RichText::new("[HTTP]").color(Color32::from_rgb(210, 153, 34))
            };
            ui.label(lock_badge);

            // Omnibox URL Text Bar
            let url_edit = ui.add(
                egui::TextEdit::singleline(&mut self.omnibox_input)
                    .desired_width(ui.available_width() - 200.0),
            );

            if url_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let target = self.omnibox_input.clone();
                self.navigate(active_id, &target);
            }

            if ui.button("Go").clicked() {
                let target = self.omnibox_input.clone();
                self.navigate(active_id, &target);
            }

            // Dual-Engine Switcher: Launch Full Real-World Browser View
            let full_web_btn = ui.button(
                RichText::new("Full Web View")
                    .color(Color32::from_rgb(88, 166, 255))
                    .strong(),
            );
            if full_web_btn.clicked() {
                let target = if self.omnibox_input.is_empty() {
                    "https://google.com"
                } else {
                    &self.omnibox_input
                };
                self.launch_full_web_engine(target);
            }
        });
    }

    fn render_x_mode_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("X-MODE WORKBENCH:")
                    .color(Color32::from_rgb(248, 81, 73))
                    .strong(),
            );

            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::DevWebView,
                "[1] Dev Web View",
            );
            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::Interceptor,
                "[2] Live Interceptor",
            );
            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::Repeater,
                "[3] Request Repeater",
            );
            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::Scanner,
                "[4] Vulnerability Radar",
            );
            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::DomAndScripts,
                "[5] DOM & Scripts",
            );
            ui.selectable_value(
                &mut self.x_mode_tab,
                XModeTab::ProxyManager,
                "[6] MITM Proxy",
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
    }

    fn render_browser_content(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        let (url, is_loading, status_code, elapsed_ms, error_msg) =
            if let Some(tab) = self.active_tab() {
                (
                    tab.url.clone(),
                    tab.is_loading,
                    tab.status_code,
                    tab.elapsed_ms,
                    tab.error_msg.clone(),
                )
            } else {
                ("about:blank".to_string(), false, None, None, None)
            };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(16.0);
                let available_width = ui.available_width();
                let content_width = available_width.min(960.0);
                let side_margin = ((available_width - content_width) / 2.0).max(16.0);

                ui.horizontal(|ui| {
                    ui.add_space(side_margin);
                    ui.vertical(|ui| {
                        ui.set_max_width(content_width);

                        // 1. Hero Card: Full Web Engine Status & Controls
                        Frame::NONE
                            .fill(Color32::from_rgb(22, 27, 34))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                            .inner_margin(Margin::same(16))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("[DEFAULT ENGINE]")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(63, 185, 80)),
                                    );
                                    ui.label(
                                        RichText::new("Full Web-View Browser")
                                            .size(18.0)
                                            .strong()
                                            .color(Color32::WHITE),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new("MITM Proxy: 127.0.0.1:8080")
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(88, 166, 255)),
                                            );
                                        },
                                    );
                                });

                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(
                                        "Excalibur routes real-world browser traffic through its zero-trust inspection engine.\nAll requests, cookies, headers, and payloads are captured live into your security pipeline.",
                                    )
                                    .size(13.0)
                                    .color(Color32::from_rgb(139, 148, 158)),
                                );

                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    let open_btn = ui.button(
                                        RichText::new("Open Full Web View")
                                            .size(14.0)
                                            .strong()
                                            .color(Color32::from_rgb(88, 166, 255)),
                                    );
                                    if open_btn.clicked() {
                                        let target = if url.is_empty() || url == "about:blank" {
                                            "https://google.com"
                                        } else {
                                            &url
                                        };
                                        self.launch_full_web_engine(target);
                                    }

                                    let dev_view_btn = ui.button(
                                        RichText::new("Open Dev Web View (X-Mode)")
                                            .size(13.0)
                                            .color(Color32::from_rgb(210, 153, 34)),
                                    );
                                    if dev_view_btn.clicked() {
                                        self.x_mode_enabled = true;
                                        self.x_mode_tab = XModeTab::DevWebView;
                                    }

                                    let x_btn = ui.button(
                                        RichText::new("Launch X-Mode Tools")
                                            .size(13.0)
                                            .strong()
                                            .color(Color32::from_rgb(248, 81, 73)),
                                    );
                                    if x_btn.clicked() {
                                        self.x_mode_enabled = true;
                                    }
                                });
                            });

                        ui.add_space(16.0);

                        // 2. Active Session Card
                        Frame::NONE
                            .fill(Color32::from_rgb(18, 22, 28))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                            .inner_margin(Margin::same(14))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Active Session:")
                                            .size(13.0)
                                            .strong()
                                            .color(Color32::from_rgb(139, 148, 158)),
                                    );
                                    ui.label(
                                        RichText::new(&url)
                                            .size(14.0)
                                            .strong()
                                            .color(Color32::from_rgb(88, 166, 255)),
                                    );
                                });

                                ui.add_space(6.0);
                                if is_loading {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label("Connecting and inspecting response stream...");
                                    });
                                } else if let Some(err) = error_msg {
                                    ui.label(
                                        RichText::new(format!("Connection Alert: {}", err))
                                            .color(Color32::from_rgb(248, 81, 73)),
                                    );
                                } else if let Some(status) = status_code {
                                    ui.horizontal(|ui| {
                                        let status_color = if status < 300 {
                                            Color32::from_rgb(63, 185, 80)
                                        } else if status < 400 {
                                            Color32::from_rgb(210, 153, 34)
                                        } else {
                                            Color32::from_rgb(248, 81, 73)
                                        };
                                        ui.label(
                                            RichText::new(format!("HTTP {}", status))
                                                .color(status_color)
                                                .strong(),
                                        );
                                        if let Some(ms) = elapsed_ms {
                                            ui.label(format!("({} ms roundtrip)", ms));
                                        }
                                        ui.label("| TLS 1.3 Interception Verified");
                                    });
                                } else {
                                    ui.label("Ready. Enter any URL in the omnibox to navigate.");
                                }
                            });

                        ui.add_space(16.0);

                        // 3. Telemetry Pipeline Grid
                        ui.columns(3, |columns| {
                            // Column 1: MITM Gateway
                            columns[0].vertical(|ui| {
                                Frame::NONE
                                    .fill(Color32::from_rgb(22, 27, 34))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                    .inner_margin(Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("MITM PROXY")
                                                .size(11.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                        ui.label(
                                            RichText::new("127.0.0.1:8080")
                                                .size(16.0)
                                                .strong()
                                                .color(Color32::from_rgb(63, 185, 80)),
                                        );
                                        ui.label(
                                            RichText::new("TLS Gateway Active")
                                                .size(11.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                    });
                            });

                            // Column 2: Captured Traffic
                            let traffic_count = self.interceptor_state.traffic_items.len();
                            columns[1].vertical(|ui| {
                                Frame::NONE
                                    .fill(Color32::from_rgb(22, 27, 34))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                    .inner_margin(Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("CAPTURED TRAFFIC")
                                                .size(11.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                        ui.label(
                                            RichText::new(format!("{} Requests", traffic_count))
                                                .size(16.0)
                                                .strong()
                                                .color(Color32::from_rgb(88, 166, 255)),
                                        );
                                        if ui.small_button("Open Interceptor").clicked() {
                                            self.x_mode_enabled = true;
                                            self.x_mode_tab = XModeTab::Interceptor;
                                        }
                                    });
                            });

                            // Column 3: Vulnerabilities
                            let vuln_count = self.scanner_state.findings.len();
                            let vuln_color = if vuln_count > 0 {
                                Color32::from_rgb(248, 81, 73)
                            } else {
                                Color32::from_rgb(63, 185, 80)
                            };
                            columns[2].vertical(|ui| {
                                Frame::NONE
                                    .fill(Color32::from_rgb(22, 27, 34))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                    .inner_margin(Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("VULNERABILITY RADAR")
                                                .size(11.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                        ui.label(
                                            RichText::new(format!("{} Findings", vuln_count))
                                                .size(16.0)
                                                .strong()
                                                .color(vuln_color),
                                        );
                                        if ui.small_button("Open Radar").clicked() {
                                            self.x_mode_enabled = true;
                                            self.x_mode_tab = XModeTab::Scanner;
                                        }
                                    });
                            });
                        });

                        ui.add_space(20.0);

                        // 4. Speed Dial / Quick Launch Targets
                        ui.heading("Quick Navigation Targets (Full Web View)");
                        ui.add_space(8.0);

                        let targets = [
                            ("Google", "https://google.com", "Search engine & web apps"),
                            ("OWASP", "https://owasp.org", "Open Worldwide Application Security Project"),
                            ("Web Security Academy", "https://portswigger.net/web-security", "PortSwigger interactive security labs"),
                            ("GitHub", "https://github.com", "Code hosting & developer platform"),
                            ("Example Domain", "https://example.com", "RFC 2606 reference standard"),
                            ("Cloudflare Trace", "https://cloudflare.com/cdn-cgi/trace", "Edge IP and TLS network diagnostics"),
                        ];

                        let mut navigate_to = None;
                        for (name, target_url, desc) in targets {
                            Frame::NONE
                                .fill(Color32::from_rgb(22, 27, 34))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(name)
                                                    .size(15.0)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            );
                                            ui.label(
                                                RichText::new(desc)
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(139, 148, 158)),
                                            );
                                            ui.label(
                                                RichText::new(target_url)
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(88, 166, 255)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("Launch").clicked() {
                                                    navigate_to = Some(target_url.to_string());
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }

                        if let Some(target) = navigate_to {
                            self.navigate(active_id, &target);
                        }

                        // 5. Recent Security Insights (if any)
                        if !self.scanner_state.findings.is_empty() {
                            ui.add_space(16.0);
                            ui.heading("Active Security Insights");
                            ui.add_space(6.0);

                            for finding in self.scanner_state.findings.iter().take(3) {
                                Frame::NONE
                                    .fill(Color32::from_rgb(26, 20, 24))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(248, 81, 73)))
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("[{}]", finding.severity))
                                                    .strong()
                                                    .color(Color32::from_rgb(248, 81, 73)),
                                            );
                                            ui.label(
                                                RichText::new(&finding.title)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            );
                                        });
                                        ui.label(
                                            RichText::new(&finding.description)
                                                .size(12.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                    });
                                ui.add_space(4.0);
                            }
                        }

                        ui.add_space(32.0);
                    });
                    ui.add_space(side_margin);
                });
            });
    }

    fn render_x_mode_workbench(&mut self, ui: &mut Ui) {
        match self.x_mode_tab {
            XModeTab::DevWebView => {
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

                if let Some(action) = self.browser_renderer.render(
                    ui,
                    &url,
                    dom.as_ref(),
                    is_loading,
                    error_msg.as_deref(),
                ) {
                    match action {
                        BrowserAction::Navigate(target) => {
                            self.navigate(active_id, &target);
                        }
                    }
                }
            }
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
                    ui.label(RichText::new("Connecting...").color(Color32::from_rgb(88, 166, 255)));
                } else if let Some(status) = tab.status_code {
                    let status_color = if status < 300 {
                        Color32::from_rgb(63, 185, 80)
                    } else if status < 400 {
                        Color32::from_rgb(210, 153, 34)
                    } else {
                        Color32::from_rgb(248, 81, 73)
                    };
                    ui.label(
                        RichText::new(format!("HTTP {}", status))
                            .color(status_color)
                            .strong(),
                    );
                    if let Some(ms) = tab.elapsed_ms {
                        ui.label(format!("({} ms)", ms));
                    }
                } else {
                    ui.label("Ready");
                }
                ui.separator();
                ui.label(format!("URL: {}", tab.url));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let issues = self.scanner_state.findings.len();
                let issue_text = if issues > 0 {
                    RichText::new(format!("[ X-Mode: {} Security Flaws ]", issues))
                        .color(Color32::from_rgb(248, 81, 73))
                        .strong()
                } else {
                    RichText::new("[ X-Mode: 0 Flaws ]").color(Color32::from_rgb(63, 185, 80))
                };

                if ui.button(issue_text).clicked() {
                    self.x_mode_enabled = true;
                    self.x_mode_tab = XModeTab::Scanner;
                }

                ui.separator();
                ui.label(format!(
                    "Proxy: 127.0.0.1:8080 (Captured: {})",
                    self.interceptor_state.traffic_items.len()
                ));
            });
        });
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_panels(ctx);
    }
}
