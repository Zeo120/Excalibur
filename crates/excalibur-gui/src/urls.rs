//! Turning what the user typed into a URL, and building correctly encoded search URLs.

use std::net::Ipv4Addr;
use url::Url;

/// Search providers selectable in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    DuckDuckGo,
    Google,
    Brave,
    SearXNG,
    YouTube,
}

impl SearchEngine {
    /// Parses the stored `search_engine` setting; unknown values fall back to DuckDuckGo.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "Google" => Self::Google,
            "Brave" => Self::Brave,
            "SearXNG" => Self::SearXNG,
            "YouTube" => Self::YouTube,
            _ => Self::DuckDuckGo,
        }
    }

    fn endpoint(self) -> (&'static str, &'static str) {
        match self {
            Self::DuckDuckGo => ("https://html.duckduckgo.com/html/", "q"),
            Self::Google => ("https://www.google.com/search", "q"),
            Self::Brave => ("https://search.brave.com/search", "q"),
            Self::SearXNG => ("https://searx.be/search", "q"),
            Self::YouTube => ("https://www.youtube.com/results", "search_query"),
        }
    }

    /// Search URL with `query` encoded as `application/x-www-form-urlencoded`.
    pub fn search_url(self, query: &str) -> String {
        let (base, param) = self.endpoint();
        with_query_param(base, param, query)
    }
}

/// Appends `name=value` to `base`, percent-encoding the value (UTF-8, `+` for space).
pub fn with_query_param(base: &str, name: &str, value: &str) -> String {
    match Url::parse_with_params(base, &[(name, value)]) {
        Ok(url) => url.to_string(),
        // `base` values are compile-time constants or already-parsed URLs.
        Err(_) => base.to_string(),
    }
}

