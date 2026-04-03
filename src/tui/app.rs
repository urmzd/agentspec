use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;

use crate::adapters::{self, Adapter};
use crate::config;
use crate::error::Result;
use crate::lockfile::LockFile;
use crate::tools::{self, CodingTool};

use super::event::poll_event;
use super::screens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Skills,
    Agents,
    Tools,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Skills, Tab::Agents, Tab::Tools]
    }

    pub fn label(&self) -> &str {
        match self {
            Tab::Skills => "Skills",
            Tab::Agents => "Agents",
            Tab::Tools => "Tools",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Tab::Skills => Tab::Agents,
            Tab::Agents => Tab::Tools,
            Tab::Tools => Tab::Skills,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Tab::Skills => Tab::Tools,
            Tab::Agents => Tab::Skills,
            Tab::Tools => Tab::Agents,
        }
    }
}

pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub source: String,
    pub linked_tools: Vec<String>,
}

pub struct AgentEntry {
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    pub linked_tools: Vec<String>,
}

pub struct ToolEntry {
    pub name: String,
    pub slug: String,
    pub installed: bool,
    pub skill_count: usize,
    pub agent_count: usize,
}

pub struct App {
    pub tab: Tab,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub tool_entries: Vec<ToolEntry>,
    pub installed_tools: Vec<String>,
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
    pub should_quit: bool,
    pub show_link_picker: bool,
    pub link_picker_checks: Vec<(String, bool)>,
}

