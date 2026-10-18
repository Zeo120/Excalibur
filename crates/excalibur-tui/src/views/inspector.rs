use crate::app::TuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render_inspector_view(f: &mut Frame, app: &TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Traffic Table
    let header_cells = ["#", "Method", "Status", "URL", "Size", "Latency"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .traffic_list
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = idx == app.selected_traffic_index;
            let status_color = if item.status < 300 {
                Color::Green
            } else if item.status < 400 {
                Color::Yellow
            } else {
                Color::Red
            };

            let cells = vec![
                Cell::from(format!("{}", idx + 1)),
                Cell::from(item.method.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{}", item.status)).style(Style::default().fg(status_color)),
                Cell::from(item.url.clone()),
                Cell::from(format!("{} B", item.size)),
                Cell::from(format!("{} ms", item.elapsed_ms)),
            ];

            let row_style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(cells).style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Percentage(55),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default().borders(Borders::ALL).title(format!(
            " Intercepted Traffic Stream ({} requests) [j/k: Select, r: Send to Repeater] ",
            app.traffic_list.len()
        )),
    );
    f.render_widget(table, main_chunks[0]);

    // Split Request / Response viewer
    let detail_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    let (req_raw, res_raw) = if let Some(item) = app.traffic_list.get(app.selected_traffic_index) {
        (&item.request_raw, &item.response_raw)
    } else {
        (
            &"No request selected.".to_string(),
            &"No response selected.".to_string(),
        )
    };

    let req_view = Paragraph::new(req_raw.as_str())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Request Inspector "),
        );
    f.render_widget(req_view, detail_chunks[0]);

    let res_view = Paragraph::new(res_raw.as_str())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Response Inspector "),
        );
    f.render_widget(res_view, detail_chunks[1]);
}
