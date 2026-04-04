use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::PathBuf;

use super::*;

pub struct ClaudeSource;

fn projects_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Other("No home directory".into()))?;
    let dir = home.join(".claude").join("projects");
    if !dir.exists() {
        return Err(AppError::Other(format!(
            "Claude projects directory not found: {}",
            dir.display()
        )));
    }
    Ok(dir)
}

fn parse_session_file(path: &std::path::Path) -> Result<Session> {
    let content = fs::read_to_string(path)?;
    let mut messages = Vec::new();
    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;

    for line in content.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if msg_type == "file-history-snapshot" {
            continue;
        }

        if cwd.is_none()
            && let Some(c) = v.get("cwd").and_then(|c| c.as_str())
        {
            cwd = Some(c.to_string());
        }

        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| t.parse::<DateTime<Utc>>().ok());

        if started_at.is_none() {
            started_at = timestamp;
        }

        match msg_type {
            "user" => {
                let content_val = &v["message"]["content"];
                let text = extract_text_content(content_val);
                if !text.is_empty() {
                    if v.get("isMeta").and_then(|m| m.as_bool()).unwrap_or(false) {
                        continue;
                    }
                    if first_prompt.is_none() {
                        first_prompt = Some(text.chars().take(100).collect());
                    }
                    messages.push(Message {
                        role: Role::User,
                        content: vec![ContentBlock::Text(text)],
                        timestamp,
                    });
                }
            }
            "assistant" => {
                let blocks = parse_assistant_content(&v["message"]["content"]);
                if !blocks.is_empty() {
                    messages.push(Message {
                        role: Role::Assistant,
                        content: blocks,
                        timestamp,
                    });
                }
            }
            "result" => {
                let content_val = &v["content"];
                let text = if content_val.is_string() {
                    content_val.as_str().unwrap_or("").to_string()
                } else if content_val.is_array() {
                    content_val
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    String::new()
                };
                if !text.is_empty() {
                    messages.push(Message {
                        role: Role::Assistant,
                        content: vec![ContentBlock::ToolResult { content: text }],
                        timestamp,
                    });
                }
            }
            _ => {}
        }
    }

    let id = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Ok(Session {
        meta: SessionMeta {
            id,
            source: Source::Claude,
            cwd,
            started_at,
            first_prompt,
        },
        messages,
    })
}

fn extract_text_content(val: &serde_json::Value) -> String {
    if val.is_string() {
        return val.as_str().unwrap_or("").to_string();
    }
    if let Some(arr) = val.as_array() {
        return arr
            .iter()
            .filter_map(|b| {
                let t = b.get("type")?.as_str()?;
                if t == "text" {
                    b.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    String::new()
}

fn parse_assistant_content(val: &serde_json::Value) -> Vec<ContentBlock> {
    let arr = match val.as_array() {
        Some(a) => a,
        None => return vec![],
    };

    let mut blocks = Vec::new();
    for item in arr {
        let block_type = match item.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };
        match block_type {
            "text" => {
                if let Some(text) = item.get("text").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    blocks.push(ContentBlock::Text(text.to_string()));
                }
            }
            "tool_use" => {
                let name = item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let input = item
                    .get("input")
                    .map(|i| serde_json::to_string_pretty(i).unwrap_or_default())
                    .unwrap_or_default();
                blocks.push(ContentBlock::ToolUse { name, input });
            }
            "tool_result" => {
                let content = item
                    .get("content")
                    .map(|c| {
                        if c.is_string() {
                            c.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string_pretty(c).unwrap_or_default()
                        }
                    })
                    .unwrap_or_default();
                blocks.push(ContentBlock::ToolResult { content });
            }
            _ => {}
        }
    }
    blocks
}

impl SessionSource for ClaudeSource {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        let projects_dir = projects_dir()?;
        let mut sessions = Vec::new();

        for project_entry in fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            if !project_entry.file_type()?.is_dir() {
                continue;
            }
            let project_path = project_entry.path();
            for entry in fs::read_dir(&project_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "jsonl")
                    && let Ok(meta) = quick_parse_meta(&path)
                {
                    sessions.push(meta);
                }
            }
        }

        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(sessions)
    }

    fn load_session(&self, id: &str) -> Result<Session> {
        let projects_dir = projects_dir()?;
        for project_entry in fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            if !project_entry.file_type()?.is_dir() {
                continue;
            }
            let path = project_entry.path().join(format!("{id}.jsonl"));
            if path.exists() {
                return parse_session_file(&path);
            }
        }
        Err(AppError::SessionNotFound(id.to_string()))
    }

    fn latest_session(&self) -> Result<Session> {
        let sessions = self.list_sessions()?;
        let latest = sessions
            .first()
            .ok_or_else(|| AppError::Other("No Claude sessions found".into()))?;
        self.load_session(&latest.id)
    }
}

fn quick_parse_meta(path: &std::path::Path) -> Result<SessionMeta> {
    let id = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let content = fs::read_to_string(path)?;
    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;

    for line in content.lines().take(20) {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if cwd.is_none()
            && let Some(c) = v.get("cwd").and_then(|c| c.as_str())
        {
            cwd = Some(c.to_string());
        }
        if started_at.is_none()
            && let Some(ts) = v.get("timestamp").and_then(|t| t.as_str())
        {
            started_at = ts.parse::<DateTime<Utc>>().ok();
        }
        if msg_type == "user" {
            if v.get("isMeta").and_then(|m| m.as_bool()).unwrap_or(false) {
                continue;
            }
            let text = extract_text_content(&v["message"]["content"]);
            if !text.is_empty() && first_prompt.is_none() {
                first_prompt = Some(text.chars().take(100).collect());
            }
        }

        if cwd.is_some() && started_at.is_some() && first_prompt.is_some() {
            break;
        }
    }

    Ok(SessionMeta {
        id,
        source: Source::Claude,
        cwd,
        started_at,
        first_prompt,
    })
}
