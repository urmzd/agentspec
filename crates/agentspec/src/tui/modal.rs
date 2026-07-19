use crossterm::event::{KeyCode, KeyEvent};

use crate::ir::ResourceKind;
use crate::mcp::McpServer;

use super::action::{Action, AgentSource};
use super::app::Tab;

// ---------------------------------------------------------------------------
// Modal result — what the App should do after a modal handles a key
// ---------------------------------------------------------------------------

pub enum ModalResult {
    /// Modal consumed the key, stays open.
    Continue,
    /// Modal dismissed, no action to take.
    Dismiss,
    /// Modal completed, dispatch these actions.
    Execute(Vec<Action>),
    /// Close this modal and open the link picker for the current selection.
    OpenLinkPicker,
    /// Fleet state selected from the state picker.
    FleetStateSelected(String),
    /// Message submitted from the fleet send prompt.
    FleetMessageSubmitted(String),
    /// Guardian event submitted from the fleet event prompt.
    FleetEventSubmitted(String),
    /// Fleet agent spawn submitted from the fleet spawn prompt.
    FleetSpawnSubmitted(Box<FleetSpawnRequest>),
}

// ---------------------------------------------------------------------------
// Modal — the currently active overlay (if any)
// ---------------------------------------------------------------------------

pub enum Modal {
    None,
    DeleteConfirm(DeleteConfirm),
    FleetEventPrompt(FleetEventPrompt),
    FleetSendPrompt(FleetSendPrompt),
    FleetSpawnPrompt(FleetSpawnPrompt),
    FleetStatePicker(FleetStatePicker),
    LinkPicker(LinkPicker),
    McpAddPrompt(McpAddPrompt),
    Preview(Preview),
}

impl Modal {
    /// Delegate a key event to the active modal.
    /// Returns `None` if no modal is active (caller handles the key).
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResult> {
        match self {
            Modal::None => None,
            Modal::DeleteConfirm(dc) => Some(dc.handle_key(key)),
            Modal::FleetEventPrompt(prompt) => Some(prompt.handle_key(key)),
            Modal::FleetSendPrompt(prompt) => Some(prompt.handle_key(key)),
            Modal::FleetSpawnPrompt(prompt) => Some(prompt.handle_key(key)),
            Modal::FleetStatePicker(picker) => Some(picker.handle_key(key)),
            Modal::LinkPicker(lp) => Some(lp.handle_key(key)),
            Modal::McpAddPrompt(prompt) => Some(prompt.handle_key(key)),
            Modal::Preview(p) => Some(p.handle_key(key)),
        }
    }
}

// ---------------------------------------------------------------------------
// Delete confirmation
// ---------------------------------------------------------------------------

pub struct DeleteConfirm {
    pub name: String,
    pub tab: Tab,
    pub agent_source: Option<AgentSource>,
}

impl DeleteConfirm {
    pub fn handle_key(&self, key: KeyEvent) -> ModalResult {
        match key.code {
            // Only an explicit `y` confirms — Enter is reserved for non-destructive
            // actions elsewhere, so muscle memory can't trigger a delete.
            KeyCode::Char('y') => {
                let action = match self.tab {
                    Tab::Skills => Action::DeleteSkill(self.name.clone()),
                    Tab::Agents => Action::DeleteAgent {
                        name: self.name.clone(),
                        source: self.agent_source.clone().unwrap_or(AgentSource::Managed),
                    },
                    Tab::Mcp => Action::DeleteMcpServer(self.name.clone()),
                    _ => return ModalResult::Dismiss,
                };
                ModalResult::Execute(vec![action])
            }
            _ => ModalResult::Dismiss,
        }
    }
}

// ---------------------------------------------------------------------------
// Fleet event prompt
// ---------------------------------------------------------------------------

pub struct FleetEventPrompt {
    pub fleet: String,
    pub input: String,
    pub status: Option<String>,
}

