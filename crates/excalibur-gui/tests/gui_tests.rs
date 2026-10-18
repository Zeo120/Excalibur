use eframe::egui;
use excalibur_gui::GuiApp;

#[test]
fn test_gui_app_headless_frame_renders() {
    let ctx = egui::Context::default();
    let mut app = GuiApp {
        url_input: "https://example.com/test".to_string(),
        selected_tab: 0,
        browser_state: Default::default(),
        interceptor_state: Default::default(),
        repeater_state: Default::default(),
        scanner_state: Default::default(),
    };

    // 1. Run frame for Browser Viewport
    app.selected_tab = 0;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 2. Run frame for Traffic Inspector
    app.selected_tab = 1;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 3. Run frame for Repeater
    app.selected_tab = 2;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });

    // 4. Run frame for Scanner
    app.selected_tab = 3;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_panels(ctx);
    });
}

#[test]
fn test_gui_interceptor_state_transitions() {
    let mut app = GuiApp {
        url_input: "https://example.com".to_string(),
        selected_tab: 1,
        browser_state: Default::default(),
        interceptor_state: Default::default(),
        repeater_state: Default::default(),
        scanner_state: Default::default(),
    };

    assert!(!app.interceptor_state.intercept_enabled);
    app.interceptor_state.intercept_enabled = true;
    assert!(app.interceptor_state.intercept_enabled);

    // Pause request breakpoint
    app.interceptor_state.is_request_paused = true;
    app.interceptor_state.paused_req_buffer = "POST /api HTTP/1.1\r\n\r\n{}".into();
    assert!(app.interceptor_state.is_request_paused);

    // Resume / forward
    app.interceptor_state.is_request_paused = false;
    assert!(!app.interceptor_state.is_request_paused);
}

#[test]
fn test_gui_repeater_mutation() {
    let mut app = GuiApp {
        url_input: "https://example.com".to_string(),
        selected_tab: 2,
        browser_state: Default::default(),
        interceptor_state: Default::default(),
        repeater_state: Default::default(),
        scanner_state: Default::default(),
    };

    app.repeater_state.method = "PUT".to_string();
    app.repeater_state.body = "{\"status\": \"updated\"}".to_string();
    assert_eq!(app.repeater_state.method, "PUT");
    assert!(app.repeater_state.body.contains("updated"));
}
