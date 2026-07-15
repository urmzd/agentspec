//! Correlate active fleet panes with known session transcripts.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::ops::fleet::{self, FleetStoreEntry};
use crate::session::{self, ir::SessionMetaIR};

#[derive(Debug, Clone, Serialize)]
pub struct ActiveSession {
    pub backend: String,
    pub fleet: String,
    pub window: String,
    pub pane: String,
    pub agent: String,
    pub tool: String,
    pub state: String,
    pub cwd: Option<String>,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub updated_at: String,
    pub session: Option<MatchedSession>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchedSession {
    pub id: String,
    pub source: String,
    pub tool_slug: String,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub first_prompt: Option<String>,
    pub score: i32,
    pub reason: String,
}

pub fn active_sessions(pane: Option<&str>) -> Result<Vec<ActiveSession>> {
    let sessions = session::discover::discover_all_sessions()?;
    let entries = fleet::active_entries()?;
    let mut active: Vec<ActiveSession> = entries
        .into_iter()
        .filter(|entry| pane.is_none_or(|p| entry.pane == p))
        .map(|entry| {
            let session = best_match(&entry, &sessions);
            ActiveSession {
                backend: entry.backend,
                fleet: entry.fleet,
                window: entry.window,
                pane: entry.pane,
                agent: entry.name,
                tool: entry.tool,
                state: entry.state,
                cwd: entry.cwd,
                message_count: entry.message_count,
                last_message: entry.last_message,
                updated_at: entry.updated_at,
                session,
            }
        })
        .collect();
    active.sort_by(|a, b| {
        a.backend
            .cmp(&b.backend)
            .then_with(|| a.fleet.cmp(&b.fleet))
            .then_with(|| a.agent.cmp(&b.agent))
    });
    Ok(active)
}

pub fn best_for_pane(pane: &str) -> Result<ActiveSession> {
    active_sessions(Some(pane))?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Other(format!("no active fleet pane found: {pane}")))
}

fn best_match(entry: &FleetStoreEntry, sessions: &[SessionMetaIR]) -> Option<MatchedSession> {
    sessions
        .iter()
        .filter_map(|session| {
            score_match(entry, session).map(|(score, reason)| (session, score, reason))
        })
        .max_by(|(a_session, a_score, _), (b_session, b_score, _)| {
            a_score
                .cmp(b_score)
                .then_with(|| a_session.started_at.cmp(&b_session.started_at))
        })
        .map(|(session, score, reason)| MatchedSession {
            id: session.id.clone(),
            source: session.tool_slug.clone(),
            tool_slug: session.tool_slug.clone(),
            cwd: session.cwd.clone(),
            started_at: session.started_at,
            first_prompt: session.first_prompt.clone(),
            score,
            reason,
        })
}

fn score_match(entry: &FleetStoreEntry, session: &SessionMetaIR) -> Option<(i32, String)> {
    let mut score = 0;
    let mut reasons = Vec::new();

    if tool_matches(&entry.tool, &session.tool_slug) {
        score += 50;
        reasons.push("tool".to_string());
    }

    if let (Some(entry_cwd), Some(session_cwd)) = (&entry.cwd, &session.cwd) {
        if same_path(entry_cwd, session_cwd) {
            score += 60;
            reasons.push("cwd-exact".to_string());
        } else if path_related(entry_cwd, session_cwd) {
            score += 30;
            reasons.push("cwd-related".to_string());
        } else {
            return None;
        }
    }

    if score == 0 {
        return None;
    }
    Some((score, reasons.join(",")))
}

fn tool_matches(tool: &str, slug: &str) -> bool {
    let normalized = tool.to_ascii_lowercase();
    matches!(
        (normalized.as_str(), slug),
        ("claude", "claude-code")
            | ("claude-code", "claude-code")
            | ("codex", "codex")
            | ("copilot", "github-copilot")
            | ("github-copilot", "github-copilot")
            | ("gemini", "gemini-cli")
            | ("gemini-cli", "gemini-cli")
    )
}

fn same_path(a: &str, b: &str) -> bool {
    normalize_path(a) == normalize_path(b)
}

fn path_related(a: &str, b: &str) -> bool {
    let a = normalize_path(a);
    let b = normalize_path(b);
    a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

fn normalize_path(path: &str) -> String {
    std::fs::canonicalize(Path::new(path))
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, tool_slug: &str, cwd: &str) -> SessionMetaIR {
        SessionMetaIR {
            id: id.to_string(),
            tool_slug: tool_slug.to_string(),
            cwd: Some(cwd.to_string()),
            started_at: None,
            first_prompt: Some(format!("prompt {id}")),
            summary: None,
            project: None,
        }
    }

    #[test]
    fn best_match_prefers_tool_and_exact_cwd() {
        let entry = FleetStoreEntry {
            backend: "store".into(),
            fleet: "f".into(),
            window: "w".into(),
            name: "api".into(),
            tool: "codex".into(),
            state: "running".into(),
            pane: "store:f:api".into(),
            cwd: Some("/repo".into()),
            message_count: 0,
            last_message: None,
            updated_at: "now".into(),
        };
        let sessions = vec![
            meta("wrong-tool", "claude-code", "/repo"),
            meta("right", "codex", "/repo"),
            meta("related", "codex", "/repo/subdir"),
        ];

        let matched = best_match(&entry, &sessions).unwrap();
        assert_eq!(matched.id, "right");
        assert_eq!(matched.score, 110);
        assert_eq!(matched.reason, "tool,cwd-exact");
    }

    #[test]
    fn best_match_rejects_unrelated_cwd_even_when_tool_matches() {
        let entry = FleetStoreEntry {
            backend: "store".into(),
            fleet: "f".into(),
            window: "w".into(),
            name: "api".into(),
            tool: "codex".into(),
            state: "running".into(),
            pane: "store:f:api".into(),
            cwd: Some("/repo-a".into()),
            message_count: 0,
            last_message: None,
            updated_at: "now".into(),
        };
        let sessions = vec![meta("wrong-repo", "codex", "/repo-b")];

        assert!(best_match(&entry, &sessions).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn best_match_canonicalizes_equivalent_paths() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("repo");
        let alias = temp.path().join("repo-link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();

        let entry = FleetStoreEntry {
            backend: "store".into(),
            fleet: "f".into(),
            window: "w".into(),
            name: "api".into(),
            tool: "codex".into(),
            state: "running".into(),
            pane: "store:f:api".into(),
            cwd: Some(alias.to_string_lossy().to_string()),
            message_count: 0,
            last_message: None,
            updated_at: "now".into(),
        };
        let sessions = vec![meta("same-repo", "codex", &real.to_string_lossy())];

        let matched = best_match(&entry, &sessions).unwrap();
        assert_eq!(matched.id, "same-repo");
        assert_eq!(matched.reason, "tool,cwd-exact");
    }
}
