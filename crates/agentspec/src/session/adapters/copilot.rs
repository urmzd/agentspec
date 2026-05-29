use crate::error::{AppError, Result};
use crate::session::adapter::SessionAdapter;
use crate::session::ir::*;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct CopilotSessionAdapter;

/// Path to the Copilot SQLite session store, if present.
fn session_store_db() -> Option<PathBuf> {
    let p = dirs::home_dir()?.join(".copilot").join("session-store.db");
    p.exists().then_some(p)
}

/// Enrich a session loaded from events.jsonl with the richer data in
/// `session-store.db`: summary, repository/branch, files touched, checkpoints,
/// and refs. A missing or locked database is a no-op (events.jsonl stands alone).
fn enrich_from_db(session: &mut SessionIR) {
    let Some(path) = session_store_db() else {
        return;
    };
    let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return;
    };
    let sid = session.id.clone();

    // Session-level metadata.
    let row = conn.query_row(
        "SELECT summary, repository, branch FROM sessions WHERE id = ?1",
        [&sid],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        },
    );
    if let Ok((summary, repo, branch)) = row {
        if session.summary.is_none() {
            session.summary = summary.filter(|s| !s.is_empty());
        }
        if session.project.is_none() {
            session.project = repo.filter(|s| !s.is_empty());
        }
        if session.branch.is_none() {
            session.branch = branch.filter(|s| !s.is_empty());
        }
    }

    // Files touched.
    if let Ok(mut stmt) = conn
        .prepare("SELECT file_path FROM session_files WHERE session_id = ?1 ORDER BY first_seen_at")
    {
        let files: Vec<String> = stmt
            .query_map([&sid], |r| r.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        if !files.is_empty() {
            session.files_touched = files;
        }
    }

    // Checkpoints → extensions["checkpoints"].
    if let Ok(mut stmt) = conn.prepare(
        "SELECT checkpoint_number, title, overview, work_done, technical_details, next_steps \
         FROM checkpoints WHERE session_id = ?1 ORDER BY checkpoint_number",
    ) {
        let checkpoints: Vec<serde_json::Value> = stmt
            .query_map([&sid], |r| {
                Ok(serde_json::json!({
                    "number": r.get::<_, Option<i64>>(0)?,
                    "title": r.get::<_, Option<String>>(1)?,
                    "overview": r.get::<_, Option<String>>(2)?,
                    "work_done": r.get::<_, Option<String>>(3)?,
                    "technical_details": r.get::<_, Option<String>>(4)?,
                    "next_steps": r.get::<_, Option<String>>(5)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        if !checkpoints.is_empty() {
            session
                .extensions
                .insert("checkpoints".into(), serde_json::Value::Array(checkpoints));
        }
    }

    // Refs (issues/PRs/commits) → extensions["refs"].
    if let Ok(mut stmt) =
        conn.prepare("SELECT ref_type, ref_value FROM session_refs WHERE session_id = ?1")
    {
        let refs: Vec<serde_json::Value> = stmt
            .query_map([&sid], |r| {
                Ok(serde_json::json!({
                    "type": r.get::<_, String>(0)?,
                    "value": r.get::<_, String>(1)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        if !refs.is_empty() {
            session
                .extensions
                .insert("refs".into(), serde_json::Value::Array(refs));
        }
    }
}

fn session_state_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Other("No home directory".into()))?;
    let dir = home.join(".copilot").join("session-state");
    if !dir.exists() {
        return Err(AppError::Other(format!(
            "Copilot session-state directory not found: {}",
            dir.display()
        )));
    }
    Ok(dir)
}

/// Find all session directories (each UUID dir contains events.jsonl).
fn find_session_dirs(root: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.path().join("events.jsonl").exists())
        .map(|e| e.path())
        .collect();
    dirs.sort_by(|a, b| b.cmp(a));
    dirs
}

fn events_file(session_dir: &Path) -> PathBuf {
    session_dir.join("events.jsonl")
}

fn quick_parse_meta(session_dir: &Path) -> Result<SessionMetaIR> {
    let id = session_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let path = events_file(session_dir);
    let file = fs::File::open(&path)?;
    let reader = BufReader::new(file);

    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;
    let mut session_id = id;

    for line in reader.lines().take(20).filter_map(|l| l.ok()) {
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "session.start" => {
                let data = &v["data"];
                if let Some(id) = data.get("sessionId").and_then(|i| i.as_str()) {
                    session_id = id.to_string();
                }
                if let Some(ts) = data.get("startTime").and_then(|t| t.as_str()) {
                    started_at = ts.parse::<DateTime<Utc>>().ok();
                }
                if let Some(c) = data
                    .get("context")
                    .and_then(|ctx| ctx.get("cwd"))
                    .and_then(|c| c.as_str())
                {
                    cwd = Some(c.to_string());
                }
            }
            "user.message" => {
                if first_prompt.is_none()
                    && let Some(content) = data_content_text(&v["data"])
                {
                    first_prompt = Some(content.chars().take(100).collect());
                }
            }
            _ => {}
        }

        if started_at.is_some() && first_prompt.is_some() {
            break;
        }
    }

    Ok(SessionMetaIR {
        id: session_id,
        tool_slug: "github-copilot".to_string(),
        cwd,
        started_at,
        first_prompt,
        summary: None,
        project: None,
    })
}

fn parse_session(session_dir: &Path) -> Result<SessionIR> {
    let path = events_file(session_dir);
    let content = fs::read_to_string(&path)?;

    let dir_id = session_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut session_id = dir_id;
    let mut cwd = None;
    let mut started_at = None;
    let mut first_prompt = None;
    let mut model = None;
    let mut branch = None;
    let mut messages = Vec::new();

    for line in content.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let timestamp = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|t| t.parse::<DateTime<Utc>>().ok());

        match event_type {
            "session.start" => {
                let data = &v["data"];
                if let Some(id) = data.get("sessionId").and_then(|i| i.as_str()) {
                    session_id = id.to_string();
                }
                if let Some(ts) = data.get("startTime").and_then(|t| t.as_str()) {
                    started_at = ts.parse::<DateTime<Utc>>().ok();
                }
                if let Some(m) = data.get("selectedModel").and_then(|m| m.as_str()) {
                    model = Some(m.to_string());
                }
                let ctx = &data["context"];
                if let Some(c) = ctx.get("cwd").and_then(|c| c.as_str()) {
                    cwd = Some(c.to_string());
                }
                if let Some(b) = ctx.get("branch").and_then(|b| b.as_str()) {
                    branch = Some(b.to_string());
                }
            }
            "user.message" => {
                if let Some(text) = data_content_text(&v["data"]) {
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
            "assistant.message" => {
                let data = &v["data"];
                let mut blocks = Vec::new();

                if let Some(text) = data.get("content").and_then(|c| c.as_str())
                    && !text.is_empty()
                {
                    blocks.push(ContentBlockIR::Text {
                        text: text.to_string(),
                    });
                }

                if let Some(tool_reqs) = data.get("toolRequests").and_then(|t| t.as_array()) {
                    for req in tool_reqs {
                        let name = req
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let input = req
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();
                        blocks.push(ContentBlockIR::ToolUse {
                            name,
                            input,
                            id: req.get("id").and_then(|i| i.as_str()).map(String::from),
                        });
                    }
                }

                if !blocks.is_empty() {
                    messages.push(MessageIR {
                        role: RoleIR::Assistant,
                        content: blocks,
                        timestamp,
                    });
                }
            }
            _ => {}
        }
    }

    let mut session = SessionIR {
        id: session_id,
        tool_slug: "github-copilot".to_string(),
        cwd,
        started_at,
        ended_at: None,
        first_prompt,
        summary: None,
        project: None,
        branch,
        messages,
        files_touched: Vec::new(),
        tools_used: Vec::new(),
        model,
        extensions: Default::default(),
    };
    session.tools_used = session.compute_tools_used();
    enrich_from_db(&mut session);
    Ok(session)
}

/// Extract the text content from a user.message data payload.
fn data_content_text(data: &serde_json::Value) -> Option<String> {
    // data.content can be a string or data.transformedContent can be used
    if let Some(text) = data.get("content").and_then(|c| c.as_str())
        && !text.is_empty()
    {
        return Some(text.to_string());
    }
    None
}

impl SessionAdapter for CopilotSessionAdapter {
    fn tool_slug(&self) -> &str {
        "github-copilot"
    }

    fn tool_name(&self) -> &str {
        "GitHub Copilot"
    }

    fn session_roots(&self) -> Vec<PathBuf> {
        dirs::home_dir()
            .map(|h| vec![h.join(".copilot").join("session-state")])
            .unwrap_or_default()
    }

    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>> {
        let root = session_state_dir()?;
        let dirs = find_session_dirs(&root);
        let mut sessions = Vec::new();
        for dir in dirs {
            if let Ok(meta) = quick_parse_meta(&dir) {
                sessions.push(meta);
            }
        }
        sessions.sort_by_key(|b| std::cmp::Reverse(b.started_at));
        Ok(sessions)
    }

    fn load_session(&self, id: &str) -> Result<SessionIR> {
        let root = session_state_dir()?;
        // Try direct directory match first (id is the UUID dir name)
        let direct = root.join(id);
        if direct.exists() && events_file(&direct).exists() {
            return parse_session(&direct);
        }
        // Otherwise scan for matching sessionId in events
        for dir in find_session_dirs(&root) {
            if let Ok(meta) = quick_parse_meta(&dir)
                && meta.id == id
            {
                return parse_session(&dir);
            }
        }
        Err(AppError::SessionNotFound(id.to_string()))
    }
}
