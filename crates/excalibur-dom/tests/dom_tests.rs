use excalibur_dom::extractor::ResourceExtractor;
use excalibur_dom::layout::LayoutEngine;
use excalibur_dom::parser::HtmlParser;

#[test]
fn test_complex_real_world_html_parsing() {
    let html = r##"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <title>Excalibur - Hacker News Benchmark</title>
            <link rel="stylesheet" href="news.css">
        </head>
        <body>
            <center>
                <table id="hnmain" border="0" cellpadding="0" cellspacing="0" width="85%" bgcolor="#f6f6ef">
                    <tr>
                        <td bgcolor="#ff6600">
                            <table border="0" cellpadding="0" cellspacing="0" width="100%">
                                <tr>
                                    <td><a href="https://news.ycombinator.com"><b class="hnname">Hacker News</b></a></td>
                                    <td><a href="newest">new</a> | <a href="threads">threads</a> | <a href="submit">submit</a></td>
                                </tr>
                            </table>
                        </td>
                    </tr>
                    <tr class="athing" id="1001">
                        <td align="right" class="title"><span class="rank">1.</span></td>
                        <td class="title"><span class="titleline"><a href="https://rust-lang.org">Rust 2.0 Released</a></span></td>
                    </tr>
                    <tr class="athing" id="1002">
                        <td align="right" class="title"><span class="rank">2.</span></td>
                        <td class="title"><span class="titleline"><a href="https://example.com/exploit">Zero Day Analysis in Browser Engines</a></span></td>
                    </tr>
                </table>
            </center>
        </body>
        </html>
    "##;

    let dom = HtmlParser::parse(html);

    // Assert title
    let titles = dom.find_by_tag("title");
    assert_eq!(titles.len(), 1);
    assert_eq!(
        titles[0].text_content(),
        "Excalibur - Hacker News Benchmark"
    );

    // Assert table element
    let table = dom.find_by_id("hnmain");
    assert!(table.is_some());
    assert_eq!(table.unwrap().attr("bgcolor"), Some("#f6f6ef"));

    // Find stories by class
    let story_rows = dom.find_by_class("athing");
    assert_eq!(story_rows.len(), 2);
    assert_eq!(story_rows[0].attr("id"), Some("1001"));
    assert_eq!(story_rows[1].attr("id"), Some("1002"));

    // Verify text content
    let text = dom.text_content();
    assert!(text.contains("Rust 2.0 Released"));
    assert!(text.contains("Zero Day Analysis in Browser Engines"));
}

#[test]
fn test_form_security_attribute_extraction() {
    let html = r##"
        <div>
            <form id="login-form" action="/auth/login" method="POST">
                <input type="text" name="user_email" value="pentester@target.com">
                <input type="password" name="user_password" value="">
                <input type="hidden" name="authenticity_token" value="SECURE_TOKEN_XYZ">
                <input type="submit" value="Log In">
            </form>
            <form id="insecure-form" action="http://insecure.target.com/reset" method="GET">
                <input type="text" name="query" value="search">
            </form>
        </div>
    "##;

    let dom = HtmlParser::parse(html);
    let forms = ResourceExtractor::extract_forms(&dom);
    assert_eq!(forms.len(), 2);

    let login_form = &forms[0];
    assert_eq!(login_form.action, "/auth/login");
    assert_eq!(login_form.method, "POST");
    assert!(login_form.has_csrf_token);
    assert!(login_form.has_password_field);
    assert_eq!(login_form.inputs.len(), 4);

    let insecure_form = &forms[1];
    assert_eq!(insecure_form.action, "http://insecure.target.com/reset");
    assert_eq!(insecure_form.method, "GET");
    assert!(!insecure_form.has_csrf_token);
    assert!(!insecure_form.has_password_field);
}

#[test]
fn test_stress_deeply_nested_and_malformed_html() {
    // 1,000 levels of nested div tags
    let mut nested_html = String::new();
    for i in 0..1000 {
        nested_html.push_str(&format!("<div id=\"nest-{}\">", i));
    }
    nested_html.push_str("<span>Deep Core Payload</span>");
    for _ in 0..1000 {
        nested_html.push_str("</div>");
    }

    let dom = HtmlParser::parse(&nested_html);
    assert!(dom.text_content().contains("Deep Core Payload"));

    // Malformed HTML with missing closing tags and unescaped characters
    let malformed_html = r##"
        <html>
            <head><title>Page Title</title></head>
            <body>
                <p>Paragraph 1 <b>Bold text without close
                <div><a href="https://target.com">Broken link</i>
                <form action=/unquoted method=post>
                <input name=unquoted value=123>
                <img src=broken.jpg
    "##;

    let malformed_dom = HtmlParser::parse(malformed_html);
    let forms = ResourceExtractor::extract_forms(&malformed_dom);
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].action, "/unquoted");
    assert_eq!(forms[0].inputs.len(), 1);
    assert_eq!(forms[0].inputs[0].name, "unquoted");

    let text_flow = LayoutEngine::render_to_plain_text(&malformed_dom);
    assert!(text_flow.contains("Paragraph 1"));
    assert!(text_flow.contains("Broken link"));
}
