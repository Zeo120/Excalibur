use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui};
use excalibur_dom::{CssColor, DomNode, NodeType, StyleResolver, StyleSheet};
use std::collections::HashMap;

#[inline]
fn to_egui_color(col: CssColor) -> Color32 {
    Color32::from_rgba_unmultiplied(col.r, col.g, col.b, col.a)
}

/// An action requested by the interactive browser view to the parent app.
pub enum BrowserAction {
    Navigate(String),
}

/// A video item extracted from YouTube SPA JSON payloads (ytInitialData).
#[derive(Clone, Debug, Default)]
pub struct YouTubeVideoItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub views: String,
    pub duration: String,
    pub published: String,
    pub thumbnail_url: String,
}

/// Watch page metadata extracted from ytInitialData or ytInitialPlayerResponse.
#[derive(Clone, Debug, Default)]
pub struct YouTubeWatchDetails {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub views: String,
    pub description: String,
}

/// Visual browser viewport rendering DOM nodes into interactive GUI elements.
pub struct BrowserRenderer {
    pub input_states: HashMap<String, String>,
    pub speed_dial_input: String,
    pub history_search_query: String,
    pub new_bookmark_url: String,
    pub new_bookmark_title: String,
    pub new_bookmark_folder: String,
    pub clear_history_confirm: bool,
    pub zoom: f32,
    pub find_query: String,
}

impl Default for BrowserRenderer {
    fn default() -> Self {
        Self {
            input_states: HashMap::new(),
            speed_dial_input: String::new(),
            history_search_query: String::new(),
            new_bookmark_url: String::new(),
            new_bookmark_title: String::new(),
            new_bookmark_folder: "Favorites".to_string(),
            clear_history_confirm: false,
            zoom: 1.0,
            find_query: String::new(),
        }
    }
}

