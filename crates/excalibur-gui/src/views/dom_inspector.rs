use crate::icons::Icon;
use crate::theme::colors;
use crate::widgets::{code_block, empty_state, overline, primary_button, segmented, text_field};
use eframe::egui::{self, text::LayoutJob, Color32, FontId, RichText, ScrollArea, TextFormat, Ui};
use excalibur_dom::{DomNode, NodeType};
use excalibur_script::JsRuntime;

#[derive(Default, PartialEq, Eq, Clone)]
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
    segmented(
        ui,
        &mut state.sub_tab,
        &[
            (DomSubTab::Tree, "Elements"),
            (DomSubTab::RawHtml, "Source"),
            (DomSubTab::HexDump, "Hex"),
            (DomSubTab::JsConsole, "Console"),
        ],
    );
    ui.add_space(10.0);

    match state.sub_tab {
        DomSubTab::Tree => render_tree_tab(ui, dom),
        DomSubTab::RawHtml => render_html_tab(ui, raw_html),
        DomSubTab::HexDump => render_hex_tab(ui, raw_html),
        DomSubTab::JsConsole => render_js_console_tab(ui, state),
    }
}

/// Syntax colors for the element tree, derived from the active palette.
struct TreeColors {
    tag: Color32,
    attr: Color32,
    text: Color32,
    comment: Color32,
    punct: Color32,
}

fn render_tree_tab(ui: &mut Ui, dom: Option<&DomNode>) {
    let Some(root) = dom else {
        empty_state(
            ui,
            Icon::Code,
            "No document loaded",
            "Open a web page to inspect its DOM.",
        );
        return;
    };
    let c = TreeColors {
        tag: colors::link(),
        attr: colors::warning(),
        text: colors::success(),
        comment: colors::text_faint(),
        punct: colors::text_faint(),
    };
    ScrollArea::both()
        .id_salt("dom_tree_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            render_node_line(ui, root, 0, &c);
        });
}

fn render_node_line(ui: &mut Ui, node: &DomNode, depth: usize, c: &TreeColors) {
    let font = FontId::monospace(12.5);
    let fmt = |color: Color32| TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    let indent = "  ".repeat(depth);
    let mut job = LayoutJob::default();
    match &node.node_type {
        NodeType::Document => {
            job.append(&format!("{}#document", indent), 0.0, fmt(c.comment));
        }
        NodeType::Element { tag, attributes } => {
            job.append(&format!("{}<", indent), 0.0, fmt(c.punct));
            job.append(tag, 0.0, fmt(c.tag));
            for (k, v) in attributes {
                job.append(&format!(" {}", k), 0.0, fmt(c.attr));
                job.append("=", 0.0, fmt(c.punct));
                job.append(&format!("\"{}\"", v), 0.0, fmt(c.text));
            }
            job.append(">", 0.0, fmt(c.punct));
        }
        NodeType::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            job.append(&format!("{}{}", indent, trimmed), 0.0, fmt(colors::text()));
        }
        NodeType::Comment(comment) => {
            job.append(
                &format!("{}<!-- {} -->", indent, comment),
                0.0,
                fmt(c.comment),
            );
        }
    }
    ui.label(job);
    for child in &node.children {
        render_node_line(ui, child, depth + 1, c);
    }
}

fn render_html_tab(ui: &mut Ui, raw_html: &str) {
    if raw_html.is_empty() {
        empty_state(
            ui,
            Icon::Code,
            "No source",
            "Open a web page to view its HTML.",
        );
        return;
    }
    ScrollArea::both()
        .id_salt("dom_source_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| code_block(ui, raw_html));
}

fn render_hex_tab(ui: &mut Ui, raw_html: &str) {
    let bytes = raw_html.as_bytes();
    let limit = bytes.len().min(1024);
    let mut hex_lines = String::new();

    for chunk_idx in (0..limit).step_by(16) {
        let end = (chunk_idx + 16).min(limit);
        let slice = &bytes[chunk_idx..end];
        let mut line = format!("{:08x}  ", chunk_idx);
        for b in slice {
            line.push_str(&format!("{:02x} ", b));
        }
        for _ in slice.len()..16 {
            line.push_str("   ");
        }
        line.push_str(" |");
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

    ScrollArea::both()
        .id_salt("dom_hex_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| code_block(ui, &hex_lines));
}

fn render_js_console_tab(ui: &mut Ui, state: &mut DomInspectorState) {
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 90.0).max(160.0);
        let resp = text_field(ui, &mut state.js_input, "Evaluate JavaScript", w);
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if primary_button(ui, "Run").clicked() || enter {
            state.js_output = match &state.js_runtime {
                Some(js) => match js.eval(&state.js_input) {
                    Ok(res) => format!("← {}", res),
                    Err(e) => format!("✕ {}", e),
                },
                None => "QuickJS runtime not available".to_string(),
            };
        }
    });
    ui.add_space(8.0);
    code_block(ui, &state.js_output);

    ui.add_space(14.0);
    overline(ui, "DOM XSS sinks");
    ui.add_space(4.0);
    if let Some(js) = &state.js_runtime {
        let findings = js.get_sink_findings();
        if findings.is_empty() {
            ui.label(
                RichText::new("No dangerous sinks (eval, innerHTML, document.write) were called.")
                    .size(12.5)
                    .color(colors::success()),
            );
        } else {
            for f in findings {
                ui.label(
                    RichText::new(format!("{}  {}", f.sink_name, f.argument))
                        .monospace()
                        .size(12.5)
                        .color(colors::danger()),
                );
            }
        }
    }
}