impl FleetEventPrompt {
    pub fn new(fleet: String) -> Self {
        Self {
            fleet,
            input: String::new(),
            status: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => {
                let line = self.input.trim();
                if line.is_empty() {
                    self.status = Some("Paste a guardian event before submitting".to_string());
                    ModalResult::Continue
                } else {
                    ModalResult::FleetEventSubmitted(line.to_string())
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.status = None;
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Fleet send prompt
// ---------------------------------------------------------------------------

pub struct FleetSendPrompt {
    pub fleet: String,
    pub agent: String,
    pub input: String,
    pub status: Option<String>,
}

impl FleetSendPrompt {
    pub fn new(fleet: String, agent: String) -> Self {
        Self {
            fleet,
            agent,
            input: String::new(),
            status: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => {
                let message = self.input.trim();
                if message.is_empty() {
                    self.status = Some("Type a message before sending".to_string());
                    ModalResult::Continue
                } else {
                    ModalResult::FleetMessageSubmitted(message.to_string())
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.status = None;
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Fleet spawn prompt
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetSpawnRequest {
    pub backend: String,
    pub fleet: String,
    pub window: String,
    pub tool: String,
    pub name: String,
    pub dir: Option<String>,
    pub worktree: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub base: Option<String>,
}

pub struct FleetSpawnPrompt {
    pub fields: Vec<(&'static str, String)>,
    pub selected: usize,
    pub status: Option<String>,
}

impl FleetSpawnPrompt {
    pub fn new(backend: Option<String>, fleet: Option<String>, window: Option<String>) -> Self {
        Self {
            fields: vec![
                ("Backend", backend.unwrap_or_else(|| "store".to_string())),
                ("Fleet", fleet.unwrap_or_default()),
                ("Window", window.unwrap_or_else(|| "main".to_string())),
                ("Tool", "codex".to_string()),
                ("Agent", String::new()),
                ("Dir", String::new()),
                ("Worktree", String::new()),
                ("Repo", String::new()),
                ("Branch", String::new()),
                ("Base", String::new()),
            ],
            selected: 0,
            status: None,
        }
    }

    pub fn request(&self) -> Result<FleetSpawnRequest, &'static str> {
        let backend = self.field_value("Backend").trim().to_string();
        let fleet = self.field_value("Fleet").trim().to_string();
        let window = self.field_value("Window").trim().to_string();
        let tool = self.field_value("Tool").trim().to_string();
        let name = self.field_value("Agent").trim().to_string();
        let dir = self.field_value("Dir").trim().to_string();
        let worktree = self.field_value("Worktree").trim().to_string();
        let repo = self.field_value("Repo").trim().to_string();
        let branch = self.field_value("Branch").trim().to_string();
        let base = self.field_value("Base").trim().to_string();

        if !matches!(backend.as_str(), "store" | "tmux" | "auto") {
            return Err("Backend must be store, tmux, or auto");
        }
        if fleet.is_empty() {
            return Err("Fleet is required");
        }
        if window.is_empty() {
            return Err("Window is required");
        }
        if tool.is_empty() {
            return Err("Tool is required");
        }
        if name.is_empty() {
            return Err("Agent is required");
        }
        if !dir.is_empty() && !worktree.is_empty() {
            return Err("Use Dir or Worktree, not both");
        }
        if worktree.is_empty() && (!repo.is_empty() || !branch.is_empty() || !base.is_empty()) {
            return Err("Repo, Branch, and Base require Worktree");
        }

        Ok(FleetSpawnRequest {
            backend,
            fleet,
            window,
            tool,
            name,
            dir: if dir.is_empty() { None } else { Some(dir) },
            worktree: if worktree.is_empty() {
                None
            } else {
                Some(worktree)
            },
            repo: if repo.is_empty() { None } else { Some(repo) },
            branch: if branch.is_empty() {
                None
            } else {
                Some(branch)
            },
            base: if base.is_empty() { None } else { Some(base) },
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => match self.request() {
                Ok(request) => ModalResult::FleetSpawnSubmitted(Box::new(request)),
                Err(message) => {
                    self.status = Some(message.to_string());
                    ModalResult::Continue
                }
            },
            KeyCode::Tab | KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.fields.len().saturating_sub(1));
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Backspace => {
                if let Some((_, value)) = self.fields.get_mut(self.selected) {
                    value.pop();
                }
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char(c) => {
                if let Some((_, value)) = self.fields.get_mut(self.selected) {
                    value.push(c);
                }
                self.status = None;
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }

    fn field_value(&self, name: &str) -> &str {
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

// ---------------------------------------------------------------------------
// Fleet state picker
// ---------------------------------------------------------------------------

pub struct FleetStatePicker {
    pub fleet: String,
    pub agent: String,
    pub states: Vec<&'static str>,
    pub selected: usize,
}

impl FleetStatePicker {
    pub fn new(fleet: String, agent: String, current_state: &str) -> Self {
        let states = vec![
            "running",
            "idle",
            "needs-permission",
            "error",
            "stuck",
            "done",
            "relayed",
        ];
        let selected = states
            .iter()
            .position(|state| *state == current_state)
            .unwrap_or(0);
        Self {
            fleet,
            agent,
            states,
            selected,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => {
                ModalResult::FleetStateSelected(self.states[self.selected].to_string())
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.states.is_empty() {
                    self.selected = (self.selected + 1).min(self.states.len() - 1);
                }
                ModalResult::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Link picker
// ---------------------------------------------------------------------------

/// What a link picker links: a store resource or an MCP server.
#[derive(Clone, Copy)]
pub enum LinkTarget {
    Resource(ResourceKind),
    Mcp,
}

pub struct LinkPicker {
    pub name: String,
    pub target: LinkTarget,
    pub checks: Vec<(String, bool)>,
    pub original: Vec<(String, bool)>,
    pub selected: usize,
}

impl LinkPicker {
    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => {
                let actions = self.diff_to_actions();
                ModalResult::Execute(actions)
            }
            KeyCode::Char(' ') => {
                if let Some((_, checked)) = self.checks.get_mut(self.selected) {
                    *checked = !*checked;
                }
                ModalResult::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.checks.is_empty() {
                    self.selected = (self.selected + 1).min(self.checks.len() - 1);
                }
                ModalResult::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }

    pub fn diff_to_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        for (i, (slug, now_checked)) in self.checks.iter().enumerate() {
            let was_checked = self.original.get(i).map(|(_, c)| *c).unwrap_or(false);
            if *now_checked == was_checked {
                continue;
            }
            let action = match (self.target, *now_checked) {
                (LinkTarget::Resource(kind), true) => Action::Link {
                    kind,
                    name: self.name.clone(),
                    tool: slug.clone(),
                },
                (LinkTarget::Resource(kind), false) => Action::Unlink {
                    kind,
                    name: self.name.clone(),
                    tool: slug.clone(),
                },
                (LinkTarget::Mcp, true) => Action::McpLink {
                    name: self.name.clone(),
                    tool: slug.clone(),
                },
                (LinkTarget::Mcp, false) => Action::McpUnlink {
                    name: self.name.clone(),
                    tool: slug.clone(),
                },
            };
            actions.push(action);
        }
        actions
    }
}

// ---------------------------------------------------------------------------
// MCP add-server prompt
// ---------------------------------------------------------------------------

pub struct McpAddPrompt {
    pub fields: Vec<(&'static str, String)>,
    pub selected: usize,
    pub status: Option<String>,
}

impl McpAddPrompt {
    pub fn new() -> Self {
        Self {
            fields: vec![
                ("Name", String::new()),
                ("Command", String::new()),
                ("Args", String::new()),
                ("Env", String::new()),
                ("URL", String::new()),
                ("Type", String::new()),
            ],
            selected: 0,
            status: None,
        }
    }

    /// Build the server from the form. Mirrors `mcp add` CLI validation.
    pub fn request(&self) -> Result<(String, McpServer), String> {
        let name = self.field_value("Name").trim().to_string();
        let command = self.field_value("Command").trim().to_string();
        let args = self.field_value("Args").trim().to_string();
        let env = self.field_value("Env").trim().to_string();
        let url = self.field_value("URL").trim().to_string();
        let server_type = self.field_value("Type").trim().to_string();

        if name.is_empty() {
            return Err("Name is required".to_string());
        }
        if !server_type.is_empty() && !matches!(server_type.as_str(), "stdio" | "http" | "sse") {
            return Err("Type must be stdio, http, or sse".to_string());
        }
        let mut env_map = std::collections::HashMap::new();
        for pair in env.split_whitespace() {
            match pair.split_once('=') {
                Some((k, v)) if !k.is_empty() => {
                    env_map.insert(k.to_string(), v.to_string());
                }
                _ => return Err(format!("Env must be KEY=VALUE pairs, got: {pair}")),
            }
        }

        let server = McpServer {
            command: if command.is_empty() {
                None
            } else {
                Some(command)
            },
            args: args.split_whitespace().map(str::to_string).collect(),
            env: env_map,
            url: if url.is_empty() { None } else { Some(url) },
            server_type: if server_type.is_empty() {
                None
            } else {
                Some(server_type)
            },
        };
        server.validate().map_err(|e| e.to_string())?;
        Ok((name, server))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc => ModalResult::Dismiss,
            KeyCode::Enter => match self.request() {
                Ok((name, server)) => {
                    ModalResult::Execute(vec![Action::AddMcpServer { name, server }])
                }
                Err(message) => {
                    self.status = Some(message);
                    ModalResult::Continue
                }
            },
            KeyCode::Tab | KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.fields.len().saturating_sub(1));
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Backspace => {
                if let Some((_, value)) = self.fields.get_mut(self.selected) {
                    value.pop();
                }
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char(c) => {
                if let Some((_, value)) = self.fields.get_mut(self.selected) {
                    value.push(c);
                }
                self.status = None;
                ModalResult::Continue
            }
            _ => ModalResult::Continue,
        }
    }

    fn field_value(&self, name: &str) -> &str {
        self.fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

// ---------------------------------------------------------------------------
// Preview — scrollable content viewer with copy and export
// ---------------------------------------------------------------------------

pub struct Preview {
    pub title: String,
    pub content: String,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub status: Option<String>,
    /// Whether the previewed resource can be linked to tools (`l` opens the picker).
    pub linkable: bool,
}

impl Preview {
    pub fn new(title: String, content: String, linkable: bool) -> Self {
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        Self {
            title,
            content,
            lines,
            scroll: 0,
            status: None,
            linkable,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => ModalResult::Dismiss,
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char('G') => {
                self.scroll = self.lines.len().saturating_sub(1);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char('g') => {
                self.scroll = 0;
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::PageDown | KeyCode::Char('d') => {
                self.scroll = self.scroll.saturating_add(20);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::PageUp | KeyCode::Char('u') => {
                self.scroll = self.scroll.saturating_sub(20);
                self.status = None;
                ModalResult::Continue
            }
            KeyCode::Char('c') => {
                self.copy_to_clipboard();
                ModalResult::Continue
            }
            KeyCode::Char('e') => {
                self.export_to_file();
                ModalResult::Continue
            }
            // Link the previewed resource to tools without leaving the flow.
            KeyCode::Char('l') if self.linkable => ModalResult::OpenLinkPicker,
            _ => ModalResult::Continue,
        }
    }

    fn copy_to_clipboard(&mut self) {
        let result = std::process::Command::new(if cfg!(target_os = "macos") {
            "pbcopy"
        } else {
            "xclip"
        })
        .args(if cfg!(target_os = "macos") {
            vec![]
        } else {
            vec!["-selection", "clipboard"]
        })
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(self.content.as_bytes())?;
            }
            child.wait()
        });

        self.status = Some(match result {
            Ok(s) if s.success() => "Copied to clipboard".to_string(),
            _ => "Copy failed (clipboard tool not available)".to_string(),
        });
    }

    fn export_to_file(&mut self) {
        let sanitized: String = self
            .title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let filename = format!("{sanitized}.md");
        match std::fs::write(&filename, &self.content) {
            Ok(()) => {
                self.status = Some(format!("Exported to {filename}"));
            }
            Err(e) => {
                self.status = Some(format!("Export failed: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_spawn_prompt_builds_request_from_fields() {
        let mut prompt = FleetSpawnPrompt::new(
            Some("store".into()),
            Some("work".into()),
            Some("api".into()),
        );
        prompt.fields[3].1 = "codex".into();
        prompt.fields[4].1 = "reviewer".into();
        prompt.fields[5].1 = "/tmp/repo".into();

        let request = prompt.request().unwrap();
        assert_eq!(request.backend, "store");
        assert_eq!(request.fleet, "work");
        assert_eq!(request.window, "api");
        assert_eq!(request.tool, "codex");
        assert_eq!(request.name, "reviewer");
        assert_eq!(request.dir.as_deref(), Some("/tmp/repo"));
        assert_eq!(request.worktree, None);
    }

    #[test]
    fn fleet_spawn_prompt_requires_agent_name() {
        let prompt = FleetSpawnPrompt::new(
            Some("store".into()),
            Some("work".into()),
            Some("api".into()),
        );
        assert_eq!(prompt.request(), Err("Agent is required"));
    }

    #[test]
    fn fleet_spawn_prompt_builds_worktree_request() {
        let mut prompt =
            FleetSpawnPrompt::new(Some("tmux".into()), Some("work".into()), Some("api".into()));
        prompt.fields[4].1 = "reviewer".into();
        prompt.fields[6].1 = "api-review".into();
        prompt.fields[7].1 = "/repo".into();
        prompt.fields[8].1 = "review/api".into();
        prompt.fields[9].1 = "origin/main".into();

        let request = prompt.request().unwrap();
        assert_eq!(request.backend, "tmux");
        assert_eq!(request.worktree.as_deref(), Some("api-review"));
        assert_eq!(request.repo.as_deref(), Some("/repo"));
        assert_eq!(request.branch.as_deref(), Some("review/api"));
        assert_eq!(request.base.as_deref(), Some("origin/main"));
    }

    #[test]
    fn fleet_spawn_prompt_rejects_dir_with_worktree() {
        let mut prompt = FleetSpawnPrompt::new(
            Some("store".into()),
            Some("work".into()),
            Some("api".into()),
        );
        prompt.fields[4].1 = "reviewer".into();
        prompt.fields[5].1 = "/repo".into();
        prompt.fields[6].1 = "api-review".into();

        assert_eq!(prompt.request(), Err("Use Dir or Worktree, not both"));
    }

    #[test]
    fn mcp_add_prompt_builds_stdio_server() {
        let mut prompt = McpAddPrompt::new();
        prompt.fields[0].1 = "sr".into();
        prompt.fields[1].1 = "sr".into();
        prompt.fields[2].1 = "mcp serve".into();
        prompt.fields[3].1 = "API_KEY=x".into();

        let (name, server) = prompt.request().unwrap();
        assert_eq!(name, "sr");
        assert_eq!(server.command.as_deref(), Some("sr"));
        assert_eq!(server.args, vec!["mcp", "serve"]);
        assert_eq!(server.env.get("API_KEY").map(String::as_str), Some("x"));
        assert_eq!(server.url, None);
    }

    #[test]
    fn mcp_add_prompt_requires_name() {
        let mut prompt = McpAddPrompt::new();
        prompt.fields[1].1 = "echo".into();
        assert_eq!(prompt.request().unwrap_err(), "Name is required");
    }

    #[test]
    fn mcp_add_prompt_rejects_command_and_url() {
        let mut prompt = McpAddPrompt::new();
        prompt.fields[0].1 = "x".into();
        prompt.fields[1].1 = "echo".into();
        prompt.fields[4].1 = "https://x".into();
        assert!(prompt.request().is_err());
    }

    #[test]
    fn mcp_add_prompt_rejects_bad_type_and_env() {
        let mut prompt = McpAddPrompt::new();
        prompt.fields[0].1 = "x".into();
        prompt.fields[1].1 = "echo".into();
        prompt.fields[5].1 = "grpc".into();
        assert_eq!(
            prompt.request().unwrap_err(),
            "Type must be stdio, http, or sse"
        );

        prompt.fields[5].1 = String::new();
        prompt.fields[3].1 = "NOEQUALS".into();
        assert!(prompt.request().unwrap_err().starts_with("Env must be"));
    }

    #[test]
    fn fleet_spawn_prompt_rejects_unknown_backend() {
        let mut prompt = FleetSpawnPrompt::new(
            Some("screen".into()),
            Some("work".into()),
            Some("api".into()),
        );
        prompt.fields[4].1 = "reviewer".into();

        assert_eq!(
            prompt.request(),
            Err("Backend must be store, tmux, or auto")
        );
    }
}
