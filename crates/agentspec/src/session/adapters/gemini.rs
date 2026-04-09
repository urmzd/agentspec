use crate::error::{AppError, Result};
use crate::session::adapter::SessionAdapter;
use crate::session::ir::*;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

pub struct GeminiSessionAdapter;

fn tmp_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Other("No home directory".into()))?;
    let dir = home.join(".gemini").join("tmp");
    if !dir.exists() {
        return Err(AppError::Other(format!(
            "Gemini tmp directory not found: {}",
            dir.display()
        )));
    }
    Ok(dir)
}

/// Find all session JSON files under ~/.gemini/tmp/<project>/chats/session-*.json
fn find_session_files(root: &Path) -> Vec<(PathBuf, String)> {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut files = Vec::new();
    for project_entry in entries.filter_map(|e| e.ok()) {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let project_name = project_entry.file_name().to_string_lossy().to_string();
        let chats_dir = project_dir.join("chats");
        if !chats_dir.exists() {
            continue;
        }
        let chat_entries = match fs::read_dir(&chats_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in chat_entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("session-"))
                && path.extension().is_some_and(|e| e == "json")
            {
                files.push((path, project_name.clone()));
            }
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
}

/// Look up the CWD for a project by reading ~/.gemini/history/<project>/.project_root
fn lookup_cwd(project_name: &str) -> Option<String> {
    let home = dirs::home_dir()?;
    let root_file = home
        .join(".gemini")
        .join("history")
        .join(project_name)
        .join(".project_root");
    fs::read_to_string(root_file)
        .ok()
        .map(|s| s.trim().to_string())
}

fn quick_parse_meta(path: &Path, project_name: &str) -> Result<SessionMetaIR> {
    let content = fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Other(format!("Failed to parse {}: {e}", path.display())))?;

    let session_id = v
        .get("sessionId")
        .and_then(|i| i.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let started_at = v
        .get("startTime")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<DateTime<Utc>>().ok());

    let first_prompt = v
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|msgs| {
            msgs.iter().find_map(|msg| {
                let msg_type = msg.get("type")?.as_str()?;
                if msg_type == "user" {
                    extract_user_text(msg)
                } else {
                    None
                }
            })
        })
        .map(|t| t.chars().take(100).collect());

    let cwd = lookup_cwd(project_name);

    Ok(SessionMetaIR {
        id: session_id,
        tool_slug: "gemini-cli".to_string(),
        cwd,
        started_at,
        first_prompt,
        summary: None,
        project: Some(project_name.to_string()),
    })
}

fn parse_session(path: &Path, project_name: &str) -> Result<SessionIR> {
    let content = fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Other(format!("Failed to parse {}: {e}", path.display())))?;

    let session_id = v
        .get("sessionId")
        .and_then(|i| i.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

    let started_at = v
        .get("startTime")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<DateTime<Utc>>().ok());

    let ended_at = v
        .get("lastUpdated")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<DateTime<Utc>>().ok());

    let cwd = lookup_cwd(project_name);
    let mut first_prompt = None;
    let mut model = None;
    let mut messages = Vec::new();

    if let Some(msgs) = v.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let msg_type = match msg.get("type").and_then(|t| t.as_str()) {
                Some(t) => t,
                None => continue,
            };

            let timestamp = msg
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|t| t.parse::<DateTime<Utc>>().ok());

            match msg_type {
                "user" => {
                    if let Some(text) = extract_user_text(msg) {
                        if first_prompt.is_none() {
                            first_prompt = Some(text.chars().take(100).collect());
                        }
                        messages.push(MessageIR {
                            role: RoleIR::User,
                            content: vec![ContentBlockIR::Text { text }],
                            timestamp,
                        });
                    }
                }
                "gemini" => {
                    if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                        if model.is_none() {
                            model = msg.get("model").and_then(|m| m.as_str()).map(String::from);
                        }
                        if !text.is_empty() {
                            messages.push(MessageIR {
                                role: RoleIR::Assistant,
                                content: vec![ContentBlockIR::Text {
                                    text: text.to_string(),
                                }],
                                timestamp,
                            });
                        }
                    }
                }
                // Skip "info" and other types
                _ => {}
            }
        }
    }

    let mut session = SessionIR {
        id: session_id,
        tool_slug: "gemini-cli".to_string(),
        cwd,
        started_at,
        ended_at,
        first_prompt,
        summary: None,
        project: Some(project_name.to_string()),
        branch: None,
        messages,
        files_touched: Vec::new(),
        tools_used: Vec::new(),
        model,
        extensions: Default::default(),
    };
    session.tools_used = session.compute_tools_used();
    Ok(session)
}

/// Extract text from a user message's content array: [{text: "..."}]
fn extract_user_text(msg: &serde_json::Value) -> Option<String> {
    let content = msg.get("content")?;
    if let Some(arr) = content.as_array() {
        let texts: Vec<&str> = arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
            .collect();
        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    } else {
        content.as_str().map(|text| text.to_string())
    }
}

impl SessionAdapter for GeminiSessionAdapter {
    fn tool_slug(&self) -> &str {
        "gemini-cli"
    }

    fn tool_name(&self) -> &str {
        "Gemini CLI"
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|h| vec![h.join(".gemini").join("tmp")])
            .unwrap_or_default()
    }

    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>> {
        let root = tmp_dir()?;
        let files = find_session_files(&root);
        let mut sessions = Vec::new();
        for (path, project_name) in &files {
            if let Ok(meta) = quick_parse_meta(path, project_name) {
                sessions.push(meta);
            }
        }
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(sessions)
    }

    fn load_session(&self, id: &str) -> Result<SessionIR> {
        let root = tmp_dir()?;
        let files = find_session_files(&root);
        for (path, project_name) in &files {
            if let Ok(meta) = quick_parse_meta(path, project_name)
                && meta.id == id
            {
                return parse_session(path, project_name);
            }
        }
        Err(AppError::SessionNotFound(id.to_string()))
    }
}
