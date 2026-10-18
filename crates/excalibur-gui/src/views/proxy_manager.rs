use eframe::egui::{Color32, RichText, Ui};

pub struct ProxyManagerState {
    pub proxy_bind_addr: String,
    pub is_proxy_running: bool,
    pub forged_certs_count: usize,
    pub ca_fingerprint: String,
    pub storage_stats: String,
}

impl Default for ProxyManagerState {
    fn default() -> Self {
        Self {
            proxy_bind_addr: "127.0.0.1:8080".to_string(),
            is_proxy_running: true,
            forged_certs_count: 0,
            ca_fingerprint: "SHA256: 4A:7B:9C:E2:1F:33:55:88:99:AA:BB:CC:DD:EE:FF:00".to_string(),
            storage_stats: "SQLite WAL Database: in-memory (0 records)".to_string(),
        }
    }
}

pub fn render_proxy_manager(ui: &mut Ui, state: &mut ProxyManagerState) {
    ui.heading("Inbound MITM Proxy & Certificate Authority");
    ui.label("Excalibur can act as an inbound proxy for external browsers, curl, Postman, or mobile devices.");

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.horizontal(|ui| {
            let status_badge = if state.is_proxy_running {
                RichText::new("[ LISTENING ]")
                    .color(Color32::from_rgb(63, 185, 80))
                    .strong()
            } else {
                RichText::new("[ STOPPED ]")
                    .color(Color32::from_rgb(248, 81, 73))
                    .strong()
            };
            ui.label(status_badge);
            ui.label(format!("Proxy Endpoint: http://{}", state.proxy_bind_addr));

            if ui
                .button(if state.is_proxy_running {
                    "Stop Listener"
                } else {
                    "Start Listener"
                })
                .clicked()
            {
                state.is_proxy_running = !state.is_proxy_running;
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Root CA Certificate:").strong());
        ui.label(format!("Fingerprint: {}", state.ca_fingerprint));
        ui.label("Leaf certificates are generated on-the-fly for every intercepted HTTPS domain.");

        ui.add_space(8.0);
        ui.label(RichText::new("Session Storage:").strong());
        ui.label(&state.storage_stats);
    });

    ui.add_space(16.0);
    ui.heading("Client Configuration Instructions");
    ui.label("To intercept traffic from external tools or secondary browsers:");
    ui.code("HTTP Proxy:  127.0.0.1:8080\nHTTPS Proxy: 127.0.0.1:8080");
}
