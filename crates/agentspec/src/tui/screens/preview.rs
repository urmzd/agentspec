use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::modal::Preview;

pub fn draw(f: &mut Frame, preview: &Preview) {
    let area = centered_rect(80, 80, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", preview.title))
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Reserve bottom row for help/status line
    let content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };

    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };

    // Render content with scroll
    let visible_height = content_area.height as usize;
    let max_scroll = preview.lines.len().saturating_sub(visible_height);
    let scroll = preview.scroll.min(max_scroll);

    let text: Vec<Line> = preview
        .lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();

    f.render_widget(Paragraph::new(text), content_area);

    // Help/status line — advertise `l` only when the resource can be linked.
    let link_hint = if preview.linkable {
        "[l] Link tools  "
    } else {
        ""
    };
    let help_text = if let Some(status) = &preview.status {
        Line::from(vec![
            Span::styled(
                format!(" {status} "),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {link_hint}[j/k] Scroll  [c] Copy  [e] Export  [q] Close"),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let position = if preview.lines.is_empty() {
            String::new()
        } else {
            let end = (scroll + visible_height).min(preview.lines.len());
            format!(" {}-{}/{} ", scroll + 1, end, preview.lines.len())
        };
        Line::from(vec![
            Span::styled(position, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{link_hint}[j/k] Scroll  [g/G] Top/Bottom  [c] Copy  [e] Export  [q] Close"
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };

    f.render_widget(Paragraph::new(help_text), help_area);
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
