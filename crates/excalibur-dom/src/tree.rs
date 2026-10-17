use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
