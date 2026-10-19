use crate::theme::{apply_cyber_theme, configure_fonts};
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
    storage::{BrowserBookmarkEntry, OmniboxMatch, OmniboxMatchType, WorkspaceStorage},
};
use excalibur_dom::{DomNode, HtmlParser};
use excalibur_script::JsRuntime;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

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
    pub zoom: f32,
    pub show_find: bool,
    pub find_query: String,
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
            zoom: 1.0,
            show_find: false,
            find_query: String::new(),
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

    // Storage and persistent browser engine state
    pub storage: Arc<WorkspaceStorage>,
    pub cached_bookmarks: Vec<BrowserBookmarkEntry>,
    pub closed_tabs_history: Vec<String>,
    pub omnibox_suggestions: Vec<OmniboxMatch>,
    pub show_omnibox_dropdown: bool,
    pub focus_omnibox: bool,

    // Async page loading channels
    pub page_tx: Sender<PageNavigationEvent>,
    pub page_rx: Receiver<PageNavigationEvent>,
    pub egui_ctx: Option<egui::Context>,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure typography and install modern fonts (Segoe UI & Consolas on Windows)
        configure_fonts(&cc.egui_ctx);

        // Install loaders for web image rendering (PNG, JPEG, SVG, GIF, WEBP)
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (tx, rx) = channel();

        // Initialize persistent SQLite workspace storage
        let storage = match WorkspaceStorage::open("excalibur_browser.db") {
            Ok(s) => Arc::new(s),
            Err(e) => {
                tracing::warn!("Falling back to in-memory workspace storage: {}", e);
                Arc::new(
                    WorkspaceStorage::in_memory()
                        .expect("Failed to initialize in-memory workspace storage"),
                )
            }
        };

        let cached_bookmarks = storage.list_browser_bookmarks().unwrap_or_default();
        let initial_layout = match storage.get_browser_setting("tab_layout") {
            Ok(Some(ref l)) if l == "sidebar" => TabNavLayout::Sidebar,
            _ => TabNavLayout::Top,
        };

        // Start background MITM proxy listener for external tools & Full Web Engine
        let proxy_storage = storage.clone();
        tokio::spawn(async move {
            if let Ok(ca) = excalibur_proxy::CertificateAuthority::new_ephemeral() {
                let ca = Arc::new(ca);
                let (event_tx, _event_rx) = tokio::sync::mpsc::channel(256);
                let interceptor = Arc::new(excalibur_proxy::TrafficInterceptor::new(event_tx));
                if let Ok(bind_addr) = "127.0.0.1:8080".parse() {
                    if let Ok(server) = excalibur_proxy::ProxyServer::new(
                        bind_addr,
                        ca,
                        interceptor,
                        Some(proxy_storage),
                        None,
                    ) {
                        let _ = server.run().await;
                    }
                }
            }
        });

        let mut app = Self {
            tabs: Vec::new(),
            active_tab_id: 1,
            next_tab_id: 2,
            omnibox_input: "excalibur://newtab".to_string(),
            browser_renderer: BrowserRenderer::new(),

            tab_nav_layout: initial_layout,
            show_favorites_bar: true,

            x_mode_enabled: false,
            x_mode_tab: XModeTab::Interceptor,
            x_mode_layout: XModeLayout::SplitDrawer,

            interceptor_state: InterceptorViewState::default(),
            repeater_state: RepeaterViewState::default(),
            scanner_state: ScannerViewState::default(),
            dom_inspector_state: DomInspectorState::default(),
            proxy_state: ProxyManagerState::default(),

            storage,
            cached_bookmarks,
            closed_tabs_history: Vec::new(),
            omnibox_suggestions: Vec::new(),
            show_omnibox_dropdown: false,
            focus_omnibox: false,

            page_tx: tx,
            page_rx: rx,
            egui_ctx: Some(cc.egui_ctx.clone()),
        };

        // Create initial tab and load default Start Page
        let initial_tab = BrowserTab::new(1, "excalibur://newtab");
        app.tabs.push(initial_tab);
        app.navigate(1, "excalibur://newtab");

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
        let normalized = if url == "about:blank"
            || url == "excalibur://newtab"
            || url == "excalibur://history"
            || url == "excalibur://bookmarks"
            || url == "excalibur://settings"
            || url == "excalibur://about"
            || url.starts_with("excalibur://")
        {
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

            if normalized == "about:blank"
                || normalized == "excalibur://newtab"
                || normalized.starts_with("excalibur://")
            {
                tab.is_loading = false;
                tab.title = match normalized.as_str() {
                    "excalibur://history" => "History".to_string(),
                    "excalibur://bookmarks" => "Bookmarks".to_string(),
                    "excalibur://settings" => "Settings".to_string(),
                    "excalibur://about" => "About Excalibur".to_string(),
                    _ => "New Tab".to_string(),
                };
                tab.parsed_dom = None;
                tab.raw_html.clear();
                return;
            }

            let ua_preset = self
                .storage
                .get_browser_setting("user_agent_preset")
                .unwrap_or_default()
                .unwrap_or_else(|| "Excalibur".to_string());

            let user_agent = match ua_preset.as_str() {
                "Chrome" => {
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"
                        .to_string()
                }
                "Firefox" => {
                    "Mozilla/5.0 (X11; Linux x86_64; rv:136.0) Gecko/20100101 Firefox/136.0"
                        .to_string()
                }
                "Safari" => {
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Safari/605.1.15"
                        .to_string()
                }
                "Curl" => "curl/8.8.0".to_string(),
                _ => {
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Excalibur/1.0 (Security Browser; Non-Chromium)"
                        .to_string()
                }
            };

            let tx = self.page_tx.clone();
            let target_url = normalized.clone();
            let ctx_opt = self.egui_ctx.clone();

            tokio::spawn(async move {
                let client = match HttpClient::new(ClientOptions::default()) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to initialize HTTP client: {}", e);
                        return;
                    }
                };

                let mut req = HttpRequest::new("GET", &target_url);
                req.headers.append("User-Agent", &user_agent);
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
                        if let Some(ctx) = ctx_opt {
                            ctx.request_repaint();
                        }
                    }
                    Err(e) => {
                        tracing::error!("Navigation error for {}: {}", target_url, e);
                        let _ = tx.send(PageNavigationEvent::Failure {
                            tab_id,
                            url: target_url,
                            error: e.to_string(),
                        });
                        if let Some(ctx) = ctx_opt {
                            ctx.request_repaint();
                        }
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
                    let loaded_title = loaded.title.clone();

                    // Record history in persistent SQLite storage
                    let _ = self
                        .storage
                        .record_browser_visit(&loaded_url, &loaded_title);
                    self.cached_bookmarks =
                        self.storage.list_browser_bookmarks().unwrap_or_default();

                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.is_loading = false;
                        tab.error_msg = None;
                        tab.url = loaded_url.clone();
                        tab.status_code = Some(loaded.status);
                        tab.elapsed_ms = Some(loaded.elapsed_ms);
                        tab.raw_html = loaded.raw_html;
                        tab.parsed_dom = Some(loaded.parsed_dom);
                        tab.title = loaded_title;
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

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let (
            ctrl,
            shift,
            key_t,
            key_w,
            key_l,
            key_r,
            key_h,
            key_d,
            key_f,
            key_plus,
            key_minus,
            key_0,
            key_f5,
            key_f6,
        ) = ctx.input(|i| {
            let ctrl = i.modifiers.command || i.modifiers.ctrl;
            let shift = i.modifiers.shift;
            (
                ctrl,
                shift,
                i.key_pressed(egui::Key::T),
                i.key_pressed(egui::Key::W),
                i.key_pressed(egui::Key::L),
                i.key_pressed(egui::Key::R),
                i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::D),
                i.key_pressed(egui::Key::F),
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                i.key_pressed(egui::Key::Minus),
                i.key_pressed(egui::Key::Num0),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::F6),
            )
        });

        // Ctrl+T: New Tab
        if ctrl && !shift && key_t {
            let new_id = self.next_tab_id;
            self.next_tab_id += 1;
            self.tabs
                .push(BrowserTab::new(new_id, "excalibur://newtab"));
            self.active_tab_id = new_id;
            self.omnibox_input = "excalibur://newtab".to_string();
        }

        // Ctrl+Shift+T: Reopen closed tab
        if ctrl && shift && key_t {
            if let Some(reopen_url) = self.closed_tabs_history.pop() {
                let new_id = self.next_tab_id;
                self.next_tab_id += 1;
                self.tabs.push(BrowserTab::new(new_id, &reopen_url));
                self.active_tab_id = new_id;
                self.navigate(new_id, &reopen_url);
            }
        }

        // Ctrl+W: Close active tab
        if ctrl && !shift && key_w && self.tabs.len() > 1 {
            let active_id = self.active_tab_id;
            if let Some(tab) = self.tabs.iter().find(|t| t.id == active_id) {
                self.closed_tabs_history.push(tab.url.clone());
            }
            self.tabs.retain(|t| t.id != active_id);
            if let Some(first) = self.tabs.first() {
                self.active_tab_id = first.id;
                self.omnibox_input = first.url.clone();
            }
        }

        // Ctrl+L or F6: Focus omnibox
        if (ctrl && key_l) || key_f6 {
            self.focus_omnibox = true;
        }

        // Ctrl+R or F5: Reload active tab
        if (ctrl && key_r) || key_f5 {
            let active_id = self.active_tab_id;
            let current_url = self.omnibox_input.clone();
            self.navigate(active_id, &current_url);
        }

        // Ctrl+H: Open History
        if ctrl && !shift && key_h {
            let active_id = self.active_tab_id;
            self.navigate(active_id, "excalibur://history");
        }

        // Ctrl+D: Toggle Bookmark
        if ctrl && !shift && key_d {
            let active_url = self.active_tab().map(|t| t.url.clone()).unwrap_or_default();
            let active_title = self
                .active_tab()
                .map(|t| t.title.clone())
                .unwrap_or_default();
            if !active_url.is_empty() && active_url != "about:blank" {
                if self.cached_bookmarks.iter().any(|b| b.url == active_url) {
                    let _ = self.storage.remove_browser_bookmark(&active_url);
                } else {
                    let _ =
                        self.storage
                            .add_browser_bookmark(&active_url, &active_title, "Favorites");
                }
                self.cached_bookmarks = self.storage.list_browser_bookmarks().unwrap_or_default();
            }
        }

        // Ctrl+F: Toggle In-Page Search
        if ctrl && !shift && key_f {
            if let Some(tab) = self.active_tab_mut() {
                tab.show_find = !tab.show_find;
            }
        }

        // Ctrl++: Zoom in
        if ctrl && key_plus {
            if let Some(tab) = self.active_tab_mut() {
                tab.zoom = (tab.zoom + 0.1).min(2.5);
            }
        }

        // Ctrl+-: Zoom out
        if ctrl && key_minus {
            if let Some(tab) = self.active_tab_mut() {
                tab.zoom = (tab.zoom - 0.1).max(0.6);
            }
        }

        // Ctrl+0: Reset zoom
        if ctrl && key_0 {
            if let Some(tab) = self.active_tab_mut() {
                tab.zoom = 1.0;
            }
        }

        // Ctrl+Tab and Ctrl+Shift+Tab
        let tab_cycle = ctx.input(|i| {
            (
                i.modifiers.command || i.modifiers.ctrl,
                i.modifiers.shift,
                i.key_pressed(egui::Key::Tab),
            )
        });
        if tab_cycle.0 && tab_cycle.2 {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == self.active_tab_id) {
                let new_idx = if tab_cycle.1 {
                    if idx == 0 {
                        self.tabs.len() - 1
                    } else {
                        idx - 1
                    }
                } else {
                    (idx + 1) % self.tabs.len()
                };
                self.active_tab_id = self.tabs[new_idx].id;
                self.omnibox_input = self.tabs[new_idx].url.clone();
            }
        }

        // Ctrl+1..9 direct tab jump
        let num_keys = [
            (egui::Key::Num1, 0),
            (egui::Key::Num2, 1),
            (egui::Key::Num3, 2),
            (egui::Key::Num4, 3),
            (egui::Key::Num5, 4),
            (egui::Key::Num6, 5),
            (egui::Key::Num7, 6),
            (egui::Key::Num8, 7),
            (egui::Key::Num9, 8),
        ];
        for (k, target_idx) in num_keys {
            if ctrl && ctx.input(|i| i.key_pressed(k)) && target_idx < self.tabs.len() {
                self.active_tab_id = self.tabs[target_idx].id;
                self.omnibox_input = self.tabs[target_idx].url.clone();
            }
        }
    }

    pub fn render_panels(&mut self, ctx: &egui::Context) {
        self.egui_ctx = Some(ctx.clone());
        apply_cyber_theme(ctx);
        self.handle_keyboard_shortcuts(ctx);
        self.poll_network_events();

        // Continuous repaint loop for smooth 60 FPS cyber loading animations
        if self.tabs.iter().any(|t| t.is_loading) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

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
                    Color32::from_rgb(20, 27, 42)
                } else {
                    Color32::from_rgb(11, 15, 23)
                };

                let stroke_color = if is_active {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgb(30, 41, 59)
                };

                Frame::NONE
                    .fill(bg_color)
                    .stroke(Stroke::new(1.0_f32, stroke_color))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(9, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let tag = site_pill_tag(&tab.url);
                            let tag_color = site_pill_color(&tab.url);
                            Frame::NONE
                                .fill(Color32::from_rgba_premultiplied(
                                    tag_color.r() / 6,
                                    tag_color.g() / 6,
                                    tag_color.b() / 6,
                                    255,
                                ))
                                .corner_radius(egui::CornerRadius::same(3))
                                .inner_margin(Margin::symmetric(4, 1))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(tag).size(9.5).color(tag_color).strong(),
                                    );
                                });

                            let label_text = if tab.title.len() > 18 {
                                format!("{}...", &tab.title[..16])
                            } else {
                                tab.title.clone()
                            };

                            let title_color = if is_active {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(148, 163, 184)
                            };

                            let title_btn = ui.selectable_label(
                                is_active,
                                RichText::new(label_text).color(title_color).strong(),
                            );
                            if title_btn.clicked() {
                                self.active_tab_id = tab.id;
                                self.omnibox_input = tab.url.clone();
                            }

                            if tab.is_loading {
                                ui.label(
                                    RichText::new("..")
                                        .color(Color32::from_rgb(56, 189, 248))
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

            // New Tab '+' button with Slate styling
            Frame::NONE
                .fill(Color32::from_rgb(15, 20, 31))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    if ui
                        .button(
                            RichText::new("+")
                                .size(13.0)
                                .strong()
                                .color(Color32::from_rgb(148, 163, 184)),
                        )
                        .clicked()
                    {
                        let new_id = self.next_tab_id;
                        self.next_tab_id += 1;
                        self.tabs
                            .push(BrowserTab::new(new_id, "excalibur://newtab"));
                        self.active_tab_id = new_id;
                        self.omnibox_input = "excalibur://newtab".to_string();
                    }
                });

            // Right-aligned X-Mode Toggle & Menu
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // THE X-MODE TOGGLE SWITCH
                let toggle_text = if self.x_mode_enabled {
                    RichText::new("X-MODE: ACTIVE")
                        .color(Color32::from_rgb(239, 68, 68))
                        .strong()
                } else {
                    RichText::new("X-MODE: OFF").color(Color32::from_rgb(148, 163, 184))
                };

                let x_btn = ui.button(toggle_text);
                if x_btn.clicked() {
                    self.x_mode_enabled = !self.x_mode_enabled;
                }

                ui.separator();
                let zen_toggle = match self.tab_nav_layout {
                    TabNavLayout::Top => "Zen Sidebar",
                    TabNavLayout::Sidebar => "Top Tabs",
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
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            });
        });
    }

    fn render_vertical_tab_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("EXCALIBUR")
                    .size(15.0)
                    .strong()
                    .color(Color32::from_rgb(241, 245, 249)),
            );
            Frame::NONE
                .fill(Color32::from_rgb(15, 23, 42))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(Margin::symmetric(5, 2))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("ZEN")
                            .size(10.0)
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    );
                });
        });
        ui.add_space(6.0);

        Frame::NONE
            .fill(Color32::from_rgb(20, 27, 42))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 51, 74)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(10, 5))
            .show(ui, |ui| {
                if ui
                    .button(
                        RichText::new("+ New Tab")
                            .size(12.5)
                            .strong()
                            .color(Color32::from_rgb(241, 245, 249)),
                    )
                    .clicked()
                {
                    let new_id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs
                        .push(BrowserTab::new(new_id, "excalibur://newtab"));
                    self.active_tab_id = new_id;
                    self.omnibox_input = "excalibur://newtab".to_string();
                }
            });

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("OPEN TABS ({})", self.tabs.len()))
                .size(10.5)
                .color(Color32::from_rgb(100, 116, 139))
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
                        Color32::from_rgb(20, 27, 42)
                    } else {
                        Color32::from_rgb(11, 15, 23)
                    };

                    let stroke_color = if is_active {
                        Color32::from_rgb(56, 189, 248)
                    } else {
                        Color32::from_rgb(30, 41, 59)
                    };

                    Frame::NONE
                        .fill(bg_color)
                        .stroke(Stroke::new(1.0_f32, stroke_color))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let tag = site_pill_tag(&tab.url);
                                let tag_color = site_pill_color(&tab.url);
                                Frame::NONE
                                    .fill(Color32::from_rgba_premultiplied(
                                        tag_color.r() / 6,
                                        tag_color.g() / 6,
                                        tag_color.b() / 6,
                                        255,
                                    ))
                                    .corner_radius(egui::CornerRadius::same(3))
                                    .inner_margin(Margin::symmetric(3, 1))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(tag).size(9.0).color(tag_color).strong(),
                                        );
                                    });

                                let label_text = if tab.title.len() > 14 {
                                    format!("{}...", &tab.title[..12])
                                } else {
                                    tab.title.clone()
                                };

                                let title_color = if is_active {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(148, 163, 184)
                                };

                                if ui
                                    .selectable_label(
                                        is_active,
                                        RichText::new(label_text).color(title_color).strong(),
                                    )
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

    fn normalize_target(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            "about:blank".to_string()
        } else if trimmed == "about:blank"
            || trimmed.starts_with("excalibur://")
            || trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
        {
            trimmed.to_string()
        } else if trimmed.contains('.') && !trimmed.contains(' ') {
            format!("https://{}", trimmed)
        } else {
            let engine = self
                .storage
                .get_browser_setting("search_engine")
                .unwrap_or_default()
                .unwrap_or_else(|| "DuckDuckGo".to_string());
            let encoded = trimmed.replace(' ', "+");
            match engine.as_str() {
                "Google" => format!("https://www.google.com/search?q={}", encoded),
                "Brave" => format!("https://search.brave.com/search?q={}", encoded),
                "SearXNG" => format!("https://searx.be/search?q={}", encoded),
                "YouTube" => format!("https://www.youtube.com/results?search_query={}", encoded),
                _ => format!("https://html.duckduckgo.com/html/?q={}", encoded),
            }
        }
    }

    fn render_omnibox_bar(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        let (can_back, can_fwd, is_https, is_internal, is_loading, current_url, current_title) =
            if let Some(tab) = self.active_tab() {
                (
                    tab.history_idx > 0,
                    tab.history_idx + 1 < tab.history.len(),
                    tab.url.starts_with("https://"),
                    tab.url.starts_with("about:") || tab.url.starts_with("excalibur:"),
                    tab.is_loading,
                    tab.url.clone(),
                    tab.title.clone(),
                )
            } else {
                (
                    false,
                    false,
                    false,
                    false,
                    false,
                    String::new(),
                    String::new(),
                )
            };

        let is_bookmarked = self.cached_bookmarks.iter().any(|b| b.url == current_url);

        Frame::NONE
            .fill(Color32::from_rgb(8, 12, 22))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                    // Back button
                    if ui
                        .add_enabled(
                            can_back,
                            egui::Button::new(
                                RichText::new("<")
                                    .color(Color32::from_rgb(56, 189, 248))
                                    .strong(),
                            ),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            if tab.history_idx > 0 {
                                tab.history_idx -= 1;
                                let prev_url = tab.history[tab.history_idx].clone();
                                self.navigate(active_id, &prev_url);
                            }
                        }
                    }

                    // Forward button
                    if ui
                        .add_enabled(
                            can_fwd,
                            egui::Button::new(
                                RichText::new(">")
                                    .color(Color32::from_rgb(56, 189, 248))
                                    .strong(),
                            ),
                        )
                        .clicked()
                    {
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
                    if ui
                        .button(
                            RichText::new(reload_label)
                                .color(Color32::from_rgb(56, 189, 248))
                                .strong(),
                        )
                        .clicked()
                    {
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
                    if ui
                        .button(
                            RichText::new("H")
                                .color(Color32::from_rgb(56, 189, 248))
                                .strong(),
                        )
                        .clicked()
                    {
                        self.navigate(active_id, "excalibur://newtab");
                    }

                    ui.add_space(4.0);

                    // Central Omnibox Pill
                    let right_tools_reserved = if self.tab_nav_layout == TabNavLayout::Sidebar {
                        450.0
                    } else {
                        350.0
                    };
                    let omnibox_width =
                        (ui.available_width() - right_tools_reserved - 10.0).max(220.0);
                    let mut navigate_target = None;
                    let mut te_rect_opt = None;
                    let mut te_has_focus = false;

                    Frame::NONE
                        .fill(Color32::from_rgb(15, 20, 31))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 51, 74)))
                        .corner_radius(egui::CornerRadius::same(16))
                        .inner_margin(Margin::symmetric(10, 3))
                        .show(ui, |ui| {
                            ui.set_width(omnibox_width);
                            ui.horizontal(|ui| {
                                // Security Pill
                                if is_internal {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(18, 30, 48))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(56, 189, 248),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(10))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("INTERNAL")
                                                    .size(9.5)
                                                    .strong()
                                                    .color(Color32::from_rgb(125, 211, 252)),
                                            );
                                        });
                                } else if is_https {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(16, 36, 26))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(34, 197, 94),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(10))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("HTTPS")
                                                    .size(9.5)
                                                    .strong()
                                                    .color(Color32::from_rgb(74, 222, 128)),
                                            );
                                        });
                                } else {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(36, 24, 12))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgb(245, 158, 11),
                                        ))
                                        .corner_radius(egui::CornerRadius::same(10))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("HTTP")
                                                    .size(9.5)
                                                    .strong()
                                                    .color(Color32::from_rgb(251, 191, 36)),
                                            );
                                        });
                                }

                                let input_w = (ui.available_width() - 110.0).max(80.0);
                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut self.omnibox_input)
                                        .hint_text("Search or enter web address...")
                                        .desired_width(input_w),
                                );

                                te_rect_opt = Some(te.rect);
                                te_has_focus = te.has_focus();

                                if self.focus_omnibox {
                                    te.request_focus();
                                    self.focus_omnibox = false;
                                }

                                if te.changed() {
                                    if !self.omnibox_input.trim().is_empty() {
                                        self.omnibox_suggestions = self
                                            .storage
                                            .search_omnibox(&self.omnibox_input, 6)
                                            .unwrap_or_default();
                                        self.show_omnibox_dropdown =
                                            !self.omnibox_suggestions.is_empty();
                                    } else {
                                        self.show_omnibox_dropdown = false;
                                    }
                                }

                                if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    navigate_target = Some(self.omnibox_input.clone());
                                    self.show_omnibox_dropdown = false;
                                }

                                if is_loading {
                                    ui.spinner();
                                }

                                // Bookmark Save button
                                let star_text = if is_bookmarked {
                                    RichText::new("Saved")
                                        .size(11.0)
                                        .color(Color32::from_rgb(56, 189, 248))
                                        .strong()
                                } else {
                                    RichText::new("Save")
                                        .size(11.0)
                                        .color(Color32::from_rgb(148, 163, 184))
                                };

                                let star_tooltip = if is_bookmarked {
                                    "Bookmarked (Click to remove)"
                                } else {
                                    "Bookmark this page"
                                };

                                if ui.button(star_text).on_hover_text(star_tooltip).clicked() {
                                    if is_bookmarked {
                                        let _ = self.storage.remove_browser_bookmark(&current_url);
                                    } else if !current_url.is_empty()
                                        && current_url != "about:blank"
                                    {
                                        let _ = self.storage.add_browser_bookmark(
                                            &current_url,
                                            &current_title,
                                            "Favorites",
                                        );
                                    }
                                    self.cached_bookmarks =
                                        self.storage.list_browser_bookmarks().unwrap_or_default();
                                }

                                if ui
                                    .button(
                                        RichText::new("Copy")
                                            .size(11.0)
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    )
                                    .clicked()
                                {
                                    ui.ctx().copy_text(self.omnibox_input.clone());
                                }

                                if ui
                                    .button(RichText::new("Go").strong().color(Color32::WHITE))
                                    .clicked()
                                {
                                    navigate_target = Some(self.omnibox_input.clone());
                                    self.show_omnibox_dropdown = false;
                                }
                            });
                        });

                    // Render floating autocomplete suggestions below omnibox
                    if self.show_omnibox_dropdown
                        && te_has_focus
                        && !self.omnibox_suggestions.is_empty()
                    {
                        if let Some(te_rect) = te_rect_opt {
                            let dropdown_pos = egui::pos2(te_rect.min.x, te_rect.max.y + 6.0);
                            let dropdown_width = te_rect.width().max(380.0);
                            let mut selected_suggestion = None;

                            egui::Area::new(egui::Id::new("omnibox_autocomplete_popup"))
                                .fixed_pos(dropdown_pos)
                                .order(egui::Order::Foreground)
                                .show(ui.ctx(), |ui| {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(15, 20, 31))
                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 51, 74)))
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .inner_margin(Margin::symmetric(10, 6))
                                        .show(ui, |ui| {
                                            ui.set_width(dropdown_width);
                                            ui.vertical(|ui| {
                                                for item in &self.omnibox_suggestions {
                                                    let (badge, badge_color) = match item.match_type
                                                    {
                                                        OmniboxMatchType::Bookmark => (
                                                            "BOOKMARK",
                                                            Color32::from_rgb(56, 189, 248),
                                                        ),
                                                        OmniboxMatchType::History => (
                                                            "HISTORY",
                                                            Color32::from_rgb(148, 163, 184),
                                                        ),
                                                        OmniboxMatchType::SearchSuggestion => (
                                                            "SEARCH",
                                                            Color32::from_rgb(16, 185, 129),
                                                        ),
                                                    };

                                                    let resp = ui.horizontal(|ui| {
                                                        Frame::NONE
                                                            .fill(Color32::from_rgba_premultiplied(
                                                                badge_color.r() / 6,
                                                                badge_color.g() / 6,
                                                                badge_color.b() / 6,
                                                                255,
                                                            ))
                                                            .corner_radius(
                                                                egui::CornerRadius::same(3),
                                                            )
                                                            .inner_margin(Margin::symmetric(4, 1))
                                                            .show(ui, |ui| {
                                                                ui.label(
                                                                    RichText::new(badge)
                                                                        .size(9.0)
                                                                        .strong()
                                                                        .color(badge_color),
                                                                );
                                                            });
                                                        ui.label(
                                                            RichText::new(&item.title)
                                                                .size(12.5)
                                                                .strong()
                                                                .color(Color32::from_rgb(
                                                                    240, 246, 252,
                                                                )),
                                                        );
                                                        ui.label(
                                                            RichText::new(&item.url)
                                                                .size(11.0)
                                                                .color(Color32::from_rgb(
                                                                    100, 116, 139,
                                                                )),
                                                        );
                                                    });

                                                    if resp
                                                        .response
                                                        .interact(egui::Sense::click())
                                                        .clicked()
                                                    {
                                                        selected_suggestion =
                                                            Some(item.url.clone());
                                                    }
                                                    ui.add_space(3.0);
                                                }
                                            });
                                        });
                                });

                            if let Some(target) = selected_suggestion {
                                self.omnibox_input = target.clone();
                                self.show_omnibox_dropdown = false;
                                navigate_target = Some(target);
                            }
                        }
                    }

                    if let Some(target) = navigate_target {
                        let normalized = self.normalize_target(&target);
                        self.navigate(active_id, &normalized);
                    }

                    // Right Toolbar Cluster (Clean left-to-right alignment, zero collision)
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

                        // Zoom Indicator
                        let zoom_pct = if let Some(tab) = self.active_tab() {
                            (tab.zoom * 100.0).round() as i32
                        } else {
                            100
                        };
                        if ui
                            .button(
                                RichText::new(format!("{}%", zoom_pct))
                                    .size(11.0)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            )
                            .on_hover_text("Content Zoom (Click to cycle)")
                            .clicked()
                        {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.zoom = match zoom_pct {
                                    100 => 1.25,
                                    125 => 1.5,
                                    150 => 0.8,
                                    _ => 1.0,
                                };
                            }
                        }

                        // Find in Page
                        if ui
                            .button(
                                RichText::new("Find")
                                    .size(11.0)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            )
                            .on_hover_text("Find in Page (Ctrl+F)")
                            .clicked()
                        {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.show_find = !tab.show_find;
                            }
                        }

                        // History
                        if ui
                            .button(
                                RichText::new("History")
                                    .size(11.0)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            )
                            .on_hover_text("Browsing History (Ctrl+H)")
                            .clicked()
                        {
                            self.navigate(active_id, "excalibur://history");
                        }

                        // Settings
                        if ui
                            .button(
                                RichText::new("Settings")
                                    .size(11.0)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            )
                            .on_hover_text("Browser Settings")
                            .clicked()
                        {
                            self.navigate(active_id, "excalibur://settings");
                        }

                        let fav_text = if self.show_favorites_bar {
                            RichText::new("Favs")
                                .size(11.0)
                                .color(Color32::from_rgb(56, 189, 248))
                        } else {
                            RichText::new("Favs")
                                .size(11.0)
                                .color(Color32::from_rgb(148, 163, 184))
                        };
                        if ui
                            .button(fav_text)
                            .on_hover_text("Toggle Favorites Bar")
                            .clicked()
                        {
                            self.show_favorites_bar = !self.show_favorites_bar;
                        }

                        if ui
                            .button(
                                RichText::new("DOM")
                                    .size(11.0)
                                    .strong()
                                    .color(Color32::from_rgb(192, 132, 252)),
                            )
                            .on_hover_text("Open DOM & Script Inspector")
                            .clicked()
                        {
                            self.x_mode_enabled = true;
                            self.x_mode_tab = XModeTab::DomAndScripts;
                        }

                        if self.tab_nav_layout == TabNavLayout::Sidebar {
                            let (toggle_text, toggle_color) = if self.x_mode_enabled {
                                ("X-MODE: ON", Color32::from_rgb(255, 60, 90))
                            } else {
                                ("X-MODE: OFF", Color32::from_rgb(125, 211, 252))
                            };
                            if ui
                                .button(
                                    RichText::new(toggle_text)
                                        .size(11.0)
                                        .strong()
                                        .color(toggle_color),
                                )
                                .clicked()
                            {
                                self.x_mode_enabled = !self.x_mode_enabled;
                            }
                        }
                    });
                });

                if is_loading {
                    let time = ui.input(|i| i.time);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 2.5),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    painter.rect_filled(rect, 1.0, Color32::from_rgb(10, 20, 36));

                    let cycle = (time * 1.5).fract() as f32;
                    let beam_width = (rect.width() * 0.35).max(40.0);
                    let beam_x = rect.min.x + (rect.width() + beam_width) * cycle - beam_width;
                    let beam_min_x = beam_x.max(rect.min.x);
                    let beam_max_x = (beam_x + beam_width).min(rect.max.x);
                    if beam_max_x > beam_min_x {
                        let beam_rect = egui::Rect::from_min_max(
                            egui::pos2(beam_min_x, rect.min.y),
                            egui::pos2(beam_max_x, rect.max.y),
                        );
                        painter.rect_filled(beam_rect, 1.0, Color32::from_rgb(0, 240, 255));
                    }
                }
            });
    }

    fn render_favorites_bar(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        Frame::NONE
            .fill(Color32::from_rgb(11, 15, 23))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("FAVORITES")
                            .size(10.0)
                            .strong()
                            .color(Color32::from_rgb(100, 116, 139)),
                    );

                    let mut navigate_to = None;
                    for bm in &self.cached_bookmarks {
                        let label = if bm.title.is_empty() {
                            &bm.url
                        } else {
                            &bm.title
                        };
                        let display_label = if label.len() > 20 {
                            format!("{}...", &label[..18])
                        } else {
                            label.to_string()
                        };
                        if ui
                            .button(
                                RichText::new(display_label)
                                    .size(11.0)
                                    .color(Color32::from_rgb(203, 213, 225)),
                            )
                            .on_hover_text(format!("{}\nFolder: {}", bm.url, bm.folder))
                            .clicked()
                        {
                            navigate_to = Some(bm.url.clone());
                        }
                    }

                    if ui
                        .button(
                            RichText::new("+ Manage")
                                .size(11.0)
                                .color(Color32::from_rgb(56, 189, 248)),
                        )
                        .on_hover_text("Open Bookmarks Manager (excalibur://bookmarks)")
                        .clicked()
                    {
                        navigate_to = Some("excalibur://bookmarks".to_string());
                    }

                    if let Some(target) = navigate_to {
                        self.navigate(active_id, &target);
                    }
                });
            });
    }

    fn render_x_mode_toolbar(&mut self, ui: &mut Ui) {
        Frame::NONE
            .fill(Color32::from_rgb(10, 16, 28))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 52, 92)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("X-MODE WORKBENCH:")
                            .color(Color32::from_rgb(255, 60, 90))
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
                                .color(Color32::from_rgb(56, 189, 248))
                                .strong()
                        } else {
                            RichText::new(label).color(Color32::from_rgb(147, 197, 253))
                        };
                        if ui.selectable_label(is_sel, text).clicked() {
                            self.x_mode_tab = tab_val;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("x Close").color(Color32::from_rgb(248, 113, 113)),
                            )
                            .clicked()
                        {
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
        let (url, dom, raw_html, is_loading, error_msg, zoom, show_find, find_query) =
            if let Some(tab) = self.active_tab() {
                (
                    tab.url.clone(),
                    tab.parsed_dom.clone(),
                    tab.raw_html.clone(),
                    tab.is_loading,
                    tab.error_msg.clone(),
                    tab.zoom,
                    tab.show_find,
                    tab.find_query.clone(),
                )
            } else {
                (
                    "about:blank".to_string(),
                    None,
                    String::new(),
                    false,
                    None,
                    1.0,
                    false,
                    String::new(),
                )
            };

        if let Some(action) = self.browser_renderer.render(
            ui,
            &url,
            dom.as_ref(),
            &raw_html,
            is_loading,
            error_msg.as_deref(),
            &self.storage,
            zoom,
            &find_query,
        ) {
            match action {
                BrowserAction::Navigate(target) => {
                    self.navigate(active_id, &target);
                }
            }
        }

        // Floating In-Page Search Overlay (Ctrl+F)
        if show_find {
            let mut close_find = false;
            let mut query_changed = None;
            let screen_rect = ui.clip_rect();
            let overlay_pos = egui::pos2(screen_rect.max.x - 340.0, screen_rect.min.y + 4.0);

            egui::Area::new(egui::Id::new("find_in_page_overlay"))
                .fixed_pos(overlay_pos)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    Frame::NONE
                        .fill(Color32::from_rgb(10, 16, 28))
                        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("FIND:")
                                        .size(11.0)
                                        .strong()
                                        .color(Color32::from_rgb(0, 240, 255)),
                                );

                                let mut cur_query = find_query.clone();
                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut cur_query)
                                        .desired_width(140.0)
                                        .hint_text("Find in page..."),
                                );
                                if te.changed() {
                                    query_changed = Some(cur_query.clone());
                                }

                                if !cur_query.is_empty() {
                                    let matches_count = raw_html
                                        .to_lowercase()
                                        .matches(&cur_query.to_lowercase())
                                        .count();
                                    let match_color = if matches_count > 0 {
                                        Color32::from_rgb(0, 255, 157)
                                    } else {
                                        Color32::from_rgb(248, 113, 113)
                                    };
                                    ui.label(
                                        RichText::new(format!("{} hits", matches_count))
                                            .size(11.0)
                                            .color(match_color),
                                    );
                                }

                                if ui
                                    .button(
                                        RichText::new("x")
                                            .size(11.0)
                                            .color(Color32::from_rgb(200, 215, 235)),
                                    )
                                    .on_hover_text("Close (Esc)")
                                    .clicked()
                                {
                                    close_find = true;
                                }
                            });
                        });
                });

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_find = true;
            }

            if let Some(tab) = self.active_tab_mut() {
                if let Some(new_q) = query_changed {
                    tab.find_query = new_q;
                }
                if close_find {
                    tab.show_find = false;
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
                        RichText::new("Connecting...")
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    );
                } else if let Some(status) = tab.status_code {
                    let status_color = if status < 300 {
                        Color32::from_rgb(16, 185, 129)
                    } else if status < 400 {
                        Color32::from_rgb(245, 158, 11)
                    } else {
                        Color32::from_rgb(239, 68, 68)
                    };
                    ui.label(
                        RichText::new(format!("HTTP {}", status))
                            .color(status_color)
                            .strong(),
                    );
                    if let Some(ms) = tab.elapsed_ms {
                        ui.label(
                            RichText::new(format!("{} ms", ms))
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    }
                } else {
                    ui.label(
                        RichText::new("Ready")
                            .color(Color32::from_rgb(148, 163, 184))
                            .strong(),
                    );
                }
                ui.separator();
                let display_url = if tab.url.len() > 50 {
                    format!("{}...", &tab.url[..47])
                } else {
                    tab.url.clone()
                };
                ui.label(RichText::new(display_url).color(Color32::from_rgb(203, 213, 225)));
                if let Some(ref dom) = tab.parsed_dom {
                    ui.separator();
                    let nodes_count = count_dom_nodes(dom);
                    ui.label(
                        RichText::new(format!("{} nodes", nodes_count))
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let issues = self.scanner_state.findings.len();
                let issue_text = if issues > 0 {
                    RichText::new(format!("Radar: {} Flaws", issues))
                        .color(Color32::from_rgb(239, 68, 68))
                        .strong()
                } else {
                    RichText::new("Radar: 0 Flaws").color(Color32::from_rgb(16, 185, 129))
                };

                if ui.button(issue_text).clicked() {
                    self.x_mode_enabled = true;
                    self.x_mode_tab = XModeTab::Scanner;
                }

                ui.separator();
                let captured = self.interceptor_state.traffic_items.len();
                let proxy_text = RichText::new(format!("Proxy (Captured: {})", captured))
                    .color(Color32::from_rgb(148, 163, 184));
                if ui.button(proxy_text).clicked() {
                    self.x_mode_enabled = true;
                    self.x_mode_tab = XModeTab::Interceptor;
                }

                ui.separator();
                let layout_btn = match self.tab_nav_layout {
                    TabNavLayout::Top => "Zen Sidebar",
                    TabNavLayout::Sidebar => "Top Tabs",
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

fn site_pill_color(url: &str) -> Color32 {
    if url.contains("owasp") {
        Color32::from_rgb(0, 240, 255) // Neon Cyan
    } else if url.contains("portswigger") {
        Color32::from_rgb(251, 146, 60) // Vivid Orange
    } else if url.contains("github") {
        Color32::from_rgb(192, 132, 252) // Luminous Purple
    } else if url.contains("ycombinator") {
        Color32::from_rgb(248, 113, 113) // Coral Red
    } else if url.contains("cloudflare") {
        Color32::from_rgb(56, 189, 248) // Electric Azure
    } else if url.contains("httpbin") {
        Color32::from_rgb(52, 211, 153) // Emerald Green
    } else if url.contains("cve") {
        Color32::from_rgb(239, 68, 68) // Crimson Red
    } else if url.contains("about:blank") || url.contains("newtab") {
        Color32::from_rgb(96, 165, 250) // Royal Blue
    } else if url.contains("127.0.0.1") || url.contains("localhost") {
        Color32::from_rgb(250, 204, 21) // Amber Gold
    } else {
        Color32::from_rgb(56, 189, 248) // Electric Blue
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
