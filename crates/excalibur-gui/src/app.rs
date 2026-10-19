use crate::chrome::ChromeState;
use crate::theme::{self, colors, configure_fonts, UiPrefs};
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
use crate::widgets::paint_gradient;
use eframe::egui::{self, Frame, Margin, Rect, Stroke, Ui};
use excalibur_audit::ResponseAuditor;
use excalibur_core::{
    client::{ClientOptions, HttpClient},
    network::HttpRequest,
    storage::{BrowserBookmarkEntry, OmniboxMatch, WorkspaceStorage},
};
use excalibur_dom::{DomNode, HtmlParser};
use excalibur_script::{ExecutionLimits, JsCancelHandle, JsRuntime, SinkFinding};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// Gap between the window edge, sidebar and the floating page card.
pub const CARD_GAP: f32 = 8.0;

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
    /// Pinned tabs sit above the divider and survive "Clear".
    pub pinned: bool,
    /// Incremented on every navigation or stop; responses tagged with an older
    /// generation are stale and discarded.
    pub nav_generation: u64,
    /// Handle to the in-flight load, used to cancel it.
    pub nav_task: Option<NavTask>,
}

/// Cancellation handles for one in-flight page load.
#[derive(Clone)]
pub struct NavTask {
    abort: tokio::task::AbortHandle,
    js_cancel: JsCancelHandle,
}

impl NavTask {
    /// Stops the network request (dropping its socket) and any running page script.
    pub fn cancel(&self) {
        self.abort.abort();
        self.js_cancel.cancel();
    }
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
            pinned: false,
            nav_generation: 0,
            nav_task: None,
        }
    }
}

pub struct PageLoadedEvent {
    pub tab_id: usize,
    pub generation: u64,
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
        generation: u64,
        url: String,
        error: String,
    },
}

/// How the vertical tab sidebar is presented.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TabNavLayout {
    /// Sidebar is always docked on the left.
    Sidebar,
    /// Sidebar is hidden and slides in when the pointer touches the left edge.
    Compact,
}

