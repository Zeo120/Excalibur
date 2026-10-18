use crate::app::TuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_repeater_view(f: &mut Frame, app: &TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left pane: Request Editor
    let req_content = format!(
        "{} {} HTTP/1.1\r\n{}\r\n\r\n{}",
        app.repeater_req_method,
        app.repeater_req_url,
        app.repeater_req_headers.trim(),
        app.repeater_req_body
    );

    let req_editor = Paragraph::new(req_content)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Request Editor [Press 'r' to Send Request] "),
        );
    f.render_widget(req_editor, main_chunks[0]);

    // Right pane: Response Viewer
    let res_title = format!(" Response Viewer [{}] ", app.repeater_status_text);
    let res_content = if app.repeater_res_raw.is_empty() {
        "No response yet. Press 'r' to send the request.".to_string()
    } else {
        app.repeater_res_raw.clone()
    };

    let res_viewer = Paragraph::new(res_content)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(res_title));
    f.render_widget(res_viewer, main_chunks[1]);
}
