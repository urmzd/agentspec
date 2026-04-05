use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Canonical session representation. Vendor-specific formats adapt to/from this IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIR {
    pub id: String,
    /// Open-ended tool slug (e.g. "claude-code", "codex", "gemini-cli").
    pub tool_slug: String,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub first_prompt: Option<String>,
    pub summary: Option<String>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub messages: Vec<MessageIR>,
    pub files_touched: Vec<String>,
    pub tools_used: Vec<String>,
    pub model: Option<String>,
    /// Vendor-specific data preserved for round-tripping.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, serde_json::Value>,
}

impl SessionIR {
    pub fn compute_tools_used(&self) -> Vec<String> {
        let mut tools: Vec<String> = self
            .messages
            .iter()
            .flat_map(|m| &m.content)
            .filter_map(|b| match b {
                ContentBlockIR::ToolUse { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tools.sort();
        tools
    }
}

/// Lightweight session metadata for listing (no messages parsed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetaIR {
    pub id: String,
    pub tool_slug: String,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub first_prompt: Option<String>,
    pub summary: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageIR {
    pub role: RoleIR,
    pub content: Vec<ContentBlockIR>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleIR {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockIR {
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    ToolResult {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_use_id: Option<String>,
        #[serde(default)]
        is_error: bool,
    },
    /// Preserves data from unrecognized formats.
    Unknown {
        raw: serde_json::Value,
    },
}
