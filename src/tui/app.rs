use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::prelude::*;

use crate::adapters::{self, Adapter};
use crate::config;
use crate::error::Result;
use crate::inventory::{self, TrackedKind};
use crate::ops::memory;
use crate::session;
use crate::tools::{self, CodingTool};

use super::event::poll_event;
use super::screens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Skills,
    Agents,
    Tools,
    Sessions,
    Memories,
    Info,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Skills,
            Tab::Agents,
            Tab::Tools,
            Tab::Sessions,
            Tab::Memories,
            Tab::Info,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            Tab::Skills => "Skills",
            Tab::Agents => "Agents",
            Tab::Tools => "Tools",
            Tab::Sessions => "Sessions",
            Tab::Memories => "Memories",
            Tab::Info => "Info",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Tab::Skills => Tab::Agents,
            Tab::Agents => Tab::Tools,
            Tab::Tools => Tab::Sessions,
            Tab::Sessions => Tab::Memories,
            Tab::Memories => Tab::Info,
            Tab::Info => Tab::Skills,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Tab::Skills => Tab::Info,
            Tab::Agents => Tab::Skills,
            Tab::Tools => Tab::Agents,
            Tab::Sessions => Tab::Tools,
            Tab::Memories => Tab::Sessions,
            Tab::Info => Tab::Memories,
        }
    }
}

// ---------------------------------------------------------------------------
// Entry types
// ---------------------------------------------------------------------------

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

pub struct SessionEntry {
    #[allow(dead_code)] // stored for future session detail view
    pub id: String,
    pub source: String,
    pub date: String,
    pub prompt: String,
}

pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub memory_type: String,
    pub project: String,
}

// ---------------------------------------------------------------------------
// Lazy loading
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub enum LazyTab<T> {
    Unloaded,
    Loaded(Vec<T>),
    Error(String),
}

impl<T> LazyTab<T> {
    pub fn items(&self) -> &[T] {
        match self {
            LazyTab::Loaded(v) => v,
            _ => &[],
        }
    }

    pub fn len(&self) -> usize {
        self.items().len()
    }

    pub fn is_unloaded(&self) -> bool {
        matches!(self, LazyTab::Unloaded)
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub tab: Tab,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub tool_entries: Vec<ToolEntry>,
    pub sessions: LazyTab<SessionEntry>,
    pub memories: LazyTab<MemoryEntry>,
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
            sessions: LazyTab::Unloaded,
            memories: LazyTab::Unloaded,
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

    /// Load data for the current tab if it hasn't been loaded yet.
    fn ensure_tab_loaded(&mut self) {
        match self.tab {
            Tab::Sessions if self.sessions.is_unloaded() => {
                self.sessions = load_sessions();
            }
            Tab::Memories if self.memories.is_unloaded() => {
                self.memories = load_memories();
            }
            _ => {}
        }
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
                self.ensure_tab_loaded();
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.tab = self.tab.prev();
                self.selected = 0;
                self.ensure_tab_loaded();
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
                if matches!(self.tab, Tab::Skills | Tab::Agents) {
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
            _ => None,
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
            _ => None,
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
    }

    pub fn current_list_len(&self) -> usize {
        match self.tab {
            Tab::Skills => self.filtered_skills().len(),
            Tab::Agents => self.filtered_agents().len(),
            Tab::Tools => self.tool_entries.len(),
            Tab::Sessions => self.filtered_sessions().len(),
            Tab::Memories => self.filtered_memories().len(),
            Tab::Info => 0,
        }
    }

    pub fn filtered_skills(&self) -> Vec<&SkillEntry> {
        fuzzy_filter(&self.filter, self.skills.iter(), |s| {
            format!("{} {}", s.name, s.description)
        })
    }

    pub fn filtered_agents(&self) -> Vec<&AgentEntry> {
        fuzzy_filter(&self.filter, self.agents.iter(), |a| {
            format!("{} {}", a.name, a.description)
        })
    }

    pub fn filtered_sessions(&self) -> Vec<&SessionEntry> {
        fuzzy_filter(&self.filter, self.sessions.items().iter(), |s| {
            format!("{} {}", s.source, s.prompt)
        })
    }

    pub fn filtered_memories(&self) -> Vec<&MemoryEntry> {
        fuzzy_filter(&self.filter, self.memories.items().iter(), |m| {
            format!("{} {} {}", m.name, m.memory_type, m.project)
        })
    }
}

/// Fuzzy-filter items by query, returning matches sorted by score (best first).
/// Returns all items in original order when query is empty.
fn fuzzy_filter<'a, T, I, F>(query: &str, items: I, searchable: F) -> Vec<&'a T>
where
    I: Iterator<Item = &'a T>,
    F: Fn(&T) -> String,
{
    if query.is_empty() {
        return items.collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(&T, i64)> = items
        .filter_map(|item| {
            let text = searchable(item);
            matcher.fuzzy_match(&text, query).map(|score| (item, score))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(item, _)| item).collect()
}

// ---------------------------------------------------------------------------
// Data loaders
// ---------------------------------------------------------------------------

fn load_skills(installed: &[Box<dyn CodingTool>]) -> Vec<SkillEntry> {
    let cfg = inventory::load_config().unwrap_or_else(|_| inventory::Config::empty());
    let skills_dir = config::shared_skills_dir();

    let mut entries: Vec<SkillEntry> = cfg
        .resources
        .iter()
        .filter(|r| r.kind == TrackedKind::Skill)
        .map(|tracked| {
            let name = &tracked.name;
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
                source: tracked.source.clone(),
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

fn load_sessions() -> LazyTab<SessionEntry> {
    let mut entries = Vec::new();

    for source_name in &["claude", "codex"] {
        let Ok(src) = session::get_source(source_name) else {
            continue;
        };
        let Ok(sessions) = src.list_sessions() else {
            continue;
        };
        for s in sessions {
            entries.push(SessionEntry {
                id: s.id,
                source: source_name.to_string(),
                date: s
                    .started_at
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                prompt: s.first_prompt.unwrap_or_else(|| "(no prompt)".to_string()),
            });
        }
    }

    entries.sort_by(|a, b| b.date.cmp(&a.date));
    LazyTab::Loaded(entries)
}

fn load_memories() -> LazyTab<MemoryEntry> {
    let mems = memory::scan_memories();
    let entries = mems
        .into_iter()
        .map(|m| MemoryEntry {
            name: m.name,
            description: m.description,
            memory_type: m.memory_type,
            project: m.project_path.unwrap_or(m.project_name),
        })
        .collect();
    LazyTab::Loaded(entries)
}
