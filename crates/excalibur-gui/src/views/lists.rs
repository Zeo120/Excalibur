//! Flattening `<ul>`/`<ol>` trees into numbered/bulleted items with nesting depth.

use excalibur_dom::{DomNode, NodeType};

/// Marker drawn before a list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListMarker {
    /// Unordered item; the renderer varies the bullet shape by depth.
    Bullet,
    /// Ordered item label, e.g. `"3."`, `"c."`, `"iv."`.
    Label(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// 0 for the outermost list.
    pub depth: usize,
    pub marker: ListMarker,
    /// The item's own text, excluding any nested lists (whitespace collapsed).
    pub text: String,
}

fn is_list(node: &DomNode) -> bool {
    matches!(node.tag(), Some("ul" | "ol" | "menu"))
}

/// Flattens a list element and its nested lists into display order.
pub fn flatten_list(list: &DomNode) -> Vec<ListItem> {
    let mut out = Vec::new();
    flatten_into(list, 0, &mut out);
    out
}

fn flatten_into(list: &DomNode, depth: usize, out: &mut Vec<ListItem>) {
    let ordered = list.tag() == Some("ol");
    let items: Vec<&DomNode> = list
        .children
        .iter()
        .filter(|c| c.tag() == Some("li"))
        .collect();
    let reversed = ordered && list.attr("reversed").is_some();
    let style = list.attr("type").unwrap_or("1");
    let mut counter: i64 = list
        .attr("start")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(if reversed { items.len() as i64 } else { 1 });

    for li in items {
        if let Some(v) = li.attr("value").and_then(|v| v.trim().parse::<i64>().ok()) {
            counter = v;
        }
        let marker = if ordered {
            ListMarker::Label(format!("{}.", format_counter(counter, style)))
        } else {
            ListMarker::Bullet
        };
        let mut text = String::new();
        collect_text_excluding_lists(li, &mut text);
        out.push(ListItem {
            depth,
            marker,
            text: collapse_whitespace(&text),
        });
        counter += if reversed { -1 } else { 1 };

        for nested in li.iter_depth_first().into_iter().filter(|n| is_list(n)) {
            // Only lists that are not themselves inside another nested list.
            if is_direct_nested_list(li, nested) {
                flatten_into(nested, depth + 1, out);
            }
        }
    }
}

/// True when `target` is a list inside `li` with no other list between them.
fn is_direct_nested_list(li: &DomNode, target: &DomNode) -> bool {
    fn walk(node: &DomNode, target: &DomNode) -> Option<bool> {
        for child in &node.children {
            if std::ptr::eq(child, target) {
                return Some(true);
            }
            if is_list(child) {
                if contains(child, target) {
                    return Some(false);
                }
                continue;
            }
            if let Some(found) = walk(child, target) {
                return Some(found);
            }
        }
        None
    }
    fn contains(node: &DomNode, target: &DomNode) -> bool {
        node.children
            .iter()
            .any(|c| std::ptr::eq(c, target) || contains(c, target))
    }
    walk(li, target).unwrap_or(false)
}

