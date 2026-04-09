use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::action::AgentSource;
use crate::tui::app::Tab;
use crate::tui::modal::DeleteConfirm;

pub fn draw(f: &mut Frame, dc: &DeleteConfirm) {
    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Delete ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Remove "),
            Span::styled(
                &dc.name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
    ];

    // Show file paths for unmanaged agents
    if dc.tab == Tab::Agents
        && let Some(AgentSource::Unmanaged(paths)) = &dc.agent_source
    {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "  Files to delete:",
            Style::default().fg(Color::DarkGray),
        )));
        for path in paths {
            text.push(Line::from(Span::styled(
                format!("    {}", path.display()),
                Style::default().fg(Color::Red),
            )));
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "  [y/Enter] Yes   [Esc/n] No",
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(Paragraph::new(text), inner);
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
