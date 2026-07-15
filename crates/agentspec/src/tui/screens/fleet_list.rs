use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if !app.fleets.is_loaded() {
        let msg = Paragraph::new(format!(
            "  {} fleet agent(s) found. Loading...",
            app.fleets.count()
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

    let fleets = app.filtered_fleets();
    let selected = fleets.get(app.selected).copied();
    let show_messages = area.height >= 14;
    let chunks = if show_messages {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Min(7)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    let header = Row::new(vec![
        Cell::from("Backend").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Fleet").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Agent").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Tool").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Session").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Msgs").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Last Message").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = fleets
        .iter()
        .map(|entry| {
            Row::new(vec![
                Cell::from(entry.backend.clone()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(entry.fleet.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(entry.name.clone()),
                Cell::from(entry.tool.clone()).style(Style::default().fg(Color::Green)),
                Cell::from(entry.state.clone()).style(state_style(&entry.state)),
                Cell::from(session_label(entry)),
                Cell::from(entry.message_count.to_string()),
                Cell::from(truncate(&entry.last_message, 70)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(24),
            Constraint::Length(6),
            Constraint::Min(24),
        ],
    )
    .header(header)
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
    if selected.is_some() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(table, chunks[0], &mut state);

    if show_messages {
        draw_message_panel(f, chunks[1], selected, app.fleet_message_scroll);
    }
}

fn state_style(state: &str) -> Style {
    match state {
        "needs-permission" | "stuck" => Style::default().fg(Color::Yellow),
        "error" => Style::default().fg(Color::Red),
        "done" => Style::default().fg(Color::Green),
        "running" => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn session_label(entry: &crate::tui::app::FleetEntry) -> String {
    match (&entry.session_source, &entry.session_id) {
        (Some(source), Some(id)) => format!("{source}:{id}"),
        _ => "-".to_string(),
    }
}

fn draw_message_panel(
    f: &mut Frame,
    area: Rect,
    entry: Option<&crate::tui::app::FleetEntry>,
    scroll_from_bottom: usize,
) {
    let (title, content) = match entry {
        Some(entry) => {
            let (content, applied_scroll) = visible_lines_from_bottom(
                &entry.message_preview,
                area.height.saturating_sub(2) as usize,
                scroll_from_bottom,
            );
            let scroll = if applied_scroll == 0 {
                "tail".to_string()
            } else {
                format!("+{applied_scroll} lines")
            };
            (
                format!(" messages: {} / {} [{scroll}] ", entry.fleet, entry.name),
                content,
            )
        }
        None => (
            " messages ".to_string(),
            "(no fleet agent selected)".to_string(),
        ),
    };

    let paragraph = Paragraph::new(content)
        .style(Style::default())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_style(Style::default().fg(Color::Cyan))
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(paragraph, area);
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

fn visible_lines_from_bottom(
    s: &str,
    max_lines: usize,
    scroll_from_bottom: usize,
) -> (String, usize) {
    if max_lines == 0 {
        return (String::new(), 0);
    }
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        return (s.to_string(), 0);
    }

    let max_scroll = lines.len().saturating_sub(max_lines);
    let applied_scroll = scroll_from_bottom.min(max_scroll);
    let start = lines.len() - max_lines - applied_scroll;
    let end = start + max_lines;
    (lines[start..end].join("\n"), applied_scroll)
}
