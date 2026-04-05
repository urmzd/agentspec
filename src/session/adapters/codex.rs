use crate::error::{AppError, Result};
use crate::session::adapter::SessionAdapter;
use crate::session::ir::*;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CodexSessionAdapter;

fn sessions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Other("No home directory".into()))?;
    let dir = home.join(".codex").join("sessions");
    if !dir.exists() {
        return Err(AppError::Other(format!(
            "Codex sessions directory not found: {}",
            dir.display()
        )));
    }
    Ok(dir)
}

fn find_all_session_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    // Iterative traversal with depth limit to avoid stack overflow on deep trees.
    let mut stack: Vec<(PathBuf, u32)> = vec![(dir.to_path_buf(), 0)];
    const MAX_DEPTH: u32 = 8;

    while let Some((current, depth)) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() && depth < MAX_DEPTH {
                stack.push((path, depth + 1));
            } else if path.extension().is_some_and(|e| e == "jsonl") {
                files.push(path);
            }
        }
    }

    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

fn extract_session_id_from_filename(path: &Path) -> String {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if let Some(idx) = stem.find("rollout-") {
        let after = &stem[idx + 8..];
        if after.len() > 20 {
            return after[20..].to_string();
        }
    }
    stem
}

fn parse_session_file(path: &Path) -> Result<SessionIR> {
    let content = fs::read_to_string(path)?;
    let mut messages = Vec::new();
    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;
    let mut session_id = extract_session_id_from_filename(path);

    for line in content.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| t.parse::<DateTime<Utc>>().ok());

        match entry_type {
            "session_meta" => {
                let payload = &v["payload"];
                if let Some(id) = payload.get("id").and_then(|i| i.as_str()) {
                    session_id = id.to_string();
                }
                if let Some(c) = payload.get("cwd").and_then(|c| c.as_str()) {
                    cwd = Some(c.to_string());
                }
                if let Some(ts) = payload.get("timestamp").and_then(|t| t.as_str()) {
                    started_at = ts.parse::<DateTime<Utc>>().ok();
                }
            }
            "response_item" => {
                let payload = &v["payload"];
                let item_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match item_type {
                    "message" => {
                        let role_str = payload.get("role").and_then(|r| r.as_str()).unwrap_or("");
                        let role = match role_str {
                            "user" => RoleIR::User,
                            "assistant" => RoleIR::Assistant,
                            "developer" | "system" => RoleIR::System,
                            _ => continue,
                        };

                        if role == RoleIR::System {
                            continue;
                        }

                        let blocks = parse_codex_content(&payload["content"]);
                        if !blocks.is_empty() {
                            if role == RoleIR::User
                                && first_prompt.is_none()
                                && let Some(ContentBlockIR::Text { text: t }) = blocks.first()
                                && !t.starts_with('<')
                            {
                                first_prompt = Some(t.chars().take(100).collect());
                            }
                            if role == RoleIR::User {
                                let all_system = blocks.iter().all(|b| match b {
                                    ContentBlockIR::Text { text: t } => {
                                        let trimmed = t.trim();
                                        trimmed.starts_with('<')
                                            || trimmed.starts_with("# AGENTS.md")
                                            || trimmed.starts_with("# ")
                                                && trimmed.contains("instructions")
                                    }
                                    _ => false,
                                });
                                if all_system {
                                    continue;
                                }
                            }
                            messages.push(MessageIR {
                                role,
                                content: blocks,
                                timestamp,
                            });
                        }
                    }
                    "function_call" => {
                        let name = payload
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let args = payload
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();
                        messages.push(MessageIR {
                            role: RoleIR::Assistant,
                            content: vec![ContentBlockIR::ToolUse {
                                name,
                                input: args,
                                id: None,
                            }],
                            timestamp,
                        });
                    }
                    "function_call_output" => {
                        let output = payload
                            .get("output")
                            .and_then(|o| o.as_str())
                            .unwrap_or("")
                            .to_string();
                        messages.push(MessageIR {
                            role: RoleIR::Assistant,
                            content: vec![ContentBlockIR::ToolResult {
                                content: output,
                                tool_use_id: None,
                                is_error: false,
                            }],
                            timestamp,
                        });
                    }
                    _ => {}
                }
            }
            "event_msg" => {
                let payload = &v["payload"];
                let evt_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if evt_type == "user_message"
                    && let Some(msg) = payload.get("message").and_then(|m| m.as_str())
                    && first_prompt.is_none()
                    && !msg.is_empty()
                {
                    first_prompt = Some(msg.chars().take(100).collect());
                }
            }
            _ => {}
        }
    }

    let mut session = SessionIR {
        id: session_id,
        tool_slug: "codex".to_string(),
        cwd,
        started_at,
        ended_at: None,
        first_prompt,
        summary: None,
        project: None,
        branch: None,
        messages,
        files_touched: Vec::new(),
        tools_used: Vec::new(),
        model: None,
        extensions: Default::default(),
    };
    session.tools_used = session.compute_tools_used();
    Ok(session)
}

fn parse_codex_content(val: &serde_json::Value) -> Vec<ContentBlockIR> {
    let arr = match val.as_array() {
        Some(a) => a,
        None => return vec![],
    };

    let mut blocks = Vec::new();
    for item in arr {
        let block_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "input_text" | "output_text" | "text" => {
                if let Some(text) = item.get("text").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    blocks.push(ContentBlockIR::Text {
                        text: text.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    blocks
}

fn quick_parse_meta(path: &Path) -> Result<SessionMetaIR> {
    let session_id = extract_session_id_from_filename(path);
    let content = fs::read_to_string(path)?;
    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;

    for line in content.lines().take(20) {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if entry_type == "session_meta" {
            let payload = &v["payload"];
            if let Some(c) = payload.get("cwd").and_then(|c| c.as_str()) {
                cwd = Some(c.to_string());
            }
            if let Some(ts) = payload.get("timestamp").and_then(|t| t.as_str()) {
                started_at = ts.parse::<DateTime<Utc>>().ok();
            }
        }

        if entry_type == "event_msg" {
            let payload = &v["payload"];
            let evt_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if evt_type == "user_message"
                && let Some(msg) = payload.get("message").and_then(|m| m.as_str())
                && first_prompt.is_none()
                && !msg.is_empty()
            {
                first_prompt = Some(msg.chars().take(100).collect());
            }
        }

        if cwd.is_some() && started_at.is_some() && first_prompt.is_some() {
            break;
        }
    }

    Ok(SessionMetaIR {
        id: session_id,
        tool_slug: "codex".to_string(),
        cwd,
        started_at,
        first_prompt,
        summary: None,
        project: None,
    })
}

impl SessionAdapter for CodexSessionAdapter {
    fn tool_slug(&self) -> &str {
        "codex"
    }

    fn tool_name(&self) -> &str {
        "Codex CLI"
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|h| vec![h.join(".codex").join("sessions")])
            .unwrap_or_default()
    }

    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>> {
        let dir = sessions_dir()?;
        let files = find_all_session_files(&dir)?;
        let mut sessions = Vec::new();
        for path in files {
            if let Ok(meta) = quick_parse_meta(&path) {
                sessions.push(meta);
            }
        }
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(sessions)
    }

    fn load_session(&self, id: &str) -> Result<SessionIR> {
        let dir = sessions_dir()?;
        let files = find_all_session_files(&dir)?;
        for path in &files {
            let file_id = extract_session_id_from_filename(path);
            if file_id == id {
                return parse_session_file(path);
            }
        }
        Err(AppError::SessionNotFound(id.to_string()))
    }
}
