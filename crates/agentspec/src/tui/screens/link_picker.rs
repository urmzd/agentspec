use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::modal::LinkPicker;

pub fn draw(f: &mut Frame, lp: &LinkPicker) {
    let area = centered_rect(40, 60, f.area());

    f.render_widget(Clear, area);

    let items: Vec<Row> = lp
        .checks
        .iter()
        .enumerate()
        .map(|(i, (slug, checked))| {
            let marker = if *checked { "[x]" } else { "[ ]" };
            let style = if i == lp.selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(format!("  {marker} {slug}"))]).style(style)
        })
        .collect();

    let table = Table::new(items, [Constraint::Min(0)]).block(
        Block::default()
            .title(format!(" Link '{}' to tools ", lp.name))
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    let mut state = TableState::default();
    state.select(Some(lp.selected));

    f.render_stateful_widget(table, area, &mut state);

    // Help text at bottom of popup
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let help = Paragraph::new("  [j/k] Move  [Space] Toggle  [Enter] Apply  [Esc] Cancel")
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
