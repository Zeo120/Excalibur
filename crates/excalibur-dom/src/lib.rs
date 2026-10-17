//! `excalibur-dom`: HTML5 parser, DOM node tree explorer, and layout tree representation.

pub mod parser;
pub mod tree;

pub use parser::HtmlParser;
pub use tree::{DomNode, NodeType};
