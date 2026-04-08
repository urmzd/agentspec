use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::config;
use crate::tools;

pub fn draw(f: &mut Frame, area: Rect, _app: &crate::tui::app::App) {
    let heading =
        |s: &'static str| Line::from(s).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let label_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines = vec![
        heading("Storage"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Skills  ", label_style),
            Span::styled(config::shared_skills_dir().display().to_string(), dim_style),
        ]),
        Line::from(vec![
            Span::styled("  Agents  ", label_style),
            Span::styled(config::shared_agents_dir().display().to_string(), dim_style),
        ]),
        Line::from(vec![
            Span::styled("  Config  ", label_style),
            Span::styled(config::config_path().display().to_string(), dim_style),
        ]),
        Line::from(""),
        heading("Detected Tools"),
        Line::from(""),
    ];

    for tool in tools::all_tools() {
        let installed = tool.is_installed();

        let status = if installed {
            Span::styled("  ● ", Style::default().fg(Color::Green))
        } else {
            Span::styled("  ○ ", dim_style)
        };

        let name_style = if installed {
            Style::default().fg(Color::White)
        } else {
            dim_style
        };
        let name = Span::styled(format!("{:<18}", tool.name()), name_style);

        let detail = if installed {
            let parts: Vec<String> = [
                tool.skills_dir()
                    .map(|d| format!("skills: {}", d.display())),
                tool.agents_dir()
                    .map(|d| format!("agents: {}", d.display())),
            ]
            .into_iter()
            .flatten()
            .collect();
            Span::styled(parts.join("  "), dim_style)
        } else {
            Span::styled("not found", dim_style)
        };

        lines.push(Line::from(vec![status, name, detail]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
