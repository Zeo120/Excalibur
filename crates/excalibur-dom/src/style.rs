use crate::tree::DomNode;
use serde::{Deserialize, Serialize};

/// RGBA 8-bit color representation for CSS styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl CssColor {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses a CSS color string (#hex, rgb(), rgba(), or named web color).
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim().to_ascii_lowercase();
        if s.is_empty() || s == "none" || s == "transparent" {
            return Some(Self::rgba(0, 0, 0, 0));
        }

        // 1. Hex colors: #rgb, #rgba, #rrggbb, #rrggbbaa
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }

        // 2. rgb(r, g, b) or rgba(r, g, b, a)
        if s.starts_with("rgb(") || s.starts_with("rgba(") {
            return Self::parse_rgb_func(&s);
        }

        // 3. Named CSS web colors
        Self::parse_named_color(&s)
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(Self::rgb(r * 17, g * 17, b * 17))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                Some(Self::rgba(r * 17, g * 17, b * 17, a * 17))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    fn parse_rgb_func(s: &str) -> Option<Self> {
        let inside = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?;
        let inside = inside.strip_suffix(')')?;
        let parts: Vec<&str> = inside.split(',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            return None;
        }

        let r = parts[0].parse::<u8>().ok()?;
        let g = parts[1].parse::<u8>().ok()?;
        let b = parts[2].parse::<u8>().ok()?;
        let a = if parts.len() >= 4 {
            if let Ok(alpha_float) = parts[3].parse::<f32>() {
                (alpha_float.clamp(0.0, 1.0) * 255.0) as u8
            } else {
                parts[3].parse::<u8>().unwrap_or(255)
            }
        } else {
            255
        };

        Some(Self::rgba(r, g, b, a))
    }

    fn parse_named_color(name: &str) -> Option<Self> {
        match name {
            "black" => Some(Self::rgb(0, 0, 0)),
            "white" => Some(Self::rgb(255, 255, 255)),
            "red" => Some(Self::rgb(255, 0, 0)),
            "lime" | "green" => Some(Self::rgb(0, 128, 0)),
            "blue" => Some(Self::rgb(0, 0, 255)),
            "yellow" => Some(Self::rgb(255, 255, 0)),
            "cyan" | "aqua" => Some(Self::rgb(0, 255, 255)),
            "magenta" | "fuchsia" => Some(Self::rgb(255, 0, 255)),
            "silver" => Some(Self::rgb(192, 192, 192)),
            "gray" | "grey" => Some(Self::rgb(128, 128, 128)),
            "maroon" => Some(Self::rgb(128, 0, 0)),
            "olive" => Some(Self::rgb(128, 128, 0)),
            "purple" => Some(Self::rgb(128, 0, 128)),
            "teal" => Some(Self::rgb(0, 128, 128)),
            "navy" => Some(Self::rgb(0, 0, 128)),
            "orange" => Some(Self::rgb(255, 165, 0)),
            "gold" => Some(Self::rgb(255, 215, 0)),
            "coral" => Some(Self::rgb(255, 127, 80)),
            "tomato" => Some(Self::rgb(255, 99, 71)),
            "crimson" => Some(Self::rgb(220, 20, 60)),
            "darkred" => Some(Self::rgb(139, 0, 0)),
            "darkblue" => Some(Self::rgb(0, 0, 139)),
            "darkgreen" => Some(Self::rgb(0, 100, 0)),
            "darkgray" | "darkgrey" => Some(Self::rgb(169, 169, 169)),
            "lightblue" => Some(Self::rgb(173, 216, 230)),
            "lightgreen" => Some(Self::rgb(144, 238, 144)),
            "lightgray" | "lightgrey" => Some(Self::rgb(211, 211, 211)),
            "pink" => Some(Self::rgb(255, 192, 203)),
            "hotpink" => Some(Self::rgb(255, 105, 180)),
            "brown" => Some(Self::rgb(165, 42, 42)),
            "indigo" => Some(Self::rgb(75, 0, 130)),
            "violet" => Some(Self::rgb(238, 130, 238)),
            "darkorange" => Some(Self::rgb(255, 140, 0)),
            "forestgreen" => Some(Self::rgb(34, 139, 34)),
            "dodgerblue" => Some(Self::rgb(30, 144, 255)),
            "royalblue" => Some(Self::rgb(65, 105, 225)),
            "slateblue" => Some(Self::rgb(106, 90, 205)),
            "midnightblue" => Some(Self::rgb(25, 25, 112)),
            _ => None,
        }
    }
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Computed visual style for a DOM node.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComputedStyle {
    pub color: Option<CssColor>,
    pub background_color: Option<CssColor>,
    pub font_size: Option<f32>,
    pub font_weight_bold: Option<bool>,
    pub font_style_italic: Option<bool>,
    pub text_decoration_underline: Option<bool>,
    pub text_align: Option<TextAlign>,
    pub margin_top: Option<f32>,
    pub margin_bottom: Option<f32>,
    pub padding: Option<f32>,
    pub border_color: Option<CssColor>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub display_none: bool,
}

