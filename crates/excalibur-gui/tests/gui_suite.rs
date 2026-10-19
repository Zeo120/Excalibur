use eframe::egui;
use excalibur_gui::app::{BrowserTab, GuiApp, TabNavLayout, XModeLayout, XModeTab};
use std::sync::mpsc::channel;

fn create_test_app() -> GuiApp {
    let (tx, rx) = channel();
    let storage = std::sync::Arc::new(excalibur_core::WorkspaceStorage::in_memory().unwrap());
    GuiApp {
        tabs: vec![BrowserTab::new(1, "https://example.com")],
        active_tab_id: 1,
        next_tab_id: 2,
        omnibox_input: "https://example.com".to_string(),
        browser_renderer: Default::default(),
        tab_nav_layout: TabNavLayout::Compact,
        show_favorites_bar: true,
        prefs: Default::default(),
        chrome: Default::default(),
        x_mode_enabled: false,
        x_mode_tab: XModeTab::Interceptor,
        x_mode_layout: XModeLayout::SplitDrawer,
        interceptor_state: Default::default(),
        repeater_state: Default::default(),
        scanner_state: Default::default(),
        dom_inspector_state: Default::default(),
        proxy_state: Default::default(),
        storage,
        cached_bookmarks: Vec::new(),
        closed_tabs_history: Vec::new(),
        omnibox_suggestions: Vec::new(),
        page_tx: tx,
        page_rx: rx,
        egui_ctx: None,
    }
}

#[test]
fn test_modern_browser_rendering_and_tabs() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    // 1. Render browser mode (X-Mode OFF, compact auto-hide sidebar)
    assert!(!app.x_mode_enabled);
    assert_eq!(app.tab_nav_layout, TabNavLayout::Compact);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 2. Add New Tab
    app.tabs.push(BrowserTab::new(2, "https://owasp.org"));
    app.active_tab_id = 2;
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active_tab().unwrap().url, "https://owasp.org");

    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 3. Dock the Zen vertical sidebar
    app.tab_nav_layout = TabNavLayout::Sidebar;
    assert_eq!(app.tab_nav_layout, TabNavLayout::Sidebar);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });
}

#[test]
fn test_x_mode_toggle_and_tools_workbench() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    // Toggle X-Mode ON
    app.x_mode_enabled = true;
    assert!(app.x_mode_enabled);

    // 1. Interceptor Tool
    app.x_mode_tab = XModeTab::Interceptor;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 2. Repeater Tool
    app.x_mode_tab = XModeTab::Repeater;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 3. Scanner Tool
    app.x_mode_tab = XModeTab::Scanner;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 4. DOM & Script Console Tool
    app.x_mode_tab = XModeTab::DomAndScripts;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 5. Proxy Manager Tool
    app.x_mode_tab = XModeTab::ProxyManager;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 6. Test Fullscreen layout mode
    app.x_mode_layout = XModeLayout::FullscreenTools;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });
}

#[test]
fn test_gui_interceptor_state_transitions() {
    let mut app = create_test_app();
    app.x_mode_enabled = true;

    assert!(!app.interceptor_state.intercept_enabled);
    app.interceptor_state.intercept_enabled = true;
    assert!(app.interceptor_state.intercept_enabled);

    // Pause request breakpoint
    app.interceptor_state.is_request_paused = true;
    app.interceptor_state.paused_req_buffer = "POST /api HTTP/1.1\r\n\r\n{}".into();
    assert!(app.intercalibur_paused());

    // Resume / forward
    app.interceptor_state.is_request_paused = false;
    assert!(!app.interceptor_state.is_request_paused);
}

trait TestHelper {
    fn intercalibur_paused(&self) -> bool;
}

impl TestHelper for GuiApp {
    fn intercalibur_paused(&self) -> bool {
        self.interceptor_state.is_request_paused
    }
}

#[test]
fn test_gui_repeater_mutation() {
    let mut app = create_test_app();
    app.x_mode_enabled = true;
    app.x_mode_tab = XModeTab::Repeater;

    app.repeater_state.method = "PUT".to_string();
    app.repeater_state.body = "{\"status\": \"updated\"}".to_string();
    assert_eq!(app.repeater_state.method, "PUT");
    assert!(app.repeater_state.body.contains("updated"));
}