fn collect_text_excluding_lists(node: &DomNode, out: &mut String) {
    for child in &node.children {
        match &child.node_type {
            NodeType::Text(t) => {
                out.push_str(t);
                out.push(' ');
            }
            NodeType::Element { tag, .. } => {
                if is_list(child) || matches!(tag.as_str(), "script" | "style" | "template") {
                    continue;
                }
                collect_text_excluding_lists(child, out);
            }
            _ => {}
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Formats an ordinal per the `<ol type>` attribute (1, a, A, i, I).
pub fn format_counter(n: i64, style: &str) -> String {
    match style {
        "a" | "A" if n >= 1 => {
            let mut v = n;
            let mut s = Vec::new();
            while v > 0 {
                v -= 1;
                s.push(b'a' + (v % 26) as u8);
                v /= 26;
            }
            s.reverse();
            let s = String::from_utf8(s).unwrap_or_default();
            if style == "A" {
                s.to_ascii_uppercase()
            } else {
                s
            }
        }
        "i" | "I" if (1..4000).contains(&n) => {
            const TABLE: [(i64, &str); 13] = [
                (1000, "m"),
                (900, "cm"),
                (500, "d"),
                (400, "cd"),
                (100, "c"),
                (90, "xc"),
                (50, "l"),
                (40, "xl"),
                (10, "x"),
                (9, "ix"),
                (5, "v"),
                (4, "iv"),
                (1, "i"),
            ];
            let mut v = n;
            let mut s = String::new();
            for (value, numeral) in TABLE {
                while v >= value {
                    s.push_str(numeral);
                    v -= value;
                }
            }
            if style == "I" {
                s.to_ascii_uppercase()
            } else {
                s
            }
        }
        // Decimal, and the fallback for values the other styles can't express.
        _ => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use excalibur_dom::HtmlParser;

    fn items(html: &str) -> Vec<(usize, String, String)> {
        let dom = HtmlParser::parse(html);
        let list = dom
            .iter_depth_first()
            .into_iter()
            .find(|n| is_list(n))
            .expect("no list in fixture");
        flatten_list(list)
            .into_iter()
            .map(|i| {
                let m = match i.marker {
                    ListMarker::Bullet => "•".to_string(),
                    ListMarker::Label(l) => l,
                };
                (i.depth, m, i.text)
            })
            .collect()
    }

    fn row(depth: usize, marker: &str, text: &str) -> (usize, String, String) {
        (depth, marker.to_string(), text.to_string())
    }

    #[test]
    fn unordered_list() {
        assert_eq!(
            items("<ul><li>One</li><li>  Two \n  words </li></ul>"),
            vec![row(0, "•", "One"), row(0, "•", "Two words")]
        );
    }

    #[test]
    fn ordered_list_numbering_attributes() {
        assert_eq!(
            items("<ol><li>a</li><li>b</li></ol>"),
            vec![row(0, "1.", "a"), row(0, "2.", "b")]
        );
        assert_eq!(
            items("<ol start=\"3\"><li>a</li><li>b</li></ol>"),
            vec![row(0, "3.", "a"), row(0, "4.", "b")]
        );
        assert_eq!(
            items("<ol reversed><li>a</li><li>b</li><li>c</li></ol>"),
            vec![row(0, "3.", "a"), row(0, "2.", "b"), row(0, "1.", "c")]
        );
        assert_eq!(
            items("<ol><li>a</li><li value=\"10\">b</li><li>c</li></ol>"),
            vec![row(0, "1.", "a"), row(0, "10.", "b"), row(0, "11.", "c")]
        );
        assert_eq!(
            items("<ol type=\"a\"><li>x</li><li>y</li></ol>"),
            vec![row(0, "a.", "x"), row(0, "b.", "y")]
        );
        assert_eq!(
            items("<ol type=\"I\" start=\"3\"><li>x</li><li>y</li></ol>"),
            vec![row(0, "III.", "x"), row(0, "IV.", "y")]
        );
    }

    #[test]
    fn nested_lists_keep_structure_and_parent_text() {
        let html = "<ul>
            <li>Fruit
                <ol><li>Apple</li><li>Pear
                    <ul><li>Conference</li></ul>
                </li></ol>
            </li>
            <li>Veg</li>
        </ul>";
        assert_eq!(
            items(html),
            vec![
                row(0, "•", "Fruit"),
                row(1, "1.", "Apple"),
                row(1, "2.", "Pear"),
                row(2, "•", "Conference"),
                row(0, "•", "Veg"),
            ]
        );
    }

    #[test]
    fn nested_list_wrapped_in_other_elements_is_found_once() {
        let html = "<ol><li>A<div><ul><li>inner</li></ul></div></li><li>B</li></ol>";
        assert_eq!(
            items(html),
            vec![row(0, "1.", "A"), row(1, "•", "inner"), row(0, "2.", "B")]
        );
    }

    #[test]
    fn non_item_children_are_ignored() {
        assert_eq!(
            items("<ul>stray<li>ok</li><script>x()</script></ul>"),
            vec![row(0, "•", "ok")]
        );
    }

    #[test]
    fn counter_formats() {
        assert_eq!(format_counter(27, "a"), "aa");
        assert_eq!(format_counter(52, "A"), "AZ");
        assert_eq!(format_counter(1994, "I"), "MCMXCIV");
        assert_eq!(format_counter(0, "a"), "0");
        assert_eq!(format_counter(-2, "i"), "-2");
    }
}
