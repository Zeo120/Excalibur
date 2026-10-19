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

#[cfg(test)]
mod tests {
    use super::*;

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
