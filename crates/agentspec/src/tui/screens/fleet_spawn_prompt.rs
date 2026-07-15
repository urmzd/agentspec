use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::modal::FleetSpawnPrompt;

pub fn draw(f: &mut Frame, prompt: &FleetSpawnPrompt) {
    let area = centered_rect(72, 58, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Spawn fleet agent ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![Line::from("")];
    for (idx, (label, value)) in prompt.fields.iter().enumerate() {
        let active = idx == prompt.selected;
        let label_style = if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let cursor = if active { "█" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(format!("  {label:>8}: "), label_style),
            Span::raw(value),
            Span::styled(cursor, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  Backend is store, tmux, or auto. Use Dir or Worktree, not both.",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "  Worktree creates/reuses <repo>/.worktrees/<name>; Repo/Branch/Base are optional.",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        prompt
            .status
            .as_deref()
            .unwrap_or("  [Tab/Up/Down] Field  [Enter] Spawn  [Esc] Cancel"),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
