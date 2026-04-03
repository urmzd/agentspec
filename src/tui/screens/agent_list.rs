use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let agents = app.filtered_agents();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Tools").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = agents
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let dots = tool_dots(&app.installed_tools, &a.linked_tools);
            let model = a.model.as_deref().unwrap_or("inherit");
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(a.name.clone()).style(Style::default().fg(Color::Magenta)),
                Cell::from(truncate(&a.description, 40)),
                Cell::from(model.to_string()).style(Style::default().fg(Color::Yellow)),
                Cell::from(dots),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(28),
            Constraint::Min(30),
            Constraint::Length(12),
            Constraint::Length(25),
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

fn tool_dots(installed: &[String], linked: &[String]) -> String {
    installed
        .iter()
        .map(|slug| if linked.contains(slug) { "●" } else { "○" })
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() <= max {
        first_line.to_string()
    } else {
        format!("{}...", &first_line[..max - 3])
    }
}
