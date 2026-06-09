use std::path::Path;

use crate::error::Result;
use crate::frontmatter;
use crate::ir::Resource;

use super::Adapter;

/// Adapter for Gemini CLI agent definition format.
/// Frontmatter: name, description, tools, model, temperature, max_turns, timeout_mins, kind
pub struct GeminiAdapter;

#[derive(serde::Deserialize)]
struct RawGeminiFm {
    name: String,
    description: String,
    #[serde(default)]
    tools: Option<Vec<String>>,
    model: Option<String>,
    temperature: Option<f64>,
    max_turns: Option<u32>,
    timeout_mins: Option<u32>,
    kind: Option<String>,
    #[serde(default, rename = "mcpServers")]
    mcp_servers: Option<serde_yaml_ng::Value>,
    /// Unknown frontmatter keys, preserved for round-tripping.
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_yaml_ng::Value>,
}

impl Adapter for GeminiAdapter {
    fn vendor(&self) -> &str {
        "gemini-cli"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: RawGeminiFm = serde_yaml_ng::from_str(&parsed.frontmatter)?;

        let mut r = Resource::new_agent(fm.name, fm.description, parsed.body);
        r.tools = fm.tools;
        r.model = fm.model;
        r.temperature = fm.temperature;
        r.max_turns = fm.max_turns;
        r.timeout_mins = fm.timeout_mins;
        // Stash Gemini-specific fields
        let mut ext = serde_yaml_ng::Mapping::new();
        if let Some(kind) = fm.kind {
            ext.insert(
                serde_yaml_ng::Value::String("kind".into()),
                serde_yaml_ng::Value::String(kind),
            );
        }
        if let Some(mcp) = fm.mcp_servers {
            ext.insert(serde_yaml_ng::Value::String("mcpServers".into()), mcp);
        }
        if !ext.is_empty() {
            r.extensions
                .insert("gemini-cli".into(), serde_yaml_ng::Value::Mapping(ext));
        }
        r.metadata = fm.extra;

        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        let mut m = serde_yaml_ng::Mapping::new();
        m.insert("name".into(), resource.name.clone().into());
        m.insert("description".into(), resource.description.clone().into());
        if let Some(tools) = &resource.tools {
            m.insert(
                "tools".into(),
                serde_yaml_ng::Value::Sequence(tools.iter().map(|t| t.clone().into()).collect()),
            );
        }
        if let Some(model) = &resource.model {
            m.insert("model".into(), model.clone().into());
        }
        if let Some(temp) = resource.temperature {
            m.insert("temperature".into(), temp.into());
        }
        if let Some(mt) = resource.max_turns {
            m.insert("max_turns".into(), mt.into());
        }
        if let Some(tm) = resource.timeout_mins {
            m.insert("timeout_mins".into(), tm.into());
        }
        if let Some(serde_yaml_ng::Value::Mapping(ext)) = resource.extensions.get("gemini-cli") {
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
        if let Some(temp) = resource.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            issues.push("temperature must be between 0.0 and 2.0".into());
        }
        if let Some(model) = &resource.model
            && !model.starts_with("gemini-")
        {
            issues.push(format!("model '{model}' may not be a valid Gemini model"));
        }
        issues
    }
}
