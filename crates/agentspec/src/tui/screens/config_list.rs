use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if !app.configs.is_loaded() {
        let msg = Paragraph::new(format!(
            "  {} project(s) found. Loading...",
            app.configs.count()
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

    let projects = app.filtered_configs();

    if projects.is_empty() {
        let msg = Paragraph::new("  No projects found.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
        f.render_widget(msg, area);
        return;
    }

    // Gather indicator column names from the first project's indicators
    let indicator_names: Vec<&str> = projects
        .first()
        .map(|p| p.indicators.iter().map(|(name, _)| *name).collect())
        .unwrap_or_default();

    // Build header
    let mut header_cells = vec![
        Cell::from("Project").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Score").style(Style::default().add_modifier(Modifier::BOLD)),
    ];
    for name in &indicator_names {
        header_cells.push(
            Cell::from(abbreviate(name)).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::DarkGray),
            ),
        );
    }
    let header = Row::new(header_cells);

    // Build rows
    let rows: Vec<Row> = projects
        .iter()
        .map(|p| {
            let present = p.indicators.iter().filter(|(_, e)| *e).count();
            let total = p.indicators.len();

            let score_color = if present == total {
                Color::Green
            } else if present >= total / 2 {
                Color::Yellow
            } else {
                Color::Red
            };

            let mut cells = vec![
                Cell::from(truncate(&p.project, 25)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{present}/{total}")).style(Style::default().fg(score_color)),
            ];

            for (_, exists) in &p.indicators {
                let (symbol, color) = if *exists {
                    ("●", Color::Green)
                } else {
                    ("○", Color::DarkGray)
                };
                cells.push(Cell::from(symbol).style(Style::default().fg(color)));
            }

            Row::new(cells)
        })
        .collect();

    // Column constraints: Project (flexible), Score (short), then one per indicator
    let mut widths = vec![Constraint::Length(27), Constraint::Length(7)];
    for name in &indicator_names {
        widths.push(Constraint::Length(abbreviate(name).len() as u16 + 1));
    }

    let table = Table::new(rows, widths)
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

/// Shorten filenames for column headers.
fn abbreviate(name: &str) -> String {
    match name {
        "AGENTS.md" => "AGNTS".to_string(),
        "llms.txt" => "LLMS".to_string(),
        "CLAUDE.md" => "CLAUD".to_string(),
        "GEMINI.md" => "GEMIN".to_string(),
        "copilot-instructions.md" => "COPLT".to_string(),
        "codex-instructions.md" => "CODEX".to_string(),
        "cursorrules" => "CURSR".to_string(),
        "cursor-rules" => "CUR-R".to_string(),
        "clinerules" => "CLINE".to_string(),
        "windsurfrules" => "WNDSR".to_string(),
        other => {
            let s: String = other.chars().take(5).collect();
            s.to_uppercase()
        }
    }
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
