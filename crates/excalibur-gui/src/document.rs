//! Turning an HTTP response into the DOM the page view renders, according to its
//! content type.

use excalibur_core::content::{classify, decode_text, BodyKind};
use excalibur_dom::{DomNode, HtmlParser, NodeType};
use std::collections::HashMap;

/// A response prepared for display.
pub struct LoadedDocument {
    pub dom: DomNode,
    /// Decoded source text (HTML or plain text); empty for images and binaries.
    pub source: String,
    pub title: String,
    pub kind: BodyKind,
}

/// Builds the displayable document for a response body.
///
/// Only `BodyKind::Html` is parsed as markup. Text is wrapped verbatim in `<pre>`,
/// images become an `<img>` pointing at `url`, and anything else becomes a short
/// "can't display" notice — all constructed directly, never by parsing the body.
pub fn document_for_response(url: &str, content_type: Option<&str>, body: &[u8]) -> LoadedDocument {
    let kind = classify(content_type, body);
    let fallback_title = title_from_url(url);
    match &kind {
        BodyKind::Html => {
            let (source, _) = decode_text(body, content_type, true);
            let dom = HtmlParser::parse(&source);
            let title = dom
                .find_by_tag("title")
                .first()
                .map(|t| t.text_content().trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(fallback_title);
            LoadedDocument {
                dom,
                source,
                title,
                kind,
            }
        }
        BodyKind::Text { .. } => {
            let (source, _) = decode_text(body, content_type, false);
            let dom = document(&fallback_title, vec![el("pre", &[], vec![text(&source)])]);
            LoadedDocument {
                dom,
                source,
                title: fallback_title,
                kind,
            }
        }
        BodyKind::Image { .. } => {
            let dom = document(
                &fallback_title,
                vec![el("img", &[("src", url), ("alt", &fallback_title)], vec![])],
            );
            LoadedDocument {
                dom,
                source: String::new(),
                title: fallback_title,
                kind,
            }
        }
        BodyKind::Other { mime } => {
            let dom = document(
                &fallback_title,
                vec![
                    el("h2", &[], vec![text("This file can't be displayed")]),
                    el(
                        "p",
                        &[],
                        vec![text(&format!("{} · {}", mime, format_size(body.len())))],
                    ),
                    el(
                        "p",
                        &[],
                        vec![text("Excalibur displays HTML pages, text and images.")],
                    ),
                ],
            );
            LoadedDocument {
                dom,
                source: String::new(),
                title: fallback_title,
                kind,
            }
        }
    }
}

/// Last non-empty path segment (percent-decoded), else the host, else the URL.
fn title_from_url(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    parsed
        .path_segments()
        .and_then(|mut segs| segs.rfind(|s| !s.is_empty()))
        .map(percent_encoding_decode)
        .or_else(|| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

fn percent_encoding_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={}", s.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| s.to_string())
}

fn format_size(bytes: usize) -> String {
    match bytes {
        b if b >= 1024 * 1024 => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
        b if b >= 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{} bytes", b),
    }
}

fn el(tag: &str, attrs: &[(&str, &str)], children: Vec<DomNode>) -> DomNode {
    let attributes: HashMap<String, String> = attrs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    DomNode {
        id: 0,
        node_type: NodeType::Element {
            tag: tag.to_string(),
            attributes,
        },
        children,
    }
}

fn text(s: &str) -> DomNode {
    DomNode::new(0, NodeType::Text(s.to_string()))
}

/// `#document > html > (head > title) + (body > children)`, with sequential ids.
fn document(title: &str, body_children: Vec<DomNode>) -> DomNode {
    let html = el(
        "html",
        &[],
        vec![
            el("head", &[], vec![el("title", &[], vec![text(title)])]),
            el("body", &[], body_children),
        ],
    );
    let mut root = DomNode {
        id: 0,
        node_type: NodeType::Document,
        children: vec![html],
    };
    let mut next = 0;
    renumber(&mut root, &mut next);
    root
}

fn renumber(node: &mut DomNode, next: &mut usize) {
    node.id = *next;
    *next += 1;
    for child in &mut node.children {
        renumber(child, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_parsed_and_titled() {
        let doc = document_for_response(
            "https://example.com/",
            Some("text/html; charset=utf-8"),
            b"<html><head><title>Hi</title></head><body><p>x</p></body></html>",
        );
        assert_eq!(doc.kind, BodyKind::Html);
        assert_eq!(doc.title, "Hi");
        assert_eq!(doc.dom.find_by_tag("p").len(), 1);
    }

    #[test]
    fn plain_text_is_shown_verbatim_not_parsed() {
        let body = b"line one\n<script>alert(1)</script>\n  indented";
        let doc = document_for_response("https://example.com/notes.txt", Some("text/plain"), body);
        assert!(matches!(doc.kind, BodyKind::Text { .. }));
        assert!(
            doc.dom.find_by_tag("script").is_empty(),
            "text must not become markup"
        );
        let pre = doc.dom.find_by_tag("pre");
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].text_content(), String::from_utf8_lossy(body));
        assert_eq!(doc.title, "notes.txt");
    }

    #[test]
    fn json_uses_declared_charset() {
        let doc = document_for_response(
            "https://api.example.com/v1/items",
            Some("application/json; charset=iso-8859-1"),
            b"{\"name\":\"caf\xe9\"}",
        );
        assert_eq!(doc.source, "{\"name\":\"café\"}");
        assert_eq!(doc.title, "items");
    }

    #[test]
    fn images_become_an_img_element() {
        let url = "https://example.com/img/cat%20photo.png";
        let doc = document_for_response(url, Some("image/png"), b"\x89PNG\r\n\x1a\n");
        let imgs = doc.dom.find_by_tag("img");
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].attr("src"), Some(url));
        assert_eq!(doc.title, "cat photo.png");
        assert!(doc.source.is_empty());
    }

    #[test]
    fn binaries_show_a_notice_instead_of_garbage() {
        let doc = document_for_response(
            "https://example.com/report.pdf",
            Some("application/pdf"),
            &[0x25, 0x50, 0x44, 0x46, 0x00, 0xff, 0xfe],
        );
        assert_eq!(
            doc.kind,
            BodyKind::Other {
                mime: "application/pdf".into()
            }
        );
        let text = doc.dom.text_content();
        assert!(text.contains("can't be displayed"));
        assert!(text.contains("application/pdf · 7 bytes"));
        assert!(doc.source.is_empty());
    }

    #[test]
    fn octet_stream_html_is_not_rendered() {
        let doc = document_for_response(
            "https://example.com/download",
            Some("application/octet-stream"),
            b"<html><script>alert(1)</script></html>",
        );
        assert!(matches!(doc.kind, BodyKind::Other { .. }));
        assert!(doc.dom.find_by_tag("script").is_empty());
    }

    #[test]
    fn synthesized_ids_are_unique() {
        let doc = document_for_response("https://e.x/a.txt", Some("text/plain"), b"x");
        let ids: Vec<usize> = doc.dom.iter_depth_first().iter().map(|n| n.id).collect();
        let mut sorted = ids.clone();
        sorted.dedup();
        assert_eq!(ids, sorted);
        assert_eq!(ids, (0..ids.len()).collect::<Vec<_>>());
    }
}
