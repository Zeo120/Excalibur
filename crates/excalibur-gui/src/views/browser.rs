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

/// Visual browser viewport rendering DOM nodes into interactive GUI elements.
#[derive(Default)]
pub struct BrowserRenderer {
    pub input_states: HashMap<String, String>,
    pub speed_dial_input: String,
}

impl BrowserRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders the current browser tab contents.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        url: &str,
        dom: Option<&DomNode>,
        is_loading: bool,
        error_msg: Option<&str>,
    ) -> Option<BrowserAction> {
        let mut action = None;

        if is_loading {
            self.render_loading(ui, url);
            return None;
        }

        if let Some(err) = error_msg {
            self.render_error(ui, url, err, &mut action);
            return action;
        }

        if url == "about:blank" || url == "excalibur://newtab" || dom.is_none() {
            self.render_speed_dial(ui, &mut action);
            return action;
        }

        if let Some(root) = dom {
            let stylesheet = StyleSheet::extract_from_dom(root);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(12.0);
                    let available_width = ui.available_width();
                    let content_width = available_width.min(940.0);
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
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.spinner();
            ui.add_space(16.0);
            ui.label(
                RichText::new("Loading Webpage...")
                    .size(18.0)
                    .strong()
                    .color(Color32::from_rgb(88, 166, 255)),
            );
            ui.label(
                RichText::new(format!("Connecting to {}", url))
                    .color(Color32::from_rgb(139, 148, 158)),
            );
            ui.add_space(8.0);
            ui.label("Performing TLS 1.3 handshake and parsing HTML5 layout tree...");
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
                                .color(Color32::from_rgb(88, 166, 255)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Independent Security Browser & Penetration Testing Workbench",
                            )
                            .size(14.0)
                            .color(Color32::from_rgb(139, 148, 158)),
                        );
                        ui.add_space(12.0);

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                            let badges = [
                                ("[ NON-CHROMIUM ]", Color32::from_rgb(88, 166, 255)),
                                ("[ PURE-RUST DOM ]", Color32::from_rgb(63, 185, 80)),
                                ("[ ZERO VENDOR BLINK ]", Color32::from_rgb(210, 153, 34)),
                                (
                                    "[ MITM PROXY 127.0.0.1:8080 ]",
                                    Color32::from_rgb(248, 81, 73),
                                ),
                            ];

                            for (badge, color) in badges {
                                Frame::NONE
                                    .fill(Color32::from_rgb(22, 27, 34))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(Margin::symmetric(8, 4))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(badge).size(11.0).strong().color(color),
                                        );
                                    });
                            }
                        });
                    });

                    ui.add_space(20.0);

                    // 2. Central Quick Launch Search Bar
                    Frame::NONE
                        .fill(Color32::from_rgb(18, 22, 28))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("URI / Search:")
                                        .size(12.0)
                                        .color(Color32::from_rgb(88, 166, 255))
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
                            Color32::from_rgb(248, 81, 73),
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
                            Color32::from_rgb(88, 166, 255),
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
                            Color32::from_rgb(63, 185, 80),
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
                                .fill(Color32::from_rgb(22, 27, 34))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
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
                                                    .color(Color32::from_rgb(139, 148, 158)),
                                            );
                                            ui.label(
                                                RichText::new(url)
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(88, 166, 255)),
                                            );
                                        });

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("Launch").clicked() {
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
                        .fill(Color32::from_rgb(16, 20, 26))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("ENGINE TELEMETRY")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(88, 166, 255)),
                                    );
                                    ui.label("Hyper + Rustls 0.23 (TLS 1.3)");
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("MITM PROXY")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(63, 185, 80)),
                                    );
                                    ui.label("127.0.0.1:8080 (Listening)");
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("DOM PARSER")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(210, 153, 34)),
                                    );
                                    ui.label("Pure-Rust Semantic AST");
                                });
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("SECURITY AUDIT")
                                            .size(11.0)
                                            .strong()
                                            .color(Color32::from_rgb(248, 81, 73)),
                                    );
                                    ui.label("QuickJS Sinks + Passive Radar");
                                });
                            });
                        });

                    ui.add_space(32.0);
                });
                ui.add_space(side_margin);
            });
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
                                ui.label(format!("{}:", input_name));
                                ui.add(egui::TextEdit::singleline(current_val).password(true));
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if input_name != "field" && input_name != "q" {
                                    ui.label(format!("{}:", input_name));
                                }
                                let te = ui.add(
                                    egui::TextEdit::singleline(current_val)
                                        .hint_text("Search or enter query...")
                                        .desired_width(ui.available_width().min(450.0)),
                                );
                                if (input_name == "q" || input_type == "search")
                                    && (ui.button("Search").clicked()
                                        || (te.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                                    && !current_val.is_empty()
                                {
                                    let search_target = format!(
                                        "https://www.google.com/search?q={}",
                                        current_val.replace(' ', "+")
                                    );
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
                    ui.label(trimmed);
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
}
