use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The canonical intermediate representation for any agent resource.
/// Vendor-specific formats adapt to/from this IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub kind: ResourceKind,
    pub name: String,
    pub description: String,
    pub body: String,

    /// Canonical fields with well-known semantics.
    /// Adapters map vendor-specific field names to these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disallowed_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_mins: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_invocable: Option<bool>,

    /// Vendor-specific metadata that doesn't map to canonical fields.
    /// Preserved for round-tripping — adapters stash unknown fields here.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_yaml::Value>,

    /// Opaque vendor extensions keyed by vendor slug.
    /// e.g. { "claude-code": { "hooks": {...} }, "gemini-cli": { "kind": "local" } }
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Skill,
    Agent,
    Session,
    Memory,
    ProjectConfig,
    LlmsTxt,
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skill => write!(f, "skill"),
            Self::Agent => write!(f, "agent"),
            Self::Session => write!(f, "session"),
            Self::Memory => write!(f, "memory"),
            Self::ProjectConfig => write!(f, "project-config"),
            Self::LlmsTxt => write!(f, "llms-txt"),
        }
    }
}

impl Resource {
    pub fn new_skill(name: String, description: String, body: String) -> Self {
        Self {
            kind: ResourceKind::Skill,
            name,
            description,
            body,
            tools: None,
            disallowed_tools: None,
            model: None,
            max_turns: None,
            temperature: None,
            timeout_mins: None,
            color: None,
            license: None,
            compatibility: None,
            permission_mode: None,
            background: None,
            effort: None,
            isolation: None,
            initial_prompt: None,
            skills: None,
            user_invocable: None,
            metadata: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn new_agent(name: String, description: String, body: String) -> Self {
        Self {
            kind: ResourceKind::Agent,
            name,
            description,
            body,
            tools: None,
            disallowed_tools: None,
            model: None,
            max_turns: None,
            temperature: None,
            timeout_mins: None,
            color: None,
            license: None,
            compatibility: None,
            permission_mode: None,
            background: None,
            effort: None,
            isolation: None,
            initial_prompt: None,
            skills: None,
            user_invocable: None,
            metadata: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn new_simple(kind: ResourceKind, name: String, description: String, body: String) -> Self {
        Self {
            kind,
            name,
            description,
            body,
            tools: None,
            disallowed_tools: None,
            model: None,
            max_turns: None,
            temperature: None,
            timeout_mins: None,
            color: None,
            license: None,
            compatibility: None,
            permission_mode: None,
            background: None,
            effort: None,
            isolation: None,
            initial_prompt: None,
            skills: None,
            user_invocable: None,
            metadata: HashMap::new(),
            extensions: HashMap::new(),
        }
    }
}
