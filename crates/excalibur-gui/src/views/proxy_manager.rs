use crate::theme::colors;
use crate::widgets::{badge, code_block, section, setting_row, toggle};
use eframe::egui::{RichText, ScrollArea, Ui};

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
    ScrollArea::vertical()
        .id_salt("proxy_manager_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(760.0);

            section(
                ui,
                "MITM proxy listener",
                "External browsers, curl, Postman or mobile devices can route traffic through Excalibur.",
                |ui| {
                    let (label, col) = if state.is_proxy_running {
                        ("Listening", colors::success())
                    } else {
                        ("Stopped", colors::danger())
                    };
                    setting_row(
                        ui,
                        &format!("http://{}", state.proxy_bind_addr),
                        "Inbound HTTP/HTTPS proxy endpoint",
                        |ui| {
                            toggle(ui, &mut state.is_proxy_running);
                            badge(ui, label, col);
                        },
                    );
                },
            );
            ui.add_space(12.0);

            section(
                ui,
                "Certificate authority",
                "Leaf certificates are forged on the fly for every intercepted HTTPS host.",
                |ui| {
                    ui.label(
                        RichText::new("Root CA fingerprint")
                            .size(12.0)
                            .color(colors::text_faint()),
                    );
                    code_block(ui, &state.ca_fingerprint);
                    ui.add_space(6.0);
                    setting_row(ui, "Certificates issued", "", |ui| {
                        badge(ui, &state.forged_certs_count.to_string(), colors::text_dim());
                    });
                    setting_row(ui, "Session storage", &state.storage_stats, |_| {});
                },
            );
            ui.add_space(12.0);

            section(
                ui,
                "Client configuration",
                "Point the client at the listener for both protocols:",
                |ui| {
                    code_block(ui, "HTTP Proxy:  127.0.0.1:8080\nHTTPS Proxy: 127.0.0.1:8080");
                },
            );
        });
}
