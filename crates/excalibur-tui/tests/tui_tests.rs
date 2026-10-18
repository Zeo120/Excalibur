use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use excalibur_tui::{ActiveTab, TrafficItem, TuiApp};
use ratatui::{backend::TestBackend, Terminal};

fn make_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn test_tui_rendering_all_tabs_headless() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("init test terminal");
    let mut app = TuiApp::new();

    // 1. Render Browser Tab
    app.active_tab = ActiveTab::Browser;
    terminal
        .draw(|f| excalibur_tui::render_ui(f, &app))
        .expect("draw browser tab");
    let buffer = terminal.backend().buffer().clone();
    let text = format!("{:?}", buffer);
    assert!(text.contains("Browser"));

    // 2. Render Inspector Tab
    app.active_tab = ActiveTab::Inspector;
    terminal
        .draw(|f| excalibur_tui::render_ui(f, &app))
        .expect("draw inspector tab");

    // 3. Render Repeater Tab
    app.active_tab = ActiveTab::Repeater;
    terminal
        .draw(|f| excalibur_tui::render_ui(f, &app))
        .expect("draw repeater tab");

    // 4. Render Scanner Tab
    app.active_tab = ActiveTab::Scanner;
    terminal
        .draw(|f| excalibur_tui::render_ui(f, &app))
        .expect("draw scanner tab");
}

#[test]
fn test_tui_tab_switching_keybindings() {
    let mut app = TuiApp::new();
    assert_eq!(app.active_tab, ActiveTab::Browser);

    // Press '2' -> Inspector
    app.handle_key(make_key(KeyCode::Char('2')));
    assert_eq!(app.active_tab, ActiveTab::Inspector);

    // Press '3' -> Repeater
    app.handle_key(make_key(KeyCode::Char('3')));
    assert_eq!(app.active_tab, ActiveTab::Repeater);

    // Press '4' -> Scanner
    app.handle_key(make_key(KeyCode::Char('4')));
    assert_eq!(app.active_tab, ActiveTab::Scanner);

    // Press 'Tab' -> Browser (cycles back)
    app.handle_key(make_key(KeyCode::Tab));
    assert_eq!(app.active_tab, ActiveTab::Browser);

    // Press 'BackTab' -> Scanner (cycles backward)
    app.handle_key(make_key(KeyCode::BackTab));
    assert_eq!(app.active_tab, ActiveTab::Scanner);
}

#[test]
fn test_url_editing_and_history_navigation() {
    let mut app = TuiApp::new();

    // Start editing URL with '/'
    app.handle_key(make_key(KeyCode::Char('/')));
    assert!(app.is_editing_url);

    // Type a new URL
    app.url_input.clear();
    for c in "https://test-target.org/api".chars() {
        app.handle_key(make_key(KeyCode::Char(c)));
    }
    assert_eq!(app.url_input, "https://test-target.org/api");

    // Press Enter to submit
    app.handle_key(make_key(KeyCode::Enter));
    assert!(!app.is_editing_url);
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.history[1], "https://test-target.org/api");

    // History Back (H)
    app.handle_key(make_key(KeyCode::Char('H')));
    assert_eq!(app.url_input, "https://example.com");

    // History Forward (L)
    app.handle_key(make_key(KeyCode::Char('L')));
    assert_eq!(app.url_input, "https://test-target.org/api");
}

#[test]
fn test_inspector_traffic_and_repeater_transfer() {
    let mut app = TuiApp::new();
    app.active_tab = ActiveTab::Inspector;

    // Add a specific traffic item
    app.record_traffic(TrafficItem {
        id: "req-999".into(),
        method: "POST".into(),
        status: 201,
        url: "https://api.test/v1/users".into(),
        size: 500,
        elapsed_ms: 120,
        request_raw: "POST /v1/users HTTP/1.1\r\n\r\n{\"name\":\"alice\"}".into(),
        response_raw: "HTTP/1.1 201 Created\r\n\r\n{\"id\":1}".into(),
    });

    // Navigate to second traffic item
    app.handle_key(make_key(KeyCode::Char('j')));
    assert_eq!(app.selected_traffic_index, 1);

    // Press 'r' to send to repeater
    app.handle_key(make_key(KeyCode::Char('r')));
    assert_eq!(app.active_tab, ActiveTab::Repeater);
    assert_eq!(app.repeater_req_method, "POST");
    assert_eq!(app.repeater_req_url, "https://api.test/v1/users");

    // Fire request in repeater ('r')
    app.handle_key(make_key(KeyCode::Char('r')));
    assert!(app.repeater_res_raw.contains("200 OK"));
}

#[test]
fn test_quit_keybinding() {
    let mut app = TuiApp::new();
    assert!(!app.should_quit);

    app.handle_key(make_key(KeyCode::Char('q')));
    assert!(app.should_quit);
}