impl TabNavLayout {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("compact") | Some("top") => TabNavLayout::Compact,
            _ => TabNavLayout::Sidebar,
        }
    }

    pub fn as_setting(self) -> &'static str {
        match self {
            TabNavLayout::Sidebar => "sidebar",
            TabNavLayout::Compact => "compact",
        }
    }
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
    // Tabs
    pub tabs: Vec<BrowserTab>,
    pub active_tab_id: usize,
    pub next_tab_id: usize,
    /// URL of the active tab, kept in sync on navigation and tab switches.
    pub omnibox_input: String,
    pub browser_renderer: BrowserRenderer,

    // Layout & appearance
    pub tab_nav_layout: TabNavLayout,
    /// Show bookmarks as "Essentials" tiles at the top of the sidebar.
    pub show_favorites_bar: bool,
    pub prefs: UiPrefs,
    pub chrome: ChromeState,

    // X-Mode security workbench
    pub x_mode_enabled: bool,
    pub x_mode_tab: XModeTab,
    pub x_mode_layout: XModeLayout,
    pub interceptor_state: InterceptorViewState,
    pub repeater_state: RepeaterViewState,
    pub scanner_state: ScannerViewState,
    pub dom_inspector_state: DomInspectorState,
    pub proxy_state: ProxyManagerState,

    // Storage
    pub storage: Arc<WorkspaceStorage>,
    pub cached_bookmarks: Vec<BrowserBookmarkEntry>,
    pub closed_tabs_history: Vec<String>,
    pub omnibox_suggestions: Vec<OmniboxMatch>,

    // Async page loading channels
    pub page_tx: Sender<PageNavigationEvent>,
    pub page_rx: Receiver<PageNavigationEvent>,
    pub egui_ctx: Option<egui::Context>,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (tx, rx) = channel();

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

            tab_nav_layout: TabNavLayout::Sidebar,
            show_favorites_bar: true,
            prefs: UiPrefs::default(),
            chrome: ChromeState::default(),

            x_mode_enabled: false,
            x_mode_tab: XModeTab::Interceptor,
            x_mode_layout: XModeLayout::SplitDrawer,
            interceptor_state: InterceptorViewState::default(),
            repeater_state: RepeaterViewState::default(),
            scanner_state: ScannerViewState::default(),
            dom_inspector_state: DomInspectorState::default(),
            proxy_state: ProxyManagerState::default(),

            storage,
            cached_bookmarks: Vec::new(),
            closed_tabs_history: Vec::new(),
            omnibox_suggestions: Vec::new(),

            page_tx: tx,
            page_rx: rx,
            egui_ctx: Some(cc.egui_ctx.clone()),
        };
        app.reload_preferences();

        app.tabs.push(BrowserTab::new(1, "excalibur://newtab"));
        app.navigate(1, "excalibur://newtab");

        app
    }

    /// Re-reads persisted appearance, layout and bookmark state.
    pub fn reload_preferences(&mut self) {
        let get = |k: &str| self.storage.get_browser_setting(k).ok().flatten();
        self.prefs = UiPrefs::load(&self.storage);
        self.tab_nav_layout = TabNavLayout::from_setting(get("tab_layout").as_deref());
        self.show_favorites_bar = get("show_essentials").as_deref() != Some("off");
        self.cached_bookmarks = self.storage.list_browser_bookmarks().unwrap_or_default();
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
            // Supersede whatever this tab was loading before.
            if let Some(task) = tab.nav_task.take() {
                task.cancel();
            }
            tab.nav_generation += 1;
            let generation = tab.nav_generation;
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
            let js_cancel = JsCancelHandle::new();
            let task_js_cancel = js_cancel.clone();

            let handle = tokio::spawn(async move {
                let client = match HttpClient::new(ClientOptions::default()) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to initialize HTTP client: {}", e);
                        let _ = tx.send(PageNavigationEvent::Failure {
                            tab_id,
                            generation,
                            url: target_url,
                            error: e.to_string(),
                        });
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

                        // DOM XSS sink tracing runs on a blocking thread under
                        // QuickJS time/memory limits so page scripts can't stall
                        // the async runtime or loop forever.
                        let scripts: Vec<String> = dom
                            .find_by_tag("script")
                            .iter()
                            .map(|s| s.text_content())
                            .filter(|c| !c.trim().is_empty())
                            .collect();
                        let audit_url = target_url.clone();
                        let sinks = tokio::task::spawn_blocking(move || {
                            audit_inline_scripts(
                                &audit_url,
                                &scripts,
                                ScriptAuditBudget::default(),
                                task_js_cancel,
                            )
                        })
                        .await
                        .unwrap_or_default();
                        for sink in sinks {
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
                            generation,
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
                            generation,
                            url: target_url,
                            error: e.to_string(),
                        });
                        if let Some(ctx) = ctx_opt {
                            ctx.request_repaint();
                        }
                    }
                }
            });
            tab.nav_task = Some(NavTask {
                abort: handle.abort_handle(),
                js_cancel,
            });
        }
    }

    /// Cancels the tab's in-flight load: aborts the request and page scripts and
    /// invalidates any response already queued for it.
    pub fn stop_loading(&mut self, tab_id: usize) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            if let Some(task) = tab.nav_task.take() {
                task.cancel();
            }
            if tab.is_loading {
                tab.nav_generation += 1;
                tab.is_loading = false;
            }
        }
    }

    fn is_current_generation(&self, tab_id: usize, generation: u64) -> bool {
        self.tabs
            .iter()
            .any(|t| t.id == tab_id && t.nav_generation == generation)
    }

    pub fn poll_network_events(&mut self) {
        while let Ok(event) = self.page_rx.try_recv() {
            match event {
                PageNavigationEvent::Success(loaded) => {
                    let tab_id = loaded.tab_id;
                    if !self.is_current_generation(tab_id, loaded.generation) {
                        tracing::debug!("Discarding stale page load for tab {}", tab_id);
                        continue;
                    }
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
                        tab.nav_task = None;
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
                    generation,
                    url: _,
                    error,
                } => {
                    if !self.is_current_generation(tab_id, generation) {
                        continue;
                    }
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.is_loading = false;
                        tab.nav_task = None;
                        tab.error_msg = Some(error);
                    }
                }
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let (ctrl, shift, alt) = ctx.input(|i| {
            (
                i.modifiers.command || i.modifiers.ctrl,
                i.modifiers.shift,
                i.modifiers.alt,
            )
        });
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k));

        if ctrl && !shift && pressed(Key::T) {
            self.open_new_tab();
        }
        if ctrl && shift && pressed(Key::T) {
            self.reopen_closed_tab();
        }
        if ctrl && !shift && pressed(Key::W) {
            self.close_tab(self.active_tab_id);
        }
        if (ctrl && pressed(Key::L)) || pressed(Key::F6) {
            self.open_command_bar(false);
        }
        if (ctrl && !shift && pressed(Key::R)) || pressed(Key::F5) {
            self.reload_active();
        }
        if alt && pressed(Key::ArrowLeft) {
            self.go_back();
        }
        if alt && pressed(Key::ArrowRight) {
            self.go_forward();
        }
        if ctrl && !shift && pressed(Key::H) {
            self.navigate(self.active_tab_id, "excalibur://history");
        }
        if ctrl && !shift && pressed(Key::B) {
            self.navigate(self.active_tab_id, "excalibur://bookmarks");
        }
        if ctrl && !shift && pressed(Key::D) {
            self.toggle_bookmark_for_active();
        }
        if ctrl && shift && pressed(Key::S) {
            self.toggle_tab_layout();
        }
        if ctrl && shift && pressed(Key::X) {
            self.x_mode_enabled = !self.x_mode_enabled;
        }
        if ctrl && !shift && pressed(Key::F) {
            if let Some(tab) = self.active_tab_mut() {
                tab.show_find = !tab.show_find;
            }
        }
        if ctrl && (pressed(Key::Plus) || pressed(Key::Equals)) {
            self.adjust_zoom(0.1);
        }
        if ctrl && pressed(Key::Minus) {
            self.adjust_zoom(-0.1);
        }
        if ctrl && pressed(Key::Num0) {
            if let Some(tab) = self.active_tab_mut() {
                tab.zoom = 1.0;
            }
        }

        // Ctrl+Tab / Ctrl+Shift+Tab cycle tabs
        if ctrl && pressed(Key::Tab) && !self.tabs.is_empty() {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == self.active_tab_id) {
                let n = self.tabs.len();
                let next = if shift {
                    (idx + n - 1) % n
                } else {
                    (idx + 1) % n
                };
                self.select_tab(self.tabs[next].id);
            }
        }

        // Ctrl+1..9 jump to tab
        let num_keys = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];
        for (i, k) in num_keys.into_iter().enumerate() {
            if ctrl && pressed(k) && i < self.tabs.len() {
                self.select_tab(self.tabs[i].id);
            }
        }
    }

    pub fn adjust_zoom(&mut self, delta: f32) {
        if let Some(tab) = self.active_tab_mut() {
            tab.zoom = ((tab.zoom + delta) * 10.0).round() / 10.0;
            tab.zoom = tab.zoom.clamp(0.6, 2.5);
        }
    }

    pub fn reload_active(&mut self) {
        if let Some(url) = self.active_tab().map(|t| t.url.clone()) {
            self.navigate(self.active_tab_id, &url);
        }
    }

    pub fn render_panels(&mut self, ctx: &egui::Context) {
        self.egui_ctx = Some(ctx.clone());
        if self.chrome.applied_prefs.as_ref() != Some(&self.prefs) {
            theme::apply(ctx, &self.prefs);
            self.chrome.applied_prefs = Some(self.prefs.clone());
        }
        self.handle_keyboard_shortcuts(ctx);
        self.poll_network_events();

        if self.tabs.iter().any(|t| t.is_loading) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        // One continuous window surface behind sidebar and cards
        let screen = ctx.screen_rect();
        paint_gradient(
            &ctx.layer_painter(egui::LayerId::background()),
            screen,
            colors::chrome_top(),
            colors::chrome_bottom(),
            0.0,
        );

        let docked = self.tab_nav_layout == TabNavLayout::Sidebar;
        if docked {
            egui::SidePanel::left("zen_sidebar")
                .resizable(true)
                .default_width(crate::chrome::SIDEBAR_WIDTH)
                .width_range(200.0..=380.0)
                .show_separator_line(false)
                .frame(Frame::NONE.inner_margin(Margin {
                    left: 8,
                    right: 0,
                    top: 8,
                    bottom: 8,
                }))
                .show(ctx, |ui| {
                    self.render_sidebar(ui);
                });
        }

        let side_gap = if docked { 0 } else { CARD_GAP as i8 };
        let gap = CARD_GAP as i8;

        if self.x_mode_enabled && self.x_mode_layout == XModeLayout::SplitDrawer {
            egui::TopBottomPanel::bottom("x_mode_drawer")
                .resizable(true)
                .min_height(200.0)
                .default_height(320.0)
                .show_separator_line(false)
                .frame(Frame::NONE.inner_margin(Margin {
                    left: side_gap + gap,
                    right: gap,
                    top: 0,
                    bottom: gap,
                }))
                .show(ctx, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    let inner = self.paint_card(ui, rect);
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
                    child.set_clip_rect(inner);
                    self.render_x_mode_panel(&mut child);
                    ui.allocate_rect(rect, egui::Sense::hover());
                });
        }

        egui::CentralPanel::default()
            .frame(Frame::NONE.inner_margin(Margin {
                left: side_gap + gap,
                right: gap,
                top: gap,
                bottom: gap,
            }))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let inner = self.paint_card(ui, rect);
                self.chrome.card_rect = rect;
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
                child.set_clip_rect(inner.expand(1.0));
                if self.x_mode_enabled && self.x_mode_layout == XModeLayout::FullscreenTools {
                    self.render_x_mode_panel(&mut child);
                } else {
                    self.render_browser_content(&mut child);
                }
                self.paint_loading_beam(ui, rect);
                ui.allocate_rect(rect, egui::Sense::hover());
            });

        if !docked {
            self.render_compact_sidebar(ctx);
        }
        self.render_popovers(ctx);
        self.render_command_bar(ctx);
    }

    /// Paints a floating rounded card (shadow, fill, hairline) and returns its content rect.
    fn paint_card(&self, ui: &Ui, rect: Rect) -> Rect {
        let p = ui.painter();
        p.add(
            egui::Shadow {
                offset: [0, 2],
                blur: 12,
                spread: 0,
                color: colors::shadow().gamma_multiply(0.6),
            }
            .as_shape(rect, 12),
        );
        p.rect_filled(rect, 12.0, colors::canvas());
        p.rect_stroke(
            rect,
            12.0,
            Stroke::new(1.0_f32, colors::border()),
            egui::StrokeKind::Inside,
        );
        rect.shrink(1.0)
    }

    fn render_x_mode_panel(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let header = Rect::from_min_size(full.min, egui::vec2(full.width(), 44.0));
        let mut head_ui =
            ui.new_child(egui::UiBuilder::new().max_rect(header.shrink2(egui::vec2(10.0, 0.0))));
        self.render_x_mode_toolbar(&mut head_ui);
        ui.painter().hline(
            full.x_range(),
            header.bottom(),
            Stroke::new(1.0_f32, colors::border()),
        );
        let body = Rect::from_min_max(egui::pos2(full.left(), header.bottom() + 1.0), full.max)
            .shrink(12.0);
        let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body));
        body_ui.set_clip_rect(body.expand(4.0));
        self.render_x_mode_workbench(&mut body_ui);
    }

    pub(crate) fn normalize_target(&self, raw: &str) -> String {
        let engine = self
            .storage
            .get_browser_setting("search_engine")
            .ok()
            .flatten()
            .unwrap_or_default();
        crate::urls::normalize_input(raw, crate::urls::SearchEngine::from_setting(&engine))
    }

    fn render_browser_content(&mut self, ui: &mut Ui) {
        let active_id = self.active_tab_id;
        let (url, dom, raw_html, is_loading, error_msg, zoom, show_find, find_query) =
            match self.active_tab() {
                Some(tab) => (
                    tab.url.clone(),
                    tab.parsed_dom.clone(),
                    tab.raw_html.clone(),
                    tab.is_loading,
                    tab.error_msg.clone(),
                    tab.zoom,
                    tab.show_find,
                    tab.find_query.clone(),
                ),
                None => (
                    "about:blank".to_string(),
                    None,
                    String::new(),
                    false,
                    None,
                    1.0,
                    false,
                    String::new(),
                ),
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
                BrowserAction::Navigate(target) => self.navigate(active_id, &target),
                BrowserAction::OpenCommandBar => self.open_command_bar(false),
                BrowserAction::PreferencesChanged => self.reload_preferences(),
            }
        }

        if show_find {
            self.render_find_overlay(ui, &raw_html, &find_query);
        }
    }

    fn render_x_mode_workbench(&mut self, ui: &mut Ui) {
        match self.x_mode_tab {
            XModeTab::Interceptor => {
                if let Some(InterceptorAction::SendToRepeater(item)) =
                    render_interceptor_view(ui, &mut self.interceptor_state)
                {
                    self.repeater_state.method = item.method;
                    self.repeater_state.url = item.url;
                    self.x_mode_tab = XModeTab::Repeater;
                }
            }
            XModeTab::Repeater => render_repeater_view(ui, &mut self.repeater_state),
            XModeTab::Scanner => render_scanner_view(ui, &mut self.scanner_state),
            XModeTab::DomAndScripts => {
                let (dom, raw_html) = match self.active_tab() {
                    Some(tab) => (tab.parsed_dom.clone(), tab.raw_html.clone()),
                    None => (None, String::new()),
                };
                render_dom_inspector(ui, &mut self.dom_inspector_state, dom.as_ref(), &raw_html);
            }
            XModeTab::ProxyManager => render_proxy_manager(ui, &mut self.proxy_state),
        }
    }
}

