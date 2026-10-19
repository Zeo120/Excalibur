//! Interpreting HTTP response bodies: character-encoding detection and decoding.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

/// Value of the `charset` parameter of a `Content-Type` header, if any.
pub fn charset_param(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|param| {
        let (name, value) = param.split_once('=')?;
        if name.trim().eq_ignore_ascii_case("charset") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            (!value.is_empty()).then_some(value)
        } else {
            None
        }
    })
}

/// Looks for a `<meta charset>` or `<meta http-equiv content="…charset=…">`
/// declaration in the first 1024 bytes, as the HTML prescan does.
fn meta_charset(body: &[u8]) -> Option<&'static Encoding> {
    let head = &body[..body.len().min(1024)];
    let lower: Vec<u8> = head.iter().map(|b| b.to_ascii_lowercase()).collect();
    let mut pos = 0;
    while let Some(off) = find(&lower[pos..], b"<meta") {
        let start = pos + off + 5;
        let end = find(&lower[start..], b">").map_or(lower.len(), |e| start + e);
        let tag = &lower[start..end];
        if let Some(c) = find(tag, b"charset") {
            let mut rest = &tag[c + 7..];
            rest = trim_ascii_start(rest);
            if let Some(after_eq) = rest.strip_prefix(b"=") {
                let value = trim_ascii_start(after_eq);
                let value = value
                    .strip_prefix(b"\"")
                    .or_else(|| value.strip_prefix(b"'"))
                    .unwrap_or(value);
                let label_end = value
                    .iter()
                    .position(|b| {
                        matches!(b, b'"' | b'\'' | b';' | b'>' | b'/') || b.is_ascii_whitespace()
                    })
                    .unwrap_or(value.len());
                if let Some(enc) = Encoding::for_label(&value[..label_end]) {
                    // Per the HTML standard, a UTF-16 meta declaration means UTF-8
                    // (the bytes were readable as ASCII), x-user-defined means 1252.
                    return Some(
                        if enc == encoding_rs::UTF_16LE || enc == encoding_rs::UTF_16BE {
                            UTF_8
                        } else if enc == encoding_rs::X_USER_DEFINED {
                            WINDOWS_1252
                        } else {
                            enc
                        },
                    );
                }
            }
        }
        pos = end;
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let n = bytes.iter().take_while(|b| b.is_ascii_whitespace()).count();
    &bytes[n..]
}

/// Determines the character encoding of `body`.
///
/// Order: byte-order mark, `Content-Type` charset, `<meta>` prescan (only when
/// `sniff_meta`), then UTF-8 if the bytes are valid UTF-8, else windows-1252.
pub fn detect_encoding(
    body: &[u8],
    content_type: Option<&str>,
    sniff_meta: bool,
) -> &'static Encoding {
    if let Some((enc, _)) = Encoding::for_bom(body) {
        return enc;
    }
    if let Some(enc) = content_type
        .and_then(charset_param)
        .and_then(|label| Encoding::for_label(label.as_bytes()))
    {
        return enc;
    }
    if sniff_meta {
        if let Some(enc) = meta_charset(body) {
            return enc;
        }
    }
    if std::str::from_utf8(body).is_ok() {
        UTF_8
    } else {
        WINDOWS_1252
    }
}

/// Decodes a text response body into a `String` using [`detect_encoding`].
/// Malformed sequences become U+FFFD rather than failing.
pub fn decode_text(
    body: &[u8],
    content_type: Option<&str>,
    is_html: bool,
) -> (String, &'static Encoding) {
    let encoding = detect_encoding(body, content_type, is_html);
    // `decode` strips a matching BOM and lets a BOM override the label.
    let (text, used, _had_errors) = encoding.decode(body);
    (text.into_owned(), used)
}

/// How a response body should be presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyKind {
    /// Parse and render as an HTML document.
    Html,
    /// Show as preformatted text (plain text, JSON, XML, CSS, JavaScript…).
    Text { mime: String },
    /// Show as an image.
    Image { mime: String },
    /// Cannot be displayed (PDF, archives, media, octet-stream…).
    Other { mime: String },
}

