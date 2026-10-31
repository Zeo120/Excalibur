use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Helper to produce a centered rect within the outer area.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_help_modal(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(80, 80, area);
    f.render_widget(Clear, popup_area);

    let text = r#"
Excalibur Security Workbench - Terminal TUI Guide
==================================================

GLOBAL SHORTCUTS:
  [1]           Switch to Web Browser View
  [2]           Switch to Protocol Inspector Stream
  [3]           Switch to Request Repeater
  [4]           Switch to Vulnerability Scanner
  [Tab]         Cycle forward through Tabs (or active panes/fields)
  [BackTab]     Cycle backward through Tabs
  [?]           Toggle this Help Overlay
  [q]           Quit Excalibur (restores terminal raw mode cleanly)

TAB 1: WEB BROWSER
  [/]           Enter URL edit mode (or Ctrl+L)
  [Enter]       Load URL & dispatch live request via Resilient Core
  [Esc]         Cancel URL editing
  [j / Down]    Scroll page downward
  [k / Up]      Scroll page upward
  [H]           History Back
  [L]           History Forward
  [r]           Reload current page

TAB 2: PROTOCOL INSPECTOR
  [j / Down]    Select next HTTP request
  [k / Up]      Select previous HTTP request
  [Tab]         Switch focus: Table <-> Request Detail <-> Response Detail
  [r]           Copy selected request to Repeater & switch tab
  [/]           Filter traffic by URL, method or status code (Esc clears)

TAB 3: REQUEST REPEATER
  [Tab]         Cycle active field (Method -> URL -> Headers -> Body)
  [e]           Edit current active field (Enter/Esc saves)
  [r / Enter]   Dispatch HTTP request upstream & view live response
  [j / k]       Scroll response view

TAB 4: VULNERABILITY SCANNER
  [j / Down]    Select next detected security finding
  [k / Up]      Select previous finding
  [c]           Clear scanner findings

PROOT & HEADLESS TIPS:
  * Pure Rust: Runs with zero graphical dependencies (X11/Wayland not required).
  * WebPKI Built-in: Does not require system CA certificates in PRoot (/etc/ssl/certs).
  * Safe Exit: Raw mode and screen buffer automatically restored on exit or panic.

[Press ? or Esc to close this help window]
"#;

    let help_widget = Paragraph::new(text.trim())
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" [ Excalibur TUI Help & Keybindings ] ")
                .title_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default().bg(Color::Rgb(15, 20, 25))),
        );

    f.render_widget(help_widget, popup_area);
}
