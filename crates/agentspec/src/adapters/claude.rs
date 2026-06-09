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
    hooks: Option<serde_yaml_ng::Value>,
    #[serde(default)]
    mcp_servers: Option<serde_yaml_ng::Value>,
    #[serde(default)]
    memory: Option<serde_yaml_ng::Value>,
    /// Unknown frontmatter keys, preserved for round-tripping.
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_yaml_ng::Value>,
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

fn string_seq(items: &[String]) -> serde_yaml_ng::Value {
    serde_yaml_ng::Value::Sequence(items.iter().map(|s| s.clone().into()).collect())
}

impl Adapter for ClaudeAdapter {
    fn vendor(&self) -> &str {
        "claude-code"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: RawClaudeFm = serde_yaml_ng::from_str(&parsed.frontmatter)?;

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
        let mut ext = serde_yaml_ng::Mapping::new();
        if let Some(hooks) = fm.hooks {
            ext.insert(serde_yaml_ng::Value::String("hooks".into()), hooks);
        }
        if let Some(mcp) = fm.mcp_servers {
            ext.insert(serde_yaml_ng::Value::String("mcpServers".into()), mcp);
        }
        if let Some(mem) = fm.memory {
            ext.insert(serde_yaml_ng::Value::String("memory".into()), mem);
        }
        if !ext.is_empty() {
            r.extensions
                .insert("claude-code".into(), serde_yaml_ng::Value::Mapping(ext));
        }
        r.metadata = fm.extra;

        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        let mut m = serde_yaml_ng::Mapping::new();
        m.insert("name".into(), resource.name.clone().into());
        m.insert("description".into(), resource.description.clone().into());
        if let Some(tools) = &resource.tools {
            m.insert("tools".into(), string_seq(tools));
        }
        if let Some(dt) = &resource.disallowed_tools {
            m.insert("disallowedTools".into(), string_seq(dt));
        }
        if let Some(model) = &resource.model {
            m.insert("model".into(), model.clone().into());
        }
        if let Some(mt) = resource.max_turns {
            m.insert("maxTurns".into(), mt.into());
        }
        if let Some(color) = &resource.color {
            m.insert("color".into(), color.clone().into());
        }
        if let Some(pm) = &resource.permission_mode {
            m.insert("permissionMode".into(), pm.clone().into());
        }
        if let Some(skills) = &resource.skills {
            m.insert("skills".into(), string_seq(skills));
        }
        if let Some(bg) = resource.background {
            m.insert("background".into(), bg.into());
        }
        if let Some(effort) = &resource.effort {
            m.insert("effort".into(), effort.clone().into());
        }
        if let Some(iso) = &resource.isolation {
            m.insert("isolation".into(), iso.clone().into());
        }
        if let Some(ip) = &resource.initial_prompt {
            m.insert("initialPrompt".into(), ip.clone().into());
        }
        if let Some(serde_yaml_ng::Value::Mapping(ext)) = resource.extensions.get("claude-code") {
            for (k, v) in ext {
                m.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in super::sorted_yaml_entries(&resource.metadata) {
            m.insert(k.into(), v);
        }
        let yaml = serde_yaml_ng::to_string(&m)?;
        Ok(crate::frontmatter::compose(&yaml, &resource.body))
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
