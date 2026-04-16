use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::config;
use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let dim_style = Style::default().fg(Color::DarkGray);
    let label_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    // Storage info line
    let storage_line = Line::from(vec![
        Span::styled("  Skills ", label_style),
        Span::styled(config::shared_skills_dir().display().to_string(), dim_style),
        Span::raw("  "),
        Span::styled("Agents ", label_style),
        Span::styled(config::shared_agents_dir().display().to_string(), dim_style),
        Span::raw("  "),
        Span::styled("Config ", label_style),
        Span::styled(config::config_path().display().to_string(), dim_style),
    ]);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // storage info
            Constraint::Min(0),   // tool table
        ])
        .split(area);

    f.render_widget(Paragraph::new(storage_line), chunks[0]);

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Slug").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Skills").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Agents").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .tool_entries
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let status = if t.installed {
                "installed"
            } else {
                "not found"
            };
            let status_style = if t.installed {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(t.name.clone()),
                Cell::from(t.slug.clone()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(status).style(status_style),
                Cell::from(t.skill_count.to_string()),
                Cell::from(t.agent_count.to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(8),
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

    f.render_stateful_widget(table, chunks[1], &mut state);
}
