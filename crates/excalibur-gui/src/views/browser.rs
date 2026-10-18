use eframe::egui::{Color32, RichText, ScrollArea, Ui};
use excalibur_dom::{DomNode, NodeType};

#[derive(Default, PartialEq, Eq)]
pub enum BrowserSubView {
    #[default]
    VisualCanvas,
    LiveDomTree,
    RawSourceAndHex,
}

pub struct BrowserViewState {
    pub sub_view: BrowserSubView,
    pub raw_html: String,
    pub parsed_dom: Option<DomNode>,
    pub test_input_val: String,
}

impl Default for BrowserViewState {
    fn default() -> Self {
        let sample_html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Excalibur Security Workbench</title>
</head>
<body>
    <h1>Application Security Portal</h1>
    <p>Target running on internal enterprise network. Security telemetry active.</p>
    <a href="https://example.com/api/v1/users">Browse User API Endpoints</a>
    <hr/>
    <form action="/login" method="POST">
        <label>Username: </label>
        <input name="username" value="admin" />
        <br/>
        <label>Password: </label>
        <input name="password" type="password" value="" />
        <br/>
        <button type="submit">Authenticate</button>
    </form>
</body>
</html>"#;

        let dom = Some(excalibur_dom::HtmlParser::parse(sample_html));

        Self {
            sub_view: BrowserSubView::VisualCanvas,
            raw_html: sample_html.to_string(),
            parsed_dom: dom,
            test_input_val: "admin".to_string(),
        }
    }
}

pub fn render_browser_view(ui: &mut Ui, state: &mut BrowserViewState, current_url: &str) {
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut state.sub_view,
            BrowserSubView::VisualCanvas,
            "Visual Viewport",
        );
        ui.selectable_value(
            &mut state.sub_view,
            BrowserSubView::LiveDomTree,
            "Live DOM Tree",
        );
        ui.selectable_value(
            &mut state.sub_view,
            BrowserSubView::RawSourceAndHex,
            "Raw Source & Hex Dump",
        );
        ui.separator();
        ui.label(
            RichText::new(format!("Location: {}", current_url))
                .color(Color32::from_rgb(88, 166, 255)),
        );
    });

    ui.separator();

    match state.sub_view {
        BrowserSubView::VisualCanvas => render_visual_canvas(ui, state),
        BrowserSubView::LiveDomTree => render_dom_tree(ui, state),
        BrowserSubView::RawSourceAndHex => render_raw_and_hex(ui, state),
    }
}

fn render_visual_canvas(ui: &mut Ui, state: &mut BrowserViewState) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.label(
                RichText::new("Application Security Portal")
                    .size(22.0)
                    .strong()
                    .color(Color32::from_rgb(88, 166, 255)),
            );
            ui.add_space(8.0);
            ui.label("Target running on internal enterprise network. Security telemetry active.");
            ui.add_space(8.0);

            if ui
                .link("Browse User API Endpoints (https://example.com/api/v1/users)")
                .clicked()
            {
                tracing::info!("Link clicked in visual canvas");
            }

            ui.separator();
            ui.heading("Authentication Form");
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.text_edit_singleline(&mut state.test_input_val);
            });
            ui.horizontal(|ui| {
                ui.label("Password:");
                let mut pass = "********".to_string();
                ui.text_edit_singleline(&mut pass);
            });
            if ui.button("Authenticate").clicked() {
                tracing::info!("Form submission button clicked");
            }
        });
    });
}

fn render_dom_tree(ui: &mut Ui, state: &BrowserViewState) {
    ScrollArea::both().show(ui, |ui| {
        if let Some(dom) = &state.parsed_dom {
            render_node(ui, dom, 0);
        } else {
            ui.label("No DOM tree parsed.");
        }
    });
}

fn render_node(ui: &mut Ui, node: &DomNode, depth: usize) {
    let indent = "  ".repeat(depth);
    match &node.node_type {
        NodeType::Document => {
            ui.label(format!("{}#document", indent));
            for child in &node.children {
                render_node(ui, child, depth + 1);
            }
        }
        NodeType::Element { tag, attributes } => {
            let mut attr_str = String::new();
            for (k, v) in attributes {
                attr_str.push_str(&format!(" {}=\"{}\"", k, v));
            }
            ui.horizontal(|ui| {
                ui.label(format!("{}<{}", indent, tag));
                if !attr_str.is_empty() {
                    ui.label(RichText::new(&attr_str).color(Color32::from_rgb(209, 154, 102)));
                }
                ui.label(">");
            });
            for child in &node.children {
                render_node(ui, child, depth + 1);
            }
        }
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                ui.label(
                    RichText::new(format!("{}\"{}\"", indent, trimmed))
                        .color(Color32::from_rgb(152, 195, 121)),
                );
            }
        }
        NodeType::Comment(comment) => {
            ui.label(RichText::new(format!("{}<!-- {} -->", indent, comment)).color(Color32::GRAY));
        }
    }
}

fn render_raw_and_hex(ui: &mut Ui, state: &BrowserViewState) {
    ScrollArea::both().show(ui, |ui| {
        ui.heading("Raw HTML Source");
        ui.code(&state.raw_html);

        ui.add_space(12.0);
        ui.heading("Hex View (First 256 bytes)");

        let bytes = state.raw_html.as_bytes();
        let limit = bytes.len().min(256);
        let mut hex_lines = String::new();

        for chunk_idx in (0..limit).step_by(16) {
            let end = (chunk_idx + 16).min(limit);
            let slice = &bytes[chunk_idx..end];

            // Offset
            let mut line = format!("{:08x}  ", chunk_idx);

            // Hex
            for b in slice {
                line.push_str(&format!("{:02x} ", b));
            }
            for _ in slice.len()..16 {
                line.push_str("   ");
            }

            line.push_str(" |");
            // ASCII
            for b in slice {
                if b.is_ascii_graphic() || *b == b' ' {
                    line.push(*b as char);
                } else {
                    line.push('.');
                }
            }
            line.push_str("|\n");
            hex_lines.push_str(&line);
        }

        ui.code(hex_lines);
    });
}
