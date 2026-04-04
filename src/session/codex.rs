use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

pub struct CodexSource;

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
    collect_jsonl_files(dir, &mut files)?;
    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

fn collect_jsonl_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, files)?;
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            files.push(path);
        }
    }
    Ok(())
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

fn parse_session_file(path: &Path) -> Result<Session> {
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
                            "user" => Role::User,
                            "assistant" => Role::Assistant,
                            "developer" | "system" => Role::System,
                            _ => continue,
                        };

                        if role == Role::System {
                            continue;
                        }

                        let blocks = parse_codex_content(&payload["content"]);
                        if !blocks.is_empty() {
                            if role == Role::User
                                && first_prompt.is_none()
                                && let Some(ContentBlock::Text(t)) = blocks.first()
                                && !t.starts_with('<')
                            {
                                first_prompt = Some(t.chars().take(100).collect());
                            }
                            if role == Role::User {
                                let all_system = blocks.iter().all(|b| match b {
                                    ContentBlock::Text(t) => {
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
                            messages.push(Message {
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
                        messages.push(Message {
                            role: Role::Assistant,
                            content: vec![ContentBlock::ToolUse { name, input: args }],
                            timestamp,
                        });
                    }
                    "function_call_output" => {
                        let output = payload
                            .get("output")
                            .and_then(|o| o.as_str())
                            .unwrap_or("")
                            .to_string();
                        messages.push(Message {
                            role: Role::Assistant,
                            content: vec![ContentBlock::ToolResult { content: output }],
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

    Ok(Session {
        meta: SessionMeta {
            id: session_id,
            source: Source::Codex,
            cwd,
            started_at,
            first_prompt,
        },
        messages,
    })
}

fn parse_codex_content(val: &serde_json::Value) -> Vec<ContentBlock> {
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
                    blocks.push(ContentBlock::Text(text.to_string()));
                }
            }
            _ => {}
        }
    }
    blocks
}

fn quick_parse_meta(path: &Path) -> Result<SessionMeta> {
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

    Ok(SessionMeta {
        id: session_id,
        source: Source::Codex,
        cwd,
        started_at,
        first_prompt,
    })
}

impl SessionSource for CodexSource {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
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

    fn load_session(&self, id: &str) -> Result<Session> {
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

    fn latest_session(&self) -> Result<Session> {
        let dir = sessions_dir()?;
        let files = find_all_session_files(&dir)?;
        let path = files
            .first()
            .ok_or_else(|| AppError::Other("No Codex sessions found".into()))?;
        parse_session_file(path)
    }
}
