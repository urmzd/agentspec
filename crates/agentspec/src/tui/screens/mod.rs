use ratatui::prelude::*;
use ratatui::widgets::*;

use super::app::{App, Tab};
use super::modal::Modal;

mod agent_list;
mod config_list;
mod delete_confirm;
mod link_picker;
mod memory_list;
mod preview;
mod session_list;
mod skill_list;
mod tool_list;

pub fn draw(f: &mut Frame, app: &App) {
    // Calculate tab row count for dynamic height
    let tab_row_count = compute_tab_rows(f.area().width, app);
    let tab_height = (tab_row_count as u16) + 2; // +2 for borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_height), // tabs (dynamic)
            Constraint::Length(3),          // help bar + filter
            Constraint::Min(0),             // content
            Constraint::Length(3),          // status bar
        ])
        .split(f.area());

    draw_tabs(f, chunks[0], app);
    draw_help_bar(f, chunks[1], app);

    match app.tab {
        Tab::Skills => skill_list::draw(f, chunks[2], app),
        Tab::Agents => agent_list::draw(f, chunks[2], app),
        Tab::Tools => tool_list::draw(f, chunks[2], app),
        Tab::Sessions => session_list::draw(f, chunks[2], app),
        Tab::Memories => memory_list::draw(f, chunks[2], app),
        Tab::Configs => config_list::draw(f, chunks[2], app),
    }

    draw_status_bar(f, chunks[3], app);

    match &app.modal {
        Modal::None => {}
        Modal::DeleteConfirm(dc) => delete_confirm::draw(f, dc),
        Modal::LinkPicker(lp) => link_picker::draw(f, lp),
        Modal::Preview(p) => preview::draw(f, p),
    }
}

/// Calculate how many rows the tab bar needs.
fn compute_tab_rows(terminal_width: u16, app: &App) -> usize {
    let available = terminal_width.saturating_sub(4) as usize;
    let mut row_count = 1;
    let mut row_width = 0;

    for tab in Tab::all() {
        let count = app.tab_count(*tab);
        let label_width = format!(" {} ({}) ", tab.label(), count).len() + 2;

        if row_width > 0 && row_width + label_width > available {
            row_count += 1;
            row_width = 0;
        }
        row_width += label_width;
    }

    row_count
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let all_tabs = Tab::all();

    // Compute tab labels with counts
    let labels: Vec<(String, bool)> = all_tabs
        .iter()
        .map(|t| {
            let count = app.tab_count(*t);
            let label = format!(" {} ({}) ", t.label(), count);
            (*t == app.tab, label)
        })
        .map(|(active, label)| (label, active))
        .collect();

    // Calculate how many tabs fit on one row
    let available_width = area.width.saturating_sub(4) as usize; // borders + padding
    let mut rows: Vec<Vec<(usize, &str, bool)>> = Vec::new();
    let mut current_row: Vec<(usize, &str, bool)> = Vec::new();
    let mut row_width = 0;

    for (idx, (label, active)) in labels.iter().enumerate() {
        let tab_width = label.len() + 2; // separator padding
        if !current_row.is_empty() && row_width + tab_width > available_width {
            rows.push(std::mem::take(&mut current_row));
            row_width = 0;
        }
        current_row.push((idx, label.as_str(), *active));
        row_width += tab_width;
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    let row_count = rows.len().max(1) as u16;
    let tab_height = row_count + 2; // +2 for borders

    // Split the tab area to accommodate multiple rows
    let tab_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: tab_height.min(area.height),
    };

    // Render as a block with manually placed tab text
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" agentspec ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(block, tab_area);

    let inner = Rect {
        x: tab_area.x + 1,
        y: tab_area.y + 1,
        width: tab_area.width.saturating_sub(2),
        height: tab_area.height.saturating_sub(2),
    };

    for (row_idx, row) in rows.iter().enumerate() {
        if row_idx as u16 >= inner.height {
            break;
        }
        let mut spans = Vec::new();
        for (_, label, active) in row {
            let style = if *active {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(*label, style));
            spans.push(Span::raw(" "));
        }
        let line = Line::from(spans);
        let row_area = Rect {
            x: inner.x,
            y: inner.y + row_idx as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(line), row_area);
    }
}

fn draw_help_bar(f: &mut Frame, area: Rect, app: &App) {
    let content = if app.filtering {
        format!("  Filter: {}█", app.filter)
    } else {
        let keys = match app.tab {
            Tab::Skills | Tab::Agents => {
                "[Enter] Preview  [/] Filter  [l] Link  [d] Delete  [Tab] Switch  [j/k] Nav  [q] Quit"
            }
            Tab::Sessions | Tab::Memories | Tab::Configs => {
                "[Enter] Preview  [/] Filter  [Tab] Switch  [j/k] Nav  [q] Quit"
            }
            Tab::Tools => "[Tab] Switch  [j/k] Nav  [q] Quit",
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
    if let Some(msg) = &app.status_message {
        let bar = Paragraph::new(format!("  {msg}"))
            .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
        f.render_widget(bar, area);
        return;
    }

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
        Tab::Sessions => {
            let sessions = app.filtered_sessions();
            if let Some(s) = sessions.get(app.selected) {
                format!(
                    "  {} | {} | {}",
                    s.source,
                    s.date,
                    truncate_str(&s.prompt, 60)
                )
            } else {
                String::new()
            }
        }
        Tab::Memories => {
            let memories = app.filtered_memories();
            if let Some(m) = memories.get(app.selected) {
                format!("  {} | {} | {}", m.name, m.memory_type, m.project)
            } else {
                String::new()
            }
        }
        Tab::Configs => {
            let configs = app.filtered_configs();
            if let Some(c) = configs.get(app.selected) {
                format!("  {} | {} | {}", c.name, c.project, c.path)
            } else {
                String::new()
            }
        }
    };

    let bar = Paragraph::new(info).style(Style::default().fg(Color::White).bg(Color::DarkGray));

    f.render_widget(bar, area);
}

fn truncate_str(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    let chars: Vec<char> = first_line.chars().collect();
    if chars.len() <= max {
        first_line.to_string()
    } else {
        let truncated: String = chars[..max - 3].iter().collect();
        format!("{truncated}...")
    }
}