/// Limits for tracing a page's inline scripts.
#[derive(Debug, Clone, Copy)]
pub struct ScriptAuditBudget {
    /// Budget for each individual script.
    pub per_script: std::time::Duration,
    /// Budget for all scripts on the page together.
    pub total: std::time::Duration,
    pub memory_limit_bytes: usize,
}

impl Default for ScriptAuditBudget {
    fn default() -> Self {
        Self {
            per_script: std::time::Duration::from_millis(500),
            total: std::time::Duration::from_secs(3),
            memory_limit_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Runs a page's inline scripts in a sandboxed, resource-limited QuickJS runtime and
/// returns the dangerous sinks they reached. Blocking: call from a blocking thread.
pub fn audit_inline_scripts(
    url: &str,
    scripts: &[String],
    budget: ScriptAuditBudget,
    cancel: JsCancelHandle,
) -> Vec<SinkFinding> {
    let limits = ExecutionLimits {
        time_budget: budget.per_script,
        memory_limit_bytes: budget.memory_limit_bytes,
        ..ExecutionLimits::default()
    };
    let Ok(js) = JsRuntime::with_limits(limits, cancel.clone()) else {
        return Vec::new();
    };
    let _ = js.set_url(url);
    let start = std::time::Instant::now();
    for code in scripts {
        if cancel.is_cancelled() || start.elapsed() >= budget.total {
            break;
        }
        let _ = js.eval(code.trim());
    }
    js.get_sink_findings()
}

pub(crate) fn count_dom_nodes(node: &DomNode) -> usize {
    1 + node.children.iter().map(count_dom_nodes).sum::<usize>()
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_panels(ctx);
    }
}
