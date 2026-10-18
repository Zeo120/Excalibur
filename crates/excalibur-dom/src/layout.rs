use crate::tree::{DomNode, NodeType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutBlockType {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
    },
    Button {
        label: String,
    },
    TextInput {
        name: String,
        value: String,
        is_password: bool,
    },
    Link {
        text: String,
        href: String,
    },
    Image {
        src: String,
        alt: String,
    },
    ListItem {
        text: String,
    },
    Container,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutBlock {
    pub node_id: usize,
    pub block_type: LayoutBlockType,
    pub children: Vec<LayoutBlock>,
}

/// Layout Engine translating DOM nodes into visual blocks for GUI canvas and TUI text flow.
pub struct LayoutEngine;

impl LayoutEngine {
    /// Translates a DomNode tree into a hierarchical LayoutBlock tree.
    pub fn build_layout(node: &DomNode) -> LayoutBlock {
        let block_type = match &node.node_type {
            NodeType::Element { tag, attributes } => {
                let tag_lower = tag.to_ascii_lowercase();
                match tag_lower.as_str() {
                    "h1" => LayoutBlockType::Heading {
                        level: 1,
                        text: node.text_content().trim().to_string(),
                    },
                    "h2" => LayoutBlockType::Heading {
                        level: 2,
                        text: node.text_content().trim().to_string(),
                    },
                    "h3" => LayoutBlockType::Heading {
                        level: 3,
                        text: node.text_content().trim().to_string(),
                    },
                    "h4" => LayoutBlockType::Heading {
                        level: 4,
                        text: node.text_content().trim().to_string(),
                    },
                    "h5" => LayoutBlockType::Heading {
                        level: 5,
                        text: node.text_content().trim().to_string(),
                    },
                    "h6" => LayoutBlockType::Heading {
                        level: 6,
                        text: node.text_content().trim().to_string(),
                    },
                    "p" => LayoutBlockType::Paragraph {
                        text: node.text_content().trim().to_string(),
                    },
                    "button" => LayoutBlockType::Button {
                        label: node.text_content().trim().to_string(),
                    },
                    "input" => {
                        let inp_type = attributes
                            .get("type")
                            .map(|s| s.to_ascii_lowercase())
                            .unwrap_or_else(|| "text".to_string());
                        let name = attributes.get("name").cloned().unwrap_or_default();
                        let value = attributes.get("value").cloned().unwrap_or_default();

                        if inp_type == "submit" || inp_type == "button" {
                            LayoutBlockType::Button {
                                label: if value.is_empty() {
                                    "Submit".to_string()
                                } else {
                                    value
                                },
                            }
                        } else {
                            LayoutBlockType::TextInput {
                                name,
                                value,
                                is_password: inp_type == "password",
                            }
                        }
                    }
                    "a" => {
                        let href = attributes.get("href").cloned().unwrap_or_default();
                        LayoutBlockType::Link {
                            text: node.text_content().trim().to_string(),
                            href,
                        }
                    }
                    "img" => {
                        let src = attributes.get("src").cloned().unwrap_or_default();
                        let alt = attributes.get("alt").cloned().unwrap_or_default();
                        LayoutBlockType::Image { src, alt }
                    }
                    "li" => LayoutBlockType::ListItem {
                        text: node.text_content().trim().to_string(),
                    },
                    _ => LayoutBlockType::Container,
                }
            }
            NodeType::Text(t) => LayoutBlockType::Paragraph {
                text: t.trim().to_string(),
            },
            _ => LayoutBlockType::Container,
        };

        let mut children = Vec::new();
        // Avoid redundant children for leaf blocks that already ingested their inner text
        match &block_type {
            LayoutBlockType::Heading { .. }
            | LayoutBlockType::Paragraph { .. }
            | LayoutBlockType::Button { .. }
            | LayoutBlockType::TextInput { .. }
            | LayoutBlockType::Link { .. }
            | LayoutBlockType::Image { .. }
            | LayoutBlockType::ListItem { .. } => {}
            _ => {
                for child in &node.children {
                    children.push(Self::build_layout(child));
                }
            }
        }

        LayoutBlock {
            node_id: node.id,
            block_type,
            children,
        }
    }

    /// Formats the layout tree into formatted terminal text representation for TUI mode.
    pub fn render_to_plain_text(node: &DomNode) -> String {
        let mut out = String::new();
        Self::format_node_to_text(node, &mut out);
        out
    }

    fn format_node_to_text(node: &DomNode, out: &mut String) {
        match &node.node_type {
            NodeType::Element { tag, attributes } => {
                let tag_lower = tag.to_ascii_lowercase();
                match tag_lower.as_str() {
                    "h1" => {
                        out.push_str("\n\n# ");
                        out.push_str(node.text_content().trim());
                        out.push('\n');
                    }
                    "h2" => {
                        out.push_str("\n\n## ");
                        out.push_str(node.text_content().trim());
                        out.push('\n');
                    }
                    "h3" => {
                        out.push_str("\n\n### ");
                        out.push_str(node.text_content().trim());
                        out.push('\n');
                    }
                    "p" => {
                        let text = node.text_content().trim().to_string();
                        if !text.is_empty() {
                            out.push('\n');
                            out.push_str(&text);
                            out.push('\n');
                        }
                    }
                    "a" => {
                        let href = attributes.get("href").map(|s| s.as_str()).unwrap_or("");
                        let text = node.text_content().trim().to_string();
                        out.push_str(&format!(" [{} ({})] ", text, href));
                    }
                    "button" => {
                        out.push_str(&format!(" [Btn: {}] ", node.text_content().trim()));
                    }
                    "input" => {
                        let name = attributes.get("name").map(|s| s.as_str()).unwrap_or("");
                        let val = attributes.get("value").map(|s| s.as_str()).unwrap_or("");
                        out.push_str(&format!(" [Input: {} = \"{}\"] ", name, val));
                    }
                    "li" => {
                        out.push_str("\n  * ");
                        out.push_str(node.text_content().trim());
                    }
                    "script" | "style" | "head" => {
                        // Suppress script and style blocks from plain text flow
                    }
                    _ => {
                        for child in &node.children {
                            Self::format_node_to_text(child, out);
                        }
                    }
                }
            }
            NodeType::Text(t) => {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    out.push(' ');
                }
            }
            _ => {
                for child in &node.children {
                    Self::format_node_to_text(child, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::HtmlParser;

    #[test]
    fn test_layout_block_construction_and_text_rendering() {
        let html = r#"
            <html>
                <body>
                    <h1>Excalibur Security</h1>
                    <p>Advanced non-Chromium browser.</p>
                    <a href="https://example.com/target">Explore Target</a>
                    <button>Scan Port</button>
                    <input type="text" name="param" value="test">
                </body>
            </html>
        "#;

        let dom = HtmlParser::parse(html);
        let layout = LayoutEngine::build_layout(&dom);
        assert_eq!(layout.block_type, LayoutBlockType::Container);

        let plain_text = LayoutEngine::render_to_plain_text(&dom);
        assert!(plain_text.contains("# Excalibur Security"));
        assert!(plain_text.contains("Advanced non-Chromium browser."));
        assert!(plain_text.contains("[Explore Target (https://example.com/target)]"));
        assert!(plain_text.contains("[Btn: Scan Port]"));
        assert!(plain_text.contains("[Input: param = \"test\"]"));
    }
}
