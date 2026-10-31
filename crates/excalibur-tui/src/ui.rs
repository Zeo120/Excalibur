use crate::app::{ActiveTab, TuiApp};
use crate::views::{
    browser::render_browser_view, help::render_help_modal, inspector::render_inspector_view,
    repeater::render_repeater_view, scanner::render_scanner_view,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    // PRoot / Android Termux safe fallback if terminal dimensions are temporarily tiny (0x0 or < 10x5)
    if area.width < 10 || area.height < 5 {
        let fallback = Paragraph::new("Terminal too small. Resize window or rotate screen.")
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(fallback, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

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

    let spinner = match app.spinner_tick {
        0 => "|",
        1 => "/",
        2 => "-",
        _ => "\\",
    };

    let title_text = if app.is_loading {
        format!(" [Excalibur Workbench] [{} LOADING IN-FLIGHT] ", spinner)
    } else {
        " [Excalibur Security Workbench] [PRoot-Ready] ".to_string()
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title_text)
                .title_style(
                    Style::default()
                        .fg(if app.is_loading {
                            Color::Yellow
                        } else {
                            Color::Cyan
                        })
                        .add_modifier(Modifier::BOLD),
                ),
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
    let status_spans = match app.active_tab {
        ActiveTab::Browser => vec![
            Span::styled(" [1-4] Tabs ", Style::default().fg(Color::Yellow)),
            Span::raw(" [/] URL "),
            Span::raw(" [Enter] Go "),
            Span::raw(" [j/k] Scroll "),
            Span::raw(" [H/L] History "),
            Span::raw(" [r] Reload "),
            Span::styled(" [?] Help ", Style::default().fg(Color::Green)),
            Span::styled(" [q] Quit ", Style::default().fg(Color::Red)),
        ],
        ActiveTab::Inspector => vec![
            Span::styled(" [1-4] Tabs ", Style::default().fg(Color::Yellow)),
            Span::raw(" [Tab] Focus Table/Req/Res "),
            Span::raw(" [j/k] Navigate "),
            Span::raw(" [r] Send to Repeater "),
            Span::raw(" [/] Filter "),
            Span::styled(" [?] Help ", Style::default().fg(Color::Green)),
            Span::styled(" [q] Quit ", Style::default().fg(Color::Red)),
        ],
        ActiveTab::Repeater => vec![
            Span::styled(" [1-4] Tabs ", Style::default().fg(Color::Yellow)),
            Span::raw(" [Tab] Next Field "),
            Span::raw(" [e] Edit Field "),
            Span::styled(" [r/Enter] Dispatch ", Style::default().fg(Color::Cyan)),
            Span::raw(" [j/k] Scroll Res "),
            Span::styled(" [?] Help ", Style::default().fg(Color::Green)),
            Span::styled(" [q] Quit ", Style::default().fg(Color::Red)),
        ],
        ActiveTab::Scanner => vec![
            Span::styled(" [1-4] Tabs ", Style::default().fg(Color::Yellow)),
            Span::raw(" [j/k] Select Issue "),
            Span::raw(" [c] Clear "),
            Span::styled(" [?] Help ", Style::default().fg(Color::Green)),
            Span::styled(" [q] Quit ", Style::default().fg(Color::Red)),
        ],
    };

    let status = Paragraph::new(Line::from(status_spans))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(status, chunks[2]);

    // Help Modal Overlay
    if app.show_help {
        render_help_modal(f, area);
    }
}
