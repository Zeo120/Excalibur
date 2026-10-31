use crate::app::{RepeaterField, TuiApp};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_repeater_view(f: &mut Frame, app: &TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left pane: Request Configuration split into (Method+URL), Headers, and Body
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(main_chunks[0]);

    // Top: Method & URL line
    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(14), Constraint::Min(0)])
        .split(left_chunks[0]);

    // 1. Method
    let method_is_active = app.repeater_active_field == RepeaterField::Method;
    let method_is_editing = method_is_active && app.is_editing_repeater;
    let method_border = if method_is_editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if method_is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let method_title = if method_is_editing {
        " Method [EDIT] "
    } else if method_is_active {
        " Method [Tab] "
    } else {
        " Method "
    };
    let method_view = Paragraph::new(app.repeater_req_method.as_str())
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(method_border)
                .title(method_title),
        );
    f.render_widget(method_view, top_row[0]);

    // 2. URL
    let url_is_active = app.repeater_active_field == RepeaterField::Url;
    let url_is_editing = url_is_active && app.is_editing_repeater;
    let url_border = if url_is_editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if url_is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let url_title = if url_is_editing {
        " Target URL [EDITING - Enter: Save] "
    } else if url_is_active {
        " Target URL [e: Edit, Tab: Next] "
    } else {
        " Target URL "
    };
    let url_view = Paragraph::new(app.repeater_req_url.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(url_border)
                .title(url_title),
        );
    f.render_widget(url_view, top_row[1]);

    // 3. Headers
    let headers_is_active = app.repeater_active_field == RepeaterField::Headers;
    let headers_is_editing = headers_is_active && app.is_editing_repeater;
    let headers_border = if headers_is_editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if headers_is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let headers_title = if headers_is_editing {
        " HTTP Headers [EDITING - Esc: Done] "
    } else if headers_is_active {
        " HTTP Headers [e: Edit, Tab: Next] "
    } else {
        " HTTP Headers "
    };
    let headers_view = Paragraph::new(app.repeater_req_headers.as_str())
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(headers_border)
                .title(headers_title),
        );
    f.render_widget(headers_view, left_chunks[1]);

    // 4. Body
    let body_is_active = app.repeater_active_field == RepeaterField::Body;
    let body_is_editing = body_is_active && app.is_editing_repeater;
    let body_border = if body_is_editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if body_is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let body_title = if body_is_editing {
        " Request Body / Payload [EDITING - Esc: Done] "
    } else if body_is_active {
        " Request Body / Payload [e: Edit, r: Send] "
    } else {
        " Request Body / Payload "
    };
    let body_text = if app.repeater_req_body.is_empty() {
        "(Empty body)"
    } else {
        app.repeater_req_body.as_str()
    };
    let body_view = Paragraph::new(body_text)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(body_border)
                .title(body_title),
        );
    f.render_widget(body_view, left_chunks[2]);

    // Right pane: Response Viewer
    let res_title = format!(
        " Response Inspector [{}] [r: Send, j/k: Scroll] ",
        app.repeater_status_text
    );
    let res_content = if app.repeater_res_raw.is_empty() {
        "No response yet. Press 'r' or Enter to dispatch request."
    } else {
        app.repeater_res_raw.as_str()
    };

    let res_viewer = Paragraph::new(res_content)
        .scroll((app.repeater_res_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(res_title),
        );
    f.render_widget(res_viewer, main_chunks[1]);
}
