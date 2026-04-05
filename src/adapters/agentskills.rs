use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::frontmatter;
use crate::ir::{Resource, ResourceKind};

use super::Adapter;

/// Adapter for the agentskills.io SKILL.md format.
pub struct AgentSkillsAdapter;

#[derive(serde::Deserialize)]
struct RawSkillFm {
    name: String,
    description: String,
    #[serde(rename = "allowed-tools")]
    allowed_tools: Option<String>,
    license: Option<String>,
    compatibility: Option<String>,
    #[serde(rename = "user-invocable")]
    user_invocable: Option<bool>,
    metadata: Option<HashMap<String, serde_yaml::Value>>,
}

impl Adapter for AgentSkillsAdapter {
    fn vendor(&self) -> &str {
        "agentskills"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: RawSkillFm = serde_yaml::from_str(&parsed.frontmatter)?;

        let tools = fm
            .allowed_tools
            .map(|s| s.split_whitespace().map(String::from).collect());

        let mut r = Resource::new_skill(fm.name, fm.description, parsed.body);
        r.tools = tools;
        r.license = fm.license;
        r.compatibility = fm.compatibility;
        r.user_invocable = fm.user_invocable;
        r.metadata = fm.metadata.unwrap_or_default();
        Ok(r)
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();

        if resource.kind != ResourceKind::Skill {
            issues.push("agentskills adapter expects a skill resource".into());
            return issues;
        }

        let name = &resource.name;
        if name.is_empty() {
            issues.push("name is required".into());
        }
        if name.len() > 64 {
            issues.push("name must be <= 64 characters".into());
        }
        if name != &name.to_lowercase() {
            issues.push("name must be lowercase".into());
        }
        if name.starts_with('-') || name.ends_with('-') {
            issues.push("name must not start or end with a hyphen".into());
        }
        if name.contains("--") {
            issues.push("name must not contain consecutive hyphens".into());
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            issues.push("name must only contain lowercase letters, digits, hyphens".into());
        }
        if resource.description.is_empty() {
            issues.push("description is required".into());
        }
        if resource.description.len() > 1024 {
            issues.push("description must be <= 1024 characters".into());
        }
        if let Some(c) = &resource.compatibility
            && c.len() > 500
        {
            issues.push("compatibility must be <= 500 characters".into());
        }

        issues
    }
}