/// Decoded value of query parameter `name` in `url`, if present.
pub fn query_param(url: &str, name: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// Interprets omnibox input: an explicit URL, something that looks like a host
/// (`example.com/path`, `localhost:3000`, `192.168.0.1`), or else a search query.
pub fn normalize_input(raw: &str, engine: SearchEngine) -> String {
    let input = raw.trim();
    if input.is_empty() {
        return "about:blank".to_string();
    }

    let lower = input.to_ascii_lowercase();
    if lower.starts_with("excalibur://") || lower == "about:blank" {
        return input.to_string();
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return match Url::parse(input) {
            Ok(url) if url.host_str().is_some_and(|h| !h.is_empty()) => url.to_string(),
            _ => engine.search_url(input),
        };
    }

    match as_host_url(input) {
        Some(url) => url,
        None => engine.search_url(input),
    }
}

/// Returns a URL when scheme-less `input` plausibly names a host, otherwise `None`.
fn as_host_url(input: &str) -> Option<String> {
    // Whitespace or an e-mail address means a search, as in mainstream browsers.
    if input.chars().any(char::is_whitespace) || input.contains('@') {
        return None;
    }
    let authority_end = input.find(['/', '?', '#']).unwrap_or(input.len());
    let authority = &input[..authority_end];
    let host = match authority.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h,
        Some(_) => return None, // `foo:bar` style input is not a host
        None => authority,
    };
    if host.is_empty() {
        return None;
    }

    let is_local = host.eq_ignore_ascii_case("localhost") || host.parse::<Ipv4Addr>().is_ok();
    let is_domain = {
        let labels: Vec<&str> = host.split('.').collect();
        labels.len() >= 2
            && labels.iter().all(|l| {
                !l.is_empty()
                    && !l.starts_with('-')
                    && !l.ends_with('-')
                    && l.chars().all(|c| c.is_alphanumeric() || c == '-')
            })
            && labels
                .last()
                .is_some_and(|tld| tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic))
    };
    if !is_local && !is_domain {
        return None;
    }

    // Local development servers rarely have TLS; everything else defaults to HTTPS.
    let scheme = if is_local { "http" } else { "https" };
    let url = Url::parse(&format!("{scheme}://{input}")).ok()?;
    url.host_str()?;
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DDG: SearchEngine = SearchEngine::DuckDuckGo;

    fn ddg(q: &str) -> String {
        format!("https://html.duckduckgo.com/html/?q={q}")
    }

    #[test]
    fn search_queries_are_form_encoded() {
        assert_eq!(normalize_input("c++", DDG), ddg("c%2B%2B"));
        assert_eq!(normalize_input("a&b", DDG), ddg("a%26b"));
        assert_eq!(normalize_input("what is rust", DDG), ddg("what+is+rust"));
        assert_eq!(
            normalize_input("100% legit #1?", DDG),
            ddg("100%25+legit+%231%3F")
        );
        assert_eq!(normalize_input("a=b", DDG), ddg("a%3Db"));
    }

    #[test]
    fn non_ascii_queries_are_utf8_percent_encoded() {
        assert_eq!(
            normalize_input("café crème", DDG),
            ddg("caf%C3%A9+cr%C3%A8me")
        );
        assert_eq!(
            normalize_input("日本語 テスト", DDG),
            ddg("%E6%97%A5%E6%9C%AC%E8%AA%9E+%E3%83%86%E3%82%B9%E3%83%88")
        );
        assert_eq!(normalize_input("😀", DDG), ddg("%F0%9F%98%80"));
    }

    #[test]
    fn queries_round_trip_through_every_engine() {
        let engines = [
            SearchEngine::DuckDuckGo,
            SearchEngine::Google,
            SearchEngine::Brave,
            SearchEngine::SearXNG,
            SearchEngine::YouTube,
        ];
        let queries = [
            "c++",
            "a&b",
            "café crème",
            "日本語 テスト",
            "x=1&y=2#frag",
            "  spaced  ",
        ];
        for engine in engines {
            let param = engine.endpoint().1;
            for q in queries {
                let url = engine.search_url(q);
                assert_eq!(
                    query_param(&url, param).as_deref(),
                    Some(q),
                    "{engine:?} {url}"
                );
            }
        }
    }

    #[test]
    fn engine_selection() {
        assert!(normalize_input("c++", SearchEngine::Google)
            .starts_with("https://www.google.com/search?q=c%2B%2B"));
        assert_eq!(
            SearchEngine::YouTube.search_url("rust lang"),
            "https://www.youtube.com/results?search_query=rust+lang"
        );
        assert_eq!(
            SearchEngine::from_setting("bogus"),
            SearchEngine::DuckDuckGo
        );
    }

    #[test]
    fn host_like_input_becomes_a_url() {
        assert_eq!(normalize_input("example.com", DDG), "https://example.com/");
        assert_eq!(
            normalize_input("example.com/path?x=1&y=é", DDG),
            "https://example.com/path?x=1&y=%C3%A9"
        );
        // Whitespace makes scheme-less input a search, as in mainstream browsers.
        assert_eq!(
            normalize_input("example.com/a b", DDG),
            ddg("example.com%2Fa+b")
        );
        assert_eq!(
            normalize_input("sub.example.co.uk:8443/p", DDG),
            "https://sub.example.co.uk:8443/p"
        );
        assert_eq!(
            normalize_input("localhost:3000", DDG),
            "http://localhost:3000/"
        );
        assert_eq!(normalize_input("192.168.1.1", DDG), "http://192.168.1.1/");
        assert_eq!(
            normalize_input("münchen.de", DDG),
            "https://xn--mnchen-3ya.de/"
        );
    }

    #[test]
    fn ambiguous_input_is_searched_not_navigated() {
        assert_eq!(normalize_input("3.14", DDG), ddg("3.14"));
        assert_eq!(
            normalize_input("user@example.com", DDG),
            ddg("user%40example.com")
        );
        assert_eq!(
            normalize_input("javascript:alert(1)", DDG),
            ddg("javascript%3Aalert%281%29")
        );
        assert_eq!(normalize_input("rust", DDG), ddg("rust"));
        assert_eq!(normalize_input("-bad-.com", DDG), ddg("-bad-.com"));
    }

    #[test]
    fn explicit_urls_and_internal_pages() {
        assert_eq!(
            normalize_input("HTTPS://Example.COM", DDG),
            "https://example.com/"
        );
        assert_eq!(
            normalize_input("https://example.com/a b", DDG),
            "https://example.com/a%20b"
        );
        assert_eq!(
            normalize_input("excalibur://settings", DDG),
            "excalibur://settings"
        );
        assert_eq!(normalize_input("about:blank", DDG), "about:blank");
        assert_eq!(normalize_input("   ", DDG), "about:blank");
        // Unparseable "URL" falls back to a search rather than a broken navigation.
        assert!(normalize_input("https://", DDG).starts_with("https://html.duckduckgo.com/"));
    }

    #[test]
    fn query_param_decodes() {
        assert_eq!(
            query_param(
                "https://www.youtube.com/results?search_query=caf%C3%A9+cr%C3%A8me",
                "search_query"
            )
            .as_deref(),
            Some("café crème")
        );
        assert_eq!(query_param("https://x.test/?a=1", "b"), None);
        assert_eq!(query_param("not a url", "a"), None);
    }
}
