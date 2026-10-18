use eframe::egui;
use excalibur_gui::app::{BrowserTab, GuiApp, TabNavLayout, XModeLayout, XModeTab};
use std::sync::mpsc::channel;

fn create_test_app() -> GuiApp {
    let (tx, rx) = channel();
    GuiApp {
        tabs: vec![BrowserTab::new(1, "https://example.com")],
        active_tab_id: 1,
        next_tab_id: 2,
        omnibox_input: "https://example.com".to_string(),
        browser_renderer: Default::default(),
        tab_nav_layout: TabNavLayout::Top,
        show_favorites_bar: true,
        x_mode_enabled: false,
        x_mode_tab: XModeTab::Interceptor,
        x_mode_layout: XModeLayout::SplitDrawer,
        interceptor_state: Default::default(),
        repeater_state: Default::default(),
        scanner_state: Default::default(),
        dom_inspector_state: Default::default(),
        proxy_state: Default::default(),
        page_tx: tx,
        page_rx: rx,
    }
}

#[test]
fn test_modern_browser_rendering_and_tabs() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    // 1. Render Modern Browser Mode (X-Mode OFF, Horizontal tabs)
    assert!(!app.x_mode_enabled);
    assert_eq!(app.tab_nav_layout, TabNavLayout::Top);
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

    // 3. Toggle to Zen / Arc Vertical Sidebar Mode
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