impl ComputedStyle {
    /// Parses an inline style string (e.g. `color: red; font-size: 16px;`).
    pub fn from_style_str(style_str: &str) -> Self {
        let mut style = Self::default();

        for declaration in style_str.split(';') {
            let decl = declaration.trim();
            if decl.is_empty() {
                continue;
            }

            if let Some((prop, val)) = decl.split_once(':') {
                let prop = prop.trim().to_ascii_lowercase();
                let val = val.trim();

                match prop.as_str() {
                    "color" => {
                        style.color = CssColor::parse(val);
                    }
                    "background-color" => {
                        style.background_color = CssColor::parse(val);
                    }
                    "background" => {
                        // Attempt to extract color from background declaration
                        if let Some(col) = CssColor::parse(val) {
                            style.background_color = Some(col);
                        } else {
                            for part in val.split_whitespace() {
                                if let Some(col) = CssColor::parse(part) {
                                    style.background_color = Some(col);
                                    break;
                                }
                            }
                        }
                    }
                    "font-size" => {
                        style.font_size = parse_dimension(val);
                    }
                    "font-weight" => {
                        let v = val.to_ascii_lowercase();
                        style.font_weight_bold = Some(
                            v == "bold" || v == "bolder" || v == "700" || v == "800" || v == "900",
                        );
                    }
                    "font-style" => {
                        let v = val.to_ascii_lowercase();
                        style.font_style_italic = Some(v == "italic" || v == "oblique");
                    }
                    "text-decoration" => {
                        let v = val.to_ascii_lowercase();
                        if v.contains("underline") {
                            style.text_decoration_underline = Some(true);
                        }
                    }
                    "text-align" => match val.to_ascii_lowercase().as_str() {
                        "center" => style.text_align = Some(TextAlign::Center),
                        "right" => style.text_align = Some(TextAlign::Right),
                        "left" => style.text_align = Some(TextAlign::Left),
                        _ => {}
                    },
                    "margin-top" => {
                        style.margin_top = parse_dimension(val);
                    }
                    "margin-bottom" => {
                        style.margin_bottom = parse_dimension(val);
                    }
                    "margin" => {
                        let dim = parse_dimension(val);
                        style.margin_top = dim;
                        style.margin_bottom = dim;
                    }
                    "padding" => {
                        style.padding = parse_dimension(val);
                    }
                    "border-color" => {
                        style.border_color = CssColor::parse(val);
                    }
                    "border-width" => {
                        style.border_width = parse_dimension(val);
                    }
                    "border-radius" => {
                        style.border_radius = parse_dimension(val);
                    }
                    "border" => {
                        // e.g. "1px solid #ff0000"
                        for part in val.split_whitespace() {
                            if let Some(dim) = parse_dimension(part) {
                                style.border_width = Some(dim);
                            } else if let Some(col) = CssColor::parse(part) {
                                style.border_color = Some(col);
                            }
                        }
                    }
                    "display" if val.eq_ignore_ascii_case("none") => {
                        style.display_none = true;
                    }
                    _ => {}
                }
            }
        }

        style
    }

