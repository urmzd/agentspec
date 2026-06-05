use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let skills = app.filtered_skills();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Source").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Tools").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = skills
        .iter()
        .map(|s| {
            let dots = tool_dots(&app.installed_tools, &s.linked_tools);

            Row::new(vec![
                Cell::from(s.name.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(truncate(&s.description, 40)),
                Cell::from(truncate(&s.source, 20)).style(Style::default().fg(Color::DarkGray)),
                Cell::from(dots),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Min(30),
            Constraint::Length(22),
            Constraint::Length(25),
        ],
    )
    .header(header)
    // Patch the whole selected row instead of painting a background, so the
    // terminal's own theme shows through.
    .row_highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
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
    let chars: Vec<char> = first_line.chars().collect();
    if chars.len() <= max {
        first_line.to_string()
    } else {
        let truncated: String = chars[..max - 3].iter().collect();
        format!("{truncated}...")
    }
}
