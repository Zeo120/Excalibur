use crate::app::{InspectorFocus, TuiApp};
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

    // Filter matching
    let filter_lower = app.inspector_filter.to_ascii_lowercase();
    let rows: Vec<Row> = app
        .traffic_list
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            if filter_lower.is_empty() {
                true
            } else {
                item.url.to_ascii_lowercase().contains(&filter_lower)
                    || item.method.to_ascii_lowercase().contains(&filter_lower)
                    || item.status.to_string().contains(&filter_lower)
            }
        })
        .map(|(idx, item)| {
            let is_selected = idx == app.selected_traffic_index;
            let status_color = if item.status < 300 {
                Color::Green
            } else if item.status < 400 {
                Color::Cyan
            } else if item.status < 500 {
                Color::Yellow
            } else {
                Color::Red
            };

            let cells = vec![
                Cell::from(format!("{}", idx + 1)),
                Cell::from(item.method.clone()).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(format!("{}", item.status)).style(
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(item.url.clone()),
                Cell::from(format!("{} B", item.size)),
                Cell::from(format!("{} ms", item.elapsed_ms)),
            ];

            let row_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(40, 50, 60))
                    .fg(Color::White)
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

    let table_border_style = if app.inspector_focus == InspectorFocus::Table {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let table_title = if app.is_filtering_inspector {
        format!(
            " Traffic Stream [FILTER EDITING: '{}' - Enter to apply, Esc to clear] ",
            app.inspector_filter
        )
    } else if !app.inspector_filter.is_empty() {
        format!(
            " Traffic Stream [Filtered: '{}' ({} matching) | Esc: Clear, /: Edit] ",
            app.inspector_filter,
            rows.len()
        )
    } else {
        format!(
            " Traffic Stream ({} requests) [Tab: Focus, /: Filter, r: To Repeater] ",
            app.traffic_list.len()
        )
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(table_border_style)
                .title(table_title),
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

    let req_border_style = if app.inspector_focus == InspectorFocus::Request {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let req_view = Paragraph::new(req_raw.as_str())
        .scroll((app.inspector_req_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(req_border_style)
                .title(format!(
                    " Request Inspector [Scroll: Line {}] ",
                    app.inspector_req_scroll + 1
                )),
        );
    f.render_widget(req_view, detail_chunks[0]);

    let res_border_style = if app.inspector_focus == InspectorFocus::Response {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let res_view = Paragraph::new(res_raw.as_str())
        .scroll((app.inspector_res_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(res_border_style)
                .title(format!(
                    " Response Inspector [Scroll: Line {}] ",
                    app.inspector_res_scroll + 1
                )),
        );
    f.render_widget(res_view, detail_chunks[1]);
}
