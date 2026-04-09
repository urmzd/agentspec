use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if !app.sessions.is_loaded() {
        let msg = Paragraph::new(format!(
            "  {} session(s) found. Loading...",
            app.sessions.count()
        ))
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(msg, area);
        return;
    }

    let sessions = app.filtered_sessions();

    let header = Row::new(vec![
        Cell::from("Source").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Date").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Prompt").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(s.source.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(s.date.clone()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(truncate(&s.prompt, 60)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(18),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    let mut state = TableState::default();
    state.select(Some(app.selected));

    f.render_stateful_widget(table, area, &mut state);
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    let chars: Vec<char> = first_line.chars().collect();
    if chars.len() <= max {
        first_line.to_string()
    } else {
        let truncated: String = chars[..max - 3].iter().collect();
        format!("{truncated}...")
    }
}
