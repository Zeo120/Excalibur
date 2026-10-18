use crate::app::TuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render_scanner_view(f: &mut Frame, app: &TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Findings Table
    let header_cells = ["#", "Severity", "Rule ID", "Title"].iter().map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app
        .audit_findings
        .iter()
        .enumerate()
        .map(|(idx, finding)| {
            let is_selected = idx == app.selected_audit_index;
            let sev_color = match finding.severity.to_ascii_uppercase().as_str() {
                "CRITICAL" => Color::Magenta,
                "HIGH" => Color::Red,
                "MEDIUM" => Color::Yellow,
                "LOW" => Color::Blue,
                _ => Color::Gray,
            };

            let cells = vec![
                Cell::from(format!("{}", idx + 1)),
                Cell::from(format!("[{}]", finding.severity))
                    .style(Style::default().fg(sev_color).add_modifier(Modifier::BOLD)),
                Cell::from(finding.rule_id.clone()),
                Cell::from(finding.title.clone()),
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
        Constraint::Length(14),
        Constraint::Length(25),
        Constraint::Percentage(60),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default().borders(Borders::ALL).title(format!(
            " Security Vulnerability Scanner ({} issues) [j/k: Select Issue] ",
            app.audit_findings.len()
        )),
    );
    f.render_widget(table, main_chunks[0]);

    // Detail pane for selected finding
    let detail_text = if let Some(item) = app.audit_findings.get(app.selected_audit_index) {
        format!(
            "Rule: {}\nSeverity: {}\nTitle: {}\n\nDescription:\n{}\n\nRecommendation:\n{}\n\nEvidence:\n{}",
            item.rule_id,
            item.severity,
            item.title,
            item.description,
            item.recommendation,
            item.evidence
        )
    } else {
        "No finding selected.".to_string()
    };

    let detail_view = Paragraph::new(detail_text)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Vulnerability Details & Remediation "),
        );
    f.render_widget(detail_view, main_chunks[1]);
}
