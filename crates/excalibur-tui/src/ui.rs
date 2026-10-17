use crate::app::{ActiveTab, TuiApp};
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
                .title(" ⚔️ Excalibur "),
        )
        .select(tab_index)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    let body_text = format!("Target URL: {}\n(Interactive TUI Active)", app.current_url);
    let body = Paragraph::new(body_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(body, chunks[1]);

    let status = Paragraph::new(Span::raw(
        " [q] Quit  [Tab] Switch Tab  [r] Repeat  [i] Intercept ",
    ))
    .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[2]);
}