    /// Merges another style on top of this one (overwriting with `other` if specified).
    pub fn merge(&mut self, other: &Self) {
        if other.color.is_some() {
            self.color = other.color;
        }
        if other.background_color.is_some() {
            self.background_color = other.background_color;
        }
        if other.font_size.is_some() {
            self.font_size = other.font_size;
        }
        if other.font_weight_bold.is_some() {
            self.font_weight_bold = other.font_weight_bold;
        }
        if other.font_style_italic.is_some() {
            self.font_style_italic = other.font_style_italic;
        }
        if other.text_decoration_underline.is_some() {
            self.text_decoration_underline = other.text_decoration_underline;
        }
        if other.text_align.is_some() {
            self.text_align = other.text_align;
        }
        if other.margin_top.is_some() {
            self.margin_top = other.margin_top;
        }
        if other.margin_bottom.is_some() {
            self.margin_bottom = other.margin_bottom;
        }
        if other.padding.is_some() {
            self.padding = other.padding;
        }
        if other.border_color.is_some() {
            self.border_color = other.border_color;
        }
        if other.border_width.is_some() {
            self.border_width = other.border_width;
        }
        if other.border_radius.is_some() {
            self.border_radius = other.border_radius;
        }
        if other.display_none {
            self.display_none = true;
        }
    }
}

fn parse_dimension(val: &str) -> Option<f32> {
    let s = val.trim();
    if let Some(num) = s.strip_suffix("px") {
        num.trim().parse::<f32>().ok()
    } else if let Some(num) = s.strip_suffix("pt") {
        num.trim().parse::<f32>().ok().map(|pt| pt * 1.333)
    } else if let Some(num) = s.strip_suffix("em") {
        num.trim().parse::<f32>().ok().map(|em| em * 16.0)
    } else if let Some(num) = s.strip_suffix("rem") {
        num.trim().parse::<f32>().ok().map(|rem| rem * 16.0)
    } else {
        s.parse::<f32>().ok()
    }
}

/// Selector types for embedded CSS rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssSelector {
    Tag(String),
    Class(String),
    Id(String),
    Universal,
}

/// A parsed CSS rule combining selectors and style declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct CssRule {
    pub selectors: Vec<CssSelector>,
    pub style: ComputedStyle,
}

/// A parsed CSS stylesheet containing ordered rules.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    pub rules: Vec<CssRule>,
}

impl StyleSheet {
    /// Parses raw CSS text into structured rules.
    pub fn parse(css_text: &str) -> Self {
        let mut stylesheet = Self::default();

        // 1. Strip CSS comments (/* ... */)
        let cleaned = strip_css_comments(css_text);

        // 2. Parse rules by block `{ ... }`
        let mut remaining = cleaned.as_str();
        while let Some(open_brace) = remaining.find('{') {
            let selector_str = remaining[..open_brace].trim();
            if let Some(close_brace) = remaining[open_brace + 1..].find('}') {
                let actual_close = open_brace + 1 + close_brace;
                let body_str = &remaining[open_brace + 1..actual_close];

                let mut selectors = Vec::new();
                for sel_raw in selector_str.split(',') {
                    let sel = sel_raw.trim();
                    if sel.is_empty() {
                        continue;
                    }

                    if let Some(class_name) = sel.strip_prefix('.') {
                        selectors.push(CssSelector::Class(class_name.trim().to_string()));
                    } else if let Some(id_name) = sel.strip_prefix('#') {
                        selectors.push(CssSelector::Id(id_name.trim().to_string()));
                    } else if sel == "*" {
                        selectors.push(CssSelector::Universal);
                    } else {
                        selectors.push(CssSelector::Tag(sel.to_ascii_lowercase()));
                    }
                }

                if !selectors.is_empty() {
                    let style = ComputedStyle::from_style_str(body_str);
                    stylesheet.rules.push(CssRule { selectors, style });
                }

                remaining = &remaining[actual_close + 1..];
            } else {
                break;
            }
        }

        stylesheet
    }

    /// Extracts and parses all embedded `<style>` elements from a DOM tree into a unified stylesheet.
    pub fn extract_from_dom(root: &DomNode) -> Self {
        let mut combined_css = String::new();
        for style_node in root.find_by_tag("style") {
            combined_css.push_str(&style_node.text_content());
            combined_css.push('\n');
        }
        Self::parse(&combined_css)
    }
}

