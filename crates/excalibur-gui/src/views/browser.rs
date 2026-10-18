use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui};
use excalibur_dom::{DomNode, NodeType};
use std::collections::HashMap;

/// An action requested by the interactive browser view to the parent app.
pub enum BrowserAction {
    Navigate(String),
}

/// Visual browser viewport rendering DOM nodes into interactive GUI elements.
#[derive(Default)]
pub struct BrowserRenderer {
    pub input_states: HashMap<String, String>,
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
                            Frame::NONE
                                .fill(Color32::from_rgb(18, 22, 28))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                                .inner_margin(Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("[ DEV WEB VIEW ]")
                                                .size(13.0)
                                                .strong()
                                                .color(Color32::from_rgb(88, 166, 255)),
                                        );
                                        ui.label(
                                            RichText::new("Pure-Rust Semantic DOM & AST Engine (Non-Chromium)")
                                                .size(12.0)
                                                .color(Color32::from_rgb(139, 148, 158)),
                                        );
                                    });
                                });
                            ui.add_space(10.0);
                            self.render_node(ui, root, &mut action, url);
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
                .show(ui, |ui| {
                    ui.label(RichText::new(err).color(Color32::from_rgb(248, 81, 73)));
                });

            ui.add_space(16.0);
            if ui.button("Try Again").clicked() {
                *action = Some(BrowserAction::Navigate(url.to_string()));
            }
        });
    }

    fn render_speed_dial(&self, ui: &mut Ui, action: &mut Option<BrowserAction>) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("Excalibur Browser")
                        .size(32.0)
                        .strong()
                        .color(Color32::from_rgb(88, 166, 255)),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Independent non-Chromium architecture with built-in penetration testing & telemetry")
                        .size(14.0)
                        .color(Color32::from_rgb(139, 148, 158)),
                );
                ui.add_space(32.0);

                ui.heading("Quick Access Targets");
                ui.add_space(12.0);

                let shortcuts = [
                    (
                        "Example Domain",
                        "https://example.com",
                        "Clean standard IANA reference page",
                    ),
                    (
                        "Hacker News",
                        "https://news.ycombinator.com",
                        "Tech news aggregator with dense HTML tables",
                    ),
                    (
                        "OWASP Foundation",
                        "https://owasp.org",
                        "Worldwide non-profit dedicated to web application security",
                    ),
                    (
                        "HTTPBin HTML Sandbox",
                        "https://httpbin.org/html",
                        "Test HTML document with moby-dick passage",
                    ),
                    (
                        "GitHub",
                        "https://github.com",
                        "Developer platform and code hosting",
                    ),
                ];

                for (name, url, desc) in shortcuts {
                    let frame = Frame::NONE
                        .fill(Color32::from_rgb(22, 27, 34))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                        .inner_margin(Margin::same(12));

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(name)
                                        .size(16.0)
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
                                    if ui.button("Open Target").clicked() {
                                        *action = Some(BrowserAction::Navigate(url.to_string()));
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn render_node(
        &mut self,
        ui: &mut Ui,
        node: &DomNode,
        action: &mut Option<BrowserAction>,
        current_url: &str,
    ) {
        match &node.node_type {
            NodeType::Document => {
                for child in &node.children {
                    self.render_node(ui, child, action, current_url);
                }
            }
            NodeType::Element { tag, attributes } => {
                let tag_lower = tag.to_ascii_lowercase();

                // Skip non-rendering head metadata
                if tag_lower == "head"
                    || tag_lower == "style"
                    || tag_lower == "script"
                    || tag_lower == "meta"
                    || tag_lower == "link"
                {
                    return;
                }

                match tag_lower.as_str() {
                    "h1" => {
                        let text = node.text_content();
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(text.trim())
                                .size(24.0)
                                .strong()
                                .color(Color32::from_rgb(88, 166, 255)),
                        );
                        ui.add_space(6.0);
                    }
                    "h2" => {
                        let text = node.text_content();
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(text.trim())
                                .size(20.0)
                                .strong()
                                .color(Color32::from_rgb(121, 192, 255)),
                        );
                        ui.add_space(4.0);
                    }
                    "h3" => {
                        let text = node.text_content();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(text.trim())
                                .size(16.0)
                                .strong()
                                .color(Color32::from_rgb(201, 209, 217)),
                        );
                        ui.add_space(2.0);
                    }
                    "p" => {
                        let text = node.text_content();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            ui.label(RichText::new(trimmed).size(14.0));
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

                        let link_label = RichText::new(display_text)
                            .color(Color32::from_rgb(88, 166, 255))
                            .underline();
                        if ui.link(link_label).clicked() && !href.is_empty() {
                            let target_url = self.resolve_url(current_url, href);
                            *action = Some(BrowserAction::Navigate(target_url));
                        }
                    }
                    "button" => {
                        let text = node.text_content();
                        let btn_text = if text.trim().is_empty() {
                            "Submit"
                        } else {
                            text.trim()
                        };
                        if ui.button(btn_text).clicked() {
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
                            ui.add_space(4.0);
                            ui.add(
                                egui::Image::from_uri(&resolved)
                                    .max_width(ui.available_width().min(600.0))
                                    .corner_radius(4.0),
                            );
                            ui.add_space(4.0);
                        }
                    }
                    "form" => {
                        Frame::NONE
                            .fill(Color32::from_rgb(22, 27, 34))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    for child in &node.children {
                                        self.render_node(ui, child, action, current_url);
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
                        Frame::NONE
                            .fill(Color32::from_rgb(13, 17, 23))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.code(code_text.trim());
                            });
                    }
                    "blockquote" => {
                        let text = node.text_content();
                        Frame::NONE
                            .fill(Color32::from_rgb(22, 27, 34))
                            .stroke(Stroke::new(2.0_f32, Color32::from_rgb(88, 166, 255)))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.label(RichText::new(text.trim()).italics());
                            });
                    }
                    "ul" | "ol" => {
                        ui.vertical(|ui| {
                            for child in &node.children {
                                if child.tag() == Some("li") {
                                    let item_text = child.text_content();
                                    ui.horizontal(|ui| {
                                        ui.label("-");
                                        ui.label(item_text.trim());
                                    });
                                }
                            }
                        });
                    }
                    "table" => {
                        Frame::NONE
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61)))
                            .inner_margin(Margin::same(6))
                            .show(ui, |ui| {
                                for child in &node.children {
                                    self.render_node(ui, child, action, current_url);
                                }
                            });
                    }
                    "tr" => {
                        ui.horizontal(|ui| {
                            for child in &node.children {
                                let cell_tag = child.tag().unwrap_or("");
                                if cell_tag == "td" || cell_tag == "th" {
                                    let is_header = cell_tag == "th";
                                    let text = child.text_content();
                                    let rich = if is_header {
                                        RichText::new(text.trim()).strong().color(Color32::WHITE)
                                    } else {
                                        RichText::new(text.trim())
                                    };
                                    ui.label(rich);
                                    ui.separator();
                                }
                            }
                        });
                    }
                    _ => {
                        // Standard container (div, span, section, body, main, etc.)
                        for child in &node.children {
                            self.render_node(ui, child, action, current_url);
                        }
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
