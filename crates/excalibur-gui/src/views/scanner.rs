use eframe::egui::{Color32, RichText, ScrollArea, Ui};

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

pub fn render_scanner_view(ui: &mut Ui, state: &mut ScannerViewState) {
    // Summary badges
    let critical_count = state
        .findings
        .iter()
        .filter(|f| f.severity == "CRITICAL")
        .count();
    let high_count = state
        .findings
        .iter()
        .filter(|f| f.severity == "HIGH")
        .count();
    let medium_count = state
        .findings
        .iter()
        .filter(|f| f.severity == "MEDIUM")
        .count();
    let low_count = state
        .findings
        .iter()
        .filter(|f| f.severity == "LOW")
        .count();

    ui.horizontal(|ui| {
        ui.label(RichText::new("Vulnerability Radar:").strong());
        ui.label(
            RichText::new(format!("[ CRITICAL: {} ]", critical_count))
                .color(Color32::from_rgb(218, 54, 51)),
        );
        ui.label(
            RichText::new(format!("[ HIGH: {} ]", high_count))
                .color(Color32::from_rgb(248, 81, 73)),
        );
        ui.label(
            RichText::new(format!("[ MEDIUM: {} ]", medium_count))
                .color(Color32::from_rgb(210, 153, 34)),
        );
        ui.label(
            RichText::new(format!("[ LOW: {} ]", low_count)).color(Color32::from_rgb(88, 166, 255)),
        );

        ui.separator();
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_query);
    });

    ui.separator();

    ui.columns(2, |cols| {
        // Left Column: Findings List
        cols[0].group(|ui| {
            ui.heading("Detected Vulnerabilities");
            ScrollArea::vertical().show(ui, |ui| {
                let query = state.search_query.to_ascii_lowercase();

                for (idx, finding) in state.findings.iter().enumerate() {
                    if !query.is_empty()
                        && !finding.title.to_ascii_lowercase().contains(&query)
                        && !finding.rule_id.to_ascii_lowercase().contains(&query)
                    {
                        continue;
                    }

                    let is_selected = idx == state.selected_idx;
                    let sev_color = match finding.severity.as_str() {
                        "CRITICAL" => Color32::from_rgb(218, 54, 51),
                        "HIGH" => Color32::from_rgb(248, 81, 73),
                        "MEDIUM" => Color32::from_rgb(210, 153, 34),
                        _ => Color32::from_rgb(88, 166, 255),
                    };

                    let label_text = format!("[{}] {}", finding.severity, finding.title);
                    if ui
                        .selectable_label(is_selected, RichText::new(label_text).color(sev_color))
                        .clicked()
                    {
                        state.selected_idx = idx;
                    }
                }
            });
        });

        // Right Column: Details Inspector
        cols[1].group(|ui| {
            if let Some(finding) = state.findings.get(state.selected_idx) {
                ui.heading(&finding.title);
                ui.label(
                    RichText::new(format!(
                        "Rule ID: {} | Severity: {}",
                        finding.rule_id, finding.severity
                    ))
                    .strong(),
                );
                ui.separator();

                ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("Technical Impact:").strong());
                    ui.label(&finding.description);
                    ui.add_space(8.0);

                    ui.label(RichText::new("Remediation Guidance:").strong());
                    ui.label(&finding.recommendation);
                    ui.add_space(8.0);

                    ui.label(RichText::new("Raw Evidence:").strong());
                    ui.code(&finding.evidence);
                    ui.add_space(8.0);

                    ui.label(RichText::new("Reproduction cURL Command:").strong());
                    ui.code(&finding.curl_command);
                });
            } else {
                ui.label("No finding selected.");
            }
        });
    });
}