/// MIME essence (`type/subtype`, lowercased, parameters removed).
pub fn mime_essence(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Classifies a response. A declared `Content-Type` is trusted; sniffing is only
/// used when it is absent, and never turns an unknown declared type into HTML.
pub fn classify(content_type: Option<&str>, body: &[u8]) -> BodyKind {
    let essence = content_type.map(mime_essence).unwrap_or_default();
    if essence.is_empty() {
        return sniff(body);
    }
    match essence.as_str() {
        "text/html" | "application/xhtml+xml" => BodyKind::Html,
        m if m.starts_with("image/") => BodyKind::Image { mime: essence },
        m if m.starts_with("text/")
            || m == "application/json"
            || m == "application/javascript"
            || m == "application/ecmascript"
            || m == "application/xml"
            || m.ends_with("+json")
            || m.ends_with("+xml") =>
        {
            BodyKind::Text { mime: essence }
        }
        _ => BodyKind::Other { mime: essence },
    }
}

/// Conservative sniffing for responses without a Content-Type.
fn sniff(body: &[u8]) -> BodyKind {
    const MAGIC: [(&[u8], &str); 6] = [
        (b"\x89PNG\r\n\x1a\n", "image/png"),
        (b"\xff\xd8\xff", "image/jpeg"),
        (b"GIF87a", "image/gif"),
        (b"GIF89a", "image/gif"),
        (b"%PDF-", "application/pdf"),
        (b"PK\x03\x04", "application/zip"),
    ];
    for (magic, mime) in MAGIC {
        if body.starts_with(magic) {
            return if mime.starts_with("image/") {
                BodyKind::Image { mime: mime.into() }
            } else {
                BodyKind::Other { mime: mime.into() }
            };
        }
    }
    if body.len() >= 12 && &body[..4] == b"RIFF" && &body[8..12] == b"WEBP" {
        return BodyKind::Image {
            mime: "image/webp".into(),
        };
    }

    let head = &body[..body.len().min(1024)];
    let looks_binary = head
        .iter()
        .any(|&b| b == 0 || (b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0c | 0x1b)));
    if looks_binary {
        return BodyKind::Other {
            mime: "application/octet-stream".into(),
        };
    }

    let start = head.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(head);
    let trimmed = trim_ascii_start(start);
    let lower: Vec<u8> = trimmed
        .iter()
        .take(64)
        .map(|b| b.to_ascii_lowercase())
        .collect();
    const HTML_STARTS: [&[u8]; 6] = [
        b"<!doctype html",
        b"<html",
        b"<head",
        b"<body",
        b"<!--",
        b"<meta",
    ];
    if HTML_STARTS.iter().any(|p| lower.starts_with(p)) {
        BodyKind::Html
    } else {
        BodyKind::Text {
            mime: "text/plain".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_types_are_trusted() {
        assert_eq!(
            classify(Some("text/html; charset=utf-8"), b"x"),
            BodyKind::Html
        );
        assert_eq!(
            classify(Some("Application/XHTML+XML"), b"x"),
            BodyKind::Html
        );
        assert_eq!(
            classify(Some("image/png"), b"not really"),
            BodyKind::Image {
                mime: "image/png".into()
            }
        );
        for (ct, mime) in [
            ("text/plain", "text/plain"),
            ("application/json", "application/json"),
            ("application/ld+json", "application/ld+json"),
            ("text/css", "text/css"),
            ("application/xml", "application/xml"),
        ] {
            assert_eq!(
                classify(Some(ct), b"{}"),
                BodyKind::Text { mime: mime.into() }
            );
        }
        // Text served as plain text must never be treated as markup.
        assert_eq!(
            classify(Some("text/plain"), b"<html><script>alert(1)</script>"),
            BodyKind::Text {
                mime: "text/plain".into()
            }
        );
    }

    #[test]
    fn unknown_or_binary_types_are_not_rendered_as_html() {
        for ct in [
            "application/pdf",
            "application/octet-stream",
            "video/mp4",
            "application/zip",
        ] {
            assert_eq!(
                classify(Some(ct), b"<!DOCTYPE html><html>"),
                BodyKind::Other { mime: ct.into() },
                "{ct}"
            );
        }
    }

    #[test]
    fn sniffing_without_content_type() {
        assert_eq!(
            classify(None, b"  <!DOCTYPE HTML><html></html>"),
            BodyKind::Html
        );
        assert_eq!(classify(Some(""), b"\xef\xbb\xbf<html>"), BodyKind::Html);
        assert_eq!(
            classify(None, b"\x89PNG\r\n\x1a\n\0\0"),
            BodyKind::Image {
                mime: "image/png".into()
            }
        );
        assert_eq!(
            classify(None, b"RIFF\0\0\0\0WEBPVP8 "),
            BodyKind::Image {
                mime: "image/webp".into()
            }
        );
        assert_eq!(
            classify(None, b"%PDF-1.7"),
            BodyKind::Other {
                mime: "application/pdf".into()
            }
        );
        assert_eq!(
            classify(None, b"hello\nworld"),
            BodyKind::Text {
                mime: "text/plain".into()
            }
        );
        assert_eq!(
            classify(None, b"abc\0def"),
            BodyKind::Other {
                mime: "application/octet-stream".into()
            }
        );
        assert_eq!(
            classify(None, b""),
            BodyKind::Text {
                mime: "text/plain".into()
            }
        );
    }

    fn html(body: &[u8], ct: Option<&str>) -> String {
        decode_text(body, ct, true).0
    }

    #[test]
    fn content_type_charset_is_honoured() {
        assert_eq!(
            html(b"caf\xe9", Some("text/html; charset=ISO-8859-1")),
            "café"
        );
        assert_eq!(
            html(b"caf\xe9", Some("text/html;Charset=\"latin1\"")),
            "café"
        );
        assert_eq!(
            html(
                b"\xcf\xf0\xe8\xe2\xe5\xf2",
                Some("text/plain; charset=windows-1251")
            ),
            "Привет"
        );
    }

    #[test]
    fn meta_charset_is_detected() {
        let mut page = b"<html><head><meta charset=\"shift_jis\"></head><body>".to_vec();
        page.extend_from_slice(&[0x93, 0xFA, 0x96, 0x7B]); // 日本 in Shift_JIS
        assert!(html(&page, Some("text/html")).ends_with("<body>日本"));

        let page = b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1251\">\xcf\xf0\xe8\xe2\xe5\xf2";
        assert!(html(page, None).ends_with("Привет"));
    }

    #[test]
    fn header_beats_meta_and_bom_beats_both() {
        let page = b"<meta charset=\"windows-1251\">caf\xe9";
        assert!(html(page, Some("text/html; charset=iso-8859-1")).ends_with("café"));

        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice("café".as_bytes());
        assert_eq!(html(&bom, Some("text/html; charset=iso-8859-1")), "café");

        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("hé".encode_utf16().flat_map(|u| u.to_le_bytes()))
            .collect();
        assert_eq!(html(&utf16, None), "hé");
    }

    #[test]
    fn fallbacks_without_declarations() {
        assert_eq!(html("naïve ✓".as_bytes(), None), "naïve ✓");
        assert_eq!(html(b"caf\xe9", None), "café"); // not UTF-8 -> windows-1252
        assert_eq!(
            html(b"caf\xe9", Some("text/html; charset=bogus-label")),
            "café"
        );
    }

    #[test]
    fn meta_rules_and_limits() {
        // utf-16 in a meta tag means utf-8.
        assert_eq!(detect_encoding(b"<meta charset=utf-16>", None, true), UTF_8);
        // Declarations past the first 1024 bytes are ignored.
        let mut late = vec![b' '; 1100];
        late.extend_from_slice(b"<meta charset=windows-1251>");
        assert_eq!(detect_encoding(&late, None, true), UTF_8);
        // Meta sniffing only applies to HTML.
        assert_eq!(
            detect_encoding(b"<meta charset=windows-1251>\xe9", None, false),
            WINDOWS_1252
        );
    }

    #[test]
    fn charset_param_parsing() {
        assert_eq!(charset_param("text/html; charset=UTF-8"), Some("UTF-8"));
        assert_eq!(
            charset_param("text/html;charset='koi8-r'; x=y"),
            Some("koi8-r")
        );
        assert_eq!(charset_param("text/html"), None);
        assert_eq!(charset_param("text/html; charset="), None);
    }
}
