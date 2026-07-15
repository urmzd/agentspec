use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::modal::FleetStatePicker;

pub fn draw(f: &mut Frame, picker: &FleetStatePicker) {
    let area = centered_rect(44, 52, f.area());

    f.render_widget(Clear, area);

    let rows: Vec<Row> = picker
        .states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let marker = if i == picker.selected { ">" } else { " " };
            let style = if i == picker.selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(format!("  {marker} {state}"))]).style(style)
        })
        .collect();

    let table = Table::new(rows, [Constraint::Min(0)]).block(
        Block::default()
            .title(format!(" Mark state: {} / {} ", picker.fleet, picker.agent))
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    let mut state = TableState::default();
    state.select(Some(picker.selected));

    f.render_stateful_widget(table, area, &mut state);

    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let help = Paragraph::new("  [j/k] Move  [Enter] Mark  [Esc] Cancel")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, help_area);
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
