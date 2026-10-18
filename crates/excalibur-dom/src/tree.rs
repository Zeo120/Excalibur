use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of DOM node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Document,
    Element {
        tag: String,
        attributes: HashMap<String, String>,
    },
    Text(String),
    Comment(String),
}

/// A node in Excalibur's DOM tree representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomNode {
    pub id: usize,
    pub node_type: NodeType,
    pub children: Vec<DomNode>,
}

impl DomNode {
    pub fn new(id: usize, node_type: NodeType) -> Self {
        Self {
            id,
            node_type,
            children: Vec::new(),
        }
    }

    /// Returns the element's tag name if this is an Element node.
    pub fn tag(&self) -> Option<&str> {
        match &self.node_type {
            NodeType::Element { tag, .. } => Some(tag.as_str()),
            _ => None,
        }
    }

    /// Returns an attribute value by name (case-insensitive) if this is an Element.
    pub fn attr(&self, name: &str) -> Option<&str> {
        match &self.node_type {
            NodeType::Element { attributes, .. } => {
                let name_lower = name.to_ascii_lowercase();
                for (k, v) in attributes {
                    if k.to_ascii_lowercase() == name_lower {
                        return Some(v.as_str());
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Sets or updates an attribute on this node.
    pub fn set_attr(&mut self, key: impl Into<String>, val: impl Into<String>) {
        if let NodeType::Element { attributes, .. } = &mut self.node_type {
            attributes.insert(key.into(), val.into());
        }
    }

    /// Extracts all text content recursively from this node and its descendants.
    pub fn text_content(&self) -> String {
        let mut text = String::new();
        self.collect_text(&mut text);
        text
    }

    fn collect_text(&self, out: &mut String) {
        match &self.node_type {
            NodeType::Text(t) => {
                out.push_str(t);
            }
            _ => {
                for child in &self.children {
                    child.collect_text(out);
                }
            }
        }
    }

    /// Finds all elements matching a tag name (case-insensitive).
    pub fn find_by_tag(&self, tag: &str) -> Vec<&DomNode> {
        let mut results = Vec::new();
        let target_tag = tag.to_ascii_lowercase();
        self.collect_by_tag(&target_tag, &mut results);
        results
    }

    fn collect_by_tag<'a>(&'a self, target_tag: &str, out: &mut Vec<&'a DomNode>) {
        if let Some(t) = self.tag() {
            if t.eq_ignore_ascii_case(target_tag) {
                out.push(self);
            }
        }
        for child in &self.children {
            child.collect_by_tag(target_tag, out);
        }
    }

    /// Finds an element by its `id` attribute.
    pub fn find_by_id(&self, id_val: &str) -> Option<&DomNode> {
        if let Some(id) = self.attr("id") {
            if id == id_val {
                return Some(self);
            }
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(id_val) {
                return Some(found);
            }
        }
        None
    }

    /// Finds all elements containing the given CSS class.
    pub fn find_by_class(&self, class_name: &str) -> Vec<&DomNode> {
        let mut results = Vec::new();
        self.collect_by_class(class_name, &mut results);
        results
    }

    fn collect_by_class<'a>(&'a self, class_name: &str, out: &mut Vec<&'a DomNode>) {
        if let Some(class_attr) = self.attr("class") {
            if class_attr.split_whitespace().any(|c| c == class_name) {
                out.push(self);
            }
        }
        for child in &self.children {
            child.collect_by_class(class_name, out);
        }
    }

    /// Returns a flat vector of all nodes in depth-first traversal order.
    pub fn iter_depth_first(&self) -> Vec<&DomNode> {
        let mut list = Vec::new();
        self.collect_depth_first(&mut list);
        list
    }

    fn collect_depth_first<'a>(&'a self, out: &mut Vec<&'a DomNode>) {
        out.push(self);
        for child in &self.children {
            child.collect_depth_first(out);
        }
    }
}