#[test]
fn test_browser_css_rendering_pipeline() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { background-color: #0d1117; color: #c9d1d9; }
                h1 { color: #58a6ff; font-size: 28px; }
                .banner { background-color: #238636; color: #ffffff; padding: 12px; }
                #warning { color: #f85149; font-weight: bold; }
            </style>
        </head>
        <body>
            <h1>Excalibur Pure-Rust Browser</h1>
            <div class="banner">
                <p style="font-weight: bold; text-decoration: underline;">Secure Session Active</p>
            </div>
            <p id="warning">Security Alert Sample</p>
            <span style="color: #d29922;">Warning: Untrusted certificate</span>
            <a href="https://example.com/login" style="color: #79c0ff;">Login Portal</a>
            <button style="color: #ffffff; background-color: #1f6feb;">Submit Audit</button>
            <pre style="background-color: #161b22; color: #58a6ff;"><code>GET /admin HTTP/1.1</code></pre>
            <table style="border: 1px solid #30363d;">
                <tr><th>Header</th><th>Value</th></tr>
                <tr><td>Content-Security-Policy</td><td>default-src 'self'</td></tr>
            </table>
        </body>
        </html>
    "#;

    let dom = excalibur_dom::HtmlParser::parse(html);
    app.tabs[0].parsed_dom = Some(dom);
    app.tabs[0].url = "https://example.com/test".to_string();

    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    assert_eq!(app.tabs.len(), 1);
    assert!(app.tabs[0].parsed_dom.is_some());
}

#[test]
fn test_youtube_initial_data_extractor() {
    let mock_html = r#"
        <!DOCTYPE html><html><head><script>
        var ytInitialData = {
            "contents": {
                "twoColumnSearchResultsRenderer": {
                    "primaryContents": {
                        "sectionListRenderer": {
                            "contents": [
                                {
                                    "itemSectionRenderer": {
                                        "contents": [
                                            {
                                                "videoRenderer": {
                                                    "videoId": "dQw4w9WgXcQ",
                                                    "title": { "runs": [{ "text": "Rick Astley - Never Gonna Give You Up" }] },
                                                    "ownerText": { "runs": [{ "text": "Rick Astley" }] },
                                                    "lengthText": { "simpleText": "3:33" },
                                                    "viewCountText": { "simpleText": "1.4B views" },
                                                    "publishedTimeText": { "simpleText": "14 years ago" },
                                                    "thumbnail": { "thumbnails": [{ "url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg" }] }
                                                }
                                            }
                                        ]
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        };
        </script></head><body></body></html>
    "#;

    let items = excalibur_gui::views::browser::extract_youtube_videos(mock_html);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "dQw4w9WgXcQ");
    assert_eq!(items[0].title, "Rick Astley - Never Gonna Give You Up");
    assert_eq!(items[0].channel, "Rick Astley");
    assert_eq!(items[0].duration, "3:33");
    assert_eq!(items[0].views, "1.4B views");

    if let Ok(content) = std::fs::read_to_string("../../target/yt_search_test.html") {
        let live_items = excalibur_gui::views::browser::extract_youtube_videos(&content);
        assert!(
            !live_items.is_empty(),
            "Search results should contain extracted video items!"
        );
    }
}

#[test]
fn test_browser_internal_pages_and_omnibox_and_zoom() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    // 1. Test navigation to internal pages
    let internal_urls = [
        ("excalibur://newtab", "New Tab"),
        ("excalibur://history", "History"),
        ("excalibur://bookmarks", "Bookmarks"),
        ("excalibur://settings", "Settings"),
        ("excalibur://about", "About Excalibur"),
    ];

    for (url, expected_title) in internal_urls {
        app.navigate(1, url);
        let tab = app.active_tab().unwrap();
        assert_eq!(tab.url, url);
        assert_eq!(tab.title, expected_title);
        assert!(!tab.is_loading);

        // Render to verify no panics
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_panels(ctx);
        });
    }

    // 2. Test Bookmarks and History SQLite persistence
    let _ = app
        .storage
        .record_browser_visit("https://rust-lang.org", "Rust Programming Language");
    let _ = app
        .storage
        .add_browser_bookmark("https://rust-lang.org", "Rust Language", "Favorites");
    assert!(app
        .storage
        .is_browser_bookmarked("https://rust-lang.org")
        .unwrap());

    app.cached_bookmarks = app.storage.list_browser_bookmarks().unwrap();
    assert!(app
        .cached_bookmarks
        .iter()
        .any(|b| b.url == "https://rust-lang.org"));

    // 3. Test Omnibox search suggestions
    let suggestions = app.storage.search_omnibox("rust", 5).unwrap();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.url.contains("rust-lang.org")));

    // 4. Test Zoom and Find In Page toggling
    {
        let tab = app.active_tab_mut().unwrap();
        tab.zoom = 1.25;
        tab.show_find = true;
        tab.find_query = "Rust".to_string();
    }
    assert_eq!(app.active_tab().unwrap().zoom, 1.25);
    assert!(app.active_tab().unwrap().show_find);
    assert_eq!(app.active_tab().unwrap().find_query, "Rust");

    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });
}

#[test]
fn test_script_audit_survives_infinite_loop_and_keeps_tracing() {
    use excalibur_gui::app::{audit_inline_scripts, ScriptAuditBudget};
    use std::time::{Duration, Instant};

    let scripts = vec![
        "while (true) {}".to_string(),
        "document.write('<b>' + location.hash + '</b>');".to_string(),
    ];
    let budget = ScriptAuditBudget {
        per_script: Duration::from_millis(200),
        total: Duration::from_secs(5),
        ..Default::default()
    };
    let start = Instant::now();
    let sinks = audit_inline_scripts(
        "https://example.com/#x",
        &scripts,
        budget,
        excalibur_script::JsCancelHandle::new(),
    );
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "audit not bounded"
    );
    assert!(sinks.iter().any(|s| s.sink_name == "document.write"));
}

