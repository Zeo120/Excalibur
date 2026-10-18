use crate::tree::DomNode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormInput {
    pub name: String,
    pub input_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedForm {
    pub action: String,
    pub method: String,
    pub inputs: Vec<FormInput>,
    pub has_csrf_token: bool,
    pub has_password_field: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Script,
    Stylesheet,
    Image,
    Iframe,
    Hyperlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedResource {
    pub resource_type: ResourceType,
    pub url: String,
}

/// Helper for extracting forms, inputs, and external resources from a parsed DOM tree.
pub struct ResourceExtractor;

impl ResourceExtractor {
    /// Extracts all interactive forms, inputs, and security attributes.
    pub fn extract_forms(root: &DomNode) -> Vec<ExtractedForm> {
        let form_nodes = root.find_by_tag("form");
        let mut forms = Vec::new();

        for f in form_nodes {
            let action = f.attr("action").unwrap_or("").to_string();
            let method = f.attr("method").unwrap_or("GET").to_uppercase();

            let input_nodes = f.find_by_tag("input");
            let mut inputs = Vec::new();
            let mut has_csrf_token = false;
            let mut has_password_field = false;

            for inp in input_nodes {
                let name = inp.attr("name").unwrap_or("").to_string();
                let input_type = inp.attr("type").unwrap_or("text").to_lowercase();
                let value = inp.attr("value").unwrap_or("").to_string();

                if input_type == "password" {
                    has_password_field = true;
                }

                let name_lower = name.to_ascii_lowercase();
                if name_lower.contains("csrf")
                    || name_lower.contains("token")
                    || name_lower.contains("xsrf")
                    || name_lower.contains("authenticity")
                {
                    has_csrf_token = true;
                }

                inputs.push(FormInput {
                    name,
                    input_type,
                    value,
                });
            }

            forms.push(ExtractedForm {
                action,
                method,
                inputs,
                has_csrf_token,
                has_password_field,
            });
        }

        forms
    }

    /// Extracts all referenced external resources (scripts, styles, images, iframes, links).
    pub fn extract_resources(root: &DomNode) -> Vec<ExtractedResource> {
        let mut resources = Vec::new();

        // Scripts
        for s in root.find_by_tag("script") {
            if let Some(src) = s.attr("src") {
                resources.push(ExtractedResource {
                    resource_type: ResourceType::Script,
                    url: src.to_string(),
                });
            }
        }

        // Stylesheets
        for link in root.find_by_tag("link") {
            if let Some(rel) = link.attr("rel") {
                if rel.eq_ignore_ascii_case("stylesheet") {
                    if let Some(href) = link.attr("href") {
                        resources.push(ExtractedResource {
                            resource_type: ResourceType::Stylesheet,
                            url: href.to_string(),
                        });
                    }
                }
            }
        }

        // Images
        for img in root.find_by_tag("img") {
            if let Some(src) = img.attr("src") {
                resources.push(ExtractedResource {
                    resource_type: ResourceType::Image,
                    url: src.to_string(),
                });
            }
        }

        // Iframes
        for iframe in root.find_by_tag("iframe") {
            if let Some(src) = iframe.attr("src") {
                resources.push(ExtractedResource {
                    resource_type: ResourceType::Iframe,
                    url: src.to_string(),
                });
            }
        }

        // Hyperlinks
        for a in root.find_by_tag("a") {
            if let Some(href) = a.attr("href") {
                resources.push(ExtractedResource {
                    resource_type: ResourceType::Hyperlink,
                    url: href.to_string(),
                });
            }
        }

        resources
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::HtmlParser;

    #[test]
    fn test_extract_forms_and_resources() {
        let html = r#"
            <html>
                <head>
                    <script src="/js/app.bundle.js"></script>
                    <link rel="stylesheet" href="/css/styles.css">
                </head>
                <body>
                    <form action="/login" method="post">
                        <input type="text" name="username" value="admin">
                        <input type="password" name="password" value="">
                        <input type="hidden" name="csrf_token" value="abc123xyz">
                        <input type="submit" value="Sign In">
                    </form>
                    <img src="/logo.png" alt="Logo">
                    <iframe src="https://partner.com/widget"></iframe>
                    <a href="/dashboard">Dashboard</a>
                </body>
            </html>
        "#;

        let dom = HtmlParser::parse(html);
        let forms = ResourceExtractor::extract_forms(&dom);
        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].action, "/login");
        assert_eq!(forms[0].method, "POST");
        assert!(forms[0].has_csrf_token);
        assert!(forms[0].has_password_field);
        assert_eq!(forms[0].inputs.len(), 4);

        let resources = ResourceExtractor::extract_resources(&dom);
        assert_eq!(resources.len(), 5);
        assert!(resources
            .iter()
            .any(|r| r.resource_type == ResourceType::Script && r.url == "/js/app.bundle.js"));
        assert!(resources
            .iter()
            .any(|r| r.resource_type == ResourceType::Stylesheet && r.url == "/css/styles.css"));
        assert!(resources
            .iter()
            .any(|r| r.resource_type == ResourceType::Image && r.url == "/logo.png"));
    }
}
