use crate::error::{AppError, Result};
use crate::session::adapter::SessionAdapter;
use crate::session::ir::*;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::PathBuf;

pub struct ClaudeSessionAdapter;

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

fn parse_session_file(path: &std::path::Path) -> Result<SessionIR> {
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
                    messages.push(MessageIR {
                        role: RoleIR::User,
                        content: vec![ContentBlockIR::Text { text }],
                        timestamp,
                    });
                }
            }
            "assistant" => {
                let blocks = parse_assistant_content(&v["message"]["content"]);
                if !blocks.is_empty() {
                    messages.push(MessageIR {
                        role: RoleIR::Assistant,
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
                    messages.push(MessageIR {
                        role: RoleIR::Assistant,
                        content: vec![ContentBlockIR::ToolResult {
                            content: text,
                            tool_use_id: None,
                            is_error: false,
                        }],
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

    let mut session = SessionIR {
        id,
        tool_slug: "claude-code".to_string(),
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

fn parse_assistant_content(val: &serde_json::Value) -> Vec<ContentBlockIR> {
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
                    blocks.push(ContentBlockIR::Text {
                        text: text.to_string(),
                    });
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
                let id = item.get("id").and_then(|i| i.as_str()).map(String::from);
                blocks.push(ContentBlockIR::ToolUse { name, input, id });
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
                let tool_use_id = item
                    .get("tool_use_id")
                    .and_then(|i| i.as_str())
                    .map(String::from);
                let is_error = item
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                blocks.push(ContentBlockIR::ToolResult {
                    content,
                    tool_use_id,
                    is_error,
                });
            }
            _ => {}
        }
    }
    blocks
}

fn quick_parse_meta(path: &std::path::Path) -> Result<SessionMetaIR> {
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

    Ok(SessionMetaIR {
        id,
        tool_slug: "claude-code".to_string(),
        cwd,
        started_at,
        first_prompt,
        summary: None,
        project: None,
    })
}

impl SessionAdapter for ClaudeSessionAdapter {
    fn tool_slug(&self) -> &str {
        "claude-code"
    }

    fn tool_name(&self) -> &str {
        "Claude Code"
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|h| vec![h.join(".claude").join("projects")])
            .unwrap_or_default()
    }

    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>> {
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

    fn load_session(&self, id: &str) -> Result<SessionIR> {
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
}
