use crate::tree::{DomNode, NodeType};
use kuchiki::traits::TendrilSink;
use kuchiki::NodeData;
use std::collections::HashMap;

/// HTML5 Tokenizer and DOM Tree Builder.
pub struct HtmlParser;

impl HtmlParser {
    /// Parses an HTML string into a structured DomNode hierarchy.
    pub fn parse(html: &str) -> DomNode {
        let doc = kuchiki::parse_html().one(html);
        let mut next_id = 0;
        Self::convert_node(&doc, &mut next_id)
    }

    fn convert_node(kuchiki_node: &kuchiki::NodeRef, next_id: &mut usize) -> DomNode {
        let id = *next_id;
        *next_id += 1;

        let node_type = match kuchiki_node.data() {
            NodeData::Document(_) => NodeType::Document,
            NodeData::Element(elem) => {
                let tag = elem.name.local.to_string();
                let mut attributes = HashMap::new();
                for (name, val) in &elem.attributes.borrow().map {
                    attributes.insert(name.local.to_string(), val.value.clone());
                }
                NodeType::Element { tag, attributes }
            }
            NodeData::Text(text) => NodeType::Text(text.borrow().clone()),
            NodeData::Comment(comment) => NodeType::Comment(comment.borrow().clone()),
            _ => NodeType::Document,
        };

        let mut children = Vec::new();
        for child in kuchiki_node.children() {
            children.push(Self::convert_node(&child, next_id));
        }

        DomNode {
            id,
            node_type,
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_parsing_and_queries() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head><title>Test Page</title></head>
                <body>
                    <div id="main" class="container dark-theme">
                        <h1>Welcome to Excalibur</h1>
                        <p class="intro">A security-focused browser.</p>
                        <a href="https://example.com" class="link primary">Click Here</a>
                    </div>
                </body>
            </html>
        "#;

        let dom = HtmlParser::parse(html);

        // Find by Tag
        let titles = dom.find_by_tag("title");
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].text_content(), "Test Page");

        // Find by ID
        let main_div = dom.find_by_id("main");
        assert!(main_div.is_some());
        let main_div = main_div.unwrap();
        assert_eq!(main_div.attr("id"), Some("main"));

        // Find by Class
        let links = dom.find_by_class("link");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].attr("href"), Some("https://example.com"));

        // Text extraction
        assert!(dom.text_content().contains("Welcome to Excalibur"));
    }
}