impl App {
    pub fn new() -> Result<Self> {
        let installed = tools::installed_tools();
        let installed_slugs: Vec<String> = installed.iter().map(|t| t.slug().to_string()).collect();

        let skills = load_skills(&installed);
        let agents = load_agents(&installed);
        let tool_entries = load_tool_entries(&installed);

        Ok(Self {
            tab: Tab::Skills,
            skills,
            agents,
            tool_entries,
            installed_tools: installed_slugs,
            selected: 0,
            filter: String::new(),
            filtering: false,
            should_quit: false,
            show_link_picker: false,
            link_picker_checks: Vec::new(),
        })
    }

    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| screens::draw(f, self))?;

            if let Some(Event::Key(key)) = poll_event(Duration::from_millis(100))? {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.show_link_picker {
            self.handle_link_picker_key(key);
            return;
        }

        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter.clear();
                }
                KeyCode::Enter => {
                    self.filtering = false;
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Tab | KeyCode::Right => {
                self.tab = self.tab.next();
                self.selected = 0;
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.tab = self.tab.prev();
                self.selected = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.current_list_len();
                if max > 0 {
                    self.selected = (self.selected + 1).min(max - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('/') => {
                self.filtering = true;
                self.filter.clear();
            }
            KeyCode::Char('l') => {
                if self.tab != Tab::Tools {
                    self.open_link_picker();
                }
            }
            _ => {}
        }
    }

    fn handle_link_picker_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.show_link_picker = false,
            KeyCode::Enter => {
                self.apply_link_picker();
                self.show_link_picker = false;
            }
            KeyCode::Char(' ') => {
                if let Some((_, checked)) = self.link_picker_checks.get_mut(self.selected) {
                    *checked = !*checked;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.link_picker_checks.is_empty() {
                    self.selected = (self.selected + 1).min(self.link_picker_checks.len() - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn open_link_picker(&mut self) {
        let name = match self.tab {
            Tab::Skills => self
                .filtered_skills()
                .get(self.selected)
                .map(|s| s.name.clone()),
            Tab::Agents => self
                .filtered_agents()
                .get(self.selected)
                .map(|a| a.name.clone()),
            Tab::Tools => None,
        };

        let Some(name) = name else { return };

        let linked = match self.tab {
            Tab::Skills => self
                .skills
                .iter()
                .find(|s| s.name == name)
                .map(|s| &s.linked_tools),
            Tab::Agents => self
                .agents
                .iter()
                .find(|a| a.name == name)
                .map(|a| &a.linked_tools),
            Tab::Tools => None,
        };

        let Some(linked) = linked else { return };

        self.link_picker_checks = self
            .installed_tools
            .iter()
            .map(|slug| (slug.clone(), linked.contains(slug)))
            .collect();
        self.selected = 0;
        self.show_link_picker = true;
    }

    fn apply_link_picker(&mut self) {
        // TODO: actually create/remove symlinks based on changed checks
        // For now this is a visual-only feature
    }

    pub fn current_list_len(&self) -> usize {
        match self.tab {
            Tab::Skills => self.filtered_skills().len(),
            Tab::Agents => self.filtered_agents().len(),
            Tab::Tools => self.tool_entries.len(),
        }
    }

    pub fn filtered_skills(&self) -> Vec<&SkillEntry> {
        self.skills
            .iter()
            .filter(|s| {
                self.filter.is_empty()
                    || s.name.contains(&self.filter)
                    || s.description
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .collect()
    }

    pub fn filtered_agents(&self) -> Vec<&AgentEntry> {
        self.agents
            .iter()
            .filter(|a| {
                self.filter.is_empty()
                    || a.name.contains(&self.filter)
                    || a.description
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .collect()
    }
}

fn load_skills(installed: &[Box<dyn CodingTool>]) -> Vec<SkillEntry> {
    let lock = LockFile::load(&config::lock_file_path()).unwrap_or_else(|_| LockFile::empty());
    let skills_dir = config::shared_skills_dir();

    let mut entries: Vec<SkillEntry> = lock
        .skills
        .iter()
        .map(|(name, locked)| {
            let skill_md = skills_dir.join(name).join("SKILL.md");
            let description = if skill_md.exists() {
                adapters::agentskills::AgentSkillsAdapter
                    .parse(&skill_md)
                    .ok()
                    .map(|r| r.description)
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let linked_tools: Vec<String> = installed
                .iter()
                .filter(|t| t.linked_skills().contains(name))
                .map(|t| t.slug().to_string())
                .collect();

            SkillEntry {
                name: name.clone(),
                description,
                source: locked.source.clone(),
                linked_tools,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn load_agents(installed: &[Box<dyn CodingTool>]) -> Vec<AgentEntry> {
    let mut entries = Vec::new();
    let agents_dir = config::shared_agents_dir();

    // Shared agents
    if agents_dir.exists()
        && let Ok(dir) = std::fs::read_dir(&agents_dir)
    {
        for entry in dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let resource = adapters::claude::ClaudeAdapter.parse(&path).ok();

            let linked_tools: Vec<String> = installed
                .iter()
                .filter(|t| t.linked_agents().contains(&name))
                .map(|t| t.slug().to_string())
                .collect();

            entries.push(AgentEntry {
                name,
                description: resource
                    .as_ref()
                    .map(|r| r.description.clone())
                    .unwrap_or_default(),
                model: resource.as_ref().and_then(|r| r.model.clone()),
                linked_tools,
            });
        }
    }

    // Tool-specific agents not in shared store
    for tool in installed {
        if let Some(dir) = tool.agents_dir() {
            if !dir.exists() {
                continue;
            }
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for entry in rd.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    if path.is_symlink() {
                        continue;
                    }
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    if entries.iter().any(|e| e.name == name) {
                        continue;
                    }

                    let resource =
                        adapters::adapter_for_path(&path).and_then(|a| a.parse(&path).ok());

                    entries.push(AgentEntry {
                        name,
                        description: resource
                            .as_ref()
                            .map(|r| r.description.clone())
                            .unwrap_or_default(),
                        model: resource.as_ref().and_then(|r| r.model.clone()),
                        linked_tools: vec![tool.slug().to_string()],
                    });
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn load_tool_entries(_installed: &[Box<dyn CodingTool>]) -> Vec<ToolEntry> {
    tools::all_tools()
        .into_iter()
        .map(|t| ToolEntry {
            name: t.name().to_string(),
            slug: t.slug().to_string(),
            installed: t.is_installed(),
            skill_count: t.linked_skills().len(),
            agent_count: t.linked_agents().len(),
        })
        .collect()
}