#[test]
fn test_script_audit_honours_cancellation() {
    use excalibur_gui::app::{audit_inline_scripts, ScriptAuditBudget};
    use std::time::{Duration, Instant};

    let cancel = excalibur_script::JsCancelHandle::new();
    let trigger = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        trigger.cancel();
    });
    let budget = ScriptAuditBudget {
        per_script: Duration::from_secs(60),
        total: Duration::from_secs(120),
        ..Default::default()
    };
    let start = Instant::now();
    let scripts = vec!["while (true) {}".to_string(), "eval('1')".to_string()];
    let sinks = audit_inline_scripts("https://example.com/", &scripts, budget, cancel);
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "cancel not honoured"
    );
    assert!(sinks.is_empty(), "scripts after cancellation must not run");
}

// ───────────── Navigation generations & cancellation (#21) ─────────────

fn loaded_event(
    tab_id: usize,
    generation: u64,
    title: &str,
) -> excalibur_gui::app::PageNavigationEvent {
    use excalibur_gui::app::{PageLoadedEvent, PageNavigationEvent};
    use excalibur_gui::views::interceptor::GuiTrafficItem;
    let html = format!("<html><head><title>{title}</title></head><body>{title}</body></html>");
    PageNavigationEvent::Success(Box::new(PageLoadedEvent {
        tab_id,
        generation,
        url: format!("https://{}.example/", title.to_lowercase()),
        status: 200,
        elapsed_ms: 1,
        raw_html: html.clone(),
        parsed_dom: excalibur_dom::HtmlParser::parse(&html),
        title: title.to_string(),
        findings: Vec::new(),
        traffic_item: GuiTrafficItem {
            id: "t".into(),
            method: "GET".into(),
            status: 200,
            url: String::new(),
            size: 0,
            elapsed_ms: 1,
            request_raw: String::new(),
            response_raw: String::new(),
        },
    }))
}

#[test]
fn test_stale_page_load_is_discarded() {
    let mut app = create_test_app();
    app.navigate(1, "excalibur://history"); // generation 1
    app.navigate(1, "excalibur://bookmarks"); // generation 2
    let current = app.tabs[0].nav_generation;
    assert_eq!(current, 2);

    // A response belonging to the first navigation arrives late.
    app.page_tx
        .send(loaded_event(1, current - 1, "Old"))
        .unwrap();
    app.poll_network_events();
    assert_eq!(app.tabs[0].url, "excalibur://bookmarks");
    assert_eq!(app.tabs[0].title, "Bookmarks");
    assert!(app.interceptor_state.traffic_items.is_empty());

    // The response for the current navigation is applied.
    app.page_tx.send(loaded_event(1, current, "Fresh")).unwrap();
    app.poll_network_events();
    assert_eq!(app.tabs[0].title, "Fresh");
    assert_eq!(app.tabs[0].url, "https://fresh.example/");
}

#[test]
fn test_stale_failure_is_discarded() {
    use excalibur_gui::app::PageNavigationEvent;
    let mut app = create_test_app();
    app.navigate(1, "excalibur://history");
    let current = app.tabs[0].nav_generation;
    app.page_tx
        .send(PageNavigationEvent::Failure {
            tab_id: 1,
            generation: current - 1,
            url: "https://old.example/".into(),
            error: "boom".into(),
        })
        .unwrap();
    app.poll_network_events();
    assert!(app.tabs[0].error_msg.is_none());
}

