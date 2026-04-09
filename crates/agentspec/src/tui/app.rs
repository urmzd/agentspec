use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::prelude::*;

use crate::adapters::{self, Adapter};
use crate::config;
use crate::error::Result;
use crate::inventory::{self, TrackedKind};
use crate::ir::ResourceKind;
use crate::ops::memory;
use crate::session;
use crate::tools::{self, CodingTool};

use super::action::{AgentSource, ReloadTarget};
use super::event::poll_event;
use super::modal::{DeleteConfirm, LinkPicker, Modal, ModalResult};
use super::screens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Skills,
    Agents,
    Tools,
    Sessions,
    Memories,
    Configs,
    Info,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Info,
            Tab::Skills,
            Tab::Agents,
            Tab::Tools,
            Tab::Sessions,
            Tab::Memories,
            Tab::Configs,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            Tab::Info => "Info",
            Tab::Skills => "Skills",
            Tab::Agents => "Agents",
            Tab::Tools => "Tools",
            Tab::Sessions => "Sessions",
            Tab::Memories => "Memories",
            Tab::Configs => "Configs",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Tab::Info => Tab::Skills,
            Tab::Skills => Tab::Agents,
            Tab::Agents => Tab::Tools,
            Tab::Tools => Tab::Sessions,
            Tab::Sessions => Tab::Memories,
            Tab::Memories => Tab::Configs,
            Tab::Configs => Tab::Info,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Tab::Info => Tab::Configs,
            Tab::Skills => Tab::Info,
            Tab::Agents => Tab::Skills,
            Tab::Tools => Tab::Agents,
            Tab::Sessions => Tab::Tools,
            Tab::Memories => Tab::Sessions,
            Tab::Configs => Tab::Memories,
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
    pub source: AgentSource,
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

pub struct ConfigEntry {
    pub name: String,
    pub kind: String,
    pub project: String,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Lazy loading with eager counts
// ---------------------------------------------------------------------------

pub enum LazyTab<T> {
    /// Count known but data not yet loaded
    CountOnly(usize),
    /// Fully loaded
    Loaded(Vec<T>),
}

impl<T> LazyTab<T> {
    pub fn items(&self) -> &[T] {
        match self {
            LazyTab::Loaded(v) => v,
            _ => &[],
        }
    }

