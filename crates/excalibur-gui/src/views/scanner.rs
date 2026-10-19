use crate::icons::Icon;
use crate::theme::colors;
use crate::widgets::{
    badge, card, code_block, empty_state, list_row, overline, search_field, severity_color,
    single_line,
};
use eframe::egui::{pos2, vec2, RichText, ScrollArea, Ui};

#[derive(Clone)]
pub struct GuiFindingItem {
    pub rule_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub evidence: String,
    pub curl_command: String,
}

pub struct ScannerViewState {
    pub findings: Vec<GuiFindingItem>,
    pub selected_idx: usize,
    pub search_query: String,
}

impl Default for ScannerViewState {
    fn default() -> Self {
        let findings = vec![
            GuiFindingItem {
                rule_id: "SEC-AWS-KEY-ID".into(),
                severity: "HIGH".into(),
                title: "AWS Access Key ID Disclosed in Response Body".into(),
                description: "The response payload contains an unencrypted Amazon Web Services access key identifier (AKIAIOSFODNN7EXAMPLE), allowing potential unauthorized cloud API access.".into(),
                recommendation: "Rotate credentials immediately and configure AWS IAM least-privilege policies. Do not return credentials in client payloads.".into(),
                evidence: "Payload offset 48: accessKeyId: 'AKIAIOSFODNN7EXAMPLE'".into(),
                curl_command: "curl -i -X GET 'https://example.com/api/v1/config'".into(),
            },
            GuiFindingItem {
                rule_id: "SEC-HSTS-MISSING".into(),
                severity: "HIGH".into(),
                title: "Missing Strict-Transport-Security (HSTS) Header".into(),
                description: "The HTTPS response does not enforce strict transport security, exposing visitors to SSL-stripping and man-in-the-middle downgrade attacks.".into(),
                recommendation: "Add 'Strict-Transport-Security: max-age=31536000; includeSubDomains; preload' to all HTTPS responses.".into(),
                evidence: "Response headers: HSTS missing".into(),
                curl_command: "curl -I 'https://example.com/'".into(),
            },
            GuiFindingItem {
                rule_id: "SEC-CSP-UNSAFE-INLINE".into(),
                severity: "MEDIUM".into(),
                title: "Content-Security-Policy Permits 'unsafe-inline' Scripts".into(),
                description: "The application's Content-Security-Policy allows inline script execution, weakening protection against Cross-Site Scripting (XSS).".into(),
                recommendation: "Remove 'unsafe-inline' and use cryptographic nonces or hashes.".into(),
                evidence: "Header: Content-Security-Policy: default-src 'self' 'unsafe-inline'".into(),
                curl_command: "curl -I 'https://example.com/'".into(),
            },
            GuiFindingItem {
                rule_id: "SEC-CORS-WILDCARD-CREDS".into(),
                severity: "HIGH".into(),
                title: "Insecure CORS: Wildcard Origin with Credentials Allowed".into(),
                description: "Access-Control-Allow-Origin is configured as '*' while Access-Control-Allow-Credentials is true.".into(),
                recommendation: "Explicitly declare trusted origins instead of using wildcard origins when credentials are supported.".into(),
                evidence: "Headers: Access-Control-Allow-Origin: *, Access-Control-Allow-Credentials: true".into(),
                curl_command: "curl -H 'Origin: https://evil.com' -I 'https://example.com/api/user'".into(),
            },
        ];

        Self {
            findings,
            selected_idx: 0,
            search_query: String::new(),
        }
    }
}

const SEVERITIES: [&str; 4] = ["CRITICAL", "HIGH", "MEDIUM", "LOW"];

pub fn render_scanner_view(ui: &mut Ui, state: &mut ScannerViewState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
        for sev in SEVERITIES {
            let n = state.findings.iter().filter(|f| f.severity == sev).count();
            let col = if n > 0 {
                severity_color(sev)
            } else {
                colors::text_faint()
            };
            badge(ui, &format!("{} {}", n, sev.to_lowercase()), col);
        }
        ui.add_space(8.0);
        search_field(
            ui,
            &mut state.search_query,
            "Search findings",
            240.0,
            "scanner_search",
        );
    });
    ui.add_space(10.0);

    let query = state.search_query.to_ascii_lowercase();
    let visible: Vec<usize> = state
        .findings
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            query.is_empty()
                || f.title.to_ascii_lowercase().contains(&query)
                || f.rule_id.to_ascii_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect();

    if state.findings.is_empty() {
        empty_state(
            ui,
            Icon::Shield,
            "No findings",
            "Passive checks run on every page you load.",
        );
        return;
    }

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        overline(ui, &format!("{} findings", visible.len()));
        ui.add_space(4.0);
        ScrollArea::vertical()
            .id_salt("findings_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                for idx in visible {
                    let f = &state.findings[idx];
                    let (rect, resp) = list_row(ui, 44.0, idx == state.selected_idx);
                    let sev = severity_color(&f.severity);
                    ui.painter()
                        .circle_filled(pos2(rect.left() + 14.0, rect.center().y), 4.0, sev);
                    let w = rect.width() - 36.0;
                    let title = single_line(ui, &f.title, 13.0, colors::text(), w);
                    ui.painter().galley(
                        pos2(rect.left() + 28.0, rect.top() + 6.0),
                        title,
                        colors::text(),
                    );
                    let meta = single_line(
                        ui,
                        &format!("{} · {}", f.severity, f.rule_id),
                        11.0,
                        colors::text_faint(),
                        w,
                    );
                    ui.painter().galley(
                        pos2(rect.left() + 28.0, rect.bottom() - 18.0),
                        meta,
                        colors::text_faint(),
                    );
                    if resp.clicked() {
                        state.selected_idx = idx;
                    }
                }
            });

        let ui = &mut cols[1];
        let Some(f) = state.findings.get(state.selected_idx) else {
            empty_state(ui, Icon::Info, "Select a finding", "Details appear here.");
            return;
        };
        ScrollArea::vertical()
            .id_salt("finding_detail")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        badge(ui, &f.severity, severity_color(&f.severity));
                        ui.label(
                            RichText::new(&f.rule_id)
                                .monospace()
                                .size(11.5)
                                .color(colors::text_faint()),
                        );
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&f.title)
                            .size(16.0)
                            .strong()
                            .color(colors::text()),
                    );
                    ui.add_space(10.0);
                    for (heading, body) in [
                        ("Impact", &f.description),
                        ("Remediation", &f.recommendation),
                    ] {
                        overline(ui, heading);
                        ui.label(RichText::new(body).size(13.0).color(colors::text_dim()));
                        ui.add_space(10.0);
                    }
                    overline(ui, "Evidence");
                    code_block(ui, &f.evidence);
                    ui.add_space(10.0);
                    overline(ui, "Reproduce");
                    code_block(ui, &f.curl_command);
                });
            });
    });
}
