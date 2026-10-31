use crate::app::TuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_browser_view(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let url_title = if app.is_editing_url {
        " URL Bar [EDITING - Enter: Load, Esc: Cancel] "
    } else if app.is_loading {
        " URL Bar [FETCHING IN BACKGROUND...] "
    } else {
        " URL Bar [/ to edit, H: Back, L: Fwd, j/k: Scroll, r: Reload] "
    };

    let url_style = if app.is_editing_url {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if app.is_loading {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let history_pos = if app.history.is_empty() {
        1
    } else {
        app.history_index + 1
    };
    let history_total = app.history.len().max(1);
    let url_display = format!(
        "{} (History: {}/{})",
        app.url_input, history_pos, history_total
    );

    let url_bar = Paragraph::new(url_display)
        .style(url_style)
        .block(Block::default().borders(Borders::ALL).title(url_title));
    f.render_widget(url_bar, chunks[0]);

    let content = if app.browser_rendered_text.is_empty() {
        "No document loaded. Type URL above and press Enter."
    } else {
        &app.browser_rendered_text
    };

    let viewport_title = if app.is_loading {
        format!(
            " Web Viewport [Loading... Line {}] ",
            app.browser_scroll + 1
        )
    } else {
        format!(
            " Web Viewport [Rendered via DOM Engine | Line {}] ",
            app.browser_scroll + 1
        )
    };

    let page_view = Paragraph::new(content)
        .scroll((app.browser_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(viewport_title));
    f.render_widget(page_view, chunks[1]);
}
