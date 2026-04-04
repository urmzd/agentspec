pub mod claude;
pub mod codex;
pub mod find;
pub mod render;

use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Claude,
    Codex,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Claude => write!(f, "claude"),
            Source::Codex => write!(f, "codex"),
        }
    }
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::Claude => "Claude Code",
            Source::Codex => "Codex CLI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolUse { name: String, input: String },
    ToolResult { content: String },
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub source: Source,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub first_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub meta: SessionMeta,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn tools_used(&self) -> Vec<String> {
        let mut tools: Vec<String> = self
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .filter_map(|b| match b {
                ContentBlock::ToolUse { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tools.sort();
        tools
    }
}

pub trait SessionSource {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>>;
    fn load_session(&self, id: &str) -> Result<Session>;
    fn latest_session(&self) -> Result<Session>;
}

pub fn get_source(name: &str) -> Result<Box<dyn SessionSource>> {
    match name {
        "claude" => Ok(Box::new(claude::ClaudeSource)),
        "codex" => Ok(Box::new(codex::CodexSource)),
        _ => Err(AppError::Other(format!(
            "Unknown source: {name}. Supported: claude, codex"
        ))),
    }
}
