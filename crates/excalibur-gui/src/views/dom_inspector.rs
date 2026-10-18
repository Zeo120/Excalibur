use eframe::egui::{Color32, RichText, ScrollArea, Ui};
use excalibur_dom::{DomNode, NodeType};
use excalibur_script::JsRuntime;

#[derive(Default, PartialEq, Eq)]
pub enum DomSubTab {
    #[default]
    Tree,
    RawHtml,
    HexDump,
    JsConsole,
}

pub struct DomInspectorState {
    pub sub_tab: DomSubTab,
    pub js_input: String,
    pub js_output: String,
    pub js_runtime: Option<JsRuntime>,
}

impl Default for DomInspectorState {
    fn default() -> Self {
        Self {
            sub_tab: DomSubTab::Tree,
            js_input: "console.log(document.title); document.location.href".to_string(),
            js_output: "Ready for JavaScript evaluation.".to_string(),
            js_runtime: JsRuntime::new().ok(),
        }
    }
}

pub fn render_dom_inspector(
    ui: &mut Ui,
    state: &mut DomInspectorState,
    dom: Option<&DomNode>,
    raw_html: &str,
) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.sub_tab, DomSubTab::Tree, "Live DOM Tree");
        ui.selectable_value(&mut state.sub_tab, DomSubTab::RawHtml, "Raw HTML");
        ui.selectable_value(&mut state.sub_tab, DomSubTab::HexDump, "Hex Dump");
        ui.selectable_value(
            &mut state.sub_tab,
            DomSubTab::JsConsole,
            "QuickJS Console & Sinks",
        );
    });

    ui.separator();

    match state.sub_tab {
        DomSubTab::Tree => render_tree_tab(ui, dom),
        DomSubTab::RawHtml => render_html_tab(ui, raw_html),
        DomSubTab::HexDump => render_hex_tab(ui, raw_html),
        DomSubTab::JsConsole => render_js_console_tab(ui, state),
    }
}

fn render_tree_tab(ui: &mut Ui, dom: Option<&DomNode>) {
    ScrollArea::both().show(ui, |ui| {
        if let Some(root) = dom {
            render_node_collapsible(ui, root, 0);
        } else {
            ui.label("No DOM loaded.");
        }
    });
}

fn render_node_collapsible(ui: &mut Ui, node: &DomNode, depth: usize) {
    let indent = "  ".repeat(depth);
    match &node.node_type {
        NodeType::Document => {
            ui.label(format!("{}#document", indent));
            for child in &node.children {
                render_node_collapsible(ui, child, depth + 1);
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
                render_node_collapsible(ui, child, depth + 1);
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

fn render_html_tab(ui: &mut Ui, raw_html: &str) {
    ScrollArea::both().show(ui, |ui| {
        ui.code(raw_html);
    });
}

fn render_hex_tab(ui: &mut Ui, raw_html: &str) {
    ScrollArea::both().show(ui, |ui| {
        let bytes = raw_html.as_bytes();
        let limit = bytes.len().min(1024);
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

        if bytes.len() > 1024 {
            hex_lines.push_str(&format!(
                "\n[... truncated {} additional bytes ...]",
                bytes.len() - 1024
            ));
        }

        ui.code(hex_lines);
    });
}

fn render_js_console_tab(ui: &mut Ui, state: &mut DomInspectorState) {
    ui.heading("QuickJS Embedded Execution Console");
    ui.label("Execute JavaScript commands within the active page context and trace XSS sinks.");

    ui.horizontal(|ui| {
        ui.label("JS Input:");
        ui.text_edit_singleline(&mut state.js_input);
        if ui.button("Execute (eval)").clicked() {
            if let Some(js) = &state.js_runtime {
                match js.eval(&state.js_input) {
                    Ok(res) => state.js_output = format!("Result => {}", res),
                    Err(e) => state.js_output = format!("Error => {}", e),
                }
            } else {
                state.js_output = "QuickJS runtime not available".to_string();
            }
        }
    });

    ui.add_space(8.0);
    ui.label(RichText::new("Execution Output:").strong());
    ui.code(&state.js_output);

    ui.separator();
    ui.heading("Intercepted DOM XSS Sinks in Document");
    if let Some(js) = &state.js_runtime {
        let findings = js.get_sink_findings();
        if findings.is_empty() {
            ui.label(
                RichText::new("No dangerous sinks (eval, innerHTML, document.write) called.")
                    .color(Color32::from_rgb(63, 185, 80)),
            );
        } else {
            for f in findings {
                ui.label(
                    RichText::new(format!("[SINK] {}: {}", f.sink_name, f.argument))
                        .color(Color32::from_rgb(248, 81, 73)),
                );
            }
        }
    }
}
