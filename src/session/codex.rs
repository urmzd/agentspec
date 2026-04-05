use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::io::{BufRead, BufReader};
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

/// Walk the known `year/month/day/` hierarchy to collect .jsonl files.
fn find_all_session_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // year dirs
    let years = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Ok(files),
    };
    for year in years.filter_map(|e| e.ok()) {
        if !year.path().is_dir() {
            continue;
        }
        // month dirs
        let months = match fs::read_dir(year.path()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for month in months.filter_map(|e| e.ok()) {
            if !month.path().is_dir() {
                continue;
            }
            // day dirs
            let days = match fs::read_dir(month.path()) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for day in days.filter_map(|e| e.ok()) {
                if !day.path().is_dir() {
                    continue;
                }
                // .jsonl files in day dir
                let entries = match fs::read_dir(day.path()) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        files.push(path);
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| b.cmp(a));
    Ok(files)
}

/// Extract session ID (UUID) from filename like `rollout-2025-11-24T06-05-13-{uuid}.jsonl`.
fn extract_session_id(path: &Path) -> String {
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

/// Parse timestamp from filename like `rollout-2025-11-24T06-05-13-{uuid}.jsonl`.
fn extract_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let after = &stem[stem.find("rollout-")? + 8..];
    if after.len() < 19 {
        return None;
    }
    let raw = &after[..19]; // "2025-11-24T06-05-13"
    let formatted = format!(
        "{}-{}-{}T{}:{}:{}Z",
        &raw[0..4],
        &raw[5..7],
        &raw[8..10],
        &raw[11..13],
        &raw[14..16],
        &raw[17..19]
    );
    formatted.parse::<DateTime<Utc>>().ok()
}

/// Build metadata from filename alone — no file I/O.
fn meta_from_filename(path: &Path) -> SessionMeta {
    SessionMeta {
        id: extract_session_id(path),
        source: Source::Codex,
        cwd: None,
        started_at: extract_timestamp(path),
        first_prompt: None,
    }
}

/// Enrich metadata by reading the first few lines for cwd and first_prompt.
fn enrich_meta(path: &Path, mut meta: SessionMeta) -> SessionMeta {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return meta,
    };
    let reader = BufReader::new(file);

    for line in reader.lines().take(20).filter_map(|l| l.ok()) {
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if entry_type == "session_meta" {
            let payload = &v["payload"];
            if meta.cwd.is_none()
                && let Some(c) = payload.get("cwd").and_then(|c| c.as_str())
            {
                meta.cwd = Some(c.to_string());
            }
        }

        if entry_type == "event_msg" {
            let payload = &v["payload"];
            let evt_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if evt_type == "user_message"
                && meta.first_prompt.is_none()
                && let Some(msg) = payload.get("message").and_then(|m| m.as_str())
                && !msg.is_empty()
            {
                meta.first_prompt = Some(msg.chars().take(100).collect());
            }
        }

        if meta.cwd.is_some() && meta.first_prompt.is_some() {
            break;
        }
    }

    meta
}

fn parse_session_file(path: &Path) -> Result<Session> {
    let content = fs::read_to_string(path)?;
    let mut messages = Vec::new();
    let mut cwd = None;
    let mut started_at = extract_timestamp(path);
    let mut first_prompt = None;
    let session_id = extract_session_id(path);

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
                if let Some(c) = payload.get("cwd").and_then(|c| c.as_str()) {
                    cwd = Some(c.to_string());
                }
                if started_at.is_none()
                    && let Some(ts) = payload.get("timestamp").and_then(|t| t.as_str())
                {
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

impl SessionSource for CodexSource {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        let dir = sessions_dir()?;
        let files = find_all_session_files(&dir)?;
        let mut sessions: Vec<SessionMeta> = files
            .iter()
            .map(|p| enrich_meta(p, meta_from_filename(p)))
            .collect();
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(sessions)
    }

    fn load_session(&self, id: &str) -> Result<Session> {
        let dir = sessions_dir()?;
        let files = find_all_session_files(&dir)?;
        for path in &files {
            if extract_session_id(path) == id {
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
