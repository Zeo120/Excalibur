use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Browser,
    Inspector,
    Repeater,
    Scanner,
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

pub struct TuiApp {
    pub active_tab: ActiveTab,
    pub should_quit: bool,

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

    // Repeater state
    pub repeater_req_method: String,
    pub repeater_req_url: String,
    pub repeater_req_headers: String,
    pub repeater_req_body: String,
    pub repeater_res_raw: String,
    pub repeater_status_text: String,

    // Scanner state
    pub audit_findings: Vec<AuditFindingItem>,
    pub selected_audit_index: usize,
}

impl TuiApp {
    pub fn new() -> Self {
        let initial_url = "https://example.com".to_string();
        let default_rendered =
            "# Example Domain\n\nThis domain is for use in illustrative examples in documents."
                .to_string();

        let mut app = Self {
            active_tab: ActiveTab::Browser,
            should_quit: false,

            url_input: initial_url.clone(),
            is_editing_url: false,
            history: vec![initial_url.clone()],
            history_index: 0,
            browser_rendered_text: default_rendered,
            browser_scroll: 0,

            traffic_list: Vec::new(),
            selected_traffic_index: 0,

            repeater_req_method: "GET".to_string(),
            repeater_req_url: initial_url,
            repeater_req_headers: "Host: example.com\r\nUser-Agent: Excalibur/0.1.0\r\nAccept: */*"
                .to_string(),
            repeater_req_body: String::new(),
            repeater_res_raw: String::new(),
            repeater_status_text: "Ready".to_string(),

            audit_findings: Vec::new(),
            selected_audit_index: 0,
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
            request_raw: "GET / HTTP/1.1\r\nHost: example.com\r\nUser-Agent: Excalibur/0.1.0\r\n\r\n"
                .into(),
            response_raw: "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 1256\r\n\r\n<!doctype html><html><body><h1>Example Domain</h1></body></html>"
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

    pub fn navigate_to(&mut self, url: &str) {
        self.url_input = url.to_string();
        self.is_editing_url = false;

        if self.history.is_empty() || self.history[self.history_index] != url {
            self.history.truncate(self.history_index + 1);
            self.history.push(url.to_string());
            self.history_index = self.history.len() - 1;
        }

        // Mock render for navigation
        self.browser_rendered_text = format!(
            "# Navigated to {}\n\n[Status: 200 OK]\n\nDocument rendered via Excalibur DOM & Layout Engine.",
            url
        );
        self.browser_scroll = 0;
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

        // Global hotkeys
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
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Browser => ActiveTab::Inspector,
                    ActiveTab::Inspector => ActiveTab::Repeater,
                    ActiveTab::Repeater => ActiveTab::Scanner,
                    ActiveTab::Scanner => ActiveTab::Browser,
                };
                return;
            }
            KeyCode::BackTab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Browser => ActiveTab::Scanner,
                    ActiveTab::Inspector => ActiveTab::Browser,
                    ActiveTab::Repeater => ActiveTab::Inspector,
                    ActiveTab::Scanner => ActiveTab::Repeater,
                };
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
                _ => {}
            },
            ActiveTab::Inspector => match key.code {
                KeyCode::Char('j') | KeyCode::Down
                    if !self.traffic_list.is_empty()
                        && self.selected_traffic_index + 1 < self.traffic_list.len() =>
                {
                    self.selected_traffic_index += 1;
                }
                KeyCode::Char('k') | KeyCode::Up if self.selected_traffic_index > 0 => {
                    self.selected_traffic_index -= 1;
                }
                KeyCode::Char('r') => {
                    // Copy selected traffic to repeater
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
            ActiveTab::Repeater => {
                if let KeyCode::Char('r') = key.code {
                    self.repeater_status_text = "Sending request...".to_string();
                    // Mock immediate response dispatch
                    self.repeater_res_raw = format!(
                        "HTTP/1.1 200 OK\r\nDate: {}\r\nServer: Excalibur-Mock/1.0\r\nContent-Type: text/plain\r\nContent-Length: 17\r\n\r\nRepeater response",
                        chrono::Utc::now().to_rfc2822()
                    );
                    self.repeater_status_text = "200 OK (18 ms)".to_string();
                }
            }
            ActiveTab::Scanner => match key.code {
                KeyCode::Char('j') | KeyCode::Down
                    if !self.audit_findings.is_empty()
                        && self.selected_audit_index + 1 < self.audit_findings.len() =>
                {
                    self.selected_audit_index += 1;
                }
                KeyCode::Char('k') | KeyCode::Up if self.selected_audit_index > 0 => {
                    self.selected_audit_index -= 1;
                }
                _ => {}
            },
        }
    }

    /// Runs the full interactive terminal event loop.
    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        res
    }

    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| crate::ui::render_ui(f, self))?;

            if event::poll(Duration::from_millis(100))? {
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
