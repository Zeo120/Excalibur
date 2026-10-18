use crate::app::{ActiveTab, TuiApp};
use crate::views::{
    browser::render_browser_view, inspector::render_inspector_view, repeater::render_repeater_view,
    scanner::render_scanner_view,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render_ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    let tab_titles = vec![
        "[1] Browser",
        "[2] Inspector",
        "[3] Repeater",
        "[4] Scanner",
    ];
    let tab_index = match app.active_tab {
        ActiveTab::Browser => 0,
        ActiveTab::Inspector => 1,
        ActiveTab::Repeater => 2,
        ActiveTab::Scanner => 3,
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" [Excalibur Security Workbench] "),
        )
        .select(tab_index)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    // Active View routing
    match app.active_tab {
        ActiveTab::Browser => render_browser_view(f, app, chunks[1]),
        ActiveTab::Inspector => render_inspector_view(f, app, chunks[1]),
        ActiveTab::Repeater => render_repeater_view(f, app, chunks[1]),
        ActiveTab::Scanner => render_scanner_view(f, app, chunks[1]),
    }

    // Status / Keybindings footer
    let status_text = match app.active_tab {
        ActiveTab::Browser => {
            " [1-4] Tabs  [/] URL Bar  [j/k] Scroll  [H/L] History Back/Fwd  [q] Quit "
        }
        ActiveTab::Inspector => {
            " [1-4] Tabs  [j/k] Navigate Requests  [r] Copy to Repeater  [q] Quit "
        }
        ActiveTab::Repeater => " [1-4] Tabs  [r] Send Request  [q] Quit ",
        ActiveTab::Scanner => " [1-4] Tabs  [j/k] Navigate Issues  [q] Quit ",
    };

    let status = Paragraph::new(Span::raw(status_text))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[2]);
}
