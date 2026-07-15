use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::modal::FleetSendPrompt;

pub fn draw(f: &mut Frame, prompt: &FleetSendPrompt) {
    let area = centered_rect(62, 26, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Send to {} / {} ", prompt.fleet, prompt.agent))
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Message: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&prompt.input),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            prompt
                .status
                .as_deref()
                .unwrap_or("[Enter] Send  [Esc] Cancel"),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
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