impl BrowserRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders the current browser tab contents.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ui: &mut Ui,
        url: &str,
        dom: Option<&DomNode>,
        raw_html: &str,
        is_loading: bool,
        error_msg: Option<&str>,
        storage: &excalibur_core::WorkspaceStorage,
        zoom: f32,
        find_query: &str,
    ) -> Option<BrowserAction> {
        self.zoom = if zoom < 0.5 { 1.0 } else { zoom };
        self.find_query = find_query.to_string();

        let mut action = None;

        if is_loading {
            self.render_loading(ui, url);
            return None;
        }

        if let Some(err) = error_msg {
            self.render_error(ui, url, err, &mut action);
            return action;
        }

        // Internal Browser Pages
        if url == "excalibur://history" {
            self.render_history_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://bookmarks" {
            self.render_bookmarks_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://settings" {
            self.render_settings_page(ui, storage, &mut action);
            return action;
        }
        if url == "excalibur://about" {
            self.render_about_page(ui, storage, &mut action);
            return action;
        }

        if url == "about:blank"
            || url == "excalibur://newtab"
            || (dom.is_none() && raw_html.is_empty())
        {
            self.render_speed_dial(ui, &mut action);
            return action;
        }

        // YouTube SPA client fallback: render interactive YouTube hub
        let is_youtube = url.contains("youtube.com")
            || (url.contains("youtube") && raw_html.contains("ytInitialData"));
        if is_youtube {
            let watch_details = extract_youtube_watch_details(raw_html);
            let videos = extract_youtube_videos(raw_html);
            self.render_youtube_hub(ui, url, watch_details.as_ref(), &videos, &mut action);
            return action;
        }

        if let Some(root) = dom {
            let stylesheet = StyleSheet::extract_from_dom(root);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(12.0);
                    let available_width = ui.available_width();
                    let content_width = (available_width.min(940.0) * self.zoom.clamp(0.6, 2.5))
                        .min(available_width);
                    let side_margin = ((available_width - content_width) / 2.0).max(16.0);

                    ui.horizontal(|ui| {
                        ui.add_space(side_margin);
                        ui.vertical(|ui| {
                            ui.set_max_width(content_width);
                            self.render_node(ui, root, &mut action, url, &stylesheet);
                        });
                        ui.add_space(side_margin);
                    });
                    ui.add_space(32.0);
                });
        }

        action
    }

    fn render_loading(&self, ui: &mut Ui, url: &str) {
        let time = ui.input(|i| i.time);
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);

            Frame::NONE
                .fill(Color32::from_rgb(6, 10, 18))
                .stroke(Stroke::new(1.5_f32, Color32::from_rgb(31, 111, 235)))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(Margin::symmetric(28, 22))
                .show(ui, |ui| {
                    ui.set_max_width(540.0);

                    // Telemetry Header
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("[NETWORK DISPATCH]")
                                .size(11.0)
                                .color(Color32::from_rgb(0, 240, 255))
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cycle = ((time * 2.5).sin() * 0.5 + 0.5) as f32;
                            let alpha = (120.0 + cycle * 135.0) as u8;
                            ui.label(
                                RichText::new("STREAMING ACTIVE...")
                                    .size(10.0)
                                    .color(Color32::from_rgba_unmultiplied(56, 189, 248, alpha))
                                    .strong(),
                            );
                        });
                    });

                    ui.add_space(14.0);

                    // High-tech Animated Progress Beam
                    let (track_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 4.0),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    painter.rect_filled(track_rect, 2.0, Color32::from_rgb(15, 23, 42));

                    let speed = 1.2;
                    let progress = (time * speed).fract() as f32;
                    let beam_len = track_rect.width() * 0.35;
                    let x_start =
                        track_rect.min.x + (track_rect.width() + beam_len) * progress - beam_len;
                    let x_clamped_start = x_start.max(track_rect.min.x);
                    let x_clamped_end = (x_start + beam_len).min(track_rect.max.x);

                    if x_clamped_end > x_clamped_start {
                        let beam_rect = egui::Rect::from_min_max(
                            egui::pos2(x_clamped_start, track_rect.min.y),
                            egui::pos2(x_clamped_end, track_rect.max.y),
                        );
                        painter.rect_filled(beam_rect, 2.0, Color32::from_rgb(0, 240, 255));
                    }

                    ui.add_space(18.0);

                    ui.label(
                        RichText::new("Loading Webpage...")
                            .size(19.0)
                            .strong()
                            .color(Color32::from_rgb(240, 246, 252)),
                    );

                    ui.add_space(6.0);

                    Frame::NONE
                        .fill(Color32::from_rgb(10, 15, 26))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 6))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(url)
                                    .size(12.0)
                                    .color(Color32::from_rgb(56, 189, 248))
                                    .monospace(),
                            );
                        });

                    ui.add_space(14.0);

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                        render_stage_badge(ui, "[DNS RESOLVED]", true);
                        render_stage_badge(ui, "[TLS 1.3 SECURE]", true);
                        render_stage_badge(ui, "[HTTP DISPATCH]", true);
                        render_stage_badge(ui, "[DOM COMPILER]", false);
                    });

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "Performing cryptographic handshake and compiling document layout...",
                        )
                        .size(11.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                    );
                });
        });
    }

    fn render_error(&self, ui: &mut Ui, url: &str, err: &str, action: &mut Option<BrowserAction>) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                RichText::new("Failed to Load Webpage")
                    .size(22.0)
                    .strong()
                    .color(Color32::from_rgb(248, 81, 73)),
            );
            ui.add_space(8.0);
            ui.label(format!("Target: {}", url));
            ui.add_space(8.0);

            Frame::NONE
                .fill(Color32::from_rgb(22, 27, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                .inner_margin(Margin::same(12))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.label(RichText::new(err).color(Color32::from_rgb(248, 81, 73)));
                });

            ui.add_space(16.0);
            if ui.button("Try Again").clicked() {
                *action = Some(BrowserAction::Navigate(url.to_string()));
            }
        });
    }

    fn render_speed_dial(&mut self, ui: &mut Ui, action: &mut Option<BrowserAction>) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(24.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(920.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    // 1. Hero Branding & Engine Badges
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("EXCALIBUR")
                                .size(36.0)
                                .strong()
                                .color(Color32::from_rgb(56, 189, 248)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Independent Security Browser & Penetration Testing Workbench",
                            )
                            .size(14.0)
                            .color(Color32::from_rgb(125, 211, 252)),
                        );
                        ui.add_space(12.0);

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                            let badges = [
                                ("[ NON-CHROMIUM ]", Color32::from_rgb(0, 240, 255)),
                                ("[ PURE-RUST DOM ]", Color32::from_rgb(0, 255, 157)),
                                ("[ ZERO VENDOR BLINK ]", Color32::from_rgb(250, 204, 21)),
                                (
                                    "[ MITM PROXY 127.0.0.1:8080 ]",
                                    Color32::from_rgb(255, 60, 90),
                                ),
                            ];

                            for (badge, color) in badges {
                                Frame::NONE
                                    .fill(Color32::from_rgb(8, 14, 26))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(badge).size(11.0).strong().color(color),
                                        );
                                    });
                            }
                        });

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                            let nav_links = [
                                (
                                    "History",
                                    "excalibur://history",
                                    Color32::from_rgb(56, 189, 248),
                                ),
                                (
                                    "Bookmarks",
                                    "excalibur://bookmarks",
                                    Color32::from_rgb(0, 255, 157),
                                ),
                                (
                                    "Settings",
                                    "excalibur://settings",
                                    Color32::from_rgb(250, 204, 21),
                                ),
                                (
                                    "About",
                                    "excalibur://about",
                                    Color32::from_rgb(192, 132, 252),
                                ),
                            ];
                            for (label, uri, col) in nav_links {
                                if ui
                                    .button(
                                        RichText::new(format!("[ {} ]", label))
                                            .size(11.5)
                                            .strong()
                                            .color(col),
                                    )
                                    .clicked()
                                {
                                    *action = Some(BrowserAction::Navigate(uri.to_string()));
                                }
                            }
                        });
                    });

                    ui.add_space(20.0);

                    // 2. Central Quick Launch Search Bar
                    Frame::NONE
                        .fill(Color32::from_rgb(4, 7, 13))
                        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(31, 80, 160)))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("URI / Search:")
                                        .size(12.0)
                                        .color(Color32::from_rgb(56, 189, 248))
                                        .strong(),
                                );

                                let te = ui.add(
                                    egui::TextEdit::singleline(&mut self.speed_dial_input)
                                        .hint_text(
                                            "Enter URL (e.g. https://owasp.org) or search query...",
                                        )
                                        .desired_width(ui.available_width() - 140.0),
                                );

                                let should_submit = (te.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                    || ui
                                        .button(
                                            RichText::new("Launch Target")
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .clicked();

                                if should_submit {
                                    let raw = self.speed_dial_input.trim();
                                    if !raw.is_empty() {
                                        let target = if raw.starts_with("http://")
                                            || raw.starts_with("https://")
                                        {
                                            raw.to_string()
                                        } else if raw.contains('.') && !raw.contains(' ') {
                                            format!("https://{}", raw)
                                        } else {
                                            format!(
                                                "https://html.duckduckgo.com/html/?q={}",
                                                raw.replace(' ', "+")
                                            )
                                        };
                                        *action = Some(BrowserAction::Navigate(target));
                                    }
                                }
                            });
                        });

                    ui.add_space(24.0);

                    // 3. Categorized Speed Dial Targets
                    let categories = [
                        (
                            "Security Testing & Labs",
                            Color32::from_rgb(255, 60, 90),
                            vec![
                                (
                                    "OWASP Foundation",
                                    "https://owasp.org",
                                    "Web application security standards and top 10",
                                ),
                                (
                                    "PortSwigger Academy",
                                    "https://portswigger.net/web-security",
                                    "Interactive vulnerability training labs",
                                ),
                                (
                                    "HackerOne Directory",
                                    "https://hackerone.com",
                                    "Vulnerability coordination & bug bounty directory",
                                ),
                                (
                                    "CVE Details",
                                    "https://cvedetails.com",
                                    "Security vulnerability database and exploit tracking",
                                ),
                            ],
                        ),
                        (
                            "Developer & Research Standards",
                            Color32::from_rgb(56, 189, 248),
                            vec![
                                (
                                    "GitHub",
                                    "https://github.com",
                                    "Code hosting, repositories & security advisories",
                                ),
                                (
                                    "Hacker News",
                                    "https://news.ycombinator.com",
                                    "Technology news aggregator with dense HTML tables",
                                ),
                                (
                                    "Example Domain",
                                    "https://example.com",
                                    "Official RFC 2606 reference standard domain",
                                ),
                                (
                                    "MDN Web Docs",
                                    "https://developer.mozilla.org",
                                    "Web platform documentation & HTML5/JS APIs",
                                ),
                            ],
                        ),
                        (
                            "Diagnostics & Local Sandboxes",
                            Color32::from_rgb(0, 255, 157),
                            vec![
                                (
                                    "Cloudflare Trace",
                                    "https://cloudflare.com/cdn-cgi/trace",
                                    "Edge CDN routing, TLS SNI, and IP diagnostics",
                                ),
                                (
                                    "HTTPBin HTML Sandbox",
                                    "https://httpbin.org/html",
                                    "HTTP request/response test suite with HTML sample",
                                ),
                                (
                                    "Local MITM Proxy",
                                    "http://127.0.0.1:8080",
                                    "Excalibur proxy listener and CA inspection",
                                ),
                            ],
                        ),
                    ];

                    for (cat_name, cat_color, targets) in categories {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("[ {} ]", cat_name))
                                    .size(13.0)
                                    .strong()
                                    .color(cat_color),
                            );
                        });
                        ui.add_space(6.0);

                        for (name, url, desc) in targets {
                            Frame::NONE
                                .fill(Color32::from_rgb(9, 14, 25))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                                .corner_radius(egui::CornerRadius::same(8))
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
                                                    .color(Color32::from_rgb(148, 163, 184)),
                                            );
                                            ui.label(
                                                RichText::new(url)
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(56, 189, 248)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .button(
                                                        RichText::new("Launch")
                                                            .strong()
                                                            .color(Color32::WHITE),
                                                    )
                                                    .clicked()
                                                {
                                                    *action = Some(BrowserAction::Navigate(
                                                        url.to_string(),
                                                    ));
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }

                        ui.add_space(14.0);
                    }

                    // 4. Engine Diagnostics Dashboard
                    Frame::NONE
                        .fill(Color32::from_rgb(5, 8, 15))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 52, 92)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("ENGINE TELEMETRY")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(0, 240, 255)),
                                    );
                                    ui.label(
                                        RichText::new("Hyper + Rustls 0.23 (TLS 1.3)")
                                            .color(Color32::from_rgb(220, 230, 245)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("MITM PROXY")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(0, 255, 157)),
                                    );
                                    ui.label(
                                        RichText::new("127.0.0.1:8080 (Listening)")
                                            .color(Color32::from_rgb(220, 230, 245)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("DOM PARSER")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(250, 204, 21)),
                                    );
                                    ui.label(
                                        RichText::new("Pure-Rust Semantic AST")
                                            .color(Color32::from_rgb(220, 230, 245)),
                                    );
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("SECURITY AUDIT")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(255, 60, 90)),
                                    );
                                    ui.label(
                                        RichText::new("QuickJS Sinks + Passive Radar")
                                            .color(Color32::from_rgb(220, 230, 245)),
                                    );
                                });
                            });
                        });

                    ui.add_space(32.0);
                });
                ui.add_space(side_margin);
            });
        });
    }

    fn render_history_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    // Header row
                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(18, 38, 72))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("[ HISTORY ]")
                                        .size(13.0)
                                        .color(Color32::from_rgb(0, 240, 255))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Browsing History & Audit Timeline")
                                .size(22.0)
                                .color(Color32::from_rgb(240, 246, 252))
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.clear_history_confirm {
                                if ui
                                    .button(
                                        RichText::new("[Confirm Clear All]")
                                            .color(Color32::from_rgb(255, 60, 90))
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    let _ = storage.clear_browser_history();
                                    self.clear_history_confirm = false;
                                }
                                if ui.small_button("Cancel").clicked() {
                                    self.clear_history_confirm = false;
                                }
                            } else if ui
                                .button(
                                    RichText::new("Clear History")
                                        .color(Color32::from_rgb(248, 113, 113)),
                                )
                                .clicked()
                            {
                                self.clear_history_confirm = true;
                            }
                        });
                    });

                    ui.add_space(14.0);

                    // Search filter box
                    Frame::NONE
                        .fill(Color32::from_rgb(6, 10, 18))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(31, 80, 160)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Filter:")
                                        .color(Color32::from_rgb(56, 189, 248))
                                        .strong(),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.history_search_query)
                                        .hint_text("Search visited URLs or page titles...")
                                        .desired_width(ui.available_width() - 80.0),
                                );
                                if !self.history_search_query.is_empty()
                                    && ui.small_button("Clear").clicked()
                                {
                                    self.history_search_query.clear();
                                }
                            });
                        });

                    ui.add_space(16.0);

                    let filter_opt = if self.history_search_query.trim().is_empty() {
                        None
                    } else {
                        Some(self.history_search_query.as_str())
                    };

                    let history_entries = storage
                        .list_browser_history(100, 0, filter_opt)
                        .unwrap_or_default();

                    if history_entries.is_empty() {
                        Frame::NONE
                            .fill(Color32::from_rgb(10, 15, 26))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(24, 20))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No browsing history found.")
                                            .size(15.0)
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Visited pages will automatically be indexed here.",
                                        )
                                        .size(12.0)
                                        .color(Color32::from_rgb(100, 116, 139)),
                                    );
                                });
                            });
                    } else {
                        let mut delete_id = None;
                        for entry in &history_entries {
                            Frame::NONE
                                .fill(Color32::from_rgb(8, 13, 24))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let tag = if entry.url.contains("owasp") {
                                            "SEC"
                                        } else if entry.url.contains("github") {
                                            "GIT"
                                        } else if entry.url.contains("youtube") {
                                            "YT"
                                        } else {
                                            "WEB"
                                        };

                                        ui.label(
                                            RichText::new(format!("[{}]", tag))
                                                .size(11.0)
                                                .color(Color32::from_rgb(56, 189, 248))
                                                .strong(),
                                        );

                                        ui.vertical(|ui| {
                                            let title_btn = ui.link(
                                                RichText::new(&entry.title)
                                                    .size(14.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(240, 246, 252)),
                                            );
                                            if title_btn.clicked() {
                                                *action = Some(BrowserAction::Navigate(
                                                    entry.url.clone(),
                                                ));
                                            }

                                            ui.label(
                                                RichText::new(&entry.url)
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(125, 211, 252)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("x").clicked() {
                                                    delete_id = Some(entry.id);
                                                }

                                                if entry.visit_count > 1 {
                                                    Frame::NONE
                                                        .fill(Color32::from_rgb(15, 23, 42))
                                                        .stroke(Stroke::new(
                                                            1.0_f32,
                                                            Color32::from_rgb(51, 65, 85),
                                                        ))
                                                        .corner_radius(egui::CornerRadius::same(4))
                                                        .inner_margin(Margin::symmetric(6, 2))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "{} visits",
                                                                    entry.visit_count
                                                                ))
                                                                .size(10.5)
                                                                .color(Color32::from_rgb(
                                                                    148, 163, 184,
                                                                )),
                                                            );
                                                        });
                                                }

                                                let time_str =
                                                    format_unix_timestamp(entry.visit_time);
                                                ui.label(
                                                    RichText::new(time_str)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(100, 116, 139)),
                                                );
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }

                        if let Some(id) = delete_id {
                            let _ = storage.delete_browser_history_entry(id);
                        }
                    }
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_bookmarks_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(18, 38, 72))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("[ BOOKMARKS ]")
                                        .size(13.0)
                                        .color(Color32::from_rgb(0, 240, 255))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Favorites & Quick Access Targets")
                                .size(22.0)
                                .color(Color32::from_rgb(240, 246, 252))
                                .strong(),
                        );
                    });

                    ui.add_space(16.0);

                    // Add Bookmark inline card
                    Frame::NONE
                        .fill(Color32::from_rgb(10, 16, 28))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(31, 111, 235)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Add New Bookmark")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(56, 189, 248)),
                            );
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label("Title:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_title)
                                        .hint_text("Page Name")
                                        .desired_width(180.0),
                                );

                                ui.label("URL:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_url)
                                        .hint_text("https://example.com")
                                        .desired_width(260.0),
                                );

                                ui.label("Folder:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_bookmark_folder)
                                        .hint_text("Favorites")
                                        .desired_width(120.0),
                                );

                                if ui
                                    .button(RichText::new("+ Save").strong().color(Color32::WHITE))
                                    .clicked()
                                {
                                    let url = self.new_bookmark_url.trim();
                                    if !url.is_empty() {
                                        let title = if self.new_bookmark_title.trim().is_empty() {
                                            url
                                        } else {
                                            self.new_bookmark_title.trim()
                                        };
                                        let folder = if self.new_bookmark_folder.trim().is_empty() {
                                            "Favorites"
                                        } else {
                                            self.new_bookmark_folder.trim()
                                        };
                                        let _ = storage.add_browser_bookmark(url, title, folder);
                                        self.new_bookmark_url.clear();
                                        self.new_bookmark_title.clear();
                                    }
                                }
                            });
                        });

                    ui.add_space(16.0);

                    // List bookmarks
                    let bookmarks = storage.list_browser_bookmarks().unwrap_or_default();
                    let mut remove_url = None;

                    for bm in &bookmarks {
                        Frame::NONE
                            .fill(Color32::from_rgb(8, 13, 24))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    Frame::NONE
                                        .fill(Color32::from_rgb(18, 38, 72))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&bm.folder)
                                                    .size(10.0)
                                                    .color(Color32::from_rgb(56, 189, 248))
                                                    .strong(),
                                            );
                                        });

                                    ui.vertical(|ui| {
                                        let title_btn = ui.link(
                                            RichText::new(&bm.title)
                                                .size(14.0)
                                                .strong()
                                                .color(Color32::from_rgb(240, 246, 252)),
                                        );
                                        if title_btn.clicked() {
                                            *action = Some(BrowserAction::Navigate(bm.url.clone()));
                                        }

                                        ui.label(
                                            RichText::new(&bm.url)
                                                .size(11.5)
                                                .color(Color32::from_rgb(125, 211, 252)),
                                        );
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("Delete").clicked() {
                                                remove_url = Some(bm.url.clone());
                                            }

                                            if ui.small_button("Copy").clicked() {
                                                ui.ctx().copy_text(bm.url.clone());
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }

                    if let Some(url) = remove_url {
                        let _ = storage.remove_browser_bookmark(&url);
                    }
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_settings_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        _action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.horizontal(|ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(18, 38, 72))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("[ SETTINGS ]")
                                        .size(13.0)
                                        .color(Color32::from_rgb(0, 240, 255))
                                        .strong(),
                                );
                            });

                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Browser Configuration & Security Controls")
                                .size(22.0)
                                .color(Color32::from_rgb(240, 246, 252))
                                .strong(),
                        );
                    });

                    ui.add_space(20.0);

                    // 1. Search Engine Preference
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Default Search Engine")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Select the primary query provider used when entering non-URL terms in the omnibox.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                            );
                            ui.add_space(10.0);

                            let current_engine = storage
                                .get_browser_setting("search_engine")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "DuckDuckGo".to_string());

                            let engines = ["DuckDuckGo", "Google", "SearXNG", "Brave", "YouTube"];
                            ui.horizontal_wrapped(|ui| {
                                for eng in engines {
                                    let is_active = current_engine == eng;
                                    if ui
                                        .selectable_label(is_active, RichText::new(eng).strong())
                                        .clicked()
                                    {
                                        let _ = storage.set_browser_setting("search_engine", eng);
                                    }
                                }
                            });
                        });

                    ui.add_space(14.0);

                    // 2. User-Agent Identity & Stealth Header
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("User-Agent Header & Browser Identity")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Configure client header identification sent during HTTP/1.1 and HTTP/2 handshakes.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                            );
                            ui.add_space(10.0);

                            let current_ua = storage
                                .get_browser_setting("user_agent_preset")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "Excalibur".to_string());

                            let presets = [
                                ("Excalibur", "Excalibur/1.0 (Security Engine; Non-Chromium)"),
                                ("Chrome", "Chrome 134 on Windows 11 (Blink Spoof)"),
                                ("Firefox", "Firefox 136 on Linux (Gecko Spoof)"),
                                ("Safari", "Safari 18 on macOS (WebKit Spoof)"),
                                ("Curl", "curl/8.8.0 (Security Scanner Emulation)"),
                            ];

                            for (key, label) in presets {
                                let is_active = current_ua == key;
                                if ui
                                    .selectable_label(is_active, RichText::new(label))
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("user_agent_preset", key);
                                }
                            }
                        });

                    ui.add_space(14.0);

                    // 3. Tab Layout Mode
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Tab Strip Layout")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Choose between standard horizontal top tabs or the modern vertical Zen/Arc sidebar.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                            );
                            ui.add_space(10.0);

                            let current_layout = storage
                                .get_browser_setting("tab_layout")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "top".to_string());

                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        current_layout == "top",
                                        "Top Tab Strip (Standard)",
                                    )
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("tab_layout", "top");
                                }
                                if ui
                                    .selectable_label(
                                        current_layout == "sidebar",
                                        "Zen Vertical Sidebar (Arc Mode)",
                                    )
                                    .clicked()
                                {
                                    let _ = storage.set_browser_setting("tab_layout", "sidebar");
                                }
                            });
                        });

                    ui.add_space(14.0);

                    // 4. Default Content Zoom
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Default Page Zoom")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Adjust default content rendering scale factor across tabs (fine-tune per tab with Ctrl++ and Ctrl+-).",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                            );
                            ui.add_space(10.0);

                            let zoom_str = storage
                                .get_browser_setting("default_zoom")
                                .unwrap_or_default()
                                .unwrap_or_else(|| "100%".to_string());

                            let zoom_levels = ["80%", "90%", "100%", "110%", "125%", "150%"];
                            ui.horizontal(|ui| {
                                for z in zoom_levels {
                                    let is_active = zoom_str == z;
                                    if ui.selectable_label(is_active, z).clicked() {
                                        let _ = storage.set_browser_setting("default_zoom", z);
                                    }
                                }
                            });
                        });
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_about_page(
        &mut self,
        ui: &mut Ui,
        storage: &excalibur_core::WorkspaceStorage,
        _action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(20.0);
            let available_width = ui.available_width();
            let content_width = available_width.min(940.0);
            let side_margin = ((available_width - content_width) / 2.0).max(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("EXCALIBUR")
                                .size(36.0)
                                .strong()
                                .color(Color32::from_rgb(56, 189, 248)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Version 0.1.0 (Cyber Modern Edition)")
                                .size(14.0)
                                .color(Color32::from_rgb(125, 211, 252)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Independent Security Browser & Penetration Testing Workbench",
                            )
                            .size(13.0)
                            .color(Color32::from_rgb(148, 163, 184)),
                        );
                    });

                    ui.add_space(24.0);

                    // Core Pillars Card
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Engine Architecture & Guarantees")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(8.0);

                            let pillars = [
                                (
                                    "[ NON-CHROMIUM ]",
                                    "Zero dependencies on Google Chromium, CEF, WebKit, or Blink runtimes.",
                                    Color32::from_rgb(0, 240, 255),
                                ),
                                (
                                    "[ PURE RUST DOM ]",
                                    "Custom semantic DOM parser and CSS style cascade resolver in safe Rust.",
                                    Color32::from_rgb(0, 255, 157),
                                ),
                                (
                                    "[ GPU HARDWARE ]",
                                    "Direct hardware-accelerated rendering using egui 0.31 over wgpu/glow.",
                                    Color32::from_rgb(250, 204, 21),
                                ),
                                (
                                    "[ SECURITY RADAR ]",
                                    "Embedded QuickJS client sink interception and RFC security header auditing.",
                                    Color32::from_rgb(255, 60, 90),
                                ),
                                (
                                    "[ MITM PROXY ]",
                                    "Integrated local proxy daemon on 127.0.0.1:8080 with on-the-fly CA generation.",
                                    Color32::from_rgb(192, 132, 252),
                                ),
                            ];

                            for (badge, desc, color) in pillars {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(badge).size(11.0).strong().color(color),
                                    );
                                    ui.label(
                                        RichText::new(desc)
                                            .size(12.0)
                                            .color(Color32::from_rgb(203, 213, 225)),
                                    );
                                });
                                ui.add_space(4.0);
                            }
                        });

                    ui.add_space(16.0);

                    // Live Diagnostics Card
                    Frame::NONE
                        .fill(Color32::from_rgb(8, 13, 24))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("System Telemetry")
                                    .size(15.0)
                                    .strong()
                                    .color(Color32::from_rgb(240, 246, 252)),
                            );
                            ui.add_space(8.0);

                            let history_count = storage
                                .list_browser_history(1, 0, None)
                                .map(|h| h.len())
                                .unwrap_or(0);
                            let bookmark_count = storage
                                .list_browser_bookmarks()
                                .map(|b| b.len())
                                .unwrap_or(0);

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Database Storage:")
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                ui.label(
                                    RichText::new("SQLite 3 with WAL & FTS5 (High-Performance)")
                                        .color(Color32::from_rgb(56, 189, 248)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Saved Bookmarks:")
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                ui.label(
                                    RichText::new(format!("{} entries", bookmark_count))
                                        .color(Color32::from_rgb(0, 255, 157)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Recent History Log:")
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                ui.label(
                                    RichText::new(format!("{} items loaded", history_count))
                                        .color(Color32::from_rgb(250, 204, 21)),
                                );
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Repository:")
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                                ui.label(
                                    RichText::new("https://github.com/Zeo120/Excalibur")
                                        .color(Color32::from_rgb(125, 211, 252)),
                                );
                            });
                        });
                });
                ui.add_space(side_margin);
            });
            ui.add_space(32.0);
        });
    }

    fn render_node(
        &mut self,
        ui: &mut Ui,
        node: &DomNode,
        action: &mut Option<BrowserAction>,
        current_url: &str,
        stylesheet: &StyleSheet,
    ) {
        match &node.node_type {
            NodeType::Document => {
                for child in &node.children {
                    self.render_node(ui, child, action, current_url, stylesheet);
                }
            }
            NodeType::Element { tag, attributes } => {
                let tag_lower = tag.to_ascii_lowercase();

                // Skip non-rendering metadata elements
                if tag_lower == "head"
                    || tag_lower == "style"
                    || tag_lower == "script"
                    || tag_lower == "meta"
                    || tag_lower == "link"
                    || tag_lower == "title"
                    || tag_lower == "noscript"
                {
                    return;
                }

                let style = StyleResolver::resolve(node, stylesheet);
                if style.display_none {
                    return;
                }

                if let Some(mt) = style.margin_top {
                    if mt > 0.0 {
                        ui.add_space(mt.clamp(0.0, 40.0));
                    }
                }

                match tag_lower.as_str() {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let (default_size, default_color) = match tag_lower.as_str() {
                                "h1" => (24.0, Color32::from_rgb(88, 166, 255)),
                                "h2" => (20.0, Color32::from_rgb(121, 192, 255)),
                                "h3" => (16.0, Color32::from_rgb(201, 209, 217)),
                                "h4" => (14.0, Color32::from_rgb(201, 209, 217)),
                                _ => (13.0, Color32::from_rgb(139, 148, 158)),
                            };

                            let font_size = style.font_size.unwrap_or(default_size);
                            let text_color =
                                style.color.map(to_egui_color).unwrap_or(default_color);

                            let mut rich = RichText::new(trimmed).size(font_size).color(text_color);
                            if style.font_weight_bold.unwrap_or(true) {
                                rich = rich.strong();
                            }
                            if style.font_style_italic.unwrap_or(false) {
                                rich = rich.italics();
                            }
                            if style.text_decoration_underline.unwrap_or(false) {
                                rich = rich.underline();
                            }

                            if let Some(bg) = style.background_color {
                                let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0) as u8;
                                Frame::NONE
                                    .fill(to_egui_color(bg))
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .corner_radius(egui::CornerRadius::same(rad))
                                    .show(ui, |ui| {
                                        ui.label(rich);
                                    });
                            } else {
                                ui.label(rich);
                            }
                            ui.add_space(4.0);
                        }
                    }
                    "p" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let font_size = style.font_size.unwrap_or(14.0);
                            let text_color = style
                                .color
                                .map(to_egui_color)
                                .unwrap_or(Color32::from_rgb(201, 209, 217));

                            let mut rich = RichText::new(trimmed).size(font_size).color(text_color);
                            if style.font_weight_bold.unwrap_or(false) {
                                rich = rich.strong();
                            }
                            if style.font_style_italic.unwrap_or(false) {
                                rich = rich.italics();
                            }
                            if style.text_decoration_underline.unwrap_or(false) {
                                rich = rich.underline();
                            }

                            if let Some(bg) = style.background_color {
                                let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0) as u8;
                                Frame::NONE
                                    .fill(to_egui_color(bg))
                                    .inner_margin(Margin::same(6))
                                    .corner_radius(egui::CornerRadius::same(rad))
                                    .show(ui, |ui| {
                                        ui.label(rich);
                                    });
                            } else {
                                ui.label(rich);
                            }
                            ui.add_space(4.0);
                        }
                    }
                    "a" => {
                        let href = attributes.get("href").map(|s| s.as_str()).unwrap_or("");
                        let text = node.text_content();
                        let display_text = if text.trim().is_empty() {
                            href
                        } else {
                            text.trim()
                        };

                        let font_size = style.font_size.unwrap_or(14.0);
                        let text_color = style
                            .color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(88, 166, 255));

                        let mut rich = RichText::new(display_text)
                            .size(font_size)
                            .color(text_color);
                        if style.text_decoration_underline.unwrap_or(true) {
                            rich = rich.underline();
                        }
                        if style.font_weight_bold.unwrap_or(false) {
                            rich = rich.strong();
                        }

                        if ui.link(rich).clicked() && !href.is_empty() {
                            let target_url = self.resolve_url(current_url, href);
                            *action = Some(BrowserAction::Navigate(target_url));
                        }
                    }
                    "span" => {
                        let has_child_elements = node
                            .children
                            .iter()
                            .any(|c| matches!(c.node_type, NodeType::Element { .. }));
                        if !has_child_elements {
                            let text = node.text_content();
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                let font_size = style.font_size.unwrap_or(14.0);
                                let text_color = style
                                    .color
                                    .map(to_egui_color)
                                    .unwrap_or(Color32::from_rgb(201, 209, 217));

                                let mut rich =
                                    RichText::new(trimmed).size(font_size).color(text_color);
                                if style.font_weight_bold.unwrap_or(false) {
                                    rich = rich.strong();
                                }
                                if style.font_style_italic.unwrap_or(false) {
                                    rich = rich.italics();
                                }
                                if style.text_decoration_underline.unwrap_or(false) {
                                    rich = rich.underline();
                                }
                                ui.label(rich);
                            }
                        } else {
                            for child in &node.children {
                                self.render_node(ui, child, action, current_url, stylesheet);
                            }
                        }
                    }
                    "strong" | "b" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let color = style.color.map(to_egui_color).unwrap_or(Color32::WHITE);
                            ui.label(RichText::new(trimmed).strong().color(color));
                        }
                    }
                    "em" | "i" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let color = style
                                .color
                                .map(to_egui_color)
                                .unwrap_or(Color32::from_rgb(201, 209, 217));
                            ui.label(RichText::new(trimmed).italics().color(color));
                        }
                    }
                    "br" => {
                        ui.add_space(4.0);
                    }
                    "button" => {
                        let text = node.text_content();
                        let btn_text = if text.trim().is_empty() {
                            "Submit"
                        } else {
                            text.trim()
                        };

                        let text_color = style.color.map(to_egui_color).unwrap_or(Color32::WHITE);
                        let font_size = style.font_size.unwrap_or(13.0);
                        let rich = RichText::new(btn_text).size(font_size).color(text_color);

                        if ui.button(rich).clicked() {
                            tracing::info!("Button clicked: {}", btn_text);
                        }
                    }
                    "img" => {
                        let src = attributes
                            .get("src")
                            .or_else(|| attributes.get("data-src"))
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        if !src.is_empty() {
                            let resolved = self.resolve_url(current_url, src);
                            let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 20.0);
                            ui.add_space(4.0);
                            ui.add(
                                egui::Image::from_uri(&resolved)
                                    .max_width(ui.available_width().min(600.0))
                                    .corner_radius(rad),
                            );
                            ui.add_space(4.0);
                        }
                    }
                    "form" => {
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(22, 27, 34));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let border_w = style.border_width.unwrap_or(1.0);
                        let rad = style.border_radius.unwrap_or(6.0).clamp(0.0, 20.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(border_w, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    for child in &node.children {
                                        self.render_node(
                                            ui,
                                            child,
                                            action,
                                            current_url,
                                            stylesheet,
                                        );
                                    }
                                });
                            });
                        ui.add_space(8.0);
                    }
                    "input" => {
                        let input_type =
                            attributes.get("type").map(|s| s.as_str()).unwrap_or("text");
                        let input_name = attributes
                            .get("name")
                            .or_else(|| attributes.get("id"))
                            .map(|s| s.as_str())
                            .unwrap_or("field");
                        let default_val = attributes.get("value").map(|s| s.as_str()).unwrap_or("");

                        let current_val = self
                            .input_states
                            .entry(input_name.to_string())
                            .or_insert_with(|| default_val.to_string());

                        if input_type == "submit" {
                            let label = if current_val.is_empty() {
                                "Submit"
                            } else {
                                current_val.as_str()
                            };
                            if ui.button(label).clicked() {
                                tracing::info!("Submit input clicked: {}", input_name);
                            }
                        } else if input_type == "password" {
                            ui.horizontal(|ui| {
                                let placeholder = attributes
                                    .get("placeholder")
                                    .or_else(|| attributes.get("aria-label"))
                                    .map(|s| s.as_str())
                                    .unwrap_or("Password");
                                ui.add(
                                    egui::TextEdit::singleline(current_val)
                                        .password(true)
                                        .hint_text(placeholder)
                                        .desired_width(ui.available_width().min(320.0)),
                                );
                            });
                        } else {
                            ui.horizontal(|ui| {
                                let placeholder = attributes
                                    .get("placeholder")
                                    .or_else(|| attributes.get("aria-label"))
                                    .or_else(|| attributes.get("title"))
                                    .map(|s| s.as_str())
                                    .unwrap_or_else(|| {
                                        if input_type == "search"
                                            || input_name == "search_query"
                                            || input_name == "q"
                                        {
                                            "Search or enter query..."
                                        } else {
                                            "Enter text..."
                                        }
                                    });

                                let te = ui.add(
                                    egui::TextEdit::singleline(current_val)
                                        .hint_text(placeholder)
                                        .desired_width(ui.available_width().min(450.0)),
                                );

                                let is_search_field = input_name == "q"
                                    || input_name == "search_query"
                                    || input_name == "query"
                                    || input_type == "search";

                                if is_search_field
                                    && (ui.button("Search").clicked()
                                        || (te.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                    && !current_val.trim().is_empty()
                                {
                                    let query = current_val.trim().replace(' ', "+");
                                    let search_target = if current_url.contains("youtube.com") {
                                        format!("https://www.youtube.com/results?search_query={}", query)
                                    } else if current_url.contains("wikipedia.org") {
                                        format!("https://en.wikipedia.org/wiki/Special:Search?search={}", query)
                                    } else if current_url.contains("duckduckgo.com") {
                                        format!("https://duckduckgo.com/?q={}", query)
                                    } else {
                                        format!("https://www.google.com/search?q={}", query)
                                    };
                                    *action = Some(BrowserAction::Navigate(search_target));
                                }
                            });
                        }
                    }
                    "hr" => {
                        ui.separator();
                    }
                    "pre" | "code" => {
                        let code_text = node.text_content();
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(13, 17, 23));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(1.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                if let Some(tc) = style.color {
                                    ui.label(
                                        RichText::new(code_text.trim())
                                            .monospace()
                                            .color(to_egui_color(tc)),
                                    );
                                } else {
                                    ui.code(code_text.trim());
                                }
                            });
                    }
                    "blockquote" => {
                        let text = node.text_content();
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(22, 27, 34));
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(88, 166, 255));
                        let text_col = style
                            .color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(201, 209, 217));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(2.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.label(RichText::new(text.trim()).italics().color(text_col));
                            });
                    }
                    "ul" | "ol" => {
                        ui.vertical(|ui| {
                            for child in &node.children {
                                if child.tag() == Some("li") {
                                    let item_style = StyleResolver::resolve(child, stylesheet);
                                    let item_text = child.text_content();
                                    let item_color = item_style
                                        .color
                                        .or(style.color)
                                        .map(to_egui_color)
                                        .unwrap_or(Color32::from_rgb(201, 209, 217));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("-").color(item_color));
                                        ui.label(RichText::new(item_text.trim()).color(item_color));
                                    });
                                }
                            }
                        });
                    }
                    "table" => {
                        let bg_col = style
                            .background_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::TRANSPARENT);
                        let border_col = style
                            .border_color
                            .map(to_egui_color)
                            .unwrap_or(Color32::from_rgb(48, 54, 61));
                        let rad = style.border_radius.unwrap_or(4.0).clamp(0.0, 16.0) as u8;

                        Frame::NONE
                            .fill(bg_col)
                            .stroke(Stroke::new(1.0_f32, border_col))
                            .corner_radius(egui::CornerRadius::same(rad))
                            .inner_margin(Margin::same(6))
                            .show(ui, |ui| {
                                for child in &node.children {
                                    self.render_node(ui, child, action, current_url, stylesheet);
                                }
                            });
                    }
                    "tr" => {
                        ui.horizontal(|ui| {
                            for child in &node.children {
                                let cell_tag = child.tag().unwrap_or("");
                                if cell_tag == "td" || cell_tag == "th" {
                                    let cell_style = StyleResolver::resolve(child, stylesheet);
                                    let is_header = cell_tag == "th";
                                    let text = child.text_content();

                                    let cell_color = cell_style
                                        .color
                                        .or(style.color)
                                        .map(to_egui_color)
                                        .unwrap_or(if is_header {
                                            Color32::WHITE
                                        } else {
                                            Color32::from_rgb(201, 209, 217)
                                        });
                                    let mut rich = RichText::new(text.trim()).color(cell_color);
                                    if is_header || cell_style.font_weight_bold.unwrap_or(false) {
                                        rich = rich.strong();
                                    }

                                    if let Some(bg) = cell_style.background_color {
                                        Frame::NONE
                                            .fill(to_egui_color(bg))
                                            .inner_margin(Margin::same(4))
                                            .show(ui, |ui| {
                                                ui.label(rich);
                                            });
                                    } else {
                                        ui.label(rich);
                                    }
                                    ui.separator();
                                }
                            }
                        });
                    }
                    _ => {
                        // Standard containers: div, section, article, main, header, footer, body, nav, etc.
                        let has_frame = style.background_color.is_some()
                            || style.border_color.is_some()
                            || style.padding.is_some();
                        if has_frame {
                            let mut frame = Frame::NONE;
                            if let Some(bg) = style.background_color {
                                frame = frame.fill(to_egui_color(bg));
                            }
                            if let Some(bc) = style.border_color {
                                let bw = style.border_width.unwrap_or(1.0);
                                frame = frame.stroke(Stroke::new(bw, to_egui_color(bc)));
                            }
                            if let Some(pad) = style.padding {
                                frame =
                                    frame.inner_margin(Margin::same(pad.clamp(0.0, 30.0) as i8));
                            }
                            if let Some(rad) = style.border_radius {
                                frame = frame.corner_radius(egui::CornerRadius::same(
                                    rad.clamp(0.0, 24.0) as u8,
                                ));
                            }

                            frame.show(ui, |ui| {
                                ui.vertical(|ui| {
                                    for child in &node.children {
                                        self.render_node(
                                            ui,
                                            child,
                                            action,
                                            current_url,
                                            stylesheet,
                                        );
                                    }
                                });
                            });
                        } else {
                            for child in &node.children {
                                self.render_node(ui, child, action, current_url, stylesheet);
                            }
                        }
                    }
                }

                if let Some(mb) = style.margin_bottom {
                    if mb > 0.0 {
                        ui.add_space(mb.clamp(0.0, 40.0));
                    }
                }
            }
            NodeType::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let font_size = 13.0 * self.zoom.clamp(0.6, 2.5);
                    if !self.find_query.trim().is_empty()
                        && trimmed
                            .to_lowercase()
                            .contains(&self.find_query.trim().to_lowercase())
                    {
                        Frame::NONE
                            .fill(Color32::from_rgb(250, 204, 21))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(254, 240, 138)))
                            .corner_radius(egui::CornerRadius::same(3))
                            .inner_margin(Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(trimmed)
                                        .size(font_size)
                                        .color(Color32::from_rgb(10, 15, 26))
                                        .strong(),
                                );
                            });
                    } else {
                        ui.label(RichText::new(trimmed).size(font_size));
                    }
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    fn resolve_url(&self, base_url: &str, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") {
            return href.to_string();
        }

        if let Ok(base) = url::Url::parse(base_url) {
            if let Ok(resolved) = base.join(href) {
                return resolved.to_string();
            }
        }

        href.to_string()
    }

    fn render_youtube_hub(
        &mut self,
        ui: &mut Ui,
        url: &str,
        watch_details: Option<&YouTubeWatchDetails>,
        videos: &[YouTubeVideoItem],
        action: &mut Option<BrowserAction>,
    ) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(12.0);
                let available_width = ui.available_width();
                let content_width = available_width.min(1080.0);
                let side_margin = ((available_width - content_width) / 2.0).max(16.0);

                ui.horizontal(|ui| {
                    ui.add_space(side_margin);
                    ui.vertical(|ui| {
                        ui.set_max_width(content_width);

                        // YouTube Top Navigation & Search Bar
                        Frame::NONE
                            .fill(Color32::from_rgb(10, 15, 26))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(31, 41, 55)))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Brand Pill
                                    Frame::NONE
                                        .fill(Color32::from_rgb(220, 38, 38))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(8, 3))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("YOUTUBE")
                                                    .size(11.0)
                                                    .color(Color32::WHITE)
                                                    .strong(),
                                            );
                                        });

                                    ui.add_space(8.0);

                                    // YouTube In-Hub Search Box
                                    let mut search_target = None;
                                    let search_width = (ui.available_width() - 120.0).max(180.0);
                                    let yt_search_val = self
                                        .input_states
                                        .entry("yt_hub_search".to_string())
                                        .or_default();

                                    let te = ui.add(
                                        egui::TextEdit::singleline(yt_search_val)
                                            .hint_text("Search YouTube videos, topics, channels...")
                                            .desired_width(search_width),
                                    );

                                    if (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                        || ui.button(RichText::new("Search").color(Color32::WHITE).strong()).clicked()
                                    {
                                        let q = yt_search_val.trim();
                                        if !q.is_empty() {
                                            search_target = Some(format!(
                                                "https://www.youtube.com/results?search_query={}",
                                                q.replace(' ', "+")
                                            ));
                                        }
                                    }

                                    if let Some(target) = search_target {
                                        *action = Some(BrowserAction::Navigate(target));
                                    }
                                });

                                ui.add_space(6.0);

                                // Topic Pills
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                                    let topics = [
                                        "All",
                                        "Cybersecurity",
                                        "Rust Programming",
                                        "Computer Science",
                                        "Tech News",
                                        "Hardware",
                                        "Podcasts",
                                        "Gaming",
                                    ];
                                    for t in topics {
                                        let is_active = url.contains(&t.replace(' ', "+"));
                                        let (bg, fg) = if is_active {
                                            (Color32::from_rgb(31, 111, 235), Color32::WHITE)
                                        } else {
                                            (Color32::from_rgb(18, 24, 38), Color32::from_rgb(203, 213, 225))
                                        };

                                        Frame::NONE
                                            .fill(bg)
                                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 99, 235)))
                                            .corner_radius(egui::CornerRadius::same(12))
                                            .inner_margin(Margin::symmetric(10, 4))
                                            .show(ui, |ui| {
                                                if ui.selectable_label(is_active, RichText::new(t).size(11.0).color(fg)).clicked() {
                                                    let target = if t == "All" {
                                                        "https://www.youtube.com".to_string()
                                                    } else {
                                                        format!("https://www.youtube.com/results?search_query={}", t.replace(' ', "+"))
                                                    };
                                                    *action = Some(BrowserAction::Navigate(target));
                                                }
                                            });
                                    }
                                });
                            });

                        ui.add_space(14.0);

                        // If on a Watch page, render the Video Player Card
                        if let Some(watch) = watch_details {
                            self.render_youtube_watch_player(ui, watch);
                            ui.add_space(16.0);
                        }

                        // Video Cards Grid / Section
                        if !videos.is_empty() {
                            let section_title = if watch_details.is_some() {
                                "Recommended Videos"
                            } else if url.contains("search_query") {
                                "Search Results"
                            } else {
                                "Trending & Featured Videos"
                            };

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(section_title)
                                        .size(17.0)
                                        .strong()
                                        .color(Color32::from_rgb(240, 246, 252)),
                                );
                                ui.label(
                                    RichText::new(format!("({} items)", videos.len()))
                                        .size(12.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                            });

                            ui.add_space(8.0);

                            // Render 2-column or 3-column video cards
                            let cols = if content_width > 750.0 { 2 } else { 1 };
                            let chunk_size = videos.len().div_ceil(cols);

                            ui.columns(cols, |columns| {
                                for (col_idx, col_ui) in columns.iter_mut().enumerate() {
                                    let start = col_idx * chunk_size;
                                    let end = (start + chunk_size).min(videos.len());
                                    for video in &videos[start..end] {
                                        self.render_video_card(col_ui, video, action);
                                        col_ui.add_space(10.0);
                                    }
                                }
                            });
                        }

                        if videos.is_empty() && watch_details.is_none() {
                            ui.add_space(20.0);
                            Frame::NONE
                                .fill(Color32::from_rgb(10, 15, 26))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(31, 111, 235)))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(24, 20))
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new("Explore YouTube on Excalibur")
                                                .size(20.0)
                                                .strong()
                                                .color(Color32::from_rgb(240, 246, 252)),
                                        );
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new("Search for any topic above or choose a category below to browse videos.")
                                                .size(13.0)
                                                .color(Color32::from_rgb(148, 163, 184)),
                                        );
                                        ui.add_space(14.0);
                                        ui.horizontal(|ui| {
                                            let quick_searches = [
                                                "Rust Programming",
                                                "Cybersecurity Research",
                                                "Computer Science",
                                                "Tech News 2026",
                                            ];
                                            for qs in quick_searches {
                                                if ui
                                                    .button(
                                                        RichText::new(qs)
                                                            .size(12.0)
                                                            .color(Color32::from_rgb(56, 189, 248))
                                                            .strong(),
                                                    )
                                                    .clicked()
                                                {
                                                    *action = Some(BrowserAction::Navigate(format!(
                                                        "https://www.youtube.com/results?search_query={}",
                                                        qs.replace(' ', "+")
                                                    )));
                                                }
                                            }
                                        });
                                    });
                                });
                        }
                    });
                    ui.add_space(side_margin);
                });
                ui.add_space(32.0);
            });
    }

    fn render_video_card(
        &self,
        ui: &mut Ui,
        video: &YouTubeVideoItem,
        action: &mut Option<BrowserAction>,
    ) {
        Frame::NONE
            .fill(Color32::from_rgb(10, 15, 26))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 43, 76)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                if !video.thumbnail_url.is_empty() {
                    ui.add(
                        egui::Image::from_uri(&video.thumbnail_url)
                            .corner_radius(6.0)
                            .max_height(140.0),
                    );
                    ui.add_space(6.0);
                }

                let title_text = if video.title.len() > 70 {
                    format!("{}...", &video.title[..67])
                } else {
                    video.title.clone()
                };

                let title_btn = ui.link(
                    RichText::new(title_text)
                        .size(13.5)
                        .strong()
                        .color(Color32::from_rgb(241, 245, 249)),
                );
                if title_btn.clicked() {
                    *action = Some(BrowserAction::Navigate(format!(
                        "https://www.youtube.com/watch?v={}",
                        video.id
                    )));
                }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&video.channel)
                            .size(11.5)
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    );

                    if !video.duration.is_empty() {
                        Frame::NONE
                            .fill(Color32::from_rgb(15, 23, 42))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(51, 65, 85)))
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&video.duration)
                                        .size(10.0)
                                        .color(Color32::from_rgb(148, 163, 184))
                                        .monospace(),
                                );
                            });
                    }
                });

                if !video.views.is_empty() || !video.published.is_empty() {
                    let meta = format!(
                        "{} {}",
                        video.views,
                        if video.published.is_empty() {
                            String::new()
                        } else {
                            format!("- {}", video.published)
                        }
                    );
                    ui.label(
                        RichText::new(meta.trim())
                            .size(11.0)
                            .color(Color32::from_rgb(100, 116, 139)),
                    );
                }

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Watch Video")
                                .size(11.0)
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .clicked()
                    {
                        *action = Some(BrowserAction::Navigate(format!(
                            "https://www.youtube.com/watch?v={}",
                            video.id
                        )));
                    }

                    if ui
                        .small_button(
                            RichText::new("Copy Link")
                                .size(10.5)
                                .color(Color32::from_rgb(125, 211, 252)),
                        )
                        .clicked()
                    {
                        ui.ctx()
                            .copy_text(format!("https://www.youtube.com/watch?v={}", video.id));
                    }
                });
            });
    }

    fn render_youtube_watch_player(&self, ui: &mut Ui, watch: &YouTubeWatchDetails) {
        Frame::NONE
            .fill(Color32::from_rgb(6, 9, 15))
            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(37, 99, 235)))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    Frame::NONE
                        .fill(Color32::from_rgb(220, 38, 38))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("NOW PLAYING")
                                    .size(10.0)
                                    .color(Color32::WHITE)
                                    .strong(),
                            );
                        });

                    ui.label(
                        RichText::new(format!("ID: {}", watch.video_id))
                            .size(11.0)
                            .color(Color32::from_rgb(148, 163, 184))
                            .monospace(),
                    );
                });

                ui.add_space(8.0);

                ui.label(
                    RichText::new(&watch.title)
                        .size(18.0)
                        .strong()
                        .color(Color32::from_rgb(240, 246, 252)),
                );

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&watch.author)
                            .size(13.0)
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    );

                    if !watch.views.is_empty() {
                        ui.label(
                            RichText::new(format!("- {} views", watch.views))
                                .size(12.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new("Watch External")
                                .size(12.0)
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .clicked()
                    {
                        let target_url =
                            format!("https://www.youtube.com/watch?v={}", watch.video_id);
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", &target_url])
                                .spawn();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(&target_url)
                                .spawn();
                        }
                    }

                    if ui
                        .button(
                            RichText::new("Copy Video URL")
                                .size(11.5)
                                .color(Color32::from_rgb(125, 211, 252)),
                        )
                        .clicked()
                    {
                        ui.ctx().copy_text(format!(
                            "https://www.youtube.com/watch?v={}",
                            watch.video_id
                        ));
                    }

                    if ui
                        .button(
                            RichText::new("Copy Embed URL")
                                .size(11.5)
                                .color(Color32::from_rgb(125, 211, 252)),
                        )
                        .clicked()
                    {
                        ui.ctx()
                            .copy_text(format!("https://www.youtube.com/embed/{}", watch.video_id));
                    }
                });

                if !watch.description.is_empty() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Description:")
                            .size(12.0)
                            .color(Color32::from_rgb(148, 163, 184))
                            .strong(),
                    );
                    ui.add_space(4.0);
                    let desc = if watch.description.len() > 300 {
                        format!("{}...", &watch.description[..297])
                    } else {
                        watch.description.clone()
                    };
                    ui.label(
                        RichText::new(desc)
                            .size(11.5)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                }
            });
    }
}

