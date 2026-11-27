/// Normalizes user omnibox input into a navigable URL or a search engine query.
pub fn normalize_url_or_search(input: &str, search_template: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_owned();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("about:")
        || lower.starts_with("file://")
    {
        return trimmed.to_owned();
    }

    let has_whitespace = trimmed.chars().any(char::is_whitespace);
    if !has_whitespace && (looks_like_host(trimmed) || lower.starts_with("localhost")) {
        let scheme = if lower.starts_with("localhost") || is_loopback_ip(trimmed) {
            "http"
        } else {
            "https"
        };
        return format!("{scheme}://{trimmed}");
    }

    let encoded = percent_encode_query(trimmed);
    search_template.replace("{query}", &encoded)
}

fn looks_like_host(input: &str) -> bool {
    let host_part = input.split('/').next().unwrap_or(input);
    let host_only = host_part.split(':').next().unwrap_or(host_part);
    host_only.contains('.') && !host_only.starts_with('.') && !host_only.ends_with('.')
}

fn is_loopback_ip(input: &str) -> bool {
    let host_part = input.split('/').next().unwrap_or(input);
    let host_only = host_part.split(':').next().unwrap_or(host_part);
    host_only == "127.0.0.1" || host_only == "::1"
}

fn percent_encode_query(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            other => {
                use std::fmt::Write;
                let _ = write!(&mut out, "%{other:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &str = "https://duckduckgo.com/?q={query}";

    #[test]
    fn preserves_explicit_schemes() {
        assert_eq!(
            normalize_url_or_search("https://example.com/a", SEARCH),
            "https://example.com/a"
        );
        assert_eq!(
            normalize_url_or_search("about:blank", SEARCH),
            "about:blank"
        );
    }

    #[test]
    fn upgrades_bare_domains_and_localhost() {
        assert_eq!(
            normalize_url_or_search("example.org/docs", SEARCH),
            "https://example.org/docs"
        );
        assert_eq!(
            normalize_url_or_search("localhost:3000/test", SEARCH),
            "http://localhost:3000/test"
        );
        assert_eq!(
            normalize_url_or_search("127.0.0.1:8080", SEARCH),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn encodes_search_queries() {
        assert_eq!(
            normalize_url_or_search("rust cef browser", SEARCH),
            "https://duckduckgo.com/?q=rust+cef+browser"
        );
    }
}