#[test]
fn test_stop_invalidates_queued_response() {
    let mut app = create_test_app();
    app.tabs[0].is_loading = true;
    let generation = app.tabs[0].nav_generation;
    app.stop_loading(1);
    assert!(!app.tabs[0].is_loading);
    // A response that was already in flight when Stop was pressed is ignored.
    app.page_tx
        .send(loaded_event(1, generation, "Late"))
        .unwrap();
    app.poll_network_events();
    assert_ne!(app.tabs[0].title, "Late");
}

/// Local server that accepts one connection, never answers, and reports when the
/// client side of the socket is closed.
async fn hanging_server() -> (
    String,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let _ = accepted_tx.send(());
        let mut buf = [0u8; 4096];
        loop {
            match sock.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
        let _ = closed_tx.send(());
    });
    (format!("http://{}/", addr), accepted_rx, closed_rx)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stop_cancels_in_flight_request() {
    use std::time::Duration;
    let (url, accepted, closed) = hanging_server().await;
    let mut app = create_test_app();
    app.navigate(1, &url);
    assert!(app.tabs[0].is_loading);
    tokio::time::timeout(Duration::from_secs(5), accepted)
        .await
        .expect("request never reached the server")
        .unwrap();

    app.stop_loading(1);
    assert!(!app.tabs[0].is_loading);
    assert!(app.tabs[0].nav_task.is_none());
    tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("Stop left the request's socket open")
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_new_navigation_cancels_previous_request() {
    use std::time::Duration;
    let (url, accepted, closed) = hanging_server().await;
    let mut app = create_test_app();
    app.navigate(1, &url);
    tokio::time::timeout(Duration::from_secs(5), accepted)
        .await
        .unwrap()
        .unwrap();
    app.navigate(1, "excalibur://history");
    assert!(!app.tabs[0].is_loading);
    tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("superseded request kept running")
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_closing_tab_cancels_its_request() {
    use std::time::Duration;
    let (url, accepted, closed) = hanging_server().await;
    let mut app = create_test_app();
    app.tabs.push(BrowserTab::new(2, "about:blank"));
    app.navigate(2, &url);
    tokio::time::timeout(Duration::from_secs(5), accepted)
        .await
        .unwrap()
        .unwrap();
    app.close_tab(2);
    tokio::time::timeout(Duration::from_secs(3), closed)
        .await
        .expect("closed tab kept its request running")
        .unwrap();
}

// ───────────── Content types & encodings end to end (#16, #17) ─────────────

/// Serves one fixed HTTP response to the first connection.
async fn one_shot_server(response: Vec<u8>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await;
        let _ = sock.write_all(&response).await;
        let _ = sock.shutdown().await;
    });
    format!("http://{}/notes.txt", addr)
}

