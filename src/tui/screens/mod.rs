use ratatui::prelude::*;
use ratatui::widgets::*;

use super::app::{App, Tab};

mod agent_list;
mod link_picker;
mod skill_list;
mod tool_list;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Length(3), // help bar + filter
            Constraint::Min(0),    // content
            Constraint::Length(3), // status bar
        ])
        .split(f.area());

    draw_tabs(f, chunks[0], app);
    draw_help_bar(f, chunks[1], app);

    match app.tab {
        Tab::Skills => skill_list::draw(f, chunks[2], app),
        Tab::Agents => agent_list::draw(f, chunks[2], app),
        Tab::Tools => tool_list::draw(f, chunks[2], app),
    }

    draw_status_bar(f, chunks[3], app);

    if app.show_link_picker {
        link_picker::draw(f, app);
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .map(|t| {
            let count = match t {
                Tab::Skills => app.skills.len(),
                Tab::Agents => app.agents.len(),
                Tab::Tools => app.tool_entries.len(),
            };
            let style = if *t == app.tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(format!(" {} ({}) ", t.label(), count)).style(style)
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" agentctl ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .select(Tab::all().iter().position(|t| *t == app.tab).unwrap_or(0))
        .highlight_style(Style::default().fg(Color::Cyan));

    f.render_widget(tabs, area);
}

fn draw_help_bar(f: &mut Frame, area: Rect, app: &App) {
    let content = if app.filtering {
        format!("  Filter: {}█", app.filter)
    } else {
        let keys = match app.tab {
            Tab::Skills | Tab::Agents => {
                "[/] Filter  [l] Link  [Tab] Switch  [j/k] Navigate  [q] Quit"
            }
            Tab::Tools => "[Tab] Switch  [j/k] Navigate  [q] Quit",
        };
        format!("  {keys}")
    };

    let bar = Paragraph::new(content)
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    f.render_widget(bar, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let info = match app.tab {
        Tab::Skills => {
            let skills = app.filtered_skills();
            if let Some(s) = skills.get(app.selected) {
                let tools_str = if s.linked_tools.is_empty() {
                    "none".to_string()
                } else {
                    s.linked_tools.join(", ")
                };
                format!("  {} | src: {} | linked: {tools_str}", s.name, s.source)
            } else {
                String::new()
            }
        }
        Tab::Agents => {
            let agents = app.filtered_agents();
            if let Some(a) = agents.get(app.selected) {
                let model = a.model.as_deref().unwrap_or("inherit");
                let tools_str = if a.linked_tools.is_empty() {
                    "none".to_string()
                } else {
                    a.linked_tools.join(", ")
                };
                format!("  {} | model: {model} | linked: {tools_str}", a.name)
            } else {
                String::new()
            }
        }
        Tab::Tools => {
            if let Some(t) = app.tool_entries.get(app.selected) {
                format!(
                    "  {} | {} skills, {} agents",
                    t.slug, t.skill_count, t.agent_count
                )
            } else {
                String::new()
            }
        }
    };

    let bar = Paragraph::new(info).style(Style::default().fg(Color::White).bg(Color::DarkGray));

    f.render_widget(bar, area);
}
