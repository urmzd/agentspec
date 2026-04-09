use std::path::Path;

use crate::error::Result;
use crate::frontmatter;
use crate::ir::Resource;

use super::Adapter;

/// Adapter for Claude Code agent definition format.
/// Frontmatter: name, description, tools, model, maxTurns, color, etc.
pub struct ClaudeAdapter;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawClaudeFm {
    name: String,
    description: String,
    #[serde(default)]
    tools: Option<StringOrVec>,
    disallowed_tools: Option<Vec<String>>,
    model: Option<String>,
    max_turns: Option<u32>,
    color: Option<String>,
    permission_mode: Option<String>,
    skills: Option<Vec<String>>,
    background: Option<bool>,
    effort: Option<String>,
    isolation: Option<String>,
    initial_prompt: Option<String>,
    #[serde(default)]
    hooks: Option<serde_yaml::Value>,
    #[serde(default)]
    mcp_servers: Option<serde_yaml::Value>,
    #[serde(default)]
    memory: Option<serde_yaml::Value>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    String(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::String(s) => s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
            Self::Vec(v) => v,
        }
    }
}

impl Adapter for ClaudeAdapter {
    fn vendor(&self) -> &str {
        "claude-code"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: RawClaudeFm = serde_yaml::from_str(&parsed.frontmatter)?;

        let mut r = Resource::new_agent(fm.name, fm.description, parsed.body);
        r.tools = fm.tools.map(|t| t.into_vec());
        r.disallowed_tools = fm.disallowed_tools;
        r.model = fm.model;
        r.max_turns = fm.max_turns;
        r.color = fm.color;
        r.permission_mode = fm.permission_mode;
        r.skills = fm.skills;
        r.background = fm.background;
        r.effort = fm.effort;
        r.isolation = fm.isolation;
        r.initial_prompt = fm.initial_prompt;
        // Stash vendor-specific fields in extensions
        let mut ext = serde_yaml::Mapping::new();
        if let Some(hooks) = fm.hooks {
            ext.insert(serde_yaml::Value::String("hooks".into()), hooks);
        }
        if let Some(mcp) = fm.mcp_servers {
            ext.insert(serde_yaml::Value::String("mcpServers".into()), mcp);
        }
        if let Some(mem) = fm.memory {
            ext.insert(serde_yaml::Value::String("memory".into()), mem);
        }
        if !ext.is_empty() {
            r.extensions
                .insert("claude-code".into(), serde_yaml::Value::Mapping(ext));
        }

        Ok(r)
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.name.is_empty() {
            issues.push("name is required".into());
        }
        if resource.description.is_empty() {
            issues.push("description is required".into());
        }
        if let Some(color) = &resource.color {
            let valid = [
                "red", "blue", "green", "yellow", "purple", "orange", "pink", "cyan",
            ];
            if !valid.contains(&color.as_str()) {
                issues.push(format!("invalid color '{color}'"));
            }
        }
        if let Some(model) = &resource.model {
            let valid = ["sonnet", "opus", "haiku", "inherit"];
            if !valid.contains(&model.as_str()) && !model.starts_with("claude-") {
                issues.push(format!("unrecognized model '{model}'"));
            }
        }
        issues
    }
}