fn render_stage_badge(ui: &mut Ui, label: &str, active: bool) {
    let (bg, border, fg) = if active {
        (
            Color32::from_rgb(11, 29, 58),
            Color32::from_rgb(31, 111, 235),
            Color32::from_rgb(56, 189, 248),
        )
    } else {
        (
            Color32::from_rgb(10, 15, 26),
            Color32::from_rgb(26, 43, 76),
            Color32::from_rgb(100, 116, 139),
        )
    };

    Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(10.0).color(fg).strong());
        });
}

pub fn extract_youtube_videos(html: &str) -> Vec<YouTubeVideoItem> {
    let mut videos = Vec::new();
    let mut search_from = 0;
    while let Some(rel_idx) = html[search_from..].find("ytInitialData") {
        let start_idx = search_from + rel_idx;
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    collect_videos_from_json(&val, &mut videos);
                    if !videos.is_empty() {
                        return videos;
                    }
                }
            }
        }
        search_from = start_idx + 13;
    }
    videos
}

pub fn extract_youtube_watch_details(html: &str) -> Option<YouTubeWatchDetails> {
    let mut search_from = 0;
    while let Some(rel_idx) = html[search_from..].find("ytInitialData") {
        let start_idx = search_from + rel_idx;
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    if let Some(details) = find_video_details_in_json(&val) {
                        return Some(details);
                    }
                }
            }
        }
        search_from = start_idx + 13;
    }

    if let Some(start_idx) = html.find("ytInitialPlayerResponse") {
        let slice = &html[start_idx..];
        if let Some(eq_idx) = slice.find('=') {
            if let Some(brace_idx) = slice[eq_idx..].find('{') {
                let json_slice = &slice[eq_idx + brace_idx..];
                let de = serde_json::Deserializer::from_str(json_slice);
                if let Some(Ok(val)) = de.into_iter::<serde_json::Value>().next() {
                    if let Some(details) = find_video_details_in_json(&val) {
                        return Some(details);
                    }
                }
            }
        }
    }

    None
}

