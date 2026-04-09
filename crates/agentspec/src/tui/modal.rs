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
}

impl Modal {
    /// Delegate a key event to the active modal.
    /// Returns `None` if no modal is active (caller handles the key).
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResult> {
        match self {
            Modal::None => None,
            Modal::DeleteConfirm(dc) => Some(dc.handle_key(key)),
            Modal::LinkPicker(lp) => Some(lp.handle_key(key)),
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

    fn diff_to_actions(&self) -> Vec<Action> {
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