fn strip_css_comments(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
            while let Some(inner) = chars.next() {
                if inner == '*' && chars.peek() == Some(&'/') {
                    chars.next(); // consume '/'
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Resolves the cascaded, computed style for an element in the DOM tree.
pub struct StyleResolver;

impl StyleResolver {
    /// Resolves the computed style for a DOM node considering document stylesheet and inline styles.
    pub fn resolve(node: &DomNode, stylesheet: &StyleSheet) -> ComputedStyle {
        let mut computed = ComputedStyle::default();

        let tag = node.tag();
        let class_attr = node.attr("class");
        let id_attr = node.attr("id");

        // 1. Match against stylesheet rules
        for rule in &stylesheet.rules {
            let mut matches = false;
            for selector in &rule.selectors {
                match selector {
                    CssSelector::Universal => {
                        matches = true;
                        break;
                    }
                    CssSelector::Tag(rule_tag) => {
                        if let Some(t) = tag {
                            if t.eq_ignore_ascii_case(rule_tag) {
                                matches = true;
                                break;
                            }
                        }
                    }
                    CssSelector::Class(rule_class) => {
                        if let Some(classes) = class_attr {
                            if classes.split_whitespace().any(|c| c == rule_class) {
                                matches = true;
                                break;
                            }
                        }
                    }
                    CssSelector::Id(rule_id) => {
                        if let Some(id_val) = id_attr {
                            if id_val == rule_id {
                                matches = true;
                                break;
                            }
                        }
                    }
                }
            }

            if matches {
                computed.merge(&rule.style);
            }
        }

        // 2. Highest Specificity: Inline `style="..."` attribute
        if let Some(inline_style_str) = node.attr("style") {
            let inline_computed = ComputedStyle::from_style_str(inline_style_str);
            computed.merge(&inline_computed);
        }

        computed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parsing() {
        assert_eq!(CssColor::parse("#fff"), Some(CssColor::rgb(255, 255, 255)));
        assert_eq!(CssColor::parse("#ff0000"), Some(CssColor::rgb(255, 0, 0)));
        assert_eq!(
            CssColor::parse("rgb(255, 128, 0)"),
            Some(CssColor::rgb(255, 128, 0))
        );
        assert_eq!(CssColor::parse("coral"), Some(CssColor::rgb(255, 127, 80)));
        assert_eq!(
            CssColor::parse("transparent"),
            Some(CssColor::rgba(0, 0, 0, 0))
        );
    }

    #[test]
    fn test_inline_style_parsing() {
        let style = ComputedStyle::from_style_str(
            "color: #ff5722; background-color: #1a1a2e; font-size: 18px; font-weight: bold; text-align: center; margin-top: 12px;",
        );

        assert_eq!(style.color, Some(CssColor::rgb(255, 87, 34)));
        assert_eq!(style.background_color, Some(CssColor::rgb(26, 26, 46)));
        assert_eq!(style.font_size, Some(18.0));
        assert_eq!(style.font_weight_bold, Some(true));
        assert_eq!(style.text_align, Some(TextAlign::Center));
        assert_eq!(style.margin_top, Some(12.0));
    }

    #[test]
    fn test_stylesheet_parsing_and_resolution() {
        let css = r#"
            /* Sample stylesheet */
            h1 { color: #58a6ff; font-size: 24px; }
            .alert { background-color: #f85149; color: #ffffff; }
            #special { text-align: right; }
        "#;

        let stylesheet = StyleSheet::parse(css);
        assert_eq!(stylesheet.rules.len(), 3);

        let mut node = DomNode::new(
            1,
            crate::tree::NodeType::Element {
                tag: "h1".to_string(),
                attributes: std::collections::HashMap::new(),
            },
        );
        node.set_attr("class", "alert");
        node.set_attr("id", "special");
        node.set_attr("style", "color: #00ff00; font-weight: bold;");

        let resolved = StyleResolver::resolve(&node, &stylesheet);
        // Inline color overrides rule
        assert_eq!(resolved.color, Some(CssColor::rgb(0, 255, 0)));
        // Inherited from .alert
        assert_eq!(resolved.background_color, Some(CssColor::rgb(248, 81, 73)));
        // Inherited from h1
        assert_eq!(resolved.font_size, Some(24.0));
        // Inherited from #special
        assert_eq!(resolved.text_align, Some(TextAlign::Right));
        // Inline bold
        assert_eq!(resolved.font_weight_bold, Some(true));
    }
}
