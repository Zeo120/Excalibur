use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Browser,
    Inspector,
    Repeater,
    Scanner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorFocus {
    Table,
    Request,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterField {
    Method,
    Url,
    Headers,
    Body,
}

#[derive(Debug, Clone)]
pub struct TrafficItem {
    pub id: String,
    pub method: String,
    pub status: u16,
    pub url: String,
    pub size: usize,
    pub elapsed_ms: u64,
    pub request_raw: String,
    pub response_raw: String,
}

#[derive(Debug, Clone)]
pub struct AuditFindingItem {
    pub rule_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub evidence: String,
}

pub enum TuiBackgroundEvent {
    PageLoaded {
        url: String,
        rendered_text: String,
        status: u16,
        elapsed_ms: u64,
        traffic_item: TrafficItem,
        audit_findings: Vec<AuditFindingItem>,
    },
    PageLoadFailed {
        url: String,
        error: String,
    },
    RepeaterResponseReceived {
        status_text: String,
        raw_response: String,
    },
    RepeaterFailed {
        error: String,
    },
}

pub struct TuiApp {
    pub active_tab: ActiveTab,
    pub should_quit: bool,
    pub show_help: bool,
    pub is_loading: bool,
    pub spinner_tick: usize,

    // Browser state
    pub url_input: String,
    pub is_editing_url: bool,
    pub history: Vec<String>,
    pub history_index: usize,
    pub browser_rendered_text: String,
    pub browser_scroll: u16,

    // Inspector state
    pub traffic_list: Vec<TrafficItem>,
    pub selected_traffic_index: usize,
    pub inspector_focus: InspectorFocus,
    pub inspector_filter: String,
    pub is_filtering_inspector: bool,
    pub inspector_req_scroll: u16,
    pub inspector_res_scroll: u16,

    // Repeater state
    pub repeater_req_method: String,
    pub repeater_req_url: String,
    pub repeater_req_headers: String,
    pub repeater_req_body: String,
    pub repeater_res_raw: String,
    pub repeater_status_text: String,
    pub repeater_active_field: RepeaterField,
    pub is_editing_repeater: bool,
    pub repeater_res_scroll: u16,

    // Scanner state
    pub audit_findings: Vec<AuditFindingItem>,
    pub selected_audit_index: usize,
    pub scanner_detail_scroll: u16,

    // Background communication channels
    pub event_tx: Sender<TuiBackgroundEvent>,
    pub event_rx: Receiver<TuiBackgroundEvent>,
}

impl TuiApp {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let initial_url = "https://example.com".to_string();
        let default_rendered =
            "# Example Domain\n\nThis domain is for use in illustrative examples in documents."
                .to_string();

        let mut app = Self {
            active_tab: ActiveTab::Browser,
            should_quit: false,
            show_help: false,
            is_loading: false,
            spinner_tick: 0,

            url_input: initial_url.clone(),
            is_editing_url: false,
            history: vec![initial_url.clone()],
            history_index: 0,
            browser_rendered_text: default_rendered,
            browser_scroll: 0,

            traffic_list: Vec::new(),
            selected_traffic_index: 0,
            inspector_focus: InspectorFocus::Table,
            inspector_filter: String::new(),
            is_filtering_inspector: false,
            inspector_req_scroll: 0,
            inspector_res_scroll: 0,

            repeater_req_method: "GET".to_string(),
            repeater_req_url: initial_url,
            repeater_req_headers: "Host: example.com\r\nUser-Agent: Excalibur/0.1.0\r\nAccept: */*"
                .to_string(),
            repeater_req_body: String::new(),
            repeater_res_raw: String::new(),
            repeater_status_text: "Ready".to_string(),
            repeater_active_field: RepeaterField::Url,
            is_editing_repeater: false,
            repeater_res_scroll: 0,

            audit_findings: Vec::new(),
            selected_audit_index: 0,
            scanner_detail_scroll: 0,

            event_tx: tx,
            event_rx: rx,
        };

        // Populate sample traffic and audit findings for initial interactive presentation
        app.seed_initial_data();
        app
    }

    fn seed_initial_data(&mut self) {
        self.record_traffic(TrafficItem {
            id: "req-101".into(),
            method: "GET".into(),
            status: 200,
            url: "https://example.com/".into(),
            size: 1256,
            elapsed_ms: 42,
            request_raw: "GET / HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Excalibur/0.1.0 (PRoot-Safe)\r\n\r\n"
                .into(),
            response_raw: "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: 1256\r\n\r\n<!doctype html><html><body><h1>Example Domain</h1><p>Illustrative example domain.</p></body></html>"
                .into(),
        });

        self.add_audit_finding(AuditFindingItem {
            rule_id: "SEC-HSTS-MISSING".into(),
            severity: "HIGH".into(),
            title: "Missing Strict-Transport-Security (HSTS) Header".into(),
            description: "HTTPS connection does not instruct the browser to enforce secure transport."
                .into(),
            recommendation: "Set 'Strict-Transport-Security: max-age=31536000; includeSubDomains'."
                .into(),
            evidence: "URL: https://example.com/".into(),
        });
        self.add_audit_finding(AuditFindingItem {
            rule_id: "SEC-CSP-MISSING".into(),
            severity: "MEDIUM".into(),
            title: "Missing Content-Security-Policy (CSP) Header".into(),
            description: "No Content-Security-Policy header defined for the document.".into(),
            recommendation: "Configure default-src 'self' to mitigate XSS risks.".into(),
            evidence: "Headers: Strict-Transport-Security missing, CSP missing".into(),
        });
    }

    pub fn record_traffic(&mut self, item: TrafficItem) {
        self.traffic_list.push(item);
    }

    pub fn add_audit_finding(&mut self, finding: AuditFindingItem) {
        self.audit_findings.push(finding);
    }

    pub fn poll_background_events(&mut self) {
        while let Ok(evt) = self.event_rx.try_recv() {
            match evt {
                TuiBackgroundEvent::PageLoaded {
                    url,
                    rendered_text,
                    status: _,
                    elapsed_ms: _,
                    traffic_item,
                    audit_findings,
                } => {
                    self.is_loading = false;
                    if self.url_input == url {
                        self.browser_rendered_text = rendered_text;
                    }
                    self.record_traffic(traffic_item);
                    for f in audit_findings {
                        self.add_audit_finding(f);
                    }
                }
                TuiBackgroundEvent::PageLoadFailed { url, error } => {
                    self.is_loading = false;
                    if self.url_input == url {
                        self.browser_rendered_text = format!(
                            "# Failed to load {}\n\nError: {}\n\nCheck URL or network connectivity.",
                            url, error
                        );
                    }
                }
                TuiBackgroundEvent::RepeaterResponseReceived {
                    status_text,
                    raw_response,
                } => {
                    self.is_loading = false;
                    self.repeater_status_text = status_text;
                    self.repeater_res_raw = raw_response;
                }
                TuiBackgroundEvent::RepeaterFailed { error } => {
                    self.is_loading = false;
                    self.repeater_status_text = format!("Error: {}", error);
                }
            }
        }
    }

    pub fn navigate_to(&mut self, url: &str) {
        let clean_url = url.trim().to_string();
        if clean_url.is_empty() {
            return;
        }

        let target_url = if !clean_url.starts_with("http://") && !clean_url.starts_with("https://")
        {
            format!("https://{}", clean_url)
        } else {
            clean_url
        };

        self.url_input = target_url.clone();
        self.is_editing_url = false;

        if self.history.is_empty() || self.history[self.history_index] != target_url {
            self.history.truncate(self.history_index + 1);
            self.history.push(target_url.clone());
            self.history_index = self.history.len() - 1;
        }

        self.is_loading = true;
        self.browser_scroll = 0;
        self.browser_rendered_text = format!(
            "# Navigating to {}\n\n[Status: In-Flight]\n\nResolving DNS and dispatching request via Excalibur Resilient Core...",
            target_url
        );

        let tx = self.event_tx.clone();
        let target_clone = target_url.clone();

        let spawn_fetch = move || async move {
            Self::async_fetch_url(target_clone, tx).await;
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(spawn_fetch());
        } else {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                if let Ok(rt) = rt {
                    rt.block_on(spawn_fetch());
                }
            });
        }
    }

    async fn async_fetch_url(url: String, tx: Sender<TuiBackgroundEvent>) {
        let client_opts = excalibur_core::ClientOptions {
            bypass_tls_verification: true,
            timeout_seconds: 15,
            ..Default::default()
        };

        let client = match excalibur_core::HttpClient::new(client_opts) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(TuiBackgroundEvent::PageLoadFailed {
                    url,
                    error: format!("HTTP Client initialization error: {}", e),
                });
                return;
            }
        };

        let req = excalibur_core::HttpRequest::new("GET", &url);
        let start = std::time::Instant::now();

        match client.dispatch(&req).await {
            Ok(res) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;

                let req_headers = req.headers.to_raw_string();
                let host_str = url::Url::parse(&url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .unwrap_or_else(|| "unknown".into());

                let req_raw = format!(
                    "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Excalibur/0.1.0 (PRoot-Safe)\r\nAccept: text/html,*/*\r\n{}\r\n",
                    url, host_str, req_headers
                );

                let res_headers = res.headers.to_raw_string();
                let res_body_str = String::from_utf8_lossy(&res.body).to_string();
                let res_raw = format!(
                    "HTTP/1.1 {} {}\r\n{}\r\n{}",
                    res.status, res.reason, res_headers, res_body_str
                );

                // Parse HTML via kuchiki / html5ever
                let dom = excalibur_dom::HtmlParser::parse(&res_body_str);
                let rendered_text = excalibur_dom::LayoutEngine::render_to_plain_text(&dom);
                let rendered_text = if rendered_text.trim().is_empty() {
                    format!(
                        "# Document {}\n\n[Status: {} {}]\n\n(No textual markup extracted from DOM)",
                        url, res.status, res.reason
                    )
                } else {
                    format!(
                        "# Document: {}\n[Status: {} {} | Latency: {} ms]\n\n{}",
                        url, res.status, res.reason, elapsed_ms, rendered_text
                    )
                };

                let traffic_item = TrafficItem {
                    id: req.id.clone(),
                    method: req.method.clone(),
                    status: res.status,
                    url: url.clone(),
                    size: res.body.len(),
                    elapsed_ms,
                    request_raw: req_raw,
                    response_raw: res_raw,
                };

                // Real-time audit
                let is_https = url.starts_with("https://");
                let auditor = excalibur_audit::ResponseAuditor::new();
                let report = auditor.audit_response(&res, is_https);

                let mut findings = Vec::new();
                for h in report.header_findings {
                    findings.push(AuditFindingItem {
                        rule_id: h.rule_id,
                        severity: h.severity.as_str().to_string(),
                        title: h.title,
                        description: h.description,
                        recommendation: h.recommendation,
                        evidence: format!("Header: {:?}", h.header_name),
                    });
                }
                for s in report.secret_findings {
                    findings.push(AuditFindingItem {
                        rule_id: s.rule_id.clone(),
                        severity: s.severity.as_str().to_string(),
                        title: s.rule_name,
                        description: format!("High-entropy token detected (entropy: {:.2})", s.entropy),
                        recommendation: "Revoke token and prevent disclosure in HTTP bodies.".into(),
                        evidence: format!("Pattern: {} | Matched: {}", s.rule_id, s.redacted_text),
                    });
                }

                let _ = tx.send(TuiBackgroundEvent::PageLoaded {
                    url,
                    rendered_text,
                    status: res.status,
                    elapsed_ms,
                    traffic_item,
                    audit_findings: findings,
                });
            }
            Err(e) => {
                let _ = tx.send(TuiBackgroundEvent::PageLoadFailed {
                    url,
                    error: format!("Network request failed: {}", e),
                });
            }
        }
    }

    pub fn send_repeater_request(&mut self) {
        let method = self.repeater_req_method.trim().to_uppercase();
        let url = self.repeater_req_url.trim().to_string();
        let headers_str = self.repeater_req_headers.clone();
        let body_str = self.repeater_req_body.clone();

        self.repeater_status_text = "Sending request...".to_string();
        self.is_loading = true;

        // Optimistic default response for immediate responsiveness
        self.repeater_res_raw = format!(
            "HTTP/1.1 200 OK\r\nDate: {}\r\nServer: Excalibur-Repeater/1.0\r\nContent-Type: text/plain\r\nContent-Length: 26\r\n\r\nRepeater request in-flight",
            chrono::Utc::now().to_rfc2822()
        );

        let tx = self.event_tx.clone();

        let spawn_repeat = move || async move {
            let client_opts = excalibur_core::ClientOptions {
                bypass_tls_verification: true,
                timeout_seconds: 15,
                ..Default::default()
            };

            let client = match excalibur_core::HttpClient::new(client_opts) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(TuiBackgroundEvent::RepeaterFailed {
                        error: format!("Client error: {}", e),
                    });
                    return;
                }
            };

            let mut req = excalibur_core::HttpRequest::new(&method, &url);
            for line in headers_str.lines() {
                let trimmed = line.trim();
                if let Some((k, v)) = trimmed.split_once(':') {
                    req.headers.append(k.trim(), v.trim());
                }
            }
            if !body_str.is_empty() {
                req.body = body_str.into_bytes();
            }

            let start = std::time::Instant::now();
            match client.dispatch(&req).await {
                Ok(res) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let res_headers = res.headers.to_raw_string();
                    let res_body_str = String::from_utf8_lossy(&res.body);
                    let raw_response = format!(
                        "HTTP/1.1 {} {}\r\n{}\r\n{}",
                        res.status, res.reason, res_headers, res_body_str
                    );
                    let status_text = format!("{} {} ({} ms)", res.status, res.reason, elapsed_ms);

                    let _ = tx.send(TuiBackgroundEvent::RepeaterResponseReceived {
                        status_text,
                        raw_response,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TuiBackgroundEvent::RepeaterFailed {
                        error: format!("Dispatch failed: {}", e),
                    });
                }
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(spawn_repeat());
        } else {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                if let Ok(rt) = rt {
                    rt.block_on(spawn_repeat());
                }
            });
        }
    }

    pub fn history_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            self.navigate_to(&url);
        }
    }

    pub fn history_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            self.navigate_to(&url);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Toggle Help modal
        if key.code == KeyCode::Char('?') && !self.is_editing_url && !self.is_editing_repeater && !self.is_filtering_inspector {
            self.show_help = !self.show_help;
            return;
        }

        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || key.code == KeyCode::Char('q') {
                self.show_help = false;
            }
            return;
        }

        // Handle URL editing input
        if self.is_editing_url {
            match key.code {
                KeyCode::Enter => {
                    let url = self.url_input.clone();
                    self.navigate_to(&url);
                }
                KeyCode::Esc => {
                    self.is_editing_url = false;
                }
                KeyCode::Backspace => {
                    self.url_input.pop();
                }
                KeyCode::Char(c) => {
                    self.url_input.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle Inspector Search Filtering
        if self.is_filtering_inspector {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.is_filtering_inspector = false;
                }
                KeyCode::Backspace => {
                    self.inspector_filter.pop();
                }
                KeyCode::Char(c) => {
                    self.inspector_filter.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle Repeater field editing
        if self.is_editing_repeater {
            match key.code {
                KeyCode::Esc => {
                    self.is_editing_repeater = false;
                }
                KeyCode::Enter => {
                    if self.repeater_active_field == RepeaterField::Headers || self.repeater_active_field == RepeaterField::Body {
                        match self.repeater_active_field {
                            RepeaterField::Headers => self.repeater_req_headers.push('\n'),
                            RepeaterField::Body => self.repeater_req_body.push('\n'),
                            _ => {}
                        }
                    } else {
                        self.is_editing_repeater = false;
                    }
                }
                KeyCode::Backspace => match self.repeater_active_field {
                    RepeaterField::Method => {
                        self.repeater_req_method.pop();
                    }
                    RepeaterField::Url => {
                        self.repeater_req_url.pop();
                    }
                    RepeaterField::Headers => {
                        self.repeater_req_headers.pop();
                    }
                    RepeaterField::Body => {
                        self.repeater_req_body.pop();
                    }
                },
                KeyCode::Char(c) => match self.repeater_active_field {
                    RepeaterField::Method => {
                        self.repeater_req_method.push(c.to_ascii_uppercase());
                    }
                    RepeaterField::Url => {
                        self.repeater_req_url.push(c);
                    }
                    RepeaterField::Headers => {
                        self.repeater_req_headers.push(c);
                    }
                    RepeaterField::Body => {
                        self.repeater_req_body.push(c);
                    }
                },
                _ => {}
            }
            return;
        }

        // Global navigation shortcuts
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.active_tab = ActiveTab::Browser;
                return;
            }
            KeyCode::Char('2') => {
                self.active_tab = ActiveTab::Inspector;
                return;
            }
            KeyCode::Char('3') => {
                self.active_tab = ActiveTab::Repeater;
                return;
            }
            KeyCode::Char('4') => {
                self.active_tab = ActiveTab::Scanner;
                return;
            }
            _ => {}
        }

        // Tab-specific controls
        match self.active_tab {
            ActiveTab::Browser => match key.code {
                KeyCode::Char('/') => {
                    self.is_editing_url = true;
                }
                KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.is_editing_url = true;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.browser_scroll = self.browser_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.browser_scroll = self.browser_scroll.saturating_sub(1);
                }
                KeyCode::Char('H') => {
                    self.history_back();
                }
                KeyCode::Char('L') => {
                    self.history_forward();
                }
                KeyCode::Char('r') => {
                    let url = self.url_input.clone();
                    self.navigate_to(&url);
                }
                KeyCode::Tab => {
                    self.active_tab = ActiveTab::Inspector;
                }
                KeyCode::BackTab => {
                    self.active_tab = ActiveTab::Scanner;
                }
                _ => {}
            },
            ActiveTab::Inspector => match key.code {
                KeyCode::Char('/') => {
                    self.is_filtering_inspector = true;
                }
                KeyCode::Esc => {
                    self.inspector_filter.clear();
                }
                KeyCode::Tab => {
                    self.inspector_focus = match self.inspector_focus {
                        InspectorFocus::Table => InspectorFocus::Request,
                        InspectorFocus::Request => InspectorFocus::Response,
                        InspectorFocus::Response => InspectorFocus::Table,
                    };
                }
                KeyCode::Char('j') | KeyCode::Down => match self.inspector_focus {
                    InspectorFocus::Table => {
                        if !self.traffic_list.is_empty()
                            && self.selected_traffic_index + 1 < self.traffic_list.len()
                        {
                            self.selected_traffic_index += 1;
                        }
                    }
                    InspectorFocus::Request => {
                        self.inspector_req_scroll = self.inspector_req_scroll.saturating_add(1);
                    }
                    InspectorFocus::Response => {
                        self.inspector_res_scroll = self.inspector_res_scroll.saturating_add(1);
                    }
                },
                KeyCode::Char('k') | KeyCode::Up => match self.inspector_focus {
                    InspectorFocus::Table => {
                        if self.selected_traffic_index > 0 {
                            self.selected_traffic_index -= 1;
                        }
                    }
                    InspectorFocus::Request => {
                        self.inspector_req_scroll = self.inspector_req_scroll.saturating_sub(1);
                    }
                    InspectorFocus::Response => {
                        self.inspector_res_scroll = self.inspector_res_scroll.saturating_sub(1);
                    }
                },
                KeyCode::Char('r') => {
                    if let Some(item) = self.traffic_list.get(self.selected_traffic_index) {
                        self.repeater_req_method = item.method.clone();
                        self.repeater_req_url = item.url.clone();
                        self.repeater_req_body = String::new();
                        self.repeater_status_text =
                            format!("Loaded from traffic #{}", self.selected_traffic_index + 1);
                        self.active_tab = ActiveTab::Repeater;
                    }
                }
                _ => {}
            },
            ActiveTab::Repeater => match key.code {
                KeyCode::Tab => {
                    self.repeater_active_field = match self.repeater_active_field {
                        RepeaterField::Method => RepeaterField::Url,
                        RepeaterField::Url => RepeaterField::Headers,
                        RepeaterField::Headers => RepeaterField::Body,
                        RepeaterField::Body => RepeaterField::Method,
                    };
                }
                KeyCode::Char('e') => {
                    self.is_editing_repeater = true;
                }
                KeyCode::Char('r') | KeyCode::Enter => {
                    self.send_repeater_request();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.repeater_res_scroll = self.repeater_res_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.repeater_res_scroll = self.repeater_res_scroll.saturating_sub(1);
                }
                _ => {}
            },
            ActiveTab::Scanner => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !self.audit_findings.is_empty()
                        && self.selected_audit_index + 1 < self.audit_findings.len()
                    {
                        self.selected_audit_index += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.selected_audit_index > 0 {
                        self.selected_audit_index -= 1;
                    }
                }
                KeyCode::Char('c') => {
                    self.audit_findings.clear();
                    self.selected_audit_index = 0;
                }
                KeyCode::Tab => {
                    self.active_tab = ActiveTab::Browser;
                }
                _ => {}
            },
        }
    }

    /// Sets up emergency panic hook so PRoot / terminal raw mode is never left broken.
    pub fn install_panic_hook() {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
            prev_hook(info);
        }));
    }

    /// Runs the full interactive terminal event loop.
    pub fn run(&mut self) -> Result<()> {
        Self::install_panic_hook();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)?;
        terminal.show_cursor()?;

        res
    }

    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        while !self.should_quit {
            self.poll_background_events();
            self.spinner_tick = (self.spinner_tick + 1) % 4;

            terminal.draw(|f| crate::ui::render_ui(f, self))?;

            if event::poll(Duration::from_millis(80))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }
        }
        Ok(())
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}
