use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::app::{App, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let name = match app.tab {
        Tab::Skills => app.filtered_skills().get(app.selected).map(|s| s.name.clone()),
        Tab::Agents => app.filtered_agents().get(app.selected).map(|a| a.name.clone()),
        _ => None,
    };
    let name = name.unwrap_or_default();

    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Delete ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Remove "),
            Span::styled(&name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("?"),
        ]),
        Line::from(""),
        Line::from(
            Span::styled(
                "  [y/Enter] Yes   [Esc/n] No",
                Style::default().fg(Color::DarkGray),
            ),
        ),
    ];

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