    pub fn count(&self) -> usize {
        match self {
            LazyTab::CountOnly(n) => *n,
            LazyTab::Loaded(v) => v.len(),
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, LazyTab::Loaded(_))
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub tab: Tab,
    pub cfg: inventory::Config,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub tool_entries: Vec<ToolEntry>,
    pub sessions: LazyTab<SessionEntry>,
    pub memories: LazyTab<MemoryEntry>,
    pub configs: LazyTab<ConfigEntry>,
    pub installed_tools: Vec<String>,
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
    pub should_quit: bool,
    pub modal: Modal,
    pub status_message: Option<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let cfg = inventory::load_config()?;
        let installed = tools::installed_tools();
        let installed_slugs: Vec<String> = installed.iter().map(|t| t.slug().to_string()).collect();

        let skills = load_skills(&installed);
        let agents = load_agents(&installed);
        let tool_entries = load_tool_entries(&installed);

        // Eager lightweight counts — full data loads on tab select
        let session_count = count_sessions();
        let memory_count = memory::scan_memories().len();
        let config_count = count_configs();

        Ok(Self {
            tab: Tab::Info,
            cfg,
            skills,
            agents,
            tool_entries,
            sessions: LazyTab::CountOnly(session_count),
            memories: LazyTab::CountOnly(memory_count),
            configs: LazyTab::CountOnly(config_count),
            installed_tools: installed_slugs,
            selected: 0,
            filter: String::new(),
            filtering: false,
            should_quit: false,
            modal: Modal::None,
            status_message: None,
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

    /// Load full data for the current tab if only counts are loaded.
    fn ensure_tab_loaded(&mut self) {
        match self.tab {
            Tab::Sessions if !self.sessions.is_loaded() => {
                self.sessions = LazyTab::Loaded(load_sessions());
            }
            Tab::Memories if !self.memories.is_loaded() => {
                self.memories = LazyTab::Loaded(load_memories());
            }
            Tab::Configs if !self.configs.is_loaded() => {
                self.configs = LazyTab::Loaded(load_configs());
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Key handling
    // -----------------------------------------------------------------------

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Clear transient status on any keypress
        self.status_message = None;

        // 1. Delegate to modal if active
        if let Some(result) = self.modal.handle_key(key) {
            match result {
                ModalResult::Continue => {}
                ModalResult::Dismiss => self.modal = Modal::None,
                ModalResult::Execute(actions) => {
                    self.modal = Modal::None;
                    self.dispatch_all(actions);
                }
            }
            return;
        }

        // 2. Filter mode
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

        // 3. Main navigation / modal-opening keys
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
            KeyCode::Char('d') => {
                if matches!(self.tab, Tab::Skills | Tab::Agents) && self.current_list_len() > 0 {
                    self.open_delete_confirm();
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch — execute actions and react to results
    // -----------------------------------------------------------------------

    fn dispatch_all(&mut self, actions: Vec<super::action::Action>) {
        for action in &actions {
            match action.execute(&mut self.cfg) {
                Ok(()) => {
                    self.status_message = Some(action.success_message());
                    self.reload(action.reload_target());
                    self.clamp_selection();
                }
                Err(e) => {
                    self.status_message = Some(format!("Error: {e}"));
                    break;
                }
            }
        }
        // Persist config changes made by actions
        if let Err(e) = inventory::save_config(&self.cfg) {
            self.status_message = Some(format!("Error saving config: {e}"));
        }
    }

    fn reload(&mut self, target: ReloadTarget) {
        match target {
            ReloadTarget::SkillsAndAgents => {
                let installed = tools::installed_tools();
                self.installed_tools =
                    installed.iter().map(|t| t.slug().to_string()).collect();
                self.skills = load_skills(&installed);
                self.agents = load_agents(&installed);
                self.tool_entries = load_tool_entries(&installed);
            }
        }
    }

    fn clamp_selection(&mut self) {
        let max = self.current_list_len();
        if max > 0 {
            self.selected = self.selected.min(max - 1);
        } else {
            self.selected = 0;
        }
    }

    // -----------------------------------------------------------------------
    // Modal openers
    // -----------------------------------------------------------------------

    fn open_delete_confirm(&mut self) {
        let (name, agent_source) = match self.tab {
            Tab::Skills => {
                let filtered = self.filtered_skills();
                let name = filtered.get(self.selected).map(|s| s.name.clone());
                (name, None)
            }
            Tab::Agents => {
                let filtered = self.filtered_agents();
                match filtered.get(self.selected) {
                    Some(a) => (Some(a.name.clone()), Some(a.source.clone())),
                    None => (None, None),
                }
            }
            _ => return,
        };

        let Some(name) = name else { return };

        self.modal = Modal::DeleteConfirm(DeleteConfirm {
            name,
            tab: self.tab,
            agent_source,
        });
    }

    fn open_link_picker(&mut self) {
        let (name, linked) = match self.tab {
            Tab::Skills => {
                let filtered = self.filtered_skills();
                match filtered.get(self.selected) {
                    Some(s) => (s.name.clone(), s.linked_tools.clone()),
                    None => return,
                }
            }
            Tab::Agents => {
                let filtered = self.filtered_agents();
                match filtered.get(self.selected) {
                    Some(a) => (a.name.clone(), a.linked_tools.clone()),
                    None => return,
                }
            }
            _ => return,
        };

        let kind = match self.tab {
            Tab::Skills => ResourceKind::Skill,
            Tab::Agents => ResourceKind::Agent,
            _ => return,
        };

        let checks: Vec<(String, bool)> = self
            .installed_tools
            .iter()
            .map(|slug| (slug.clone(), linked.contains(slug)))
            .collect();

        self.modal = Modal::LinkPicker(LinkPicker {
            name,
            kind,
            original: checks.clone(),
            checks,
            selected: 0,
        });
    }

    // -----------------------------------------------------------------------
    // List helpers
    // -----------------------------------------------------------------------

    pub fn current_list_len(&self) -> usize {
        match self.tab {
            Tab::Skills => self.filtered_skills().len(),
            Tab::Agents => self.filtered_agents().len(),
            Tab::Tools => self.tool_entries.len(),
            Tab::Sessions => self.filtered_sessions().len(),
            Tab::Memories => self.filtered_memories().len(),
            Tab::Configs => self.filtered_configs().len(),
            Tab::Info => 0,
        }
    }

    /// Total count for a tab (works even before full data is loaded).
    pub fn tab_count(&self, tab: Tab) -> usize {
        match tab {
            Tab::Skills => self.skills.len(),
            Tab::Agents => self.agents.len(),
            Tab::Tools => self.tool_entries.len(),
            Tab::Sessions => self.sessions.count(),
            Tab::Memories => self.memories.count(),
            Tab::Configs => self.configs.count(),
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

    pub fn filtered_configs(&self) -> Vec<&ConfigEntry> {
        fuzzy_filter(&self.filter, self.configs.items().iter(), |c| {
            format!("{} {} {}", c.name, c.kind, c.project)
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

    // Pass 1: managed agents from shared store
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
                source: AgentSource::Managed,
            });
        }
    }

    // Pass 2: unmanaged agents from tool-specific directories
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
                    // Symlinks point to the shared store — already counted in Pass 1
                    if path.is_symlink() {
                        continue;
                    }
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();

                    // Merge paths if same-name unmanaged agent exists across tools
                    if let Some(existing) = entries.iter_mut().find(|e| e.name == name) {
                        if let AgentSource::Unmanaged(ref mut paths) = existing.source {
                            paths.push(path.clone());
                        }
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
                        source: AgentSource::Unmanaged(vec![path.clone()]),
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

/// Lightweight session count — avoids loading full session data.
fn count_sessions() -> usize {
    session::adapters::available_session_adapters()
        .iter()
        .filter_map(|a| a.list_sessions().ok())
        .map(|s| s.len())
        .sum()
}

/// Lightweight config count — just count files, don't parse.
fn count_configs() -> usize {
    use crate::project_files;
    memory::scan_project_infos()
        .iter()
        .filter_map(|p| p.project_path.as_ref())
        .filter(|pp| pp.join(".git").exists())
        .map(|pp| project_files::find_in_project(pp).len())
        .sum()
}

fn load_sessions() -> Vec<SessionEntry> {
    let all = match session::discover::discover_all_sessions() {
        Ok(sessions) => sessions,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<SessionEntry> = all
        .into_iter()
        .map(|s| {
            let source_name = session::adapters::tool_name_for_slug(&s.tool_slug);
            SessionEntry {
                id: s.id,
                source: source_name,
                date: s
                    .started_at
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                prompt: s.first_prompt.unwrap_or_else(|| "(no prompt)".to_string()),
            }
        })
        .collect();

    entries.sort_by(|a, b| b.date.cmp(&a.date));
    entries
}

fn load_memories() -> Vec<MemoryEntry> {
    memory::scan_memories()
        .into_iter()
        .map(|m| MemoryEntry {
            name: m.name,
            description: m.description,
            memory_type: m.memory_type,
            project: m.project_path.unwrap_or(m.project_name),
        })
        .collect()
}

fn load_configs() -> Vec<ConfigEntry> {
    use crate::project_files;

    let infos = memory::scan_project_infos();
    let mut entries = Vec::new();

    for p in &infos {
        let Some(ref pp) = p.project_path else {
            continue;
        };
        if !pp.join(".git").exists() {
            continue;
        }
        let project = pp
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.encoded_name.clone());

        for (spec, file_path) in project_files::find_in_project(pp) {
            entries.push(ConfigEntry {
                name: spec.filename.to_string(),
                kind: format!("{}", spec.kind),
                project: project.clone(),
                path: file_path.to_string_lossy().to_string(),
            });
        }
    }

    entries.sort_by(|a, b| a.project.cmp(&b.project).then(a.name.cmp(&b.name)));
    entries
}
