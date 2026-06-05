use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if !app.memories.is_loaded() {
        let msg = Paragraph::new(format!(
            "  {} memory file(s) found. Loading...",
            app.memories.count()
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

    let memories = app.filtered_memories();

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Project").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = memories
        .iter()
        .map(|m| {
            Row::new(vec![
                Cell::from(m.name.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(m.memory_type.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(truncate(&m.project, 25)).style(Style::default().fg(Color::DarkGray)),
                Cell::from(truncate(&m.description, 40)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(25),
            Constraint::Length(12),
            Constraint::Length(27),
            Constraint::Min(30),
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