async fn load(app: &mut excalibur_gui::GuiApp, url: &str) {
    app.navigate(1, url);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.tabs[0].is_loading {
        assert!(
            std::time::Instant::now() < deadline,
            "page never finished loading"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        app.poll_network_events();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_plain_text_page_is_decoded_and_not_parsed() {
    let body: &[u8] = b"caf\xe9\n<script>alert(1)</script>";
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    let url = one_shot_server(response).await;

    let mut app = create_test_app();
    load(&mut app, &url).await;

    let tab = &app.tabs[0];
    assert!(tab.error_msg.is_none(), "{:?}", tab.error_msg);
    assert_eq!(tab.title, "notes.txt");
    assert_eq!(tab.raw_html, "café\n<script>alert(1)</script>");
    let dom = tab.parsed_dom.as_ref().unwrap();
    assert!(dom.find_by_tag("script").is_empty());
    assert_eq!(dom.find_by_tag("pre")[0].text_content(), tab.raw_html);
}

// ───────────── Image load states (#5) ─────────────

async fn serve_once(status_line: &str, content_type: &str, body: Vec<u8>) -> String {
    let mut response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(&body);
    let url = one_shot_server(response).await;
    url.replace("/notes.txt", "/image")
}

async fn settle_image(
    cache: &mut excalibur_gui::views::browser::ThumbnailCache,
    url: &str,
) -> excalibur_gui::views::browser::ImageState {
    use excalibur_gui::views::browser::ImageState;
    let ctx = egui::Context::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let _ = cache.get_or_request(&ctx, url);
        match cache.state(url) {
            ImageState::Loading | ImageState::NotRequested => {}
            settled => return settled,
        }
        assert!(std::time::Instant::now() < deadline, "image never settled");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_image_http_error_is_reported() {
    use excalibur_gui::views::browser::{ImageState, ThumbnailCache};
    let url = serve_once("404 Not Found", "text/html", b"nope".to_vec()).await;
    let mut cache = ThumbnailCache::new();
    match settle_image(&mut cache, &url).await {
        ImageState::Failed(reason) => assert!(reason.contains("404"), "{reason}"),
        other => panic!("expected failure, got {other:?}"),
    }
    // Failures are cached: no new request, same answer.
    assert!(cache
        .get_or_request(&egui::Context::default(), &url)
        .is_none());
    assert!(matches!(cache.state(&url), ImageState::Failed(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_undecodable_image_is_reported() {
    use excalibur_gui::views::browser::{ImageState, ThumbnailCache};
    let url = serve_once("200 OK", "image/png", b"definitely not a png".to_vec()).await;
    let mut cache = ThumbnailCache::new();
    match settle_image(&mut cache, &url).await {
        ImageState::Failed(reason) => assert!(reason.contains("corrupt"), "{reason}"),
        other => panic!("expected failure, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unreachable_image_is_reported() {
    use excalibur_gui::views::browser::{ImageState, ThumbnailCache};
    // Bind then drop to obtain a port nothing listens on.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{port}/x.png");
    let mut cache = ThumbnailCache::new();
    assert!(matches!(
        settle_image(&mut cache, &url).await,
        ImageState::Failed(_)
    ));
}

#[tokio::test]
async fn test_data_url_image_fails_without_network() {
    use excalibur_gui::views::browser::{ImageState, ThumbnailCache};
    let mut cache = ThumbnailCache::new();
    let url = "data:image/png;base64,iVBORw0KGgo=";
    assert!(cache
        .get_or_request(&egui::Context::default(), url)
        .is_none());
    match cache.state(url) {
        ImageState::Failed(reason) => assert!(reason.contains("data:")),
        other => panic!("expected immediate failure, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_valid_png_loads() {
    use excalibur_gui::views::browser::{ImageState, ThumbnailCache};
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let url = serve_once("200 OK", "image/png", png).await;
    let mut cache = ThumbnailCache::new();
    assert_eq!(settle_image(&mut cache, &url).await, ImageState::Ready);
}

#[test]
fn test_nested_and_ordered_lists_render() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let html = r#"<html><body>
        <ol start="2" type="i"><li>Two<ul><li>nested<ol reversed><li>a</li><li>b</li></ol></li></ul></li><li>Three</li></ol>
        <ul><li>Bullet</li></ul>
    </body></html>"#;
    app.tabs[0].parsed_dom = Some(excalibur_dom::HtmlParser::parse(html));
    app.tabs[0].raw_html = html.to_string();
    app.tabs[0].url = "https://example.com/lists".to_string();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_panels(ctx));
    }
}

// ───────────── Non-ASCII text must never be sliced mid-character ─────────────

#[test]
fn test_watch_link_scanner_handles_multibyte_text_after_link() {
    // "/watch?v=" followed by fewer than 11 ASCII bytes and then Telugu text:
    // byte offset start+11 falls inside a 3-byte character.
    let html = "<a title=\"తెలుగు\" href=\"/watch?v=abcతెలుగు వీడియో\">x</a>";
    let videos = excalibur_gui::views::browser::extract_youtube_videos(html);
    assert!(videos.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_youtube_grid_renders_long_non_ascii_titles() {
    // Long Telugu title (3-byte chars): byte 62 is not a char boundary.
    let title = "తెలుగు వార్తలు ఈరోజు ముఖ్యాంశాలు మరియు తాజా సమాచారం ప్రత్యక్ష ప్రసారం";
    assert!(title.len() > 65 && !title.is_char_boundary(62));
    let html = format!(
        r#"<html><head><script>var ytInitialData = {{"contents":{{"twoColumnSearchResultsRenderer":{{"primaryContents":{{"sectionListRenderer":{{"contents":[{{"itemSectionRenderer":{{"contents":[{{"videoRenderer":{{"videoId":"dQw4w9WgXcQ","title":{{"runs":[{{"text":"{title}"}}]}},"ownerText":{{"runs":[{{"text":"ఛానల్"}}]}},"lengthText":{{"simpleText":"3:33"}},"viewCountText":{{"simpleText":"1K views"}},"publishedTimeText":{{"simpleText":"1 day ago"}},"thumbnail":{{"thumbnails":[{{"url":"https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"}}]}}}}}}]}}}}]}}}}}}}}}};</script></head><body></body></html>"#
    );
    let videos = excalibur_gui::views::browser::extract_youtube_videos(&html);
    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].title, title);

    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.tabs[0].url = "https://www.youtube.com/results?search_query=telugu".to_string();
    app.tabs[0].parsed_dom = Some(excalibur_dom::HtmlParser::parse(&html));
    app.tabs[0].raw_html = html.clone();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_panels(ctx));
    }
}
