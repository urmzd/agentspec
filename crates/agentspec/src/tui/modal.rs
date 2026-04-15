use crossterm::event::{KeyCode, KeyEvent};

use crate::ir::ResourceKind;

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
}

// ---------------------------------------------------------------------------
// Modal — the currently active overlay (if any)
// ---------------------------------------------------------------------------

pub enum Modal {
    None,
    DeleteConfirm(DeleteConfirm),
    LinkPicker(LinkPicker),
    Preview(Preview),
}

impl Modal {
    /// Delegate a key event to the active modal.
    /// Returns `None` if no modal is active (caller handles the key).
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResult> {
        match self {
            Modal::None => None,
            Modal::DeleteConfirm(dc) => Some(dc.handle_key(key)),
            Modal::LinkPicker(lp) => Some(lp.handle_key(key)),
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
            KeyCode::Char('y') | KeyCode::Enter => {
                let action = match self.tab {
                    Tab::Skills => Action::DeleteSkill(self.name.clone()),
                    Tab::Agents => Action::DeleteAgent {
                        name: self.name.clone(),
                        source: self.agent_source.clone().unwrap_or(AgentSource::Managed),
                    },
                    _ => return ModalResult::Dismiss,
                };
                ModalResult::Execute(vec![action])
            }
            _ => ModalResult::Dismiss,
        }
    }
}

// ---------------------------------------------------------------------------
// Link picker
// ---------------------------------------------------------------------------

pub struct LinkPicker {
    pub name: String,
    pub kind: ResourceKind,
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
            if *now_checked && !was_checked {
                actions.push(Action::Link {
                    kind: self.kind,
                    name: self.name.clone(),
                    tool: slug.clone(),
                });
            } else if !now_checked && was_checked {
                actions.push(Action::Unlink {
                    kind: self.kind,
                    name: self.name.clone(),
                    tool: slug.clone(),
                });
            }
        }
        actions
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
}

impl Preview {
    pub fn new(title: String, content: String) -> Self {
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        Self {
            title,
            content,
            lines,
            scroll: 0,
            status: None,
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
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
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