fn collect_videos_from_json(val: &serde_json::Value, videos: &mut Vec<YouTubeVideoItem>) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(vr) = map.get("videoRenderer").and_then(|v| v.as_object()) {
                if let Some(item) = parse_video_renderer(vr) {
                    if !videos.iter().any(|existing| existing.id == item.id) {
                        videos.push(item);
                    }
                }
            } else if let Some(cvr) = map.get("compactVideoRenderer").and_then(|v| v.as_object()) {
                if let Some(item) = parse_video_renderer(cvr) {
                    if !videos.iter().any(|existing| existing.id == item.id) {
                        videos.push(item);
                    }
                }
            }
            for child in map.values() {
                collect_videos_from_json(child, videos);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                collect_videos_from_json(child, videos);
            }
        }
        _ => {}
    }
}

fn parse_video_renderer(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<YouTubeVideoItem> {
    let id = map.get("videoId")?.as_str()?.to_string();
    if id.is_empty() {
        return None;
    }

    let title = map
        .get("title")
        .and_then(|t| {
            t.get("runs")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|txt| txt.as_str())
                .or_else(|| t.get("simpleText").and_then(|st| st.as_str()))
        })
        .unwrap_or("Untitled Video")
        .to_string();

    let channel = map
        .get("ownerText")
        .or_else(|| map.get("longBylineText"))
        .or_else(|| map.get("shortBylineText"))
        .and_then(|o| {
            o.get("runs")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|txt| txt.as_str())
                .or_else(|| o.get("simpleText").and_then(|st| st.as_str()))
        })
        .unwrap_or("YouTube Channel")
        .to_string();

    let views = map
        .get("viewCountText")
        .or_else(|| map.get("shortViewCountText"))
        .and_then(|v| {
            v.get("simpleText").and_then(|st| st.as_str()).or_else(|| {
                v.get("runs")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|f| f.get("text"))
                    .and_then(|t| t.as_str())
            })
        })
        .unwrap_or("")
        .to_string();

    let duration = map
        .get("lengthText")
        .and_then(|l| l.get("simpleText").and_then(|st| st.as_str()))
        .unwrap_or("")
        .to_string();

    let published = map
        .get("publishedTimeText")
        .and_then(|p| p.get("simpleText").and_then(|st| st.as_str()))
        .unwrap_or("")
        .to_string();

    let thumbnail_url = map
        .get("thumbnail")
        .and_then(|th| th.get("thumbnails"))
        .and_then(|arr| arr.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    Some(YouTubeVideoItem {
        id,
        title,
        channel,
        views,
        duration,
        published,
        thumbnail_url,
    })
}

fn find_video_details_in_json(val: &serde_json::Value) -> Option<YouTubeWatchDetails> {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(vd) = map.get("videoDetails").and_then(|v| v.as_object()) {
                let video_id = vd
                    .get("videoId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = vd
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YouTube Video")
                    .to_string();
                let author = vd
                    .get("author")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YouTube Channel")
                    .to_string();
                let views = vd
                    .get("viewCount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = vd
                    .get("shortDescription")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !video_id.is_empty() {
                    return Some(YouTubeWatchDetails {
                        video_id,
                        title,
                        author,
                        views,
                        description,
                    });
                }
            }

            for child in map.values() {
                if let Some(res) = find_video_details_in_json(child) {
                    return Some(res);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                if let Some(res) = find_video_details_in_json(child) {
                    return Some(res);
                }
            }
        }
        _ => {}
    }
    None
}

fn format_unix_timestamp(ts: i64) -> String {
    if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        ts.to_string()
    }
}
