//! `excalibur-dom`: HTML5 parser, DOM node tree explorer, and layout tree representation.

pub mod extractor;
pub mod layout;
pub mod parser;
pub mod tree;

pub use extractor::{ExtractedForm, ExtractedResource, FormInput, ResourceExtractor, ResourceType};
pub use layout::{LayoutBlock, LayoutBlockType, LayoutEngine};
pub use parser::HtmlParser;
pub use tree::{DomNode, NodeType};
